#![deny(clippy::unwrap_used)]

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

use std::path::PathBuf;
use crate::cli::Cli;
use crate::config::{CONFIG_FILENAME, Config};
use clap::Parser;
use figment::Figment;
use figment::providers::{Serialized, Toml, Format, Env};
use homedir::my_home;
use lazy_static::lazy_static;

lazy_static! {
    static ref CONFIG: Config = {
        let mut figment = Figment::new()
        .merge(Serialized::defaults(Config::default()));

        let env_fig = Figment::new().merge(Env::prefixed("SBP_"));
        let env_cli_source: Option<String> = env_fig.extract_inner("cli_source").ok();
        if let Some(cli_source) = env_cli_source {
            match cli_source.as_str() {
                "Npx" => {
                    figment = figment.merge(Serialized::defaults(Config{cli_source: abstraction::CliSource::Npx}));
                }
                "FromPath" => {
                    let env_cli_path: Option<PathBuf> = env_fig.extract_inner("cli_path").ok();
                    if let Some(path) = env_cli_path {
                        figment = figment.merge(Serialized::defaults(Config{cli_source: abstraction::CliSource::FromPath(path)}));
                    }
                }
                _ => { /* do nothing, not found env variables or value out of range */ }
            }
        }

        if let Ok(Some(home)) = my_home() {
            figment = figment.merge(Toml::file(&home.join(CONFIG_FILENAME)));
        }

        figment = figment.merge(Toml::file(CONFIG_FILENAME));
        figment.extract().expect("Config Loading Failed")
    };
}

#[tokio::main]
async fn main() {
    sys::run_before_hook();

    if let Err(err) = Cli::parse().command.to_object().run().await {
        supercli::error!(&format!("Error: {}", err));
        std::process::exit(1);
    }
}
