// Copyright 2021 monaqa. All rights reserved. MIT license.
// This code (including submodules declared in this file) partially uses
// the source code from the project `deno` [^1].
// [^1]: https://github.com/denoland/deno
//! The SATySFi language server.

use anyhow::Result;
use lspower::{LspService, Server};

#[macro_use]
extern crate pest_derive;

mod language_server;
mod parser;

mod capabilities;
mod completion;
mod config;
mod diagnostics;
mod documents;

pub async fn start_language_server() -> Result<()> {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, messages) = LspService::new(language_server::LanguageServer::new);
    Server::new(stdin, stdout)
        .interleave(messages)
        .serve(service)
        .await;

    Ok(())
}

pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_owned()
}
