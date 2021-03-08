// Copyright 2021 monaqa. All rights reserved. MIT license.
// This code (including submodules declared in this file) partially uses
// the source code from the project `deno` [^1].
// [^1]: https://github.com/denoland/deno
//! The SATySFi language server.

use anyhow::Result;
use lspower::{LspService, Server};

mod config;
mod language_server;

mod capabilities {
    use lspower::lsp::{ClientCapabilities, ServerCapabilities};

    /// Client の capabilities に合わせて Server 側の capabilities を返す。
    /// 現在は Client 側の capabilities を一切見ずに固定の値を返す。
    pub fn server_capabilities(_client_capabilities: &ClientCapabilities) -> ServerCapabilities {
        ServerCapabilities {
            text_document_sync: None,
            selection_range_provider: None,
            hover_provider: None,
            completion_provider: None,
            signature_help_provider: None,
            definition_provider: None,
            type_definition_provider: None,
            implementation_provider: None,
            references_provider: None,
            document_highlight_provider: None,
            document_symbol_provider: None,
            workspace_symbol_provider: None,
            code_action_provider: None,
            code_lens_provider: None,
            document_formatting_provider: None,
            document_range_formatting_provider: None,
            document_on_type_formatting_provider: None,
            rename_provider: None,
            document_link_provider: None,
            color_provider: None,
            folding_range_provider: None,
            declaration_provider: None,
            execute_command_provider: None,
            workspace: None,
            call_hierarchy_provider: None,
            semantic_tokens_provider: None,
            moniker_provider: None,
            linked_editing_range_provider: None,
            experimental: None,
        }
    }
}

mod diagnostics {
    use lspower::lsp::{Diagnostic, Url};
    use std::collections::HashMap;

    #[derive(Debug, Default)]
    pub struct DiagnosticCollection {
        map: HashMap<Url, Vec<Diagnostic>>,
    }
}

mod documents {
    use std::collections::HashMap;

    use lspower::lsp::Url;

    #[derive(Debug, Default)]
    pub struct DocumentCache {
        docs: HashMap<Url, DocumentData>,
    }

    #[derive(Debug)]
    pub struct DocumentData {
        text: String,
        // cst: Option<Cst>,
    }
}

pub async fn start_language_server() -> Result<()> {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, messages) = LspService::new(language_server::LanguageServer::new);
    Server::new(stdin, stdout)
        .interleave(messages)
        .serve(service)
        .await;

    Ok(())
}

pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_owned()
}
