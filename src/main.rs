// Copyright 2021 monaqa. All rights reserved. MIT license.
// This code partially uses the source code from the project `deno` [^1].
// [^1]: https://github.com/denoland/deno

use anyhow::Result;
use lspower::{LspService, Server};

use structopt::StructOpt;
use tokio::net::TcpListener;

#[derive(Debug, StructOpt)]
struct Opts {
    #[structopt(long)]
    tcp: bool,
    #[structopt(short, long, default_value = "9527")]
    port: u32,
}

#[tokio::main]
async fn main() -> Result<()> {
    let opts = Opts::from_args();

    if opts.tcp {
        if std::env::var("RUST_LOG").is_err() {
            std::env::set_var("RUST_LOG", "info")
        }
        env_logger::init();

        let listener = TcpListener::bind(format!("127.0.0.1:{}", opts.port)).await?;
        let (stream, _) = listener.accept().await?;
        let (read, write) = tokio::io::split(stream);

        let (service, messages) = LspService::new(satysfi_language_server::LanguageServer::new);
        Server::new(read, write)
            .interleave(messages)
            .serve(service)
            .await;
    } else {
        let stdin = tokio::io::stdin();
        let stdout = tokio::io::stdout();

        let (service, messages) = LspService::new(satysfi_language_server::LanguageServer::new);
        Server::new(stdin, stdout)
            .interleave(messages)
            .serve(service)
            .await;
    };

    Ok(())
}
