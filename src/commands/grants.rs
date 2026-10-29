use super::prelude::*;
use crate::abstraction::{AccessMatrix, GrantsFix, SupabaseProject};

use anyhow::Context;

#[async_trait]
impl CliSubcommand for Grants {
    async fn run(self: Box<Self>) -> anyhow::Result<()> {
        let project = SupabaseProject::from_cwd().await?;
        let matrix = AccessMatrix::load(&project, &self.schema).await?;

        if matrix.rows.is_empty() {
            crate::styled_bail!(
                "You don't seem to have any tables, views or functions in `{}` schema",
                (&self.schema, "id")
            );
        }

        println!("{matrix}\n");

        if matrix.auto_granting {
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
