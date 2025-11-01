use std::{fs::File, io::Write, process::exit};
use crate::{abstraction::SupabaseProject, cli::CliSubcommand, cli::Realtime};
use async_trait::async_trait;
use chrono::Utc;
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

        let (sql, timecode) = {
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

            let sql = format!(
                r#"alter publication supabase_realtime add table {schema}.{name};"#,
            );

            let timecode = Utc::now().format("%Y%m%d%H%M%S").to_string();
            let migration_name = format!("{timecode}_add_rt_to_{schema}_{name}");

            let mut file = File::create(format!("supabase/migrations/{migration_name}.sql"))
                .expect("Failed to create migration file");

            file.write_all(sql.as_bytes())
                .expect("Failed to write to migration file");

            file.sync_all().expect("Failed to sync migration file");

            let shall_run = promptuity
                .prompt(
                    Confirm::new(
                        "Would you like to run this specific migration and set to applied?",
                    )
                    .with_default(true),
                )
                .unwrap_or_else(|_| exit(0));

            let _ = promptuity.finish();

            if !shall_run {
                println!("Migration file created successfully!");
                exit(0);
            }

            (sql, timecode)
        };

        SupabaseProject::run_query(&sql)
            .await
            .expect("Failed to run query");

        cmd!(
            "npx --yes supabase@latest migration repair --local --status applied {}",
            &timecode
        )
        .run()
        .unwrap();

        println!("Migration file created successfully, migration applied locally!");
    }
}
