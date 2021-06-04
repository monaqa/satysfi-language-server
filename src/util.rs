use itertools::Itertools;
use lspower::lsp::{Position, Url};
use satysfi_parser::{structure::ProgramText, CstText, LineCol, Span};

/// Position を convert する関数の提供。
pub trait ConvertPosition {
    fn get_position(&self, pos: usize) -> Option<Position>;
    fn from_position(&self, pos: &Position) -> Option<usize>;
}

impl ConvertPosition for CstText {
    fn get_position(&self, pos: usize) -> Option<Position> {
        let LineCol { line, .. } = self.get_line_col(pos)?;
        let span = {
            let start = self.from_line_col(line, 0).unwrap();
            Span { start, end: pos }
        };
        let text = self.get_text_from_span(span);
        let character = text.encode_utf16().collect_vec().len();
        Some(Position {
            line: line as u32,
            character: character as u32,
        })
    }

    fn from_position(&self, pos: &Position) -> Option<usize> {
        let &Position { line, character } = pos;
        // position が属する行のテキストを取り出す。
        let text = {
            let start = self.from_line_col(line as usize, 0).unwrap();
            let end = self
                .from_line_col((line + 1) as usize, 0)
                .unwrap_or(self.cst.span.end);
            self.get_text_from_span(Span { start, end })
        };
        let vec_utf16 = text.encode_utf16().take(character as usize).collect_vec();
        let text = String::from_utf16_lossy(&vec_utf16);
        let column = text.len();
        self.from_line_col(line as usize, column)
    }
}

impl ConvertPosition for ProgramText {
    fn get_position(&self, pos: usize) -> Option<Position> {
        let LineCol { line, .. } = self.get_line_col(pos)?;
        let span = {
            let start = self.from_line_col(line, 0).unwrap();
            Span { start, end: pos }
        };
        let text = self.get_text_from_span(span);
        let character = text.encode_utf16().collect_vec().len();
        Some(Position {
            line: line as u32,
            character: character as u32,
        })
    }

    fn from_position(&self, pos: &Position) -> Option<usize> {
        let &Position { line, character } = pos;
        // position が属する行のテキストを取り出す。
        let text = {
            let start = self.from_line_col(line as usize, 0).unwrap();
            let end = self
                .from_line_col((line + 1) as usize, 0)
                .unwrap_or(self.cst.span.end);
            self.get_text_from_span(Span { start, end })
        };
        let vec_utf16 = text.encode_utf16().take(character as usize).collect_vec();
        let text = String::from_utf16_lossy(&vec_utf16);
        let column = text.len();
        self.from_line_col(line as usize, column)
    }
}

pub struct UrlPos {
    pub url: Url,
    pub pos: Position,
}
