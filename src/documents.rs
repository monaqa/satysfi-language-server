use itertools::Itertools;
use log::{debug, info, warn};
use lspower::lsp::{Diagnostic, Position, Range, TextDocumentContentChangeEvent, Url};
use pest::error::ErrorVariant;
use std::{collections::HashMap, path::PathBuf};

use crate::parser::{Cst, Rule, SyntaxErrorRule};

use self::environments::Environments;

pub mod environments;

#[derive(Debug, Default)]
pub struct DocumentCache {
    pub docs: HashMap<Url, DocumentData>,
    pub environments: Environments,
}

impl DocumentCache {
    pub fn insert(&mut self, uri: &Url, text: &str) {
        let document_data = DocumentData::new(text);
        self.environments.update(uri, &document_data);

        let deps = document_data.get_dependencies(uri);
        for dep in deps {
            let fpath = match dep.url.to_file_path() {
                Ok(f) => f,
                Err(_) => {
                    continue;
                }
            };
            let text = match std::fs::read_to_string(fpath) {
                Ok(t) => t,
                Err(_) => {
                    continue;
                }
            };
            let dep_data = DocumentData::new(&text);
            self.environments.update(&dep.url, &dep_data);
            self.docs.insert(dep.url.clone(), dep_data);
        }

        self.docs.insert(uri.clone(), document_data);
    }

    pub fn update(&mut self, uri: &Url, changes: &[TextDocumentContentChangeEvent]) {
        // text document sync は一旦 full で行う
        // TODO: TextDocumentSyncKind::Incremental のほうがおそらくパフォーマンスが高い
        if let Some(change) = changes.get(0) {
            let text = &change.text;
            let document_data = DocumentData::new(text);
            self.environments.update(&uri, &document_data);

            let deps = document_data.get_dependencies(uri);
            for dep in deps {
                let fpath = match dep.url.to_file_path() {
                    Ok(f) => f,
                    Err(_) => {
                        continue;
                    }
                };
                let text = match std::fs::read_to_string(fpath) {
                    Ok(t) => t,
                    Err(_) => {
                        continue;
                    }
                };
                let dep_data = DocumentData::new(&text);
                self.environments.update(&dep.url, &dep_data);
                self.docs.insert(dep.url.clone(), dep_data);
            }

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

    // for debug
    pub fn show_environments(&self) {
        for var in &self.environments.variable {
            debug!("name: {}, definition: {}", var.name, var.definition);
        }
    }
}

#[derive(Debug)]
pub struct DocumentData {
    pub text: String,
    pub parsed_result: std::result::Result<Cst, pest::error::Error<Rule>>,
}

impl DocumentData {
    pub fn new(text: &str) -> Self {
        let parsed_result = Cst::parse(text, Rule::program);
        let text = text.to_owned();
        Self {
            text,
            parsed_result,
        }
    }

    pub fn get_dependencies(&self, url: &Url) -> Vec<Dependency> {
        let mut deps = vec![];
        let program = match &self.parsed_result {
            Ok(p) => p,
            Err(_) => return vec![],
        };
        let home_path = std::env::var("HOME").map(PathBuf::from).ok();
        let file_path = url.to_file_path().ok();

        let require_pkgnames = program
            .pickup(Rule::header_require)
            .into_iter()
            .map(|require| require.inner.get(0).unwrap().as_str(&self.text));
        let import_pkgnames = program
            .pickup(Rule::header_import)
            .into_iter()
            .map(|import| import.inner.get(0).unwrap().as_str(&self.text));

        // require 系のパッケージの依存関係追加
        if let Some(home_path) = home_path {
            let dist_path = home_path.join(".satysfi/dist/packages");

            for pkgname in require_pkgnames {
                // TODO: *.satyg file
                let pkg_path = dist_path.join(format!("{}.satyh", pkgname));
                if pkg_path.exists() {
                    if let Ok(url) = Url::from_file_path(pkg_path) {
                        deps.push(Dependency {
                            kind: DependencyKind::Require,
                            url,
                        });
                    }
                }
            }
        }

        if let Some(file_path) = file_path {
            // unwrap して大丈夫？
            let parent_path = file_path.parent().unwrap();

            for pkgname in import_pkgnames {
                // TODO: *.satyg file
                let pkg_path = parent_path.join(format!("{}.satyh", pkgname));
                if pkg_path.exists() {
                    if let Ok(url) = Url::from_file_path(pkg_path) {
                        deps.push(Dependency {
                            kind: DependencyKind::Import,
                            url,
                        });
                    }
                }
            }
        }

        deps
    }
}

#[derive(Debug)]
pub struct Dependency {
    kind: DependencyKind,
    url: Url,
}

#[derive(Debug)]
pub enum DependencyKind {
    Require,
    Import,
}
