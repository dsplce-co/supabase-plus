#[macro_use]
extern crate rust_shell;

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
mod sys;

use crate::cli::Cli;
use crate::config::{CONFIG_FILENAME, Config};
use clap::Parser;
use figment::Figment;
use figment::providers::{Format, Toml};
use lazy_static::lazy_static;

lazy_static! {
    static ref CONFIG: Option<Config> = Figment::new()
        .merge(Toml::file(CONFIG_FILENAME))
        .extract()
        .ok();
}

#[tokio::main]
async fn main() {
    sys::run_before_hook();

    if let Err(err) = Cli::parse().command.to_object().run().await {
        supercli::error!(&format!("Error: {}", err));
        std::process::exit(1);
    }
}
