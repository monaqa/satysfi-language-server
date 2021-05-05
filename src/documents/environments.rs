use itertools::Itertools;
use log::debug;
use lspower::lsp::Url;

use crate::documents::ConvertPos;

use super::{DocumentData, SourceSpan};
use satysfi_parser::{Rule, Span};

#[derive(Debug, Default)]
pub struct Environments {
    pub package: Vec<EnvPackage>,
    pub variable: Vec<EnvVariable>,
}

#[derive(Debug)]
pub struct EnvPackage {
    name: String,
    url: Url,
}

#[derive(Debug)]
pub struct EnvVariable {
    /// 変数・コマンドの種類
    pub kind: VariableKind,
    /// 変数・コマンドの名前
    pub name: String,
    /// そのコマンドが定義されているモジュールの名前（あれば）
    pub mod_name: Option<String>,
    /// definition が記載されている場所の Cst（テキストは Url を参照する必要がある）
    pub definition: SourceSpan,
    /// declaration が記載されている場所の Cst（テキストは Url を参照する必要がある）
    pub declaration: Option<SourceSpan>,
    // /// そのコマンドが有効な場所。補完候補を出すときや変数の shadowing があるときに重要
    // scope: Vec<SourceRange>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum VariableKind {
    /// 変数
    Variable,
    /// 関数（変数のうち、特に 'a -> 'b の形をしているもの）
    Function,
    /// インラインコマンド
    InlineCmd,
    /// ブロックコマンド
    BlockCmd,
    /// 数式コマンド
    MathCmd,
    /// ユーザ定義型
    CustomType,
}

impl Environments {
    /// その url のファイル内で定義されたすべてのコマンド情報を削除する。
    pub fn remove_defined_in_url(&mut self, url: &Url) {
        // self.variable.drain_filter(|var| var.definition.url == url);
        let mut i = 0;
        while i != self.variable.len() {
            if &self.variable[i].definition.url == url {
                self.variable.remove(i);
            } else {
                i += 1;
            }
        }
    }

    pub fn update(&mut self, url: &Url, data: &DocumentData) {
        self.remove_defined_in_url(url);
        self.register_inline_cmd(url, data);
        self.register_block_cmd(url, data);
        self.register_math_cmd(url, data);
        self.register_variable(url, data);
    }

    fn register_inline_cmd(&mut self, url: &Url, data: &DocumentData) {
        if let DocumentData::ParseSuccessful(csttext) = &data {
            let cst = &csttext.cst;
            let stmts_ctx = cst.pickup(Rule::let_inline_stmt_ctx);
            let stmts_noctx = cst.pickup(Rule::let_inline_stmt_noctx);
            for stmt in stmts_ctx.into_iter().chain(stmts_noctx) {
                let inner = stmt.inner.get(0);
                let cmd = match inner.map(|cst| cst.rule) {
                    Some(Rule::let_inline_stmt_ctx) => {
                        let cmd_name = inner
                            .unwrap()
                            .inner
                            .get(1)
                            .expect("expected Rule::inline_cmd_name, got nothing");
                        EnvVariable {
                            kind: VariableKind::InlineCmd,
                            name: csttext.get_text(cmd_name).to_owned(),
                            mod_name: None,
                            definition: SourceSpan {
                                url: url.clone(),
                                span: stmt.span,
                            },
                            declaration: None,
                        }
                    }
                    Some(Rule::let_inline_stmt_noctx) => {
                        let cmd_name = inner
                            .unwrap()
                            .inner
                            .get(0)
                            .expect("expected Rule::inline_cmd_name, got nothing");
                        EnvVariable {
                            kind: VariableKind::InlineCmd,
                            name: csttext.get_text(cmd_name).to_owned(),
                            mod_name: None,
                            definition: SourceSpan {
                                url: url.clone(),
                                span: stmt.span,
                            },
                            declaration: None,
                        }
                    }
                    _ => unreachable!(),
                };
                self.variable.push(cmd);
            }
        }
    }

    fn register_block_cmd(&mut self, url: &Url, data: &DocumentData) {
        if let DocumentData::ParseSuccessful(csttext) = &data {
            let cst = &csttext.cst;
            let stmts_ctx = cst.pickup(Rule::let_block_stmt_ctx);
            let stmts_noctx = cst.pickup(Rule::let_block_stmt_noctx);
            for stmt in stmts_ctx.into_iter().chain(stmts_noctx) {
                let inner = stmt.inner.get(0);
                let cmd = match inner.map(|cst| cst.rule) {
                    Some(Rule::let_block_stmt_ctx) => {
                        let cmd_name = inner
                            .unwrap()
                            .inner
                            .get(1)
                            .expect("expected Rule::block_cmd_name, got nothing");
                        EnvVariable {
                            kind: VariableKind::BlockCmd,
                            name: csttext.get_text(cmd_name).to_owned(),
                            mod_name: None,
                            definition: SourceSpan {
                                url: url.clone(),
                                span: stmt.span,
                            },
                            declaration: None,
                        }
                    }
                    Some(Rule::let_block_stmt_noctx) => {
                        let cmd_name = inner
                            .unwrap()
                            .inner
                            .get(0)
                            .expect("expected Rule::block_cmd_name, got nothing");
                        EnvVariable {
                            kind: VariableKind::BlockCmd,
                            name: csttext.get_text(cmd_name).to_owned(),
                            mod_name: None,
                            definition: SourceSpan {
                                url: url.clone(),
                                span: stmt.span,
                            },
                            declaration: None,
                        }
                    }
                    _ => unreachable!(),
                };
                self.variable.push(cmd);
            }
        }
    }

    fn register_math_cmd(&mut self, url: &Url, data: &DocumentData) {
        if let DocumentData::ParseSuccessful(csttext) = &data {
            let cst = &csttext.cst;
            let stmts = cst.pickup(Rule::let_math_stmt);
            for stmt in stmts {
                let inner = stmt.inner.get(0);
                let cmd = match inner.map(|cst| cst.rule) {
                    Some(Rule::let_math_stmt) => {
                        let cmd_name = inner
                            .unwrap()
                            .inner
                            .get(0)
                            .expect("expected Rule::math_cmd_name, got nothing");
                        EnvVariable {
                            kind: VariableKind::MathCmd,
                            name: csttext.get_text(cmd_name).to_owned(),
                            mod_name: None,
                            definition: SourceSpan {
                                url: url.clone(),
                                span: stmt.span,
                            },
                            declaration: None,
                        }
                    }
                    _ => unreachable!(),
                };
                self.variable.push(cmd);
            }
        }
    }

    fn register_variable(&mut self, url: &Url, data: &DocumentData) {
        if let DocumentData::ParseSuccessful(csttext) = &data {
            let cst = &csttext.cst;
            let stmts = cst.pickup(Rule::let_stmt);
            for stmt in stmts {
                let cst_pattern = stmt
                    .inner
                    .get(0)
                    .expect("expected Rule::pattern, got nothing");
                if let Some(Rule::var) = cst_pattern.inner.get(0).map(|cst| cst.rule) {
                    let cmd_name = cst_pattern
                        .inner
                        .get(0)
                        .expect("expected Rule::math_cmd_name, got nothing");
                    let cmd = EnvVariable {
                        kind: VariableKind::Variable,
                        name: csttext.get_text(cmd_name).to_owned(),
                        mod_name: None,
                        definition: SourceSpan {
                            url: url.clone(),
                            span: stmt.span,
                        },
                        declaration: None,
                    };
                    self.variable.push(cmd);
                };
            }
        }
    }
}
