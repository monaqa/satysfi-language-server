use lspower::lsp::{GotoDefinitionResponse, Location, Range};
use satysfi_parser::Rule;

use crate::{
    documents::{DocumentCache, DocumentData},
    util::{ConvertPosition, UrlPos},
};

impl DocumentCache {
    pub fn get_definition_list(&self, curpos: &UrlPos) -> Option<GotoDefinitionResponse> {
        let UrlPos { url, pos } = curpos;
        if let Some(DocumentData::Parsed {
            program_text: csttext,
            environment,
        }) = self.0.get(&url)
        {
            let pos_usize = csttext.from_position(&pos).unwrap();
            // カーソル上にある variable や inline-cmd の CST を抽出する
            let cst = csttext
                .cst
                .dig(pos_usize)
                .into_iter()
                .find(|&cst| [Rule::var, Rule::inline_cmd_name].contains(&cst.rule))?;
            // カーソル上にある variable や inline-cmd の CST
            // 検索したい変数・コマンド名
            let name = csttext.get_text(cst);

            let (url, pos_definition) = match cst.rule {
                Rule::var => {
                    let local = environment
                        .variables_external(&[])
                        .iter()
                        // カーソルがスコープ内にあって、かつ名前の一致するもの
                        .find(|var| var.scope.includes(pos_usize) && var.name == name)
                        .map(|var| (&var.url, var.pos_definition));
                    let deps = environment.dependencies().iter().find_map(|dep| {
                        if let Some(DocumentData::Parsed {
                            environment: env_dep,
                            ..
                        }) = dep.url.as_ref().and_then(|url| self.get(url))
                        {
                            env_dep
                                .variables_external(&[])
                                .iter()
                                .find(|var| var.name == name)
                                .map(|var| (&var.url, var.pos_definition))
                        } else {
                            None
                        }
                    });
                    local.or(deps)
                }
                Rule::inline_cmd_name => {
                    let local = environment
                        .inline_cmds_external(&[])
                        .iter()
                        // カーソルがスコープ内にあって、かつ名前の一致するもの
                        .find(|var| var.scope.includes(pos_usize) && var.name == name)
                        .map(|var| (&var.url, var.pos_definition));
                    let deps = environment.dependencies().iter().find_map(|dep| {
                        if let Some(DocumentData::Parsed {
                            environment: env_dep,
                            ..
                        }) = dep.url.as_ref().and_then(|url| self.get(url))
                        {
                            env_dep
                                .inline_cmds_external(&[])
                                .iter()
                                .find(|var| var.name == name)
                                .map(|var| (&var.url, var.pos_definition))
                        } else {
                            None
                        }
                    });
                    local.or(deps)
                }
                _ => unreachable!(),
            }?;

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
        } else {
            None
        }
    }
}
