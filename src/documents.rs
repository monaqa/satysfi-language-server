use std::collections::HashMap;

use lspower::lsp::Url;
use satysfi_parser::{CstText, LineCol, Span};

/// オンメモリで取り扱うデータをまとめたデータ構造。
#[derive(Debug, Default)]
pub struct DocumentCache(HashMap<Url, DocumentData>);

/// 一つのファイルに関するデータを纏めたデータ構造。
#[derive(Debug)]
pub enum DocumentData {
    /// パーサによって正常にパースできたデータ。
    Parsed {
        /// パース結果の具象構文木 + テキスト本体。
        csttext: CstText,
        /// 変数やコマンドに関する情報。
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
    pub fn new(text: &str) -> DocumentData {
        match CstText::parse(text, satysfi_parser::grammar::program) {
            Ok(csttext) => {
                let environment = Environment::new(&csttext);
                DocumentData::Parsed {
                    csttext,
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
}

/// 変数やコマンドに関する情報。
#[derive(Debug, Default)]
pub struct Environment {
    pub dependencys: Vec<Package>,
    pub modules: Vec<Module>,
    /// package にて定義された変数。
    pub variables: Vec<Variable>,
    /// package にて定義された型。
    pub types: Vec<CustomType>,
    /// package にて定義されたヴァリアント。
    pub variants: Vec<Variant>,
    /// package にて定義されたインラインコマンド。
    pub inline_cmds: Vec<InlineCmd>,
    /// package にて定義されたブロックコマンド。
    pub block_cmds: Vec<BlockCmd>,
    /// package にて定義された数式コマンド。
    pub math_cmds: Vec<MathCmd>,
}

impl Environment {
    fn new(csttext: &CstText) -> Environment {
        todo!()
    }
}

#[derive(Debug)]
pub struct Package {
    /// パッケージ名。
    name: String,
    /// 場所。
    url: Url,
}

impl Package {
    /// 具象構文木からパッケージ情報を取り出す。
    fn extract(csttext: &CstText) -> Vec<Package> {
        todo!()
    }
}

#[derive(Debug)]
pub struct Module {
    /// Module の名前。
    pub name: String,
    /// module にて定義された変数。
    pub variables: Vec<Variable>,
    /// module にて定義された型。
    pub types: Vec<CustomType>,
    /// module にて定義されたヴァリアント。
    pub variants: Vec<Variant>,
    /// module にて定義されたインラインコマンド。
    pub inline_cmds: Vec<InlineCmd>,
    /// module にて定義されたブロックコマンド。
    pub block_cmds: Vec<BlockCmd>,
    /// module にて定義された数式コマンド。
    pub math_cmds: Vec<MathCmd>,
}

/// package 内で定義された変数やコマンドなど。
#[derive(Debug)]
pub struct PackageComponent<T> {
    /// 可視性。外から見えるかどうか。
    pub visibility: PackageVisibility,
    /// 本体。
    pub body: T,
    /// スコープ。すなわち、対象となるファイルの中で、
    /// その変数や型、コマンドなどを使うことができる領域。
    pub scope: Span,
    /// 定義がどこにあるか (position)。
    pub pos_definition: usize,
}

/// module 内で定義された変数やコマンドなど。
#[derive(Debug)]
pub struct ModuleComponent<T> {
    /// 可視性。外から見えるかどうか。
    pub visibility: ModuleVisibility,
    /// 本体。
    pub body: T,
    /// スコープ。すなわち、対象となるファイルの中で、
    /// その変数や型、コマンドなどを使うことができる領域。
    pub scope: Span,
    /// sig 内部で declaration しているとき、その declaration がどこにあるか (position)。
    pub pos_declaration: Option<usize>,
    /// 定義がどこにあるか (position)。
    pub pos_definition: usize,
}

#[derive(Debug)]
pub struct Variable {
    /// 変数名。
    name: String,
    /// 変数の型（既知の場合）。
    type_: Option<String>,
    /// let 式に型情報を書いている場合、その場所。
    type_declaration: Option<Span>,
}

#[derive(Debug)]
pub struct CustomType {
    /// 型名。
    name: String,
    /// 型の定義。
    definition: String,
}

#[derive(Debug)]
pub struct Variant {
    /// variant 名。
    name: String,
    /// その Variant を持つ型の名前。
    type_name: String,
}

#[derive(Debug)]
pub struct InlineCmd {
    /// コマンド名。
    name: String,
    /// 型情報。
    type_: Option<Vec<String>>,
    /// 型情報の載っている場所。
    type_declaration: Option<Span>,
}

#[derive(Debug)]
pub struct BlockCmd {
    /// コマンド名。
    name: String,
    /// 型情報。
    type_: Option<Vec<String>>,
    /// 型情報の載っている場所。
    type_declaration: Option<Span>,
}

#[derive(Debug)]
pub struct MathCmd {
    /// コマンド名。
    name: String,
    /// 型情報。
    type_: Option<Vec<String>>,
    /// 型情報の載っている場所。
    type_declaration: Option<Span>,
}

#[derive(Debug)]
pub enum PackageVisibility {
    /// let_stmt などで定義された値。そのpackageを追加すると使用できる類のもの。
    Public,
    /// なにかの式で変数束縛を行う際に定義された一時的な変数。
    Binded,
}

#[derive(Debug)]
pub enum ModuleVisibility {
    /// sig にて direct に宣言されたもの。 Module. が無くとも使うことができる。
    Direct,
    /// sig にて val で宣言されたもの。 Module.* の形で、または open Module すれば使用できる。
    Public,
    /// sig にて宣言されていないもの。 Module の外からは呼び出せない。
    Private,
    /// なにかの式で変数束縛を行う際に定義された一時的な変数。
    Binded,
}
