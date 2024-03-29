use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Result};
use itertools::Itertools;
use log::info;
use lspower::lsp::{Position, Url};
use satysfi_parser::{
    grammar::{type_block_cmd, type_inline_cmd, type_math_cmd},
    structure::{Header, LetRecInner, Program, ProgramText, Signature, Statement, TypeInner},
    Cst, CstText, LineCol, Rule, Span,
};

use crate::util::{ConvertPosition, UrlPos};

/// オンメモリで取り扱うデータをまとめたデータ構造。
#[derive(Debug, Default)]
pub struct DocumentCache(pub HashMap<Url, DocumentData>);

impl DocumentCache {
    pub fn get(&self, url: &Url) -> Option<&DocumentData> {
        self.0.get(url)
    }

    pub fn get_doc_info(&self, url: &Url) -> Option<(&ProgramText, &Environment)> {
        if let Some(DocumentData::Parsed {
            program_text,
            environment,
        }) = self.get(url)
        {
            Some((program_text, environment))
        } else {
            None
        }
    }

    pub fn get_text_from_span<'a>(&'a self, url: &Url, span: Span) -> Option<&'a str> {
        let doc = self.0.get(url)?;
        if let DocumentData::Parsed { program_text, .. } = doc {
            Some(program_text.get_text_from_span(span))
        } else {
            None
        }
    }

    /// カーソルを含む行内容を抽出する。
    pub fn get_line<'a>(&'a self, urlpos: &UrlPos) -> Option<&'a str> {
        let UrlPos { url, pos } = &urlpos;
        match self.0.get(url)? {
            DocumentData::Parsed { program_text, .. } => {
                let pos_usize = program_text.from_position(&pos)?;
                let LineCol { line, .. } = program_text.get_line_col(pos_usize)?;
                let start = *program_text.lines.get(line)?;
                let end = *program_text
                    .lines
                    .get(line + 1)
                    .unwrap_or(&program_text.text.len());
                Some(&program_text.text.as_str()[start..end])
            }
            DocumentData::NotParsed { text, .. } => {
                let line = pos.line as usize;
                text.split('\n').nth(line)
            }
        }
    }

    /// dependencies の中のパッケージについてパースし、 Environment 情報の登録を行う。
    /// この操作は再帰的に行う。
    pub fn register_dependencies(&mut self, deps: &[Dependency]) {
        for dep in deps {
            if let Some(url) = &dep.url {
                // 既に登録されている url は一度読んでいるので skip
                if self.0.get(url).is_none() {
                    if let Ok(doc_data) = DocumentData::new_from_file(url) {
                        self.0.insert(url.clone(), doc_data);
                        // 上で格納したファイルの中に dependencies 情報があればクローンして取り出す
                        let dependencies = self.0.get(url).and_then(|doc| {
                            if let DocumentData::Parsed { environment, .. } = doc {
                                Some(environment.dependencies.clone())
                            } else {
                                None
                            }
                        });
                        if let Some(dependencies) = dependencies {
                            self.register_dependencies(&dependencies);
                        }
                    }
                }
            }
        }
    }

    pub fn get_dependencies_recursive<'a>(&'a self, deps: &'a [Dependency]) -> Vec<&'a Dependency> {
        deps.iter()
            .map(|dep| self.get_dependency_recursive(dep).into_iter().collect_vec())
            .concat()
            .into_iter()
            .collect::<HashMap<_, _>>() // 重複した URL を排除
            .into_iter()
            .map(|(_, dep)| dep)
            .collect_vec()
    }

    /// その dependency 先のファイルを読み、そのファイルが依存しているものを再帰的に取り出す。
    fn get_dependency_recursive<'a>(&'a self, dep: &'a Dependency) -> HashMap<Url, &'a Dependency> {
        let mut hm = HashMap::new();
        if let Some(url) = &dep.url {
            if !hm.contains_key(url) {
                hm.insert(url.clone(), dep);
            }
        }

        if let Some(DocumentData::Parsed {
            program_text,
            environment,
        }) = dep.url.as_ref().and_then(|url| self.0.get(url))
        {
            for dep in &environment.dependencies {
                if let Some(url) = &dep.url {
                    if !hm.contains_key(url) {
                        hm.insert(url.clone(), dep);
                        let child_hm = self.get_dependency_recursive(dep);
                        hm = hm.into_iter().chain(child_hm).collect();
                    }
                }
            }
        }
        hm
    }
}

/// 一つのファイルに関するデータを纏めたデータ構造。
#[derive(Debug)]
pub enum DocumentData {
    /// パーサによって正常にパースできたデータ。
    Parsed {
        /// パース結果の具象構文木 + テキスト本体。
        program_text: ProgramText,
        /// このファイルで定義されている変数やコマンドに関する情報。
        environment: Environment,
    },

    /// パーサによってパースできなかったデータ。
    NotParsed {
        /// テキスト本体。
        text: String,
        /// エラー箇所。
        linecol: LineCol,
        /// エラー箇所にて期待するパターン（終端記号）列。
        expect: Vec<&'static str>,
    },
}

impl DocumentData {
    /// テキストから新たな DocumentData を作成する。
    pub fn new(text: &str, url: &Url) -> DocumentData {
        match ProgramText::parse(text) {
            Ok(program_text) => {
                let environment = Environment::from_program(&program_text, &url);
                DocumentData::Parsed {
                    program_text,
                    environment,
                }
            }
            Err((linecol, expect)) => {
                let text = text.to_owned();
                DocumentData::NotParsed {
                    text,
                    linecol,
                    expect,
                }
            }
        }
    }

    pub fn new_from_file(url: &Url) -> Result<DocumentData> {
        if let Ok(fpath) = url.to_file_path() {
            let text = std::fs::read_to_string(&fpath)?;
            Ok(DocumentData::new(&text, url))
        } else {
            Err(anyhow!("Failed to convert url to file path."))
        }
    }

    pub fn show_envs_debug(&self) {
        match self {
            DocumentData::Parsed {
                environment,
                program_text,
            } => {
                environment.show_debug();
                let cst_text = CstText {
                    text: program_text.text.clone(),
                    lines: program_text.lines.clone(),
                    cst: program_text.cst.clone(),
                };
                info!("{cst_text}");
            }
            DocumentData::NotParsed { .. } => {}
        }
    }

    /// その position において、 Open している module 名の一覧を表示する。
    pub fn get_open_modules(&self, pos: usize) -> Vec<String> {
        match self {
            DocumentData::Parsed {
                program_text,
                environment,
            } => {
                let open_stmts = environment
                    .open_modules
                    .iter()
                    .filter(|opmod| opmod.scope.includes(pos))
                    .map(|opmod| opmod.name.clone());
                let csts = program_text.cst.dig(pos);
                let binded_open_stmts = csts
                    .iter()
                    .filter(|&cst| {
                        cst.rule == Rule::bind_stmt
                            && cst.inner.get(0).unwrap().rule == Rule::open_stmt
                    })
                    .map(|cst| program_text.get_text(cst).to_owned());
                binded_open_stmts.chain(open_stmts).collect()
            }
            DocumentData::NotParsed { .. } => vec![],
        }
    }

    /// その position において、 Module.( | ) のようになっている module 名の一覧を表示する。
    pub fn get_localized_modules(&self, pos: usize) -> Vec<String> {
        match self {
            DocumentData::Parsed { program_text, .. } => program_text
                .cst
                .dig(pos)
                .into_iter()
                .filter(|cst| cst.rule == Rule::expr_with_mod)
                .flat_map(|cst| {
                    cst.inner.iter().find_map(|inner| {
                        if inner.rule == Rule::module_name {
                            Some(inner.span)
                        } else {
                            None
                        }
                    })
                })
                .map(|span| program_text.get_text_from_span(span).to_owned())
                .collect_vec(),
            DocumentData::NotParsed { .. } => vec![],
        }
    }
}

/// 変数やコマンドに関する情報。
#[derive(Debug, Default)]
pub struct Environment {
    dependencies: Vec<Dependency>,
    components: Vec<Component>,
    open_modules: Vec<OpenModule>,
}

impl Environment {
    pub fn from_program(program_text: &ProgramText, url: &Url) -> Self {
        match &program_text.structure {
            Ok(structure) => {
                let (header, preamble) = match structure {
                    Program::Saty {
                        header, preamble, ..
                    } => (header, preamble),
                    Program::Satyh {
                        header, preamble, ..
                    } => (header, preamble),
                };
                let header = header.iter().collect_vec();
                let preamble = preamble.iter().collect_vec();
                let dependencies = Dependency::from_header(&header, program_text, url);
                let components = Component::from_preamble(&preamble, program_text, url);
                let open_modules = OpenModule::from_preamble(&preamble, program_text, url);
                Environment {
                    dependencies,
                    components,
                    open_modules,
                }
            }
            Err(_) => Environment::default(),
        }
    }

    /// Get a reference to the environment's dependencies.
    pub fn dependencies(&self) -> &[Dependency] {
        self.dependencies.as_slice()
    }

    pub fn modules(&self) -> Vec<&Component> {
        self.components
            .iter()
            .filter(|c| matches!(c.body, ComponentBody::Module { .. }))
            .collect_vec()
    }

    pub fn variables(&self) -> Vec<&Component> {
        self.components
            .iter()
            .filter(|c| matches!(c.body, ComponentBody::Variable { .. }))
            .collect_vec()
    }

    pub fn types(&self) -> Vec<&Component> {
        self.components
            .iter()
            .filter(|c| matches!(c.body, ComponentBody::Type { .. }))
            .collect_vec()
    }

    pub fn variants(&self) -> Vec<&Component> {
        self.components
            .iter()
            .filter(|c| matches!(c.body, ComponentBody::Variant { .. }))
            .collect_vec()
    }

    pub fn inline_cmds(&self) -> Vec<&Component> {
        self.components
            .iter()
            .filter(|c| matches!(c.body, ComponentBody::InlineCmd { .. }))
            .collect_vec()
    }

    pub fn block_cmds(&self) -> Vec<&Component> {
        self.components
            .iter()
            .filter(|c| matches!(c.body, ComponentBody::BlockCmd { .. }))
            .collect_vec()
    }

    pub fn math_cmds(&self) -> Vec<&Component> {
        self.components
            .iter()
            .filter(|c| matches!(c.body, ComponentBody::MathCmd { .. }))
            .collect_vec()
    }

    pub fn variables_external(&self, open_modules: &[String]) -> Vec<&Component> {
        let local = self.variables();
        let in_mods = self
            .modules()
            .iter()
            .filter(|&module| open_modules.contains(&module.name))
            .map(|module| match &module.body {
                ComponentBody::Module { components } => components
                    .iter()
                    .filter(|c| matches!(c.visibility, Visibility::Public))
                    .filter(|c| matches!(c.body, ComponentBody::Variable { .. }))
                    .collect_vec(),
                _ => unreachable!(),
            })
            .concat();
        [local, in_mods].concat()
    }

    pub fn types_external(&self, open_modules: &[String]) -> Vec<&Component> {
        let local = self.types();
        let in_mods = self
            .modules()
            .iter()
            .filter(|&module| open_modules.contains(&module.name))
            .map(|module| match &module.body {
                ComponentBody::Module { components } => components
                    .iter()
                    .filter(|c| matches!(c.visibility, Visibility::Public))
                    .filter(|c| matches!(c.body, ComponentBody::Type { .. }))
                    .collect_vec(),
                _ => unreachable!(),
            })
            .concat();
        [local, in_mods].concat()
    }

    pub fn variants_external(&self, open_modules: &[String]) -> Vec<&Component> {
        let local = self.variants();
        let in_mods = self
            .modules()
            .iter()
            .map(|module| match &module.body {
                ComponentBody::Module { components } => components
                    .iter()
                    .filter(|c| matches!(c.visibility, Visibility::Public))
                    .filter(|c| matches!(c.body, ComponentBody::Variant { .. }))
                    .collect_vec(),
                _ => unreachable!(),
            })
            .concat();
        [local, in_mods].concat()
    }

    pub fn inline_cmds_external(&self, open_modules: &[String]) -> Vec<&Component> {
        let local = self.inline_cmds();

        let in_mods = self
            .modules()
            .iter()
            .filter(|&module| open_modules.contains(&module.name))
            .map(|module| match &module.body {
                ComponentBody::Module { components } => components
                    .iter()
                    .filter(|c| matches!(c.visibility, Visibility::Public))
                    .filter(|c| matches!(c.body, ComponentBody::InlineCmd { .. }))
                    .collect_vec(),
                _ => unreachable!(),
            })
            .concat();

        let in_mods_direct = self
            .modules()
            .iter()
            .map(|module| match &module.body {
                ComponentBody::Module { components } => components
                    .iter()
                    .filter(|c| matches!(c.visibility, Visibility::Direct))
                    .filter(|c| matches!(c.body, ComponentBody::InlineCmd { .. }))
                    .collect_vec(),
                _ => unreachable!(),
            })
            .concat();

        [local, in_mods, in_mods_direct].concat()
    }

    pub fn block_cmds_external(&self, open_modules: &[String]) -> Vec<&Component> {
        let local = self.block_cmds();

        let in_mods = self
            .modules()
            .iter()
            .filter(|&module| open_modules.contains(&module.name))
            .map(|module| match &module.body {
                ComponentBody::Module { components } => components
                    .iter()
                    .filter(|c| matches!(c.visibility, Visibility::Public))
                    .filter(|c| matches!(c.body, ComponentBody::BlockCmd { .. }))
                    .collect_vec(),
                _ => unreachable!(),
            })
            .concat();

        let in_mods_direct = self
            .modules()
            .iter()
            .map(|module| match &module.body {
                ComponentBody::Module { components } => components
                    .iter()
                    .filter(|c| matches!(c.visibility, Visibility::Direct))
                    .filter(|c| matches!(c.body, ComponentBody::BlockCmd { .. }))
                    .collect_vec(),
                _ => unreachable!(),
            })
            .concat();

        [local, in_mods, in_mods_direct].concat()
    }

    pub fn math_cmds_external(&self, open_modules: &[String]) -> Vec<&Component> {
        let local = self.math_cmds();

        let in_mods = self
            .modules()
            .iter()
            .filter(|&module| open_modules.contains(&module.name))
            .map(|module| match &module.body {
                ComponentBody::Module { components } => components
                    .iter()
                    .filter(|c| matches!(c.visibility, Visibility::Public))
                    .filter(|c| matches!(c.body, ComponentBody::MathCmd { .. }))
                    .collect_vec(),
                _ => unreachable!(),
            })
            .concat();

        let in_mods_direct = self
            .modules()
            .iter()
            .map(|module| match &module.body {
                ComponentBody::Module { components } => components
                    .iter()
                    .filter(|c| matches!(c.visibility, Visibility::Direct))
                    .filter(|c| matches!(c.body, ComponentBody::MathCmd { .. }))
                    .collect_vec(),
                _ => unreachable!(),
            })
            .concat();

        [local, in_mods, in_mods_direct].concat()
    }

    pub fn show_debug(&self) {
        for dep in &self.dependencies {
            info!("Dependency: {:?}", dep.name);
        }
        for module in self.modules() {
            info!("Module: {:?}", module.name);
        }
        for var in self.variables() {
            info!("Varable: {:?}", var.name);
        }
        for cmd in self.inline_cmds() {
            info!("InlineCmd: {:?}", cmd.name);
        }
        for cmd in self.block_cmds() {
            info!("BlockCmd: {:?}", cmd.name);
        }
        for cmd in self.math_cmds() {
            info!("BlockCmd: {:?}", cmd.name);
        }
    }
}

#[derive(Debug, Clone)]
pub struct Dependency {
    /// パッケージ名。
    pub name: String,
    /// require か import か。
    pub kind: DependencyKind,
    /// `@require:` や `@import` が呼ばれている場所。
    pub definition: Span,
    /// 実際のファイルパス。パスを解決できなかったら None を返す。
    pub url: Option<Url>,
}

impl Dependency {
    fn from_header(headers: &[&Header], program_text: &ProgramText, url: &Url) -> Vec<Dependency> {
        let require_packages = headers.iter().map(|header| &header.name);
        let import_packages = headers.iter().map(|header| &header.name);

        let mut deps = vec![];
        let home_path = std::env::var("HOME").map(PathBuf::from).ok();
        let file_path = url.to_file_path().ok();
        let parent_path = file_path.as_ref().map(|p| p.parent().unwrap().to_owned());

        // require 系のパッケージの依存関係追加
        let require_dependencies = require_packages.map(|pkg| {
            let pkgname = program_text.get_text(pkg);
            // TODO: consider satyg file
            for pkgpath in
                require_candidate_paths(pkgname, parent_path.as_deref(), home_path.as_deref())
            {
                if pkgpath.exists() {
                    let url = Url::from_file_path(pkgpath).ok();
                    return Dependency {
                        name: pkgname.to_owned(),
                        kind: DependencyKind::Require,
                        definition: pkg.span,
                        url,
                    };
                }
            }
            Dependency {
                name: pkgname.to_owned(),
                kind: DependencyKind::Require,
                definition: pkg.span,
                url: None,
            }
        });
        deps.extend(require_dependencies);

        if let Some(file_path) = file_path {
            // TODO: add validate
            let parent_path = file_path.parent().unwrap();

            let import_dependencies = import_packages.map(|pkg| {
                let pkgname = program_text.get_text(pkg);
                // TODO: consider satyg file
                let pkg_path = parent_path.join(format!("{}.satyh", pkgname));
                let url = if pkg_path.exists() {
                    Url::from_file_path(pkg_path).ok()
                } else {
                    None
                };
                Dependency {
                    name: pkgname.to_owned(),
                    kind: DependencyKind::Import,
                    definition: pkg.span,
                    url,
                }
            });

            deps.extend(import_dependencies);
        }

        deps
    }
}

/// 以下の4箇所から探す。
/// - $PARENT_PATH/.satysfi/{kind}/packages/a.{ext}
/// - $HOME/.satysfi/{kind}/packages/a.{ext}
/// - /usr/local/share/satysfi/{kind}/packages/a.{ext}
/// - /usr/share/satysfi/{kind}/packages/a.{ext}
/// kind: local, dist
/// ext: satyh, satyg
fn require_candidate_paths(
    pkgname: &str,
    parent: Option<&Path>,
    home: Option<&Path>,
) -> Vec<PathBuf> {
    let usr_local_share = Some(PathBuf::from("/usr/local/share/satysfi"));
    let usr_share = Some(PathBuf::from("/usr/share/satysfi"));
    let home = home.map(|p| p.join(".satysfi"));
    let parent = parent.map(|p| p.join(".satysfi"));
    [parent, home, usr_local_share, usr_share]
        .iter()
        .filter_map(|x| x.clone())
        .map(|path| {
            vec![
                path.join(format!("local/packages/{}.satyh", pkgname)),
                path.join(format!("local/packages/{}.satyg", pkgname)),
                path.join(format!("dist/packages/{}.satyh", pkgname)),
                path.join(format!("dist/packages/{}.satyg", pkgname)),
            ]
        })
        .concat()
}

pub fn require_candidate_dirs(parent: Option<&Path>, home: Option<&Path>) -> Vec<PathBuf> {
    let usr_local_share = Some(PathBuf::from("/usr/local/share/satysfi"));
    let usr_share = Some(PathBuf::from("/usr/share/satysfi"));
    let home = home.map(|p| p.join(".satysfi"));
    let parent = parent.map(|p| p.join(".satysfi"));
    [parent, home, usr_local_share, usr_share]
        .iter()
        .filter_map(|x| x.clone())
        .map(|path| vec![path.join("local/packages"), path.join("dist/packages")])
        .concat()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DependencyKind {
    Require,
    Import,
}

#[derive(Debug)]
pub struct Component {
    /// 名前。
    pub name: String,
    /// 種類。
    pub body: ComponentBody,
    /// その変数がそのファイル内で有効なスコープ。
    pub scope: Span,
    /// 定義がどこにあるか。
    pub pos_definition: Span,
    /// 可視性。パッケージ内に直接定義されていたら public。
    /// module 内のときは signature があるかどうかで変わる。
    pub visibility: Visibility,
    /// モジュール内のパブリック変数のとき、宣言がどこにあるか。
    pub pos_declaration: Option<Span>,
    /// そのコンポーネントが定義されている URL。
    pub url: Url,
}

/// モジュールについての情報。モジュール内で定義された変数を格納するのに用いる。
struct ModuleInfo<'a> {
    module_span: Span,
    sigs: &'a [Signature],
}

impl<'a> ModuleInfo<'a> {
    fn map_types<'b>(&self, program_text: &'b ProgramText) -> HashMap<&'b str, &Signature> {
        self.sigs
            .iter()
            .filter_map(|sig| match sig {
                Signature::Type { name, .. } => Some((program_text.get_text(name), sig)),
                _ => None,
            })
            .collect()
    }
    fn map_val<'b>(&self, program_text: &'b ProgramText) -> HashMap<&'b str, &Signature> {
        self.sigs
            .iter()
            .filter_map(|sig| match sig {
                Signature::Val { var, .. } => Some((program_text.get_text(var), sig)),
                _ => None,
            })
            .collect()
    }
    fn map_direct<'b>(&self, program_text: &'b ProgramText) -> HashMap<&'b str, &Signature> {
        self.sigs
            .iter()
            .filter_map(|sig| match sig {
                Signature::Direct { var, .. } => Some((program_text.get_text(var), sig)),
                _ => None,
            })
            .collect()
    }
}

impl Component {
    fn from_preamble(
        preamble: &[&Statement],
        program_text: &ProgramText,
        url: &Url,
    ) -> Vec<Component> {
        preamble
            .iter()
            .map(|stmt| Component::from_stmt(stmt, None, program_text, url))
            .concat()
    }

    fn from_struct_stmts(
        module_info: &ModuleInfo,
        struct_stmts: &[&Statement],
        program_text: &ProgramText,
        url: &Url,
    ) -> Vec<Component> {
        struct_stmts
            .iter()
            .map(|stmt| Component::from_stmt(stmt, Some(module_info), program_text, url))
            .concat()
    }

    /// Statement から Component を生成する。
    /// Component は複数出てくることもあるため、戻り値はベクトル。というのも
    /// let (x, y) = ...
    /// みたいな式では x, y という2つの Component が作成されるため。
    fn from_stmt(
        stmt: &Statement,
        module_info: Option<&ModuleInfo>,
        program_text: &ProgramText,
        url: &Url,
    ) -> Vec<Component> {
        match stmt {
            Statement::Let { pat, expr, .. } => {
                let vars = pat.pickup(Rule::var);
                let scope = {
                    let start = expr.span.end;
                    let end = if let Some(info) = module_info {
                        info.module_span.end
                    } else {
                        program_text.cst.span.end
                    };
                    Span { start, end }
                };
                vars.into_iter()
                    .map(|var| Component::new_variable(var, scope, module_info, program_text, url))
                    .collect()
            }

            Statement::LetRec(inners) => {
                let scope = {
                    // recursive のため自身の関数の定義内で自身の関数を呼び出せる
                    let start = inners.get(0).unwrap().pattern.span.end;
                    let end = if let Some(info) = module_info {
                        info.module_span.end
                    } else {
                        program_text.cst.span.end
                    };
                    Span { start, end }
                };
                inners
                    .iter()
                    .map(|LetRecInner { pattern, .. }| {
                        let vars = pattern.pickup(Rule::var);
                        vars.into_iter()
                            .map(|var| {
                                Component::new_variable(var, scope, module_info, program_text, url)
                            })
                            .collect()
                    })
                    .concat()
            }

            Statement::LetInline { cmd, expr, .. } => {
                let name = program_text.get_text(cmd).to_owned();
                let scope = {
                    let start = expr.span.end;
                    let end = if let Some(info) = module_info {
                        info.module_span.end
                    } else {
                        program_text.cst.span.end
                    };
                    Span { start, end }
                };
                let pos_definition = cmd.span;
                let (visibility, pos_declaration, signature) = {
                    if let Some(info) = module_info {
                        let sig_val_map = info.map_val(program_text);
                        let sig_direct_map = info.map_direct(program_text);
                        let name = program_text.get_text(cmd);
                        match (sig_direct_map.get(name), sig_val_map.get(name)) {
                            (Some(Signature::Direct { var, signature, .. }), _) => {
                                (Visibility::Direct, Some(var.span), Some(signature))
                            }
                            (None, Some(Signature::Val { var, signature, .. })) => {
                                (Visibility::Public, Some(var.span), Some(signature))
                            }
                            _ => (Visibility::Private, None, None),
                        }
                    } else {
                        (Visibility::Public, None, None)
                    }
                };
                let body = if let Some(signature) = signature {
                    let text = program_text.get_text(signature);
                    let csts = type_inline_cmd(text).ok().unwrap().inner;
                    ComponentBody::InlineCmd {
                        type_declaration: Some(signature.span),
                        type_args: csts
                            .into_iter()
                            .map(|cst| text[cst.span.start..cst.span.end].to_owned())
                            .collect_vec(),
                    }
                } else {
                    ComponentBody::InlineCmd {
                        type_declaration: None,
                        type_args: vec![],
                    }
                };
                vec![Component {
                    name,
                    body,
                    scope,
                    pos_definition,
                    visibility,
                    pos_declaration,
                    url: url.clone(),
                }]
            }

            Statement::LetBlock { cmd, expr, .. } => {
                let name = program_text.get_text(cmd).to_owned();
                let start = expr.span.end;
                let end = if let Some(info) = module_info {
                    info.module_span.end
                } else {
                    program_text.cst.span.end
                };
                let scope = Span { start, end };
                let pos_definition = cmd.span;
                let (visibility, pos_declaration, signature) = {
                    if let Some(info) = module_info {
                        let sig_val_map = info.map_val(program_text);
                        let sig_direct_map = info.map_direct(program_text);
                        let name = program_text.get_text(cmd);
                        match (sig_direct_map.get(name), sig_val_map.get(name)) {
                            (Some(Signature::Direct { var, signature, .. }), _) => {
                                (Visibility::Direct, Some(var.span), Some(signature))
                            }
                            (None, Some(Signature::Val { var, signature, .. })) => {
                                (Visibility::Public, Some(var.span), Some(signature))
                            }
                            _ => (Visibility::Private, None, None),
                        }
                    } else {
                        (Visibility::Public, None, None)
                    }
                };
                let body = if let Some(signature) = signature {
                    let text = program_text.get_text(signature);
                    let csts = type_block_cmd(text).ok().unwrap().inner;
                    ComponentBody::BlockCmd {
                        type_declaration: Some(signature.span),
                        type_args: csts
                            .into_iter()
                            .map(|cst| text[cst.span.start..cst.span.end].to_owned())
                            .collect_vec(),
                    }
                } else {
                    ComponentBody::BlockCmd {
                        type_declaration: None,
                        type_args: vec![],
                    }
                };
                vec![Component {
                    name,
                    body,
                    scope,
                    pos_definition,
                    visibility,
                    pos_declaration,
                    url: url.clone(),
                }]
            }

            Statement::LetMath { cmd, expr, .. } => {
                let name = program_text.get_text(cmd).to_owned();
                let start = expr.span.end;
                let end = if let Some(info) = module_info {
                    info.module_span.end
                } else {
                    program_text.cst.span.end
                };
                let scope = Span { start, end };
                let pos_definition = cmd.span;
                let (visibility, pos_declaration, signature) = {
                    if let Some(info) = module_info {
                        let sig_val_map = info.map_val(program_text);
                        let sig_direct_map = info.map_direct(program_text);
                        let name = program_text.get_text(cmd);
                        match (sig_direct_map.get(name), sig_val_map.get(name)) {
                            (Some(Signature::Direct { var, signature, .. }), _) => {
                                (Visibility::Direct, Some(var.span), Some(signature))
                            }
                            (None, Some(Signature::Val { var, signature, .. })) => {
                                (Visibility::Public, Some(var.span), Some(signature))
                            }
                            _ => (Visibility::Private, None, None),
                        }
                    } else {
                        (Visibility::Public, None, None)
                    }
                };
                let body = if let Some(signature) = signature {
                    let text = program_text.get_text(signature);
                    let csts = type_math_cmd(text).ok().unwrap().inner;
                    ComponentBody::MathCmd {
                        type_declaration: Some(signature.span),
                        type_args: csts
                            .into_iter()
                            .map(|cst| text[cst.span.start..cst.span.end].to_owned())
                            .collect_vec(),
                    }
                } else {
                    ComponentBody::MathCmd {
                        type_declaration: None,
                        type_args: vec![],
                    }
                };
                vec![Component {
                    name,
                    body,
                    scope,
                    pos_definition,
                    visibility,
                    pos_declaration,
                    url: url.clone(),
                }]
            }

            Statement::LetMutable { var, expr } => {
                let name = program_text.get_text(var).to_owned();
                let body = ComponentBody::Variable {
                    type_declaration: None,
                };
                let scope = {
                    let start = expr.span.end;
                    let end = program_text.cst.span.end;
                    Span { start, end }
                };
                let pos_definition = var.span;
                let (visibility, pos_declaration) = if let Some(info) = module_info {
                    let sig_val_map = info.map_val(program_text);
                    let name = program_text.get_text(var);
                    match sig_val_map.get(name) {
                        Some(Signature::Val { var, .. }) => {
                            let pos_declaration = var.span;
                            (Visibility::Public, Some(pos_declaration))
                        }
                        _ => (Visibility::Private, None),
                    }
                } else {
                    (Visibility::Public, None)
                };
                vec![Component {
                    name,
                    body,
                    scope,
                    pos_definition,
                    visibility,
                    pos_declaration,
                    url: url.clone(),
                }]
            }

            Statement::Type(inners) => inners
                .iter()
                .map(
                    |TypeInner {
                         name: type_name, ..
                     }| {
                        let name = program_text.get_text(type_name).to_owned();
                        let stmt_span = program_text.cst.get_parent(type_name).unwrap().span;
                        let body = ComponentBody::Type;
                        let scope = {
                            let start = stmt_span.end;
                            let end = program_text.cst.span.end;
                            Span { start, end }
                        };
                        let pos_definition = type_name.span;
                        let (visibility, pos_declaration) = if let Some(info) = module_info {
                            let sig_val_map = info.map_val(program_text);
                            let name = program_text.get_text(type_name);
                            match sig_val_map.get(name) {
                                Some(Signature::Val { var, .. }) => {
                                    let pos_declaration = var.span;
                                    (Visibility::Public, Some(pos_declaration))
                                }
                                _ => (Visibility::Private, None),
                            }
                        } else {
                            (Visibility::Public, None)
                        };
                        Component {
                            name,
                            body,
                            scope,
                            pos_definition,
                            visibility,
                            pos_declaration,
                            url: url.clone(),
                        }
                    },
                )
                .collect(),

            Statement::Module {
                name: mod_name,
                signature,
                statements,
            } => {
                let name = program_text.get_text(mod_name).to_owned();
                let module_span = program_text.cst.get_parent(mod_name).unwrap().span;
                let body = {
                    let module_info = ModuleInfo {
                        module_span,
                        sigs: &signature,
                    };
                    let struct_stmt = statements.iter().collect_vec();
                    let components =
                        Component::from_struct_stmts(&module_info, &struct_stmt, program_text, url);
                    ComponentBody::Module { components }
                };
                let scope = {
                    let start = module_span.end;
                    let end = program_text.cst.span.end;
                    Span { start, end }
                };
                let pos_definition = mod_name.span;
                let visibility = Default::default();
                let pos_declaration = None;
                vec![Component {
                    name,
                    body,
                    scope,
                    pos_definition,
                    visibility,
                    pos_declaration,
                    url: url.clone(),
                }]
            }

            Statement::Open(_) => vec![],
        }
    }

    fn new_variable(
        var: &Cst,
        scope: Span,
        module_info: Option<&ModuleInfo>,
        program_text: &ProgramText,
        url: &Url,
    ) -> Component {
        let name = program_text.get_text(var).to_owned();
        let pos_definition = var.span;
        let (visibility, pos_declaration, type_declaration) = if let Some(info) = module_info {
            let sig_val_map = info.map_val(program_text);
            let name = program_text.get_text(var);
            match sig_val_map.get(name) {
                Some(Signature::Val { var, signature, .. }) => {
                    let pos_declaration = var.span;
                    let type_declaration = signature.span;
                    (
                        Visibility::Public,
                        Some(pos_declaration),
                        Some(type_declaration),
                    )
                }
                _ => (Visibility::Private, None, None),
            }
        } else {
            (Visibility::Public, None, None)
        };
        let body = ComponentBody::Variable { type_declaration };
        Component {
            name,
            body,
            scope,
            pos_definition,
            visibility,
            pos_declaration,
            url: url.clone(),
        }
    }
}

#[derive(Debug)]
pub enum ComponentBody {
    Module {
        components: Vec<Component>,
    },
    Variable {
        /// let 式や signature に型情報を書いている場合、その場所。
        type_declaration: Option<Span>,
    },
    Type,
    Variant {
        /// その Variant が属する型の名前。
        type_name: String,
    },
    InlineCmd {
        /// signature に型情報がある場合、その場所。
        type_declaration: Option<Span>,
        type_args: Vec<String>,
    },
    BlockCmd {
        /// signature に型情報がある場合、その場所。
        type_declaration: Option<Span>,
        type_args: Vec<String>,
    },
    MathCmd {
        /// signature に型情報がある場合、その場所。
        type_declaration: Option<Span>,
        type_args: Vec<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Public,
    Private,
    Direct,
}

impl Default for Visibility {
    fn default() -> Self {
        Visibility::Public
    }
}

#[derive(Debug)]
pub struct OpenModule {
    name: String,
    scope: Span,
    url: Url,
}

impl OpenModule {
    fn from_preamble(
        preamble: &[&Statement],
        program_text: &ProgramText,
        url: &Url,
    ) -> Vec<OpenModule> {
        preamble
            .iter()
            .filter_map(|stmt| OpenModule::from_stmt(stmt, None, program_text, url))
            .collect_vec()
    }

    fn from_stmt(
        stmt: &Statement,
        module_info: Option<&ModuleInfo>,
        program_text: &ProgramText,
        url: &Url,
    ) -> Option<OpenModule> {
        if let Statement::Open(cst) = stmt {
            let name = program_text.get_text(cst).to_owned();
            let scope = {
                let start = cst.span.end;
                let end = if let Some(info) = module_info {
                    info.module_span.end
                } else {
                    program_text.cst.span.end
                };
                Span { start, end }
            };
            let url = url.clone();
            Some(OpenModule { name, scope, url })
        } else {
            None
        }
    }
}
