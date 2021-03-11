use itertools::Itertools;
use log::{info, warn};
use lspower::lsp::{Diagnostic, Position, Range, TextDocumentContentChangeEvent, Url};
use pest::error::ErrorVariant;
use std::collections::HashMap;

use crate::parser::{Cst, Rule, SyntaxErrorRule};

#[derive(Debug, Default)]
pub struct DocumentCache {
    docs: HashMap<Url, DocumentData>,
}

impl DocumentCache {
    pub fn insert(&mut self, uri: &Url, text: &str) {
        let document_data = DocumentData::new(text);
        self.docs.insert(uri.clone(), document_data);
    }

    pub fn update(&mut self, uri: &Url, changes: &[TextDocumentContentChangeEvent]) {
        // text document sync は一旦 full で行う
        // TODO: TextDocumentSyncKind::Incremental のほうがおそらくパフォーマンスが高い
        if let Some(change) = changes.get(0) {
            let text = &change.text;
            let document_data = DocumentData::new(text);
            self.docs.insert(uri.clone(), document_data);
        }
    }

    pub fn get_diagnostics(&self, url: &Url) -> Vec<Diagnostic> {
        if let Some(doc) = self.docs.get(url) {
            match &doc.parsed_result {
                Ok(cst) => {
                    // dummy syntax をリストアップして diagnostics として返す
                    return cst
                        .listup()
                        .into_iter()
                        .filter(|cst| cst.rule.is_error())
                        .map(|err_cst| Diagnostic {
                            range: err_cst.range.into(),
                            severity: Some(lspower::lsp::DiagnosticSeverity::Error),
                            message: err_cst.rule.error_description().unwrap(),
                            ..Default::default()
                        })
                        .collect_vec();
                }
                Err(e) => {
                    let message = match &e.variant {
                        ErrorVariant::ParsingError {
                            positives,
                            negatives,
                        } => {
                            format!(
                                "Syntax error: positives: {:?}, negatives: {:?}",
                                positives, negatives
                            )
                        }
                        ErrorVariant::CustomError { message } => message.to_owned(),
                    };
                    let range = match e.line_col {
                        pest::error::LineColLocation::Pos((line, col)) => Range {
                            start: Position {
                                line: (line - 1) as u32,
                                character: (col - 1) as u32,
                            },
                            end: Position {
                                line: (line - 1) as u32,
                                character: (col - 1) as u32,
                            },
                        },
                        pest::error::LineColLocation::Span((sline, scol), (eline, ecol)) => Range {
                            start: Position {
                                line: (sline - 1) as u32,
                                character: (scol - 1) as u32,
                            },
                            end: Position {
                                line: (eline - 1) as u32,
                                character: (ecol - 1) as u32,
                            },
                        },
                    };

                    let diag = Diagnostic {
                        range,
                        severity: Some(lspower::lsp::DiagnosticSeverity::Error),
                        // code: (),
                        // code_description: (),
                        // source: (),
                        message,
                        // related_information: (),
                        // tags: (),
                        // data: (),
                        ..Default::default()
                    };
                    return vec![diag];
                }
            }
        }
        vec![]
    }

    pub fn get(&self, uri: &Url) -> Option<&DocumentData> {
        self.docs.get(uri)
    }
}

#[derive(Debug)]
pub struct DocumentData {
    pub text: String,
    pub parsed_result: std::result::Result<Cst, pest::error::Error<Rule>>,
}

impl DocumentData {
    pub fn new(text: &str) -> Self {
        info!("text: {}", text);
        let parsed_result = Cst::parse(text, Rule::program);
        let text = text.to_owned();
        warn!("parsed: {:?}", parsed_result);
        Self {
            text,
            parsed_result,
        }
    }
}
