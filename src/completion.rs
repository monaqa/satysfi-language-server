use std::collections::HashMap;

use itertools::Itertools;
use lspower::lsp::{CompletionItem, Documentation, InsertTextFormat, MarkupContent, MarkupKind};
use serde::Deserialize;

const COMPLETION_RESOUCES: &str = include_str!("resource/completion_items.toml");

pub fn get_primitive_list() -> Vec<CompletionItem> {
    let resources: HashMap<String, Vec<CompletionResourceItem>> =
        toml::from_str(COMPLETION_RESOUCES).expect("[FATAL] Failed to read toml file.");
    let items = resources
        .into_iter()
        // .filter(|(key, _)| key == "primitive" || key == "statement")
        .map(|(_, val)| val)
        .concat();
    // .ok_or_else(|| anyhow!("No field 'primitive' found in completion.toml."))?;
    items.into_iter().map(CompletionItem::from).collect()
}

/// TOML ファイルに記述する completion items.
#[derive(Debug, Deserialize)]
struct CompletionResourceItem {
    /// The label of this completion item. By default also the text that is inserted when selecting
    /// this completion.
    label: String,
    /// A human-readable string with additional information about this item, like type or symbol
    /// information.
    detail: Option<String>,
    /// A human-readable string that represents a doc-comment.
    documentation: Option<String>,
    /// A string that should be inserted a document when selecting this completion. When falsy the
    /// label is used.
    insert_text: Option<String>,
    /// The format of the insert text. The format applies to both the insertText property and the
    /// newText property of a provided textEdit.
    insert_text_format: Option<String>,
}

impl From<CompletionResourceItem> for CompletionItem {
    fn from(resource_item: CompletionResourceItem) -> Self {
        CompletionItem {
            label: resource_item.label,
            detail: resource_item.detail,
            insert_text: resource_item.insert_text,
            insert_text_format: if resource_item.insert_text_format == Some("snippet".to_owned()) {
                Some(InsertTextFormat::Snippet)
            } else {
                None
            },
            documentation: resource_item.documentation.map(|s| {
                Documentation::MarkupContent(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: s,
                })
            }),
            ..Default::default()
        }
    }
}
