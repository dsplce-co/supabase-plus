use crate::abstraction::SupabaseProject;

use super::prelude::*;
use std::process::exit;
use heck::ToKebabCase;

#[async_trait]
impl CliSubcommand for Commit {
    async fn run(self: Box<Self>) -> anyhow::Result<()> {
        let Commit { schema } = *self;
        let project = SupabaseProject::from_cwd().await?;

        let file = use_promptuity!(promptuity => {
            promptuity
            .with_intro("Committing changes")
            .begin()
            .expect("Failed to start interactive mode");

            let file = promptuity
                .prompt(
                    Input::new(
                        "How would you like to name this migration?",
                    )
                    .with_default("commit-changes"),
                )
                .unwrap_or_else(|_| exit(0));

            let _ = promptuity.finish();

            file
        });

        let output = project.runtime().command_silent(&format!("db diff --schema {schema}")).await?;
        let sql = String::from_utf8(output.stdout)?;

        if sql.is_empty() {
            supercli::info!("No changes detected in the schema. Nothing to commit.");
            return Ok(());
        }

        project.create_migration((sql, file.to_kebab_case()), false, true).await?;
        Ok(())
    }
}
