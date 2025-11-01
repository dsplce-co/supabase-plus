use std::{process::exit};
use crate::{abstraction::SupabaseProject, cli::CliSubcommand, cli::Realtime};
use async_trait::async_trait;
use promptuity::{
    Promptuity, Term,
    prompts::{Confirm, Input},
    themes::FancyTheme,
};

#[async_trait]
impl CliSubcommand for Realtime {
    async fn run(self: Box<Self>) {
        let mut term = Term::default();
        let mut theme = FancyTheme::default();

        let (name, sql, shall_run) = {
            let mut promptuity = Promptuity::new(&mut term, &mut theme);
            let _ = promptuity.term().clear();

            promptuity
                .with_intro("Realtime")
                .begin()
                .expect("Failed to start interactive mode");

            let name = promptuity
                .prompt(
                    Input::new("Please enter table name")
                        .with_placeholder("products"),
                )
                .ok()
                .unwrap_or_else(|| exit(0));

            let schema = promptuity
                .prompt(
                    Input::new("What schema is your table in?")
                        .with_default("public"),
                )
                .ok()
                .unwrap_or_else(|| exit(0));

            let migration_name = format!("add_rt_to_{schema}_{name}");

            let sql = format!(
                r#"alter publication supabase_realtime add table {schema}.{name};"#,
            );

            let shall_run = promptuity
                .prompt(
                    Confirm::new(
                        "Would you like to run this migration immediately and set it to applied?",
                    )
                    .with_default(true),
                )
                .unwrap_or_else(|_| exit(0));

            let _ = promptuity.finish();

            (migration_name, sql, shall_run)
        };

        SupabaseProject::create_migration(&name, &sql, shall_run).await.expect("Failed to create migration");

        println!("Migration file created successfully!");
    }
}
