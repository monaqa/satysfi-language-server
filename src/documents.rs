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
