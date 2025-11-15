use crate::abstraction::{DbDiffError, containers};
use crate::errors::NoWay;
use crate::{abstraction::SupabaseProject, commands::db::Commit};

use crate::commands::prelude::*;
use heck::ToKebabCase;
use spinoff::{Color, Spinner, spinners};
use std::process::{Output, exit};
use tokio::sync::oneshot;

#[async_trait]
impl CliSubcommand for Commit {
    async fn run(self: Box<Self>) -> anyhow::Result<()> {
        let Commit { schema } = *self;
        let project = SupabaseProject::from_cwd().await?;

        let (tx, rx) = oneshot::channel::<anyhow::Result<Output, DbDiffError>>();

        tokio::spawn({
            let project = project.clone();

            async move {
                let output = project.db_diff(&schema).await;
                tx.send(output).no_way_because("`oneshot` just created");
            }
        });

        let message = use_promptuity!(promptuity => {
            promptuity
                .with_intro(&format!("Committing changes ({})", project.id()))
                .begin()
                .expect("Failed to start interactive mode");

            let message = promptuity.prompt(
                Input::new(
                    "How would you like to name this migration?",
                )
                    .with_hint("press enter to sustain default name")
                    .with_placeholder("Commited changes")
                    .with_required(false),
            );

            match message.ok() {
                Some(message) => {
                    let _ = promptuity.finish();
                    Some((!message.is_empty()).then_some(message).unwrap_or("Commited changes".into()))
                },
                _ => None
            }
        });

        let Some(message) = message else {
            project.kill_shadow_db().await?;
            exit(0);
        };

        let mut spinner = Spinner::new(spinners::Line, " diffing schemas…", Color::Blue);

        let output = &rx.await?;

        let Ok(output) = output else {
            spinner.clear();

            let error = output.as_ref().unwrap_err();

            if let DbDiffError::Terminated = error {
                exit(0);
            };

            styled_bail!(
                "Failed to execute `{}`\n> {}",
                ("db diff", "command"),
                (error.to_string(), "highlight")
            )
        };

        let sql = String::from_utf8(output.stdout.clone())?;
        spinner.clear();

        if sql.is_empty() {
            supercli::info!(" No changes detected in the schema. Nothing to commit.");
            return Ok(());
        }

        project
            .create_migration((sql, message.to_kebab_case()), false, true)
            .await?;

        Ok(())
    }
}
