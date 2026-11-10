use super::prelude::*;
use crate::abstraction::{AccessMatrix, GrantsFix, RemoteSource, SupabaseProject};

use anyhow::Context;

#[async_trait]
impl CliSubcommand for Grants {
    async fn run(self: Box<Self>) -> anyhow::Result<()> {
        let project = SupabaseProject::discover()?;
        let remote = RemoteSource::requested(&project, self.linked, self.db_url.clone())?;
        let is_remote = remote.is_some();
        let client = project.database(remote).await?;
        let matrix = AccessMatrix::load(&client, &self.schema).await?;

        if matrix.rows.is_empty() {
            crate::styled_bail!(
                "You don't seem to have any tables, views or functions in `{}` schema",
                (&self.schema, "id")
            );
        }

        println!("{matrix}\n");

        if matrix.auto_granting && !is_remote {
            supercli::warning!(&format!(
                "Your local database still grants new objects in `{}` to these roles by itself, so it can't show what production will be missing, set `auto_expose_new_tables = false` under `[api]` in supabase/config.toml and run `supabase db reset`",
                self.schema
            ));
        }

        let skipped: Vec<&str> = matrix.left_out().map(|row| row.name.as_str()).collect();

        if !skipped.is_empty() {
            supercli::warning!(&format!(
                "Won't be opened automatically (no RLS, runs as owner or security definer): {}",
                skipped.join(", ")
            ));
        }

        let Some(fix) = GrantsFix::new(&matrix) else {
            if self.fix {
                supercli::info!("Nothing to fix");
            }

            return Ok(());
        };

        if !self.fix {
            supercli::info!(
                "Run `sbp grants --fix` to generate a migration with the missing grants"
            );

            return Ok(());
        }

        if is_remote {
            project.create_migration(fix, false, false).await?;
            supercli::success!("Migration file created successfully!");
            supercli::info!("It hasn't been applied anywhere, deploy it with `supabase db push`");

            return Ok(());
        }

        let shall_run = use_promptuity!(promptuity => {
            promptuity
                .with_intro(format!("Fixing grants ({})", project.id()))
                .begin()
                .context("Failed to start interactive mode")?;

            let shall_run = promptuity
                .prompt(
                    Confirm::new(
                        "Would you like to run this migration immediately and set it to applied?",
                    )
                    .with_default(true),
                )
                .unwrap_or_else(|_| exit(0));

            let _ = promptuity.finish();

            shall_run
        });

        project.create_migration(fix, shall_run, shall_run).await?;
        supercli::success!("Migration file created successfully!");

        Ok(())
    }
}
