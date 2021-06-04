use log::{debug, error, info};
use lspower::{
    jsonrpc::Result as LspResult,
    lsp::{
        CompletionList, CompletionParams, CompletionResponse, DidChangeTextDocumentParams,
        DidOpenTextDocumentParams, DidSaveTextDocumentParams, GotoDefinitionParams,
        GotoDefinitionResponse, Hover, HoverContents, HoverParams, InitializeParams,
        InitializeResult, LanguageString, Location, MarkedString, MarkupContent, Range, ServerInfo,
    },
};
use std::{collections::HashSet, sync::Arc};

use lspower::Client;

use crate::{
    completion::{get_completion_list, get_primitive_list},
    config::Config,
    diagnostics::{get_diagnostics, DiagnosticCollection},
    documents::{DocumentCache, DocumentData},
    util::{ConvertPosition, UrlPos},
};
use satysfi_parser::Rule;

use crate::capabilities;

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
        if let Some(doc_data) = self.documents.0.get(&url) {
            // Ok(get_completion_list(doc_data, &url, &pos))
            let curpos = UrlPos { url, pos };
            Ok(self.documents.get_completion_list(&curpos))
        } else {
            Ok(None)
        }
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
        }

        self.documents.show_envs();
    }

    async fn goto_definition(
        &mut self,
        params: GotoDefinitionParams,
    ) -> LspResult<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;

        if let Some(DocumentData::Parsed {
            program_text: csttext,
            environment,
        }) = self.documents.0.get(&uri)
        {
            let pos_usize = csttext.from_position(&pos).unwrap();
            // カーソル上にある variable や inline-cmd の CST を抽出する
            let cst = csttext
                .cst
                .dig(pos_usize)
                .into_iter()
                .find(|&cst| [Rule::var, Rule::inline_cmd_name].contains(&cst.rule));
            if cst.is_none() {
                return Ok(None);
            }
            // カーソル上にある variable や inline-cmd の CST
            let cst = cst.unwrap();
            // 検索したい変数・コマンド名
            let name = csttext.get_text(cst);

            let pos_definition = match cst.rule {
                Rule::var => environment
                    .variables()
                    .iter()
                    // カーソルがスコープ内にあって、かつ名前の一致するもの
                    .find(|var| var.scope.includes(pos_usize) && var.name == name)
                    .map(|var| var.pos_definition),
                Rule::inline_cmd_name => environment
                    .inline_cmds()
                    .iter()
                    .find(|var| var.scope.includes(pos_usize) && var.name == name)
                    .map(|var| var.pos_definition),
                _ => unreachable!(),
            };

            if pos_definition.is_none() {
                return Ok(None);
            }
            let pos_definition = pos_definition.unwrap();
            let range = Range {
                start: csttext.get_position(pos_definition.start).unwrap(),
                end: csttext.get_position(pos_definition.end).unwrap(),
            };
            let loc = Location { uri, range };
            let resp = GotoDefinitionResponse::Scalar(loc);

            Ok(Some(resp))
        } else {
            Ok(None)
        }
    }

    async fn hover(&mut self, params: HoverParams) -> LspResult<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;

        Ok(None)
    }
}
