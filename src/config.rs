use anyhow::Context;
use figment::{
    Figment,
    providers::{Format, Toml},
};
use serde::Deserialize;

pub static CONFIG_FILENAME: &str = "sbp.toml";

#[derive(Deserialize, Debug, Default)]
#[serde(default)]
pub(crate) struct Config {
    pub clone: CloneConfig,
}

#[derive(Deserialize, Debug, Default)]
#[serde(default)]
pub(crate) struct CloneConfig {
    pub mask: Vec<String>,
    pub salt: Option<String>,
}

impl Config {
    pub fn load(root: &std::path::Path) -> anyhow::Result<Self> {
        Figment::new()
            .merge(Toml::file(root.join(CONFIG_FILENAME)))
            .extract()
            .with_context(|| styled_error!("Invalid `{}`", (CONFIG_FILENAME, "file_path")))
    }
}
