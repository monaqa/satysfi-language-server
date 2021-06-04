use lspower::lsp::{GotoDefinitionResponse, Location, Range};
use satysfi_parser::{Cst, Rule};

use crate::{
    documents::{Component, DocumentCache, DocumentData},
    util::{ConvertPosition, UrlPos},
};

impl DocumentCache {
    pub fn get_definition_list(&self, curpos: &UrlPos) -> Option<GotoDefinitionResponse> {
        let (url, pos_definition) = self
            .find_component_under_cursor(curpos)
            .map(|(_, comp)| (&comp.url, comp.pos_definition))?;

        let range = if let DocumentData::Parsed { program_text, .. } = self.get(url).unwrap() {
            Range {
                start: program_text.get_position(pos_definition.start).unwrap(),
                end: program_text.get_position(pos_definition.end).unwrap(),
            }
        } else {
            unreachable!()
        };
        let loc = Location {
            uri: url.to_owned(),
            range,
        };
        let resp = GotoDefinitionResponse::Scalar(loc);

        Some(resp)
    }

    pub fn _find_word_under_cursor<'a>(&'a self, curpos: &UrlPos) -> Option<&'a Cst> {
        let UrlPos { url, pos } = curpos;
        if let Some(DocumentData::Parsed { program_text, .. }) = self.0.get(&url) {
            let pos_usize = program_text.from_position(&pos).unwrap();
            // カーソル上にある variable や inline-cmd の CST を抽出する
            program_text.cst.dig(pos_usize).into_iter().find(|&cst| {
                [
                    Rule::var,
                    Rule::type_name,
                    Rule::variant_name,
                    Rule::module_name,
                    Rule::inline_cmd_name,
                    Rule::block_cmd_name,
                    Rule::math_cmd_name,
                ]
                .contains(&cst.rule)
            })
        } else {
            None
        }
    }

    /// カーソル下にあるコンポーネント（変数、コマンド、型など）と同じものを検索する。
    pub fn find_component_under_cursor<'a>(
        &'a self,
        curpos: &UrlPos,
    ) -> Option<(&'a Cst, &'a Component)> {
        let UrlPos { url, pos } = curpos;
        if let Some(DocumentData::Parsed {
            program_text,
            environment,
        }) = self.0.get(&url)
        {
            let pos_usize = program_text.from_position(&pos).unwrap();
            // カーソル上にある variable や inline-cmd の CST を抽出する
            let cst = program_text.cst.dig(pos_usize).into_iter().find(|&cst| {
                [
                    Rule::var,
                    Rule::type_name,
                    Rule::variant_name,
                    Rule::module_name,
                    Rule::inline_cmd_name,
                    Rule::block_cmd_name,
                    Rule::math_cmd_name,
                ]
                .contains(&cst.rule)
            })?;
            // カーソル上にある variable や inline-cmd の CST
            // 検索したい変数・コマンド名
            let name = program_text.get_text(cst);

            let component = match cst.rule {
                Rule::var => {
                    // カーソルがスコープ内にあって、かつ名前の一致するもの
                    let local = environment
                        .variables_external(&[])
                        .into_iter()
                        .find(|&var| var.scope.includes(pos_usize) && var.name == name);

                    // dependency 内にある public な変数で、名前が一致するもの
                    let deps = environment.dependencies().iter().find_map(|dep| {
                        if let Some(DocumentData::Parsed {
                            environment: env_dep,
                            ..
                        }) = dep.url.as_ref().and_then(|url| self.get(url))
                        {
                            env_dep
                                .variables_external(&[])
                                .into_iter()
                                .find(|&var| var.name == name)
                        } else {
                            None
                        }
                    });
                    local.or(deps)
                }
                Rule::type_name => {
                    todo!();
                }
                Rule::variant_name => {
                    todo!();
                }
                Rule::module_name => {
                    todo!();
                }
                Rule::inline_cmd_name => {
                    // カーソルがスコープ内にあって、かつ名前の一致するもの
                    let local = environment
                        .inline_cmds_external(&[])
                        .into_iter()
                        .find(|&cmd| cmd.scope.includes(pos_usize) && cmd.name == name);

                    // dependency 内にある public な変数で、名前が一致するもの
                    let deps = environment.dependencies().iter().find_map(|dep| {
                        if let Some(DocumentData::Parsed {
                            environment: env_dep,
                            ..
                        }) = dep.url.as_ref().and_then(|url| self.get(url))
                        {
                            env_dep
                                .inline_cmds_external(&[])
                                .into_iter()
                                .find(|&cmd| cmd.name == name)
                        } else {
                            None
                        }
                    });
                    local.or(deps)
                }
                Rule::block_cmd_name => {
                    todo!();
                }
                Rule::math_cmd_name => {
                    todo!();
                }
                _ => unreachable!(),
            };
            component.map(|c| (cst, c))
        } else {
            None
        }
    }
}
