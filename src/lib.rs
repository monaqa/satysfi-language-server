// Copyright 2021 monaqa. All rights reserved. MIT license.
// This code (including submodules declared in this file) partially uses
// the source code from the project `deno` [^1].
// [^1]: https://github.com/denoland/deno
//! The SATySFi language server.

mod language_server;

pub use language_server::LanguageServer;

mod capabilities;
mod completion;
mod config;
mod definition;
mod diagnostics;
mod documents;
mod util;

pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_owned()
}
