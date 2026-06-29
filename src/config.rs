use serde::{Deserialize, Serialize};
use crate::abstraction::CliSource;

#[allow(dead_code)]
pub static CONFIG_FILENAME: &str = "sbp.toml";

#[allow(dead_code)]
#[derive(Deserialize, Serialize, Debug)]
pub(crate) struct Config {
    pub cli_source: CliSource
}

impl Default for Config{
    fn default() -> Self {
        Self{
            cli_source: CliSource::Npx
        }
    }
}
