use crate::config::CONFIG_FILENAME;

pub fn run_before_hook() {}

#[allow(dead_code)]
pub fn does_config_exist() -> bool {
    let mut config_path = std::env::current_dir().unwrap();
    config_path.push(CONFIG_FILENAME);

    config_path.is_file()
}
