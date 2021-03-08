// Copyright 2021 monaqa. All rights reserved. MIT license.
// This code partially uses the source code from the project `deno` [^1].
// [^1]: https://github.com/denoland/deno

use std::path::PathBuf;

use anyhow::Result;
use log::LevelFilter;
use satysfi_language_server::start_language_server;

use simplelog::{ConfigBuilder, WriteLogger};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
struct Opts {
    #[structopt(short, long)]
    write_log: bool,

    #[structopt(short, long, default_value = "satysfi-language-server.log")]
    output_log: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    let log_conf = ConfigBuilder::new()
        .set_time_to_local(true)
        .set_location_level(LevelFilter::Info)
        .build();

    let opts = Opts::from_args();

    if opts.write_log {
        WriteLogger::init(
            LevelFilter::Debug,
            log_conf,
            std::fs::File::create(opts.output_log).unwrap(),
        )
        .unwrap();
    }

    start_language_server().await
}
