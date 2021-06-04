// Copyright 2021 monaqa. All rights reserved. MIT license.
// This code (including submodules declared in this file) partially uses
// the source code from the project `deno` [^1].
// [^1]: https://github.com/denoland/deno
//! The SATySFi language server.

mod language_server;

mod config;
mod documents;
mod primitive;
mod util;

pub use language_server::LanguageServer;

pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_owned()
}
