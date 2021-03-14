use std::fmt::{format, Display};

use crate::parser::{Pair, Rule, SatysfiParser};
use itertools::Itertools;
use pest::Parser;
use thiserror::Error;

use anyhow::Result;

/// assert_parsed! マクロ内で用いられる AST を表すためのデータ構造。
#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedAst {
    /// parse の要素となる文字列。
    text: String,
    /// 対象ルール。このルールに当てはまらなければ assert_parsed に fail する。
    rule: Rule,
    /// 子要素。Some(vec![]) は「子要素がなにもないことを要請する」ことを表し、
    /// None は「子要素については何もチェックしない」ことを表す。
    inner: Option<Vec<ParsedAst>>,
}

impl ParsedAst {
    fn check_parsed(&self) -> std::result::Result<Pair, AstParseError> {
        let text = &self.text;
        let rule = self.rule;

        let pair = SatysfiParser::parse(rule, text)
            .map_err(AstParseError::ParseFailed)?
            .next()
            .unwrap();

        self.check_equality(pair.clone(), AstBranchSequence(vec![(0, rule)]))?;

        Ok(pair)
    }

    fn check_equality(
        &self,
        pair: Pair,
        branch: AstBranchSequence,
    ) -> std::result::Result<(), AstParseError> {
        if self.text != pair.as_str() {
            return Err(AstParseError::TextDoesNotMatch {
                expect: self.text.to_owned(),
                actual: pair.as_str().to_owned(),
                branch,
            });
        }
        if self.rule != pair.as_rule() {
            return Err(AstParseError::RuleDoesNotMatch {
                expect: self.rule,
                actual: pair.as_rule(),
                branch,
            });
        }

        // inner が None だった場合はこれ以上チェックしない
        if self.inner.is_none() {
            return Ok(());
        }

        let actual_inners = pair.into_inner().collect_vec();

        let expect_inners = self.inner.as_ref().unwrap();

        if expect_inners.len() < actual_inners.len() {
            return Err(AstParseError::InnerExcessive {
                actual: actual_inners.iter().map(|i| i.as_rule()).collect_vec(),
                expect: expect_inners.iter().map(|i| i.rule).collect_vec(),
                branch,
            });
        }

        if expect_inners.len() > actual_inners.len() {
            return Err(AstParseError::InnerLacked {
                actual: actual_inners.iter().map(|i| i.as_rule()).collect_vec(),
                expect: expect_inners.iter().map(|i| i.rule).collect_vec(),
                branch,
            });
        }

        for (idx, (expect_inner, actual_inner)) in
            expect_inners.iter().zip(actual_inners).enumerate()
        {
            let ast_branch = {
                let mut branch = branch.0.clone();
                branch.push((idx, expect_inner.rule));
                AstBranchSequence(branch)
            };
            expect_inner.check_equality(actual_inner, ast_branch)?;
        }

        Ok(())
    }
}

#[derive(Debug, Default)]
struct AstBranchSequence(Vec<(usize, Rule)>);

impl Display for AstBranchSequence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = self
            .0
            .iter()
            .map(|(idx, rule)| format!("{:?}({}-th child)", rule, idx))
            .join(" -> ");
        write!(f, "{}", s)
    }
}

#[derive(Debug, Error)]
enum AstParseError {
    /// parse に失敗した
    #[error("Failed to Parse: {:?}", .0)]
    ParseFailed(pest::error::Error<Rule>),
    /// text が合わない
    #[error("Text does not match at {}.\n  written in test: \"{}\"\n  actually parsed: \"{}\"", .branch, .expect, .actual)]
    TextDoesNotMatch {
        actual: String,
        expect: String,
        branch: AstBranchSequence,
    },
    /// rule が合わない
    #[error("Text does not match at {}.\n  written in test: {:?}\n  actually parsed: {:?}", .branch, .expect, .actual)]
    RuleDoesNotMatch {
        actual: Rule,
        expect: Rule,
        branch: AstBranchSequence,
    },
    #[error("Inner element in {} is excessive.\n  written in test: {:?}\n  actually parsed: {:?}", .branch, .expect, .actual)]
    InnerExcessive {
        actual: Vec<Rule>,
        expect: Vec<Rule>,
        branch: AstBranchSequence,
    },
    #[error("Inner element in {} is lacked.\n  written in test: {:?}\n  actually parsed: {:?}", .branch, .expect, .actual)]
    InnerLacked {
        actual: Vec<Rule>,
        expect: Vec<Rule>,
        branch: AstBranchSequence,
    },
}

/// 以下のようなルールで ParsedAst を構築する。
///
/// ```
/// // 最も単純な例
/// assert_equal!(
///     ast!("foo" var : []),
///     ParsedAst {text: "foo", rule: Rule::var, inner: Some(vec![])}
/// );
///
/// // inner に相当する箇所を `[_]` とすれば、 inner のチェックを無効化出来る
/// assert_equal!(
///     ast!("foo" var : [_]),
///     ParsedAst {text: "foo", rule: Rule::var, inner: None}
/// );
///
/// // inner に相当する箇所は親と同様の文法 + セミコロン区切りで記述する
/// assert_equal!(
///     ast!("List.map" modvar : [
///         "List" module_name : [_];
///         "map" var_ptn : [];
///     ]),
///     ParsedAst {text: "List.map", rule: Rule::modvar, inner: vec![
///         ParsedAst {text: "List", rule: Rule::module_name, inner: None}
///         ParsedAst {text: "map", rule: Rule::var_ptn, inner: vec![]}
///     ]}
/// );
///
/// // inner が 1 つしかなく text も同じ場合、 rule はカンマ区切りで連結させられる
/// assert_equal!(
///     // parsed_ast!("foo": expr ["foo": unary [_]]) と同じことになる
///     ast!("foo" expr, unary : [_]),
///     ParsedAst {text: "foo", rule: Rule::expr, inner: vec![
///         ParsedAst {text: "foo", rule: Rule::unary, inner: None}
///     ]}
/// );
/// ```
macro_rules! ast {
    ($s:literal $r:ident : $t:tt) => {
        ParsedAst {
            rule: Rule::$r,
            text: $s.to_string(),
            inner: ast_inner!($t),
        }
    };
    ($s:literal $r:ident, $($rest:ident),+ : $t:tt) => {
        ParsedAst {
            rule: Rule::$r,
            text: $s.to_string(),
            inner: Some(vec![ast!($s $($rest),+ : $t)]),
        }
    };
}

macro_rules! ast_inner {
    ([_]) => {
        None
    };
    ([]) => {
        Some(vec![])
    };
    ([$s:literal $($r:ident),+ : $t:tt $(;$s2:literal $($r2:ident),+: $t2:tt)*]) => {
        Some(
        vec![
            ast!($s $($r),+ $t),
            $( ast!($s2 $($r2),+: $t2), )*
        ]
        )
    };
    ([$($s:literal $($r:ident),+ : $t:tt;)+]) => {
        Some(
        vec![
            $( ast!($s $($r),+: $t), )*
        ]
        )
    };
}

macro_rules! assert_parsed {
    ($s:literal $($rest:ident),+ : $t:tt) => {
        let ast = ast!($s $($rest),+: $t );
        if let Err(e) = ast.check_parsed() {
            panic!("assertion failed (parse failed): {}", e)
        }
    };
}

macro_rules! assert_not_parsed {
    ($s:literal $($rest:ident),+ : $t:tt) => {
        let ast = ast!($s $($rest),+: $t );
        if let Ok(pair) = ast.check_parsed() {
            panic!(
                "assertion failed (successfully parsed): \"{}\" as {:?}. pair: {:?}",
                ast.text, ast.rule, pair
            )
        }
    };
}

#[test]
fn test_assert_parsed() {
    assert_parsed!("foo" expr, unary, var: []);
    // assert_parsed!("foo" expr, unary, variant_name: [_]);

    assert_parsed!("List.map" modvar: [
        "List" module_name: [];
        "map" var_ptn : [];
    ]);
}

mod statement {
    use super::*;

    #[test]
    fn let_stmt() {
        assert_parsed!("let hoge = 1" let_stmt: [
            "hoge" pattern, var: [];
            "1" expr, unary, literal: [_];
        ]);
    }
}

mod expr {
    use super::*;

    #[test]
    fn match_expr() {
        assert_parsed!("match hoge with | Some(hoge) -> 1 | None -> 0" match_expr : [
            "hoge" expr: [_];
            "Some(hoge) -> 1" match_arm: [_];
            "None -> 0" match_arm: [_];
        ]);
    }

    #[test]
    fn match_arm() {
        assert_parsed!("Some(hoge) -> 1" match_arm: [
            "Some(hoge)" match_ptn: [
                "Some(hoge)" pat_variant: [
                    "Some" variant_name: [];
                    "(hoge)" pattern : [_];
                ];
            ];
            "1" expr: [_];
        ]);
    }

    #[test]
    fn dyadic_expr() {
        assert_not_parsed!("None -> 0" dyadic_expr: [_]);
        assert_not_parsed!("None ->0" dyadic_expr: [_]);
        assert_not_parsed!("None ->0" dyadic_expr: [_]);
        assert_not_parsed!("None -> 0" dyadic_expr: [_]);
        assert_parsed!("None ->| 0" dyadic_expr: [_]);
    }
}
