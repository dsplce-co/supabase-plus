use crate::commands::prelude::*;

use crate::{
    abstraction::{RealtimeChange, SupabaseProject},
    commands::manage::Realtime,
};

#[async_trait]
impl CliSubcommand for Realtime {
    async fn run(self: Box<Self>) {
        let tables = SupabaseProject::tables(&self.schema).await.unwrap();

        let enabled_for = SupabaseProject::realtime_tables(&self.schema)
            .await
            .unwrap();

        if tables.is_empty() {
            println!("You don't seem to have any tables");
            exit(1);
        }

        let (rt_change, shall_run) = use_promptuity!(promptuity => {
            let Ok(rt_change) = RealtimeChange::new_interactively(
                &mut promptuity,
                &self.schema,
                tables,
                enabled_for,
            ) else {
                exit(0);
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

        SupabaseProject::create_migration(rt_change, shall_run)
            .await
            .expect("Failed to create migration");

        println!("Migration file created successfully!");
    }
}
