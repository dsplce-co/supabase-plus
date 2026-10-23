#![deny(clippy::unwrap_used)]
#![cfg_attr(test, allow(clippy::unwrap_used))]

#[macro_use]
mod codegen;
#[macro_use]
mod utils;
#[macro_use]
pub(crate) mod colors;
pub(crate) mod abstraction;
mod cli;
mod commands;
mod config;
mod errors;
mod patched;
mod sys;

use crate::cli::Cli;
use clap::Parser;

#[tokio::main]
async fn main() {
    sys::run_before_hook();

    if let Err(err) = Cli::parse().command.to_object().run().await {
        let causes: String = err
            .chain()
            .skip(1)
            .map(|cause| styled_error!("\n> {}", (cause.to_string(), "muted")))
            .collect();

        supercli::error!(&format!("Error: {}{}", err, causes));
        std::process::exit(1);
    }
}
