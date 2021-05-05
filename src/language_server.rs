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
    completion::get_primitive_list,
    config::Config,
    diagnostics::DiagnosticCollection,
    documents::{ConvertPos, DocumentCache, DocumentData},
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

    async fn did_change(&mut self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        let changes = params.content_changes;
        self.documents.update(&uri, &changes);
        let diags = self.documents.get_diagnostics(&uri);

        self.client.publish_diagnostics(uri, diags, None).await;
    }

    async fn did_open(&mut self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = params.text_document.text;

        // 開いたドキュメントの情報を追加
        self.documents.insert(&uri, &text);
        let diags = self.documents.get_diagnostics(&uri);

        // 依存先の情報を追加
        // self.documents.get(&uri).

        self.client.publish_diagnostics(uri, diags, None).await;
    }

    async fn did_save(&mut self, params: DidSaveTextDocumentParams) {
        self.documents.show_environments();
    }

    async fn goto_definition(
        &mut self,
        params: GotoDefinitionParams,
    ) -> LspResult<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        debug!("pos: {:?}", pos);
        debug!("Doing goto definition");

        if let Some(DocumentData::ParseSuccessful(csttext)) = self.documents.get(&uri) {
            let cst = &csttext.cst;
            let envs = &self.documents.environments;
            let available_rules: HashSet<_> = [
                Rule::var,
                Rule::inline_cmd_name,
                Rule::block_cmd_name,
                Rule::math_cmd_name,
            ]
            .iter()
            .collect();
            let pos = csttext.pos_from(&pos);
            let cst_var = cst.dig(pos).into_iter().find(|&cst| {
                // debug!("{:?}: {:?}", cst.rule, cst.span);
                available_rules.contains(&cst.rule)
            });
            let source_range = if let Some(cst_var) = cst_var {
                let var_name = csttext.get_text(cst_var);
                info!("cst_var: {:?}", var_name);
                match cst_var.rule {
                    Rule::var => {
                        let variable = envs.variable.iter().find(|v| {
                            v.kind == crate::documents::environments::VariableKind::Variable
                                && v.name == var_name
                        });
                        variable.map(|variable| &variable.definition)
                    }
                    Rule::inline_cmd_name => {
                        let cmd = envs.variable.iter().find(|v| {
                            v.kind == crate::documents::environments::VariableKind::InlineCmd
                                && v.name == var_name
                        });
                        cmd.map(|cmd| &cmd.definition)
                    }
                    Rule::block_cmd_name => {
                        let cmd = envs.variable.iter().find(|v| {
                            debug!("v: {:?}", v);
                            v.kind == crate::documents::environments::VariableKind::BlockCmd
                                && v.name == var_name
                        });
                        info!("matched command: {:?}", cmd);
                        cmd.map(|cmd| &cmd.definition)
                    }
                    Rule::math_cmd_name => {
                        let cmd = envs.variable.iter().find(|v| {
                            v.kind == crate::documents::environments::VariableKind::MathCmd
                                && v.name == var_name
                        });
                        cmd.map(|cmd| &cmd.definition)
                    }
                    _ => unreachable!(),
                }
            } else {
                None
            };
            let resp = source_range.map(|source_range| {
                GotoDefinitionResponse::Scalar(Location {
                    uri: source_range.url.clone(),
                    range: csttext.span_into(source_range.span),
                })
            });
            return Ok(resp);
        }
        Ok(None)
    }

    async fn hover(&mut self, params: HoverParams) -> LspResult<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;

        if let Some(DocumentData::ParseSuccessful(csttext)) = self.documents.get(&uri) {
            // 与えられた pos が含むような Rule::var を探す
            let pos = csttext.pos_from(&pos);
            let cst_var = csttext
                .cst
                .dig(pos)
                .into_iter()
                .find(|&cst| cst.rule == Rule::var);
            if let Some(cst_var) = cst_var {
                let var_name = csttext.get_text(cst_var);
                let cst_range = csttext.span_into(cst_var.span);
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

        Ok(None)
    }
}
