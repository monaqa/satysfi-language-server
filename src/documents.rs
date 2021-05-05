use itertools::Itertools;
use log::{debug, error, info, warn};
use lspower::lsp::{Diagnostic, Position, Range, TextDocumentContentChangeEvent, Url};
use satysfi_parser::{CstText, LineCol, Rule, Span};
use std::{collections::HashMap, path::PathBuf};

use self::environments::Environments;

pub mod environments;

pub trait ConvertPos {
    /// 位置情報 (usize) を lspower の Position へと変換する。
    fn pos_into(&self, pos: usize) -> Position;

    /// lspower の Position を位置情報 (usize) へと変換する。
    fn pos_from(&self, pos: &Position) -> usize;

    /// Span を lspower Range へと変換する。
    fn span_into(&self, span: Span) -> Range;

    /// lspower Range を Span へと変換する。
    fn span_from(&self, range: Range) -> Span;
}

impl ConvertPos for CstText {
    fn pos_into(&self, pos: usize) -> Position {
        let lc = self.get_line_col(pos).unwrap_or_else(|| {
            error!("Converting position (parser -> LSP) failed. pos:{}", pos);
            panic!()
        });
        Position {
            line: lc.line as u32,
            character: lc.column as u32,
        }
    }

    fn pos_from(&self, pos: &Position) -> usize {
        self.from_line_col(pos.line as usize, pos.character as usize)
            .unwrap_or_else(|| {
                error!("Converting position (LSP -> parser) failed. pos: {:?}", pos);
                panic!()
            })
    }

    fn span_into(&self, span: Span) -> Range {
        let start = self.pos_into(span.start);
        let end = self.pos_into(span.end);
        Range { start, end }
    }

    fn span_from(&self, range: Range) -> Span {
        let start = self.pos_from(&range.start);
        let end = self.pos_from(&range.end);
        Span { start, end }
    }
}

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
            match doc {
                DocumentData::ParseSuccessful(csttext) => {
                    let cst = &csttext.cst;
                    return cst
                        .listup()
                        .into_iter()
                        .filter(|cst| cst.rule.is_error())
                        .map(|err_cst| Diagnostic {
                            range: csttext.span_into(err_cst.span),
                            severity: Some(lspower::lsp::DiagnosticSeverity::Error),
                            message: err_cst
                                .rule
                                .error_description()
                                .unwrap_or_else(|| "No message".to_owned()),
                            ..Default::default()
                        })
                        .collect_vec();
                }

                DocumentData::ParseFailed {
                    linecol, expect, ..
                } => {
                    let message = format!("Expect: {}", expect.join("\n"));
                    let err_pos = Position {
                        line: linecol.line as u32,
                        character: linecol.column as u32,
                    };
                    let range = Range {
                        start: err_pos,
                        end: err_pos,
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
        } else {
            vec![]
        }
    }

    pub fn get(&self, uri: &Url) -> Option<&DocumentData> {
        self.docs.get(uri)
    }

    // for debug
    pub fn show_environments(&self) {
        for var in &self.environments.variable {
            debug!("name: {}, definition: {:?}", var.name, var.definition);
        }
    }
}

#[derive(Debug)]
pub enum DocumentData {
    /// peg のパースに成功した場合。
    ParseSuccessful(CstText),
    /// peg のパースに失敗した場合。
    ParseFailed {
        /// テキストそのもの
        text: String,
        /// エラー位置
        linecol: LineCol,
        /// 期待する文字集合
        expect: Vec<&'static str>,
    },
}

impl DocumentData {
    pub fn new(text: &str) -> Self {
        match CstText::parse(text, satysfi_parser::grammar::program) {
            Ok(csttext) => DocumentData::ParseSuccessful(csttext),
            Err((linecol, expect)) => DocumentData::ParseFailed {
                text: text.to_string(),
                linecol,
                expect,
            },
        }
    }

    pub fn get_dependencies(&self, url: &Url) -> Vec<Dependency> {
        let mut deps = vec![];
        let csttext = match self {
            DocumentData::ParseFailed { .. } => return vec![],
            DocumentData::ParseSuccessful(csttext) => csttext,
        };
        let home_path = std::env::var("HOME").map(PathBuf::from).ok();
        let file_path = url.to_file_path().ok();

        let program = &csttext.cst;

        let require_pkgnames = program
            .pickup(Rule::header_require)
            .into_iter()
            .map(|require| require.inner.get(0).unwrap().as_str(&csttext.text));
        let import_pkgnames = program
            .pickup(Rule::header_import)
            .into_iter()
            .map(|import| import.inner.get(0).unwrap().as_str(&csttext.text));

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

#[derive(Debug, PartialEq, Eq)]
pub struct SourceSpan {
    pub url: Url,
    pub span: Span,
}
