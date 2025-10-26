use crate::cli::{Cli, CliSubcommand, Completions};
use clap::ValueEnum;
use std::{fs::File, process::exit};

use async_trait::*;
use clap::CommandFactory;

#[async_trait]
impl CliSubcommand for Completions {
    async fn run(self: Box<Self>) {
        let shell_name = self
            .shell
            .to_possible_value()
            .unwrap()
            .get_name()
            .to_string();

        if shell_name != "zsh" || self.never_write {
            self.shell
                .generate(&mut Cli::command(), &mut std::io::stdout());

            exit(0);
        }

        let completions_path = {
            let mut home = homedir::my_home()
                .ok()
                .and_then(|option| option)
                .expect("No home directory detected");

            let directory = format!(".{shell_name}/completion");

            home.push(directory);

            home
        };

        cmd!("mkdir -p {}", &completions_path.to_string_lossy())
            .run()
            .expect(&format!(
                "Error creating completion directory `{}`",
                completions_path.to_string_lossy()
            ));

        let file_path = completions_path.join("_spb");

        let mut file = File::create(&file_path).expect("Error creating file");
        self.shell.generate(&mut Cli::command(), &mut file);

        file.sync_all().expect("Error closing completions file");

        println!(
            r#"Completion file correctly written to `{}`.
In order to use it, you will also need to add:

fpath=($fpath ~/.zsh/completion)

to your ~/.zshrc file if you haven't done that so far.

Remember to source .zshrc or restart your shell."#,
            file_path.to_string_lossy()
        );
    }
}
