use anyhow::bail;

use crate::commands::prelude::*;

use crate::{
    abstraction::{RealtimeChange, SupabaseProject},
    commands::manage::Realtime,
};

#[async_trait]
impl CliSubcommand for Realtime {
    async fn run(self: Box<Self>) -> anyhow::Result<()> {
        let project = SupabaseProject::from_cwd().await?;

        let tables = project.tables(&self.schema).await?;
        let enabled_for = project.realtime_tables(&self.schema).await?;

        if tables.is_empty() {
            bail!("You don't seem to have any tables");
        }

        let (rt_change, shall_run) = use_promptuity!(promptuity => {
            let Ok(rt_change) = RealtimeChange::new_interactively(
                &mut promptuity,
                &self.schema,
                tables,
                enabled_for,
            ) else {
                return Ok(());
            };

            let shall_run = promptuity
                .prompt(
                    Confirm::new(
                        "Would you like to run this migration immediately and set it to applied?",
                    )
                    .with_default(true),
                )
                .unwrap_or_else(|_| exit(0));

            let _ = promptuity.finish();

            (rt_change, shall_run)
        });

        project.create_migration(rt_change, shall_run).await?;
        println!("Migration file created successfully!");

        Ok(())
    }
}
