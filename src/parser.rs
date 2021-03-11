/// SATySFi の PEG パーサ本体。
#[allow(missing_docs)]
mod satysfi_parser {
    #[derive(Parser)]
    #[grammar = "parser/satysfi.pest"]
    pub struct SatysfiParser;
}

#[cfg(test)]
mod tests;

// pub mod relation;

use itertools::Itertools;
use lspower::lsp::{Position, Range};
use pest::{Parser, Span};
pub use satysfi_parser::{Rule, SatysfiParser};

/// CalculatorParser で用いられる Pair.
pub type Pair<'i> = pest::iterators::Pair<'i, Rule>;

/// 参照をなくして BufferCst が自己参照構造体になることを回避した
/// pest::iterators::Pair 的なもの。再帰構造を持つ。
#[derive(Debug, Clone)]
pub struct Cst {
    /// そのルールが何であるか。
    pub rule: Rule,
    /// Cst が表す範囲。
    pub range: CstRange,
    /// 子 Cst。
    pub inner: Vec<Cst>,
}

impl<'a> From<Pair<'a>> for Cst {
    fn from(pair: Pair<'a>) -> Self {
        let rule = pair.as_rule();
        let range = CstRange::from(pair.as_span());
        let inner = pair.into_inner().map(Cst::from).collect_vec();
        Self { rule, range, inner }
    }
}

/// ルールで検索したり、ある位置を含む Pair を探索したりできるもの。
impl Cst {
    pub fn parse(text: &str, rule: Rule) -> Result<Self, pest::error::Error<Rule>> {
        let pair = SatysfiParser::parse(rule, text)?.next().unwrap();
        Ok(Cst::from(pair))
    }

    /// 与えられたルールの Cst を再帰的に抽出する。
    pub fn pickup(&self, rule: Rule) -> Vec<&Cst> {
        let mut vec = vec![];
        for cst in &self.inner {
            if cst.rule == rule {
                vec.push(cst)
            }
            let v = cst.pickup(rule);
            vec.extend(v);
        }
        vec
    }

    /// 自分の子のうち、与えられた pos を含むものを返す。
    pub fn choose(&self, pos: &Position) -> Option<&Cst> {
        for cst in &self.inner {
            if cst.range.includes(pos) {
                return Some(cst);
            }
        }
        None
    }

    /// 与えられた pos を含む Pair を再帰的に探索する。
    pub fn dig(&self, pos: &Position) -> Vec<&Cst> {
        let child = self.choose(pos);
        if let Some(child) = child {
            let mut v = child.dig(pos);
            v.push(child);
            v
        } else {
            vec![]
        }
    }

    /// Cst の構造を箇条書き形式で出力する。
    pub fn pretty_text(&self, text: &str, indent: usize) -> String {
        let content = if self.inner.len() == 0 {
            format!(
                "| [{rule:?}] ({sl}:{sc}..{el}:{ec}): \"{text}\"\n",
                rule = self.rule,
                sl = self.range.start.line,
                sc = self.range.start.character,
                el = self.range.end.line,
                ec = self.range.end.character,
                text = self.as_str(text)
            )
        } else {
            let children = self
                .inner
                .iter()
                .map(|cst| cst.pretty_text(text, indent + 2))
                .join("");
            format!(
                "- [{rule:?}] ({sl}:{sc}..{el}:{ec})\n{children}",
                rule = self.rule,
                sl = self.range.start.line,
                sc = self.range.start.character,
                el = self.range.end.line,
                ec = self.range.end.character,
                children = children
            )
        };

        format!(
            "{indent}{content}",
            indent = " ".repeat(indent),
            content = content
        )
    }

    pub fn as_str<'a>(&self, text: &'a str) -> &'a str {
        let start = self.range.start.byte;
        let end = self.range.end.byte;
        std::str::from_utf8(&text.as_bytes()[start..end]).unwrap()
    }

    pub fn mode(&self, pos: &Position) -> Mode {
        let csts = self.dig(pos);
        let rules = csts.iter().map(|cst| cst.rule);

        for rule in rules {
            match rule {
                Rule::vertical_mode => return Mode::Vertical,
                Rule::horizontal_mode => return Mode::Horizontal,
                Rule::math_mode => return Mode::Math,
                Rule::headers | Rule::header_stage => return Mode::Header,
                Rule::COMMENT => return Mode::Comment,
                Rule::string_interior => return Mode::Literal,
                Rule::cmd_expr_arg
                | Rule::cmd_expr_option
                | Rule::math_cmd_expr_arg
                | Rule::math_cmd_expr_option => return Mode::Program,
                _ => continue,
            }
        }
        Mode::Program
    }
}

#[derive(Debug, Clone)]
pub struct CstRange {
    /// 始まりの位置。
    start: CstPosition,
    /// 終わりの位置。
    end: CstPosition,
}

impl<'a> From<Span<'a>> for CstRange {
    fn from(span: Span<'a>) -> Self {
        let start = CstPosition::from(span.start_pos());
        let end = CstPosition::from(span.end_pos());
        Self { start, end }
    }
}

impl Into<Range> for CstRange {
    fn into(self) -> Range {
        Range {
            start: self.start.into(),
            end: self.end.into(),
        }
    }
}

impl CstRange {
    pub fn includes(&self, pos: &Position) -> bool {
        let start: Position = self.start.clone().into();
        let end: Position = self.end.clone().into();
        pos >= &start && pos <= &end
    }
}

#[derive(Debug, Clone)]
pub struct CstPosition {
    /// スタートから何バイト目にあるか。
    byte: usize,
    /// 何行目にあるか。
    line: u32,
    /// その行の何文字目にあるか。
    character: u32,
}

impl<'a> From<pest::Position<'a>> for CstPosition {
    fn from(pos: pest::Position<'a>) -> Self {
        let byte = pos.pos();
        let (line, character) = pos.line_col();
        let line = (line - 1) as u32;
        let character = (character - 1) as u32;
        Self {
            byte,
            line,
            character,
        }
    }
}

impl Into<lspower::lsp::Position> for CstPosition {
    fn into(self) -> lspower::lsp::Position {
        lspower::lsp::Position {
            line: self.line,
            character: self.character,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Mode {
    /// プログラムモード。
    Program,
    /// 垂直モード。
    Vertical,
    /// 水平モード。
    Horizontal,
    /// 数式モード。
    Math,
    /// ヘッダ。
    Header,
    /// 文字列リテラル。
    Literal,
    /// コメント。
    Comment,
}
