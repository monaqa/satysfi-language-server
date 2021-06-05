use std::collections::HashMap;

use itertools::Itertools;
use log::info;
use lspower::lsp::{
    CompletionItem, CompletionResponse, Documentation, InsertTextFormat, MarkupContent, MarkupKind,
};
use satysfi_parser::Mode;
use serde::Deserialize;

use crate::{
    documents::{DocumentCache, DocumentData, Visibility},
    util::{ConvertPosition, UrlPos},
};

pub const COMPLETION_RESOUCES: &str = include_str!("../resource/completion_items.toml");

// pub fn get_completion_list(
//     doc_data: &DocumentData,
//     url: &Url,
//     pos: &Position,
// ) -> Option<CompletionResponse> {
//     match doc_data {
//         DocumentData::Parsed {
//             program_text: csttext,
//             ..
//         } => {
//             let pos_usize = csttext.from_position(pos)?;
//             if csttext.is_comment(pos_usize) {
//                 return None;
//             }
//             let mode = csttext.cst.mode(pos_usize);
//             info!("{:?}", mode);
//             match mode {
//                 Mode::Program => {
//                     let mut items = vec![];
//                     items.extend(get_variable_list(doc_data, url, pos));
//                     items.extend(get_primitive_list());
//                     Some(CompletionResponse::List(CompletionList {
//                         is_incomplete: false,
//                         items,
//                     }))
//                 }
//                 Mode::Horizontal => {
//                     let items = get_inline_cmd_list(doc_data, url, pos);
//                     Some(CompletionResponse::List(CompletionList {
//                         is_incomplete: false,
//                         items,
//                     }))
//                 }
//                 Mode::Vertical => {
//                     let items = get_block_cmd_list(doc_data, url, pos);
//                     Some(CompletionResponse::List(CompletionList {
//                         is_incomplete: false,
//                         items,
//                     }))
//                 }
//                 _ => None,
//             }
//         }
//         DocumentData::NotParsed { .. } => None,
//     }
// }
//
// pub fn get_variable_list(
//     doc_data: &DocumentData,
//     url: &Url,
//     pos: &Position,
// ) -> Vec<CompletionItem> {
//     match doc_data {
//         DocumentData::Parsed {
//             program_text: csttext,
//             environment,
//         } => {
//             if let Some(pos_usize) = csttext.from_position(pos) {
//                 // TODO: 依存パッケージを遡って検索
//                 environment
//                     .variables()
//                     .iter()
//                     .filter(|var| var.scope.includes(pos_usize))
//                     .map(|var| {
//                         CompletionItem::new_simple(var.name.clone(), "in this file".to_owned())
//                     })
//                     .collect()
//             } else {
//                 vec![]
//             }
//         }
//         DocumentData::NotParsed { .. } => vec![],
//     }
// }
//
// pub fn get_inline_cmd_list(
//     doc_data: &DocumentData,
//     url: &Url,
//     pos: &Position,
// ) -> Vec<CompletionItem> {
//     match doc_data {
//         DocumentData::Parsed {
//             program_text: csttext,
//             environment,
//         } => {
//             if let Some(pos_usize) = csttext.from_position(pos) {
//                 // TODO: 依存パッケージを遡って検索
//                 environment
//                     .inline_cmds()
//                     .iter()
//                     .filter(|cmd| cmd.scope.includes(pos_usize))
//                     .map(|cmd| {
//                         CompletionItem::new_simple(cmd.name.clone(), "in this file".to_owned())
//                     })
//                     .collect()
//             } else {
//                 vec![]
//             }
//         }
//         DocumentData::NotParsed { .. } => vec![],
//     }
// }
//
// pub fn get_block_cmd_list(
//     doc_data: &DocumentData,
//     url: &Url,
//     pos: &Position,
// ) -> Vec<CompletionItem> {
//     match doc_data {
//         DocumentData::Parsed {
//             program_text: csttext,
//             environment,
//         } => {
//             let pos_usize = csttext.from_line_col(pos.line as usize, pos.character as usize);
//             if let Some(pos_usize) =
//                 csttext.from_line_col(pos.line as usize, pos.character as usize)
//             {
//                 // TODO: 依存パッケージを遡って検索
//                 environment
//                     .block_cmds()
//                     .iter()
//                     .filter(|cmd| cmd.scope.includes(pos_usize))
//                     .map(|cmd| {
//                         CompletionItem::new_simple(cmd.name.clone(), "in this file".to_owned())
//                     })
//                     .collect()
//             } else {
//                 vec![]
//             }
//         }
//         DocumentData::NotParsed { .. } => vec![],
//     }
// }

pub fn get_primitive_list() -> Vec<CompletionItem> {
    let resources = get_resouce_items();
    let items = resources
        .into_iter()
        // .filter(|(key, _)| key == "primitive" || key == "statement")
        .map(|(_, val)| val)
        .concat();
    // .ok_or_else(|| anyhow!("No field 'primitive' found in completion.toml."))?;
    items.into_iter().map(CompletionItem::from).collect()
}

pub fn get_resouce_items() -> HashMap<String, Vec<CompletionResourceItem>> {
    toml::from_str(COMPLETION_RESOUCES).expect("[FATAL] Failed to read toml file.")
}

/// TOML ファイルに記述する completion items.
#[derive(Debug, Deserialize)]
pub struct CompletionResourceItem {
    /// The label of this completion item. By default also the text that is inserted when selecting
    /// this completion.
    pub label: String,
    /// A human-readable string with additional information about this item, like type or symbol
    /// information.
    pub detail: Option<String>,
    /// A human-readable string that represents a doc-comment.
    pub documentation: Option<String>,
    /// A string that should be inserted a document when selecting this completion. When falsy the
    /// label is used.
    pub insert_text: Option<String>,
    /// The format of the insert text. The format applies to both the insertText property and the
    /// newText property of a provided textEdit.
    pub insert_text_format: Option<String>,
}

impl From<CompletionResourceItem> for CompletionItem {
    fn from(resource_item: CompletionResourceItem) -> Self {
        CompletionItem {
            label: resource_item.label,
            detail: resource_item.detail,
            insert_text: resource_item.insert_text,
            insert_text_format: if resource_item.insert_text_format == Some("snippet".to_owned()) {
                Some(InsertTextFormat::Snippet)
            } else {
                None
            },
            documentation: resource_item.documentation.map(|s| {
                Documentation::MarkupContent(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: s,
                })
            }),
            ..Default::default()
        }
    }
}

impl DocumentCache {
    pub fn get_completion_list(&self, curpos: &UrlPos) -> Option<CompletionResponse> {
        match self.get_mode(curpos) {
            Mode::Program => Some(CompletionResponse::Array(
                self.get_completion_list_program(curpos),
            )),
            Mode::ProgramType => None,
            Mode::Vertical => Some(CompletionResponse::Array(
                self.get_completion_list_vertical(curpos),
            )),
            Mode::Horizontal => Some(CompletionResponse::Array(
                self.get_completion_list_horizontal(curpos),
            )),
            Mode::Math => Some(CompletionResponse::Array(
                self.get_completion_list_math(curpos),
            )),
            Mode::Header => None,
            Mode::Literal => None,
            Mode::Comment => None,
        }
    }

    fn get_mode(&self, curpos: &UrlPos) -> Mode {
        let UrlPos { url, pos } = curpos;
        if let Some(DocumentData::Parsed { program_text, .. }) = self.get(url) {
            let pos_usize = program_text.from_position(pos);
            pos_usize
                .map(|pos| program_text.cst.mode(pos))
                .unwrap_or(Mode::Comment)
        } else {
            Mode::Comment
        }
    }

    fn get_completion_list_program(&self, curpos: &UrlPos) -> Vec<CompletionItem> {
        let UrlPos { url, pos } = curpos;
        if let Some(DocumentData::Parsed {
            program_text,
            environment,
        }) = self.get(url)
        {
            let pos_usize = program_text.from_position(pos);
            if pos_usize.is_none() {
                return vec![];
            }
            let pos_usize = pos_usize.unwrap();

            let local_variables = environment
                .variables()
                .iter()
                .filter(|var| var.scope.includes(pos_usize))
                .map(|var| {
                    CompletionItem::new_simple(
                        var.name.clone(),
                        "variable defined in this file".to_owned(),
                    )
                })
                .collect_vec();

            // TODO: 直接 require/import していない変数も取れるようにする
            // let open_in = program_text.cst.dig(curpos).iter().filter(|cst| cst.rule == Rule::bind_stmt && cst.inner[0].rule == Rule::open_stmt)
            let deps_variables = self
                .get_dependencies_recursive(environment.dependencies())
                .iter()
                .map(|dep| {
                    if let Some(DocumentData::Parsed {
                        environment: env_dep,
                        ..
                    }) = dep.url.as_ref().and_then(|url| self.get(url))
                    {
                        env_dep
                            .variables_external(&[])
                            .iter()
                            .map(|var| {
                                CompletionItem::new_simple(
                                    var.name.clone(),
                                    format!("variable defined in package '{}'", dep.name),
                                )
                            })
                            .collect_vec()
                    } else {
                        vec![]
                    }
                })
                .concat();

            let primitives = get_primitive_list();

            [local_variables, deps_variables, primitives].concat()
        } else {
            vec![]
        }
    }

    fn get_completion_list_horizontal(&self, curpos: &UrlPos) -> Vec<CompletionItem> {
        let UrlPos { url, pos } = curpos;
        if let Some(DocumentData::Parsed {
            program_text,
            environment,
        }) = self.get(url)
        {
            let pos_usize = program_text.from_position(pos);
            if pos_usize.is_none() {
                return vec![];
            }
            let pos_usize = pos_usize.unwrap();

            let local_commands = environment
                .inline_cmds()
                .iter()
                .filter(|var| var.scope.includes(pos_usize))
                .map(|cmd| {
                    CompletionItem::new_simple(
                        cmd.name.clone(),
                        "inline-cmd defined in this file".to_owned(),
                    )
                })
                .collect_vec();

            // TODO: 直接 require/import していない変数も取れるようにする
            let deps_commands = self
                .get_dependencies_recursive(environment.dependencies())
                .iter()
                .map(|dep| {
                    if let Some(DocumentData::Parsed {
                        environment: env_dep,
                        ..
                    }) = dep.url.as_ref().and_then(|url| self.get(url))
                    {
                        env_dep
                            .inline_cmds_external(&[])
                            .iter()
                            .filter(|&cmd| {
                                matches!(cmd.visibility, Visibility::Public | Visibility::Direct)
                            })
                            .map(|cmd| {
                                CompletionItem::new_simple(
                                    cmd.name.clone(),
                                    format!("inline-cmd defined in package '{}'", dep.name),
                                )
                            })
                            .collect_vec()
                    } else {
                        vec![]
                    }
                })
                .concat();

            [local_commands, deps_commands].concat()
        } else {
            vec![]
        }
    }

    fn get_completion_list_vertical(&self, curpos: &UrlPos) -> Vec<CompletionItem> {
        let UrlPos { url, pos } = curpos;
        if let Some(DocumentData::Parsed {
            program_text,
            environment,
        }) = self.get(url)
        {
            let pos_usize = program_text.from_position(pos);
            if pos_usize.is_none() {
                return vec![];
            }
            let pos_usize = pos_usize.unwrap();

            let local_commands = environment
                .block_cmds()
                .iter()
                .filter(|var| var.scope.includes(pos_usize))
                .map(|cmd| {
                    CompletionItem::new_simple(
                        cmd.name.clone(),
                        "block-cmd defined in this file".to_owned(),
                    )
                })
                .collect_vec();

            // TODO: 直接 require/import していない変数も取れるようにする
            let deps_commands = self
                .get_dependencies_recursive(environment.dependencies())
                .iter()
                .map(|dep| {
                    if let Some(DocumentData::Parsed {
                        environment: env_dep,
                        ..
                    }) = dep.url.as_ref().and_then(|url| self.get(url))
                    {
                        env_dep
                            .block_cmds_external(&[])
                            .iter()
                            .filter(|&cmd| {
                                matches!(cmd.visibility, Visibility::Public | Visibility::Direct)
                            })
                            .map(|cmd| {
                                CompletionItem::new_simple(
                                    cmd.name.clone(),
                                    format!("block-cmd defined in package '{}'", dep.name),
                                )
                            })
                            .collect_vec()
                    } else {
                        vec![]
                    }
                })
                .concat();

            [local_commands, deps_commands].concat()
        } else {
            vec![]
        }
    }

    fn get_completion_list_math(&self, curpos: &UrlPos) -> Vec<CompletionItem> {
        let UrlPos { url, pos } = curpos;
        if let Some(DocumentData::Parsed {
            program_text,
            environment,
        }) = self.get(url)
        {
            let pos_usize = program_text.from_position(pos);
            if pos_usize.is_none() {
                return vec![];
            }
            let pos_usize = pos_usize.unwrap();

            let local_commands = environment
                .math_cmds()
                .iter()
                .filter(|var| var.scope.includes(pos_usize))
                .map(|cmd| {
                    CompletionItem::new_simple(
                        cmd.name.clone(),
                        "math-cmd defined in this file".to_owned(),
                    )
                })
                .collect_vec();

            // TODO: 直接 require/import していない変数も取れるようにする
            let deps_commands = self
                .get_dependencies_recursive(environment.dependencies())
                .iter()
                .map(|dep| {
                    if let Some(DocumentData::Parsed {
                        environment: env_dep,
                        ..
                    }) = dep.url.as_ref().and_then(|url| self.get(url))
                    {
                        env_dep
                            .math_cmds_external(&[])
                            .iter()
                            .filter(|&cmd| {
                                matches!(cmd.visibility, Visibility::Public | Visibility::Direct)
                            })
                            .map(|cmd| {
                                CompletionItem::new_simple(
                                    cmd.name.clone(),
                                    format!("math-cmd defined in package '{}'", dep.name),
                                )
                            })
                            .collect_vec()
                    } else {
                        vec![]
                    }
                })
                .concat();

            [local_commands, deps_commands].concat()
        } else {
            vec![]
        }
    }
}