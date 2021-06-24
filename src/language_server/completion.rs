use std::collections::HashMap;

use itertools::Itertools;
use log::info;
use lspower::lsp::{
    CompletionItem, CompletionItemKind, CompletionResponse, Documentation, InsertTextFormat,
    MarkupContent, MarkupKind, Url,
};
use satysfi_parser::Mode;
use serde::Deserialize;

use crate::{
    documents::{ComponentBody, DocumentCache, DocumentData, Visibility},
    util::{ConvertPosition, UrlPos},
};

pub const COMPLETION_RESOUCES: &str = include_str!("../resource/completion_items.toml");

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
                self.get_completion_list_program(curpos)?,
            )),
            Mode::ProgramType => None,
            Mode::Vertical => Some(CompletionResponse::Array(
                self.get_completion_list_vertical(curpos)?,
            )),
            Mode::Horizontal => Some(CompletionResponse::Array(
                self.get_completion_list_horizontal(curpos)?,
            )),
            Mode::Math => Some(CompletionResponse::Array(
                self.get_completion_list_math(curpos)?,
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

    fn get_completion_list_program(&self, curpos: &UrlPos) -> Option<Vec<CompletionItem>> {
        let UrlPos { url, pos } = curpos;
        let doc_data = self.get(url)?;
        let (program_text, environment) = self.get_doc_info(url)?;
        let pos_usize = program_text.from_position(pos)?;

        let local_variables = environment
            .variables()
            .iter()
            .filter(|var| var.scope.includes(pos_usize))
            .map(|var| {
                variable_completion_item(
                    var.name.clone(),
                    "variable defined in this file".to_owned(),
                    if let ComponentBody::Variable {
                        type_declaration: Some(span),
                    } = var.body
                    {
                        self.get_text_from_span(&var.url, span)
                            .map(|s| s.to_owned())
                    } else {
                        None
                    },
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
                        .variables_external(&doc_data.get_open_modules(pos_usize))
                        .iter()
                        .map(|var| {
                            variable_completion_item(
                                var.name.clone(),
                                format!("variable defined in package `{}`", dep.name),
                                if let ComponentBody::Variable {
                                    type_declaration: Some(span),
                                } = var.body
                                {
                                    self.get_text_from_span(&var.url, span)
                                        .map(|s| s.to_owned())
                                } else {
                                    None
                                },
                            )
                        })
                        .collect_vec()
                } else {
                    vec![]
                }
            })
            .concat();

        let primitives = get_primitive_list();

        Some([local_variables, deps_variables, primitives].concat())
    }

    fn get_completion_list_horizontal(&self, curpos: &UrlPos) -> Option<Vec<CompletionItem>> {
        let UrlPos { url, pos } = curpos;
        let doc_data = self.get(url)?;
        let (program_text, environment) = self.get_doc_info(url)?;
        let pos_usize = program_text.from_position(pos)?;

        let local_commands = environment
            .inline_cmds()
            .iter()
            .filter(|var| var.scope.includes(pos_usize))
            .map(|cmd| {
                self.command_completion_item(
                    cmd.name.clone(),
                    "inline-cmd defined in this file".to_owned(),
                    &cmd.body,
                    &cmd.url,
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
                        .inline_cmds_external(&doc_data.get_open_modules(pos_usize))
                        .iter()
                        .filter(|&cmd| {
                            matches!(cmd.visibility, Visibility::Public | Visibility::Direct)
                        })
                        .map(|cmd| {
                            self.command_completion_item(
                                cmd.name.clone(),
                                format!("inline-cmd defined in package `{}`", dep.name),
                                &cmd.body,
                                &cmd.url,
                            )
                        })
                        .collect_vec()
                } else {
                    vec![]
                }
            })
            .concat();

        Some([local_commands, deps_commands].concat())
    }

    fn get_completion_list_vertical(&self, curpos: &UrlPos) -> Option<Vec<CompletionItem>> {
        let UrlPos { url, pos } = curpos;
        let doc_data = self.get(url)?;
        let (program_text, environment) = self.get_doc_info(url)?;
        let pos_usize = program_text.from_position(pos)?;

        let local_commands = environment
            .block_cmds()
            .iter()
            .filter(|var| var.scope.includes(pos_usize))
            .map(|cmd| {
                self.command_completion_item(
                    cmd.name.clone(),
                    "block-cmd defined in this file".to_owned(),
                    &cmd.body,
                    &cmd.url,
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
                        .block_cmds_external(&doc_data.get_open_modules(pos_usize))
                        .iter()
                        .filter(|&cmd| {
                            matches!(cmd.visibility, Visibility::Public | Visibility::Direct)
                        })
                        .map(|cmd| {
                            self.command_completion_item(
                                cmd.name.clone(),
                                format!("block-cmd defined in package `{}`", dep.name),
                                &cmd.body,
                                &cmd.url,
                            )
                        })
                        .collect_vec()
                } else {
                    vec![]
                }
            })
            .concat();

        Some([local_commands, deps_commands].concat())
    }

    fn get_completion_list_math(&self, curpos: &UrlPos) -> Option<Vec<CompletionItem>> {
        let UrlPos { url, pos } = curpos;
        let doc_data = self.get(url)?;
        let (program_text, environment) = self.get_doc_info(url)?;
        let pos_usize = program_text.from_position(pos)?;

        let local_commands = environment
            .math_cmds()
            .iter()
            .filter(|var| var.scope.includes(pos_usize))
            .map(|cmd| {
                self.command_completion_item(
                    cmd.name.clone(),
                    "math-cmd defined in this file".to_owned(),
                    &cmd.body,
                    &cmd.url,
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
                        .math_cmds_external(&doc_data.get_open_modules(pos_usize))
                        .iter()
                        .filter(|&cmd| {
                            matches!(cmd.visibility, Visibility::Public | Visibility::Direct)
                        })
                        .map(|cmd| {
                            self.command_completion_item(
                                cmd.name.clone(),
                                format!("math-cmd defined in package `{}`", dep.name),
                                &cmd.body,
                                &cmd.url,
                            )
                        })
                        .collect_vec()
                } else {
                    vec![]
                }
            })
            .concat();

        Some([local_commands, deps_commands].concat())
    }

    fn command_completion_item(
        &self,
        name: String,
        desc: String,
        body: &ComponentBody,
        url: &Url,
    ) -> CompletionItem {
        let (detail, insert_text, insert_text_format) = match body {
            ComponentBody::InlineCmd {
                type_declaration: Some(dec),
                type_args,
            } => (
                self.get_text_from_span(url, *dec).map(|s| s.to_owned()),
                Some(form_command_text_snippet(&name, type_args)),
                Some(InsertTextFormat::Snippet),
            ),
            ComponentBody::BlockCmd {
                type_declaration: Some(dec),
                type_args,
            } => (
                self.get_text_from_span(url, *dec).map(|s| s.to_owned()),
                Some(form_command_text_snippet(&name, type_args)),
                Some(InsertTextFormat::Snippet),
            ),
            ComponentBody::MathCmd {
                type_declaration: Some(dec),
                ..
            } => (
                self.get_text_from_span(url, *dec).map(|s| s.to_owned()),
                None,
                None,
            ),
            _ => (None, None, None),
        };
        CompletionItem {
            label: name,
            kind: Some(CompletionItemKind::Variable),
            detail,
            documentation: Some(Documentation::MarkupContent(MarkupContent {
                kind: MarkupKind::Markdown,
                value: desc,
            })),
            deprecated: None,
            preselect: None,
            sort_text: None,
            filter_text: None,
            insert_text,
            insert_text_format,
            insert_text_mode: None,
            text_edit: None,
            additional_text_edits: None,
            command: None,
            data: None,
            tags: None,
            commit_characters: None,
        }
    }
}

fn variable_completion_item(
    name: String,
    desc: String,
    type_declaration: Option<String>,
) -> CompletionItem {
    CompletionItem {
        label: name,
        kind: Some(CompletionItemKind::Function),
        detail: type_declaration,
        documentation: Some(Documentation::MarkupContent(MarkupContent {
            kind: MarkupKind::Markdown,
            value: desc,
        })),
        deprecated: None,
        preselect: None,
        sort_text: None,
        filter_text: None,
        insert_text: None,
        insert_text_format: None,
        insert_text_mode: None,
        text_edit: None,
        additional_text_edits: None,
        command: None,
        data: None,
        tags: None,
        commit_characters: None,
    }
}

/// コマンド名と型情報からコマンドのスニペットを自動生成する。
fn form_command_text_snippet(name: &str, type_args: &[String]) -> String {
    let args_str = type_args
        .iter()
        .map(|arg| ArgType::from_str(arg.as_str()))
        .filter(|arg| !arg.optional) // オプショナル引数はスニペットに含めない
        .collect_vec(); // rev() を行うため一旦 Vec に格納

    let mut snips = vec![];

    let mut require_semicolon = true;
    let mut compactible = true;
    for (idx, arg) in args_str.iter().enumerate().rev() {
        if !arg.is_compactible() {
            compactible = false;
        }
        snips.push(arg.as_snippet(idx + 1, compactible));
        if compactible {
            require_semicolon = false;
        }
    }

    snips.reverse();

    let name = if let Some("\\") = name.get(0..1) {
        // `\` はスニペットの場合エスケープする必要がある
        format!("\\{}", name)
    } else {
        name.to_owned()
    };

    format!(
        "{name}{args}{semicolon}$0",
        name = name,
        args = snips.into_iter().join(""),
        semicolon = if require_semicolon { ";" } else { "" }
    )
}

struct ArgType<'a> {
    name: &'a str,
    optional: bool,
}

impl<'a> ArgType<'a> {
    fn as_snippet(&self, idx: usize, short: bool) -> String {
        if self.optional {
            // 今のところはこのコードは使わない（はず）だけど一応
            let name = self.name;
            if name.len() > 5 && &name[name.len() - 4..] == "list" {
                return format!("${{{}:?:[]}}", idx);
            }
            return format!("${{{}:?:()}}", idx);
        }
        match self.name {
            "inline-text" => {
                if short {
                    format!("{{${}}}", idx)
                } else {
                    format!("({{${}}})", idx)
                }
            }
            "inline-text list" => {
                if short {
                    format!("{{|${}|}}", idx)
                } else {
                    format!("({{|${}|}})", idx)
                }
            }
            "itemize" => {
                if short {
                    format!("{{\n  * ${}\n}}", idx)
                } else {
                    format!("({{* ${}}})", idx)
                }
            }
            "block-text" => {
                if short {
                    format!("<\n  ${}\n>", idx)
                } else {
                    format!("('<${}>)", idx)
                }
            }
            s if s.len() > 5 && &s[s.len() - 4..] == "list" => format!("[${}]", idx),
            _ => format!("(${})", idx),
        }
    }

    fn from_str(text: &'a str) -> Self {
        let text = text.trim();
        if let Some('?') = text.chars().last() {
            ArgType {
                name: &text[..text.len() - 1].trim(),
                optional: true,
            }
        } else {
            ArgType {
                name: text,
                optional: false,
            }
        }
    }

    fn is_compactible(&self) -> bool {
        !self.optional
            && matches!(
                self.name,
                "inline-text" | "inline-text list" | "itemize" | "block-text"
            )
    }
}
