use serde::Deserialize;

#[allow(dead_code)]
pub static CONFIG_FILENAME: &str = "sbp.toml";

#[allow(dead_code)]
#[derive(Deserialize, Debug)]
pub(crate) struct Config {}
