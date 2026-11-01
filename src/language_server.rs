use log::{error, info};
use lspower::{
    jsonrpc::Result as LspResult,
    lsp::{
        CompletionParams, CompletionResponse, DidChangeTextDocumentParams,
        DidOpenTextDocumentParams, DidSaveTextDocumentParams, DocumentFormattingParams,
        GotoDefinitionParams, GotoDefinitionResponse, Hover, HoverParams, InitializeParams,
        InitializeResult, Position, Range, ServerInfo, TextEdit,
    },
};
use std::sync::Arc;

use lspower::Client;

use crate::{
    config::Config,
    documents::{DocumentCache, DocumentData},
    util::UrlPos,
};

use self::diagnostics::{get_diagnostics, DiagnosticCollection};

mod capabilities;
mod completion;
mod definition;
mod diagnostics;
mod hover;

#[derive(Debug, Clone)]
pub struct LanguageServer(Arc<tokio::sync::Mutex<Inner>>);

impl LanguageServer {
    pub fn new(client: Client) -> Self {
        Self(Arc::new(tokio::sync::Mutex::new(Inner::new(client))))
    }
}

#[lspower::async_trait]
impl lspower::LanguageServer for LanguageServer {
    async fn initialize(&self, params: InitializeParams) -> LspResult<InitializeResult> {
        self.0.lock().await.initialize(params).await
    }

    async fn completion(&self, params: CompletionParams) -> LspResult<Option<CompletionResponse>> {
        self.0.lock().await.get_completion(params).await
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        self.0.lock().await.did_change(params).await;
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.0.lock().await.did_open(params).await;
    }

    async fn hover(&self, params: HoverParams) -> LspResult<Option<Hover>> {
        self.0.lock().await.hover(params).await
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        self.0.lock().await.did_save(params).await;
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> LspResult<Option<GotoDefinitionResponse>> {
        self.0.lock().await.goto_definition(params).await
    }

    async fn formatting(
        &self,
        params: DocumentFormattingParams,
    ) -> LspResult<Option<Vec<TextEdit>>> {
        Ok(self.0.lock().await.formatting(params).await)
    }

    async fn shutdown(&self) -> LspResult<()> {
        Ok(())
    }
}

#[derive(Debug)]
pub struct Inner {
    /// The LSP client that this LSP server is connected to.
    client: Client,
    /// Configuration information.
    config: Config,
    /// A collection of diagnostics from different sources.
    diagnostics: DiagnosticCollection,
    /// The "in-memory" documents in the editor which can be updated and changed.
    documents: DocumentCache,
}

impl Inner {
    fn new(client: Client) -> Self {
        Self {
            client,
            config: Config::default(),
            diagnostics: DiagnosticCollection::default(),
            documents: DocumentCache::default(),
        }
    }

    async fn initialize(&mut self, params: InitializeParams) -> LspResult<InitializeResult> {
        let capabilities = capabilities::server_capabilities(&params.capabilities);
        let server_info = ServerInfo {
            name: "satysfi-language-server".to_owned(),
            version: Some(crate::version()),
        };

        if let Some(client_info) = params.client_info {
            info!(
                "Connected to \"{}\" {}",
                client_info.name,
                client_info.version.unwrap_or_default(),
            );
        }

        Ok(InitializeResult {
            capabilities,
            server_info: Some(server_info),
        })
    }

    async fn get_completion(
        &self,
        params: CompletionParams,
    ) -> LspResult<Option<CompletionResponse>> {
        let url = params.text_document_position.text_document.uri;
        let pos = params.text_document_position.position;
        let trigger = params.context.and_then(|ctx| ctx.trigger_character);
        if self.documents.0.get(&url).is_some() {
            let curpos = UrlPos { url, pos };
            Ok(self
                .documents
                .get_completion_list(&curpos, trigger.as_deref()))
        } else {
            Ok(None)
        }
    }

    async fn formatting(&self, params: DocumentFormattingParams) -> Option<Vec<TextEdit>> {
        use std::process::Stdio;
        use tokio::{io::AsyncWriteExt, process::Command};

        let uri = params.text_document.uri;
        let d_data = self.documents.0.get(&uri)?;
        let text = match &*d_data {
            DocumentData::Parsed { program_text, .. } => program_text.text.clone(),
            DocumentData::NotParsed { text, .. } => text.clone(),
        };

        // satysfi-fmt を別プロセスとして呼ぶ。設定は文書のパスから探される
        // コマンドは環境変数 SATYSFI_FORMATTER で差し替えられる
        let fmt = std::env::var("SATYSFI_FORMATTER").unwrap_or_else(|_| "satysfi-fmt".to_string());
        let mut command = Command::new(fmt);
        if let Ok(path) = uri.to_file_path() {
            command.arg("--assume-filename").arg(path);
        }
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|err| error!("failed to spawn satysfi-fmt: {}", err))
            .ok()?;
        let mut stdin = child.stdin.take()?;
        let input = text.clone();
        tokio::spawn(async move { stdin.write_all(input.as_bytes()).await });
        let output = child.wait_with_output().await.ok()?;
        if !output.status.success() {
            error!(
                "satysfi-fmt failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
            return None;
        }
        let formatted = String::from_utf8(output.stdout).ok()?;

        let line = text.matches('\n').count() as u32;
        let character = text.rsplit('\n').next()?.encode_utf16().count() as u32;
        Some(vec![TextEdit {
            range: Range {
                start: Position { line: 0, character: 0 },
                end: Position { line, character },
            },
            new_text: formatted,
        }])
    }

    async fn did_change(&mut self, params: DidChangeTextDocumentParams) {
        let url = params.text_document.uri;
        if let Some(cc) = params.content_changes.into_iter().last() {
            let text = cc.text;
            let doc_data = DocumentData::new(&text, &url);

            if let DocumentData::Parsed { environment, .. } = &doc_data {
                self.documents
                    .register_dependencies(environment.dependencies());
            }

            let diags = get_diagnostics(&doc_data);
            self.documents.0.insert(url.clone(), doc_data);
            self.client.publish_diagnostics(url, diags, None).await;
        } else {
            error!("failed to extract changes of document {:?}!", url);
        }
    }

    async fn did_open(&mut self, params: DidOpenTextDocumentParams) {
        let url = params.text_document.uri;
        let text = params.text_document.text;
        let doc_data = DocumentData::new(&text, &url);

        if let DocumentData::Parsed { environment, .. } = &doc_data {
            self.documents
                .register_dependencies(environment.dependencies());
        }

        let diags = get_diagnostics(&doc_data);
        self.documents.0.insert(url.clone(), doc_data);
        self.client.publish_diagnostics(url, diags, None).await;
    }

    async fn did_save(&mut self, params: DidSaveTextDocumentParams) {
        let url = params.text_document.uri;
        let doc_data = self.documents.0.get(&url);

        if let Some(doc_data) = doc_data {
            let diags = get_diagnostics(&doc_data);
            self.client.publish_diagnostics(url, diags, None).await;

            doc_data.show_envs_debug();
        }
    }

    async fn goto_definition(
        &mut self,
        params: GotoDefinitionParams,
    ) -> LspResult<Option<GotoDefinitionResponse>> {
        let url = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;

        if self.documents.0.get(&url).is_some() {
            let curpos = UrlPos { url, pos };
            Ok(self.documents.get_definition_list(&curpos))
        } else {
            Ok(None)
        }
    }

    async fn hover(&mut self, params: HoverParams) -> LspResult<Option<Hover>> {
        let url = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;

        if self.documents.0.get(&url).is_some() {
            let curpos = UrlPos { url, pos };
            Ok(self.documents.get_hover(&curpos))
        } else {
            Ok(None)
        }
    }
}
