use lspower::lsp::{Diagnostic, Url};
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct DiagnosticCollection {
    map: HashMap<Url, Vec<Diagnostic>>,
}
