use lspower::lsp::{Hover, HoverContents, LanguageString, MarkedString, Range};

use crate::{
    documents::{ComponentBody, DocumentCache, DocumentData},
    util::{ConvertPosition, UrlPos},
};

impl DocumentCache {
    pub fn get_hover(&self, curpos: &UrlPos) -> Option<Hover> {
        let UrlPos { url, .. } = curpos;
        let (cst, component) = self.find_component_under_cursor(curpos)?;

        if let DocumentData::Parsed { program_text, .. } = self.get(url).unwrap() {
            let range = Range {
                start: program_text.get_position(cst.span.start).unwrap(),
                end: program_text.get_position(cst.span.end).unwrap(),
            };

            let contents = match &component.body {
                ComponentBody::Module { .. } => {
                    vec![MarkedString::String("module".to_owned())]
                }
                ComponentBody::Variable { type_declaration } => {
                    let mut v = vec![MarkedString::String("variable".to_owned())];
                    if let Some(span) = type_declaration {
                        v.push(MarkedString::LanguageString(LanguageString {
                            language: "satysfi".to_owned(),
                            value: self.get_text_from_span(&component.url, *span)?.to_owned(),
                        }));
                    }
                    v
                }
                ComponentBody::Type => {
                    vec![MarkedString::String("type".to_owned())]
                }
                ComponentBody::Variant { type_name } => {
                    vec![MarkedString::String(format!(
                        "variant of type {}",
                        type_name
                    ))]
                }
                ComponentBody::InlineCmd {
                    type_declaration, ..
                } => {
                    let mut v = vec![MarkedString::String("inline command".to_owned())];
                    if let Some(span) = type_declaration {
                        v.push(MarkedString::LanguageString(LanguageString {
                            language: "satysfi".to_owned(),
                            value: self.get_text_from_span(&component.url, *span)?.to_owned(),
                        }))
                    }
                    v
                }
                ComponentBody::BlockCmd {
                    type_declaration, ..
                } => {
                    let mut v = vec![MarkedString::String("block command".to_owned())];
                    if let Some(span) = type_declaration {
                        v.push(MarkedString::LanguageString(LanguageString {
                            language: "satysfi".to_owned(),
                            value: self.get_text_from_span(&component.url, *span)?.to_owned(),
                        }))
                    }
                    v
                }
                ComponentBody::MathCmd {
                    type_declaration, ..
                } => {
                    let mut v = vec![MarkedString::String("math command".to_owned())];
                    if let Some(span) = type_declaration {
                        v.push(MarkedString::LanguageString(LanguageString {
                            language: "satysfi".to_owned(),
                            value: self.get_text_from_span(&component.url, *span)?.to_owned(),
                        }))
                    }
                    v
                }
            };

            Some(Hover {
                contents: HoverContents::Array(contents),
                range: Some(range),
            })
        } else {
            unreachable!()
        }
    }
}
