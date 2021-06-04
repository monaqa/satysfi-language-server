use itertools::Itertools;
use log::info;
use lspower::lsp::{Diagnostic, DiagnosticSeverity, Position, Range, Url};
use satysfi_parser::Rule;
use std::collections::HashMap;

use crate::{documents::DocumentData, util::ConvertPosition};

#[derive(Debug, Default)]
pub struct DiagnosticCollection {
    map: HashMap<Url, Vec<Diagnostic>>,
}

const DUMMY_RULES: &[Rule] = &[
    Rule::dummy_stmt,
    Rule::dummy_header,
    Rule::dummy_sig_stmt,
    Rule::dummy_block_cmd_incomplete,
    Rule::dummy_inline_cmd_incomplete,
];

pub fn get_diagnostics(doc_data: &DocumentData) -> Vec<Diagnostic> {
    match doc_data {
        DocumentData::Parsed {
            program_text: csttext,
            ..
        } => {
            let dummy_csts = DUMMY_RULES
                .iter()
                .map(|&dummy_rule| csttext.cst.pickup(dummy_rule))
                .concat();
            dummy_csts
                .into_iter()
                .map(|cst| {
                    let range = Range {
                        start: csttext.get_position(cst.span.start).unwrap(),
                        end: csttext.get_position(cst.span.end).unwrap(),
                    };
                    Diagnostic {
                        range,
                        severity: Some(DiagnosticSeverity::Error),
                        code: None,
                        code_description: None,
                        source: Some("Syntax Error".to_owned()),
                        message: cst.rule.error_description().unwrap(),
                        related_information: None,
                        tags: None,
                        data: None,
                    }
                })
                .collect()
        }
        DocumentData::NotParsed {
            linecol,
            expect,
            text,
        } => {
            info!("Not parsed!: {:?}", linecol);
            let line_text = text.split('\n').nth(linecol.line).unwrap();
            let character = line_text
                .chars()
                .take(linecol.column)
                .collect::<String>()
                .encode_utf16()
                .collect_vec()
                .len();
            let pos = Position {
                line: linecol.line as u32,
                character: character as u32,
            };
            let range = Range {
                start: pos,
                end: pos,
            };

            let message = format!(
                "Unexpected character. Expected:\n{}",
                expect.iter().map(|s| format!("- {}", s)).join("\n")
            );

            let diagnostics = Diagnostic {
                range,
                severity: Some(DiagnosticSeverity::Error),
                code: None,
                code_description: None,
                source: Some("Syntax Error".to_owned()),
                message,
                related_information: None,
                tags: None,
                data: None,
            };
            vec![diagnostics]
        }
    }
}
