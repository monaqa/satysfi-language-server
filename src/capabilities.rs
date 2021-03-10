use lspower::lsp::{
    ClientCapabilities, CompletionOptions, ServerCapabilities, TextDocumentSyncCapability,
    TextDocumentSyncKind,
};

/// Client の capabilities に合わせて Server 側の capabilities を返す。
/// 現在は Client 側の capabilities を一切見ずに固定の値を返す。
pub fn server_capabilities(_client_capabilities: &ClientCapabilities) -> ServerCapabilities {
    ServerCapabilities {
        // text document sync は一旦 full で行う
        // TODO: TextDocumentSyncKind::Incremental のほうがおそらくパフォーマンスが高い
        text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::Full)),
        selection_range_provider: None,
        hover_provider: None,
        completion_provider: Some(CompletionOptions {
            trigger_characters: Some(vec!["\\".to_owned(), "+".to_owned(), "#".to_owned()]),
            ..Default::default()
        }),
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
