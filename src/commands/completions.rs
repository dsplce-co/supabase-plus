use std::fs::File;

use crate::errors::NoWay;

use super::prelude::*;

use anyhow::Context;
use clap::{CommandFactory, ValueEnum};
use duct::cmd;

#[async_trait]
impl CliSubcommand for Completions {
    async fn run(self: Box<Self>) -> anyhow::Result<()> {
        let shell_name = self
            .shell
            .to_possible_value()
            .no_way_because("already deserialized by clap")
            .get_name()
            .to_string();

        if shell_name != "zsh" || self.never_write {
            self.shell
                .generate(&mut Cli::command(), &mut std::io::stdout());

            return Ok(());
        }

        let completions_path = {
            let mut home = homedir::my_home()
                .ok()
                .and_then(|option| option)
                .context("No home directory detected")?;

            let directory = format!(".{shell_name}/completion");

            home.push(directory);

            home
        };

        if let Err(error) = cmd!("mkdir", "-p", completions_path.to_string_lossy().as_ref()).run() {
            crate::styled_bail!(
                "Error creating completion directory `{}`\n> {}",
                (completions_path.to_string_lossy(), "file_path"),
                (format!("{:?}", error), "dimmed")
            );
        };

        let file_path = completions_path.join("_sbp");

        let mut file = File::create(&file_path).context("Error creating file")?;
        self.shell.generate(&mut Cli::command(), &mut file);

        file.sync_all().context("Error closing completions file")?;

        supercli::styled!(
            r#"{} Completion file correctly written to `{}`.
In order to use it, you will also need to add:

{}

to your {} file if you haven't done that so far.

Remember to source {} or restart your shell."#,
            ("✔", "success_symbol"),
            (file_path.to_string_lossy(), "file_path"),
            ("fpath=($fpath ~/.zsh/completion)", "command"),
            ("~/.zshrc", "file_path"),
            (".zshrc", "file_path")
        );

        Ok(())
    }
}
