use anyhow::Context;

use crate::abstraction::MigrationStatus;
use crate::commands::prelude::*;

use crate::{abstraction::SupabaseProject, commands::manage::Migrations};

#[async_trait]
impl CliSubcommand for Migrations {
    async fn run(self: Box<Self>) -> anyhow::Result<()> {
        let project = SupabaseProject::from_cwd().await?;

        let migrations = project.migrations_table(self.linked).await?;

        if migrations.is_empty() {
            crate::styled_bail!("You don't seem to have any migrations");
        }

        let (to_add, to_remove) = use_promptuity!(promptuity => {
            promptuity
                .with_intro(format!("Migrations ({})", project.id()))
                .begin()
                .context("Failed to start interactive mode")?;

            let matrix = promptuity
                .prompt(
                    MultiSelect::new(
                        "Below you can see migrations matrix, the state indicates if is perceived as applied",
                        migrations
                            .iter()
                            .map(|(timecode, run)| MultiSelectOption {
                                label: timecode.clone(),
                                value: timecode.clone(),
                                selected: *run,
                                hint: None,
                            })
                            .collect(),
                    )
                    .with_required(false)
                    .with_hint("switch the state to mark as applied or not")
                    .as_mut(),
                ).unwrap_or_else(|_| exit(0));

            let mut to_add = Vec::<String>::new();
            let mut to_remove = Vec::<String>::new();

            for (timecode, value) in migrations {
                if matrix.contains(&timecode) && !value {
                    &mut to_add
                } else if !matrix.contains(&timecode) && value {
                    &mut to_remove
                } else {
                    continue;
                }.push(timecode.clone());
            }

            let _ = promptuity.finish();

            (to_add, to_remove)
        });

        for timecode in &to_add {
            if let Err(err) = project
                .mark_timecode(&timecode, MigrationStatus::Applied, self.linked)
                .await
            {
                supercli::error!(&format!(
                    "Failed to mark `{}` migration as applied: {}",
                    timecode, err
                ));
            }
        }

        for timecode in &to_remove {
            if let Err(err) = project
                .mark_timecode(&timecode, MigrationStatus::Reverted, self.linked)
                .await
            {
                supercli::error!(&format!(
                    "Failed to mark `{}` migration as reverted: {}",
                    timecode, err
                ));
            }
        }

        supercli::styled!(
            "Marked {} migrations as applied and {} migrations as reverted",
            (to_add.len().to_string(), "number"),
            (to_remove.len().to_string(), "number")
        );

        Ok(())
    }
}
