use log::info;
use lspower::{
    jsonrpc::Result as LspResult,
    lsp::{
        CompletionList, CompletionParams, CompletionResponse, DidChangeTextDocumentParams, Hover,
        HoverContents, HoverParams, InitializeParams, InitializeResult, LanguageString,
        MarkedString, MarkupContent, ServerInfo,
    },
};
use std::sync::Arc;

use lspower::Client;

use crate::{
    completion::get_primitive_list, config::Config, diagnostics::DiagnosticCollection,
    documents::DocumentCache, parser::Rule,
};

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

    async fn hover(&self, params: HoverParams) -> LspResult<Option<Hover>> {
        self.0.lock().await.hover(params).await
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

    async fn did_change(&mut self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        let changes = params.content_changes;
        self.documents.update(&uri, &changes);
        let diags = self.documents.publish_diagnostics(&uri);

        self.client.publish_diagnostics(uri, diags, None).await;
    }

    async fn hover(&mut self, params: HoverParams) -> LspResult<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;

        if let Some(doc_data) = self.documents.get(&uri) {
            if let Ok(cst) = &doc_data.parsed_result {
                // 与えられた pos が含むような Rule::var を探す
                let cst_var = cst.dig(&pos).into_iter().find(|&cst| cst.rule == Rule::var);
                if let Some(cst_var) = cst_var {
                    let var_name = cst_var.as_str(&doc_data.text);
                    let cst_range = cst_var.range.clone().into();
                    // 与えられた var_name の primitive を探す
                    let items = crate::completion::get_resouce_items();
                    let primitive = items
                        .get("primitive")
                        .and_then(|v| v.iter().find(|&item| item.label == var_name));
                    if let Some(primitive) = primitive {
                        let hover = Hover {
                            contents: HoverContents::Array(vec![
                                MarkedString::LanguageString(LanguageString {
                                    language: "satysfi".to_owned(),
                                    value: primitive
                                        .detail
                                        .as_deref()
                                        .unwrap_or("primitive")
                                        .to_owned(),
                                }),
                                MarkedString::String(
                                    primitive
                                        .documentation
                                        .as_deref()
                                        .unwrap_or("undocumented")
                                        .to_string(),
                                ),
                            ]),
                            range: Some(cst_range),
                        };
                        return Ok(Some(hover));
                    }
                }
            }
        }

        Ok(None)
    }
}
