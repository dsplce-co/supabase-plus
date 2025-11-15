use crate::abstraction::DbDiffError;
use crate::errors::NoWay;
use crate::{abstraction::SupabaseProject, commands::db::Commit};

use crate::commands::prelude::*;
use heck::ToKebabCase;
use std::process::exit;
use throbberous::Throbber;
use tokio::sync::oneshot;

#[async_trait]
impl CliSubcommand for Commit {
    async fn run(self: Box<Self>) -> anyhow::Result<()> {
        let Commit { schema } = *self;
        let project = SupabaseProject::from_cwd().await?;

        let (tx, rx) = oneshot::channel::<anyhow::Result<Option<String>, DbDiffError>>();

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
                Input::new("How would you like to name this migration?")
                    .with_hint("press enter to sustain default name")
                    .with_placeholder("Commited changes")
                    .with_required(false)
                    .with_transformer(|value: &str| {
                        if value.is_empty() {
                            "commited-changes".into()
                        } else {
                            value.to_kebab_case()
                        }
                    }),
            );

            match message.ok() {
                Some(message) => {
                    let _ = promptuity.finish();
                    Some(
                        (!message.is_empty())
                            .then_some(message)
                            .unwrap_or("Commited changes".into()),
                    )
                }
                _ => None,
            }
        });

        let Some(message) = message else {
            project.kill_shadow_db().await?;
            exit(0);
        };

        let throbber = Throbber::new();
        throbber.set_message(" awaiting `db diff`…").await;
        throbber.start().await;

        let output = &rx.await?;

        let Ok(sql) = output else {
            let error = output.as_ref().unwrap_err();

            if let DbDiffError::Terminated = error {
                throbber.stop_err(" terminated").await;
                exit(0);
            };

            throbber.stop_success(" `db diff` has run").await;

            styled_bail!(
                "Couldn't generate diff, stderr from `{}`:\n> {}",
                ("supabase db diff", "command"),
                (error.to_string(), "muted")
            )
        };

        throbber.stop_success(" `db diff` completed").await;

        let Some(sql) = sql else {
            supercli::info!(" No changes detected in the schema. Nothing to commit.");
            return Ok(());
        };

        project
            .create_migration((sql.to_owned(), message), false, true)
            .await?;

        supercli::success!(" Changes have been committed to the migration directory!");

        Ok(())
    }
}
