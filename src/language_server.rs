use itertools::Itertools;
use log::{error, info};
use lspower::{
    jsonrpc::Result as LspResult,
    lsp::{
        CompletionParams, CompletionResponse, DidChangeTextDocumentParams,
        DidOpenTextDocumentParams, DidSaveTextDocumentParams, GotoDefinitionParams,
        GotoDefinitionResponse, Hover, HoverParams, InitializeParams, InitializeResult, Location,
        Range, ServerInfo,
    },
};
use std::sync::Arc;

use lspower::Client;

use crate::{
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
        if self.documents.0.get(&url).is_some() {
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
