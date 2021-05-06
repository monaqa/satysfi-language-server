use log::{debug, info};
use lspower::{
    jsonrpc::Result as LspResult,
    lsp::{
        CompletionList, CompletionParams, CompletionResponse, DidChangeTextDocumentParams,
        DidOpenTextDocumentParams, DidSaveTextDocumentParams, GotoDefinitionParams,
        GotoDefinitionResponse, Hover, HoverContents, HoverParams, InitializeParams,
        InitializeResult, LanguageString, Location, MarkedString, MarkupContent, ServerInfo,
    },
};
use std::{collections::HashSet, sync::Arc};

use lspower::Client;

use crate::{
    completion::get_primitive_list, config::Config, diagnostics::DiagnosticCollection,
    documents::DocumentCache,
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
        _params: CompletionParams,
    ) -> LspResult<Option<CompletionResponse>> {
        let items = get_primitive_list();
        let resp = CompletionResponse::List(CompletionList {
            is_incomplete: true,
            items,
        });
        Ok(Some(resp))
    }

    async fn did_change(&mut self, params: DidChangeTextDocumentParams) {}

    async fn did_open(&mut self, params: DidOpenTextDocumentParams) {}

    async fn did_save(&mut self, params: DidSaveTextDocumentParams) {}

    async fn goto_definition(
        &mut self,
        params: GotoDefinitionParams,
    ) -> LspResult<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;

        todo!()
    }

    async fn hover(&mut self, params: HoverParams) -> LspResult<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;

        Ok(None)
    }
}
