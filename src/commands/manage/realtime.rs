use crate::{abstraction::SupabaseProject, cli::CliSubcommand, commands::manage::Realtime};
use async_trait::async_trait;
use promptuity::{
    Promptuity, Term,
    prompts::{Confirm, MultiSelect, MultiSelectOption},
    themes::FancyTheme,
};

use std::process::exit;

#[async_trait]
impl CliSubcommand for Realtime {
    async fn run(self: Box<Self>) {
        let schema = self.schema;

        let tables = SupabaseProject::tables(&schema).await.unwrap();
        let realtime_tables = SupabaseProject::realtime_tables(&schema).await.unwrap();

        if tables.is_empty() {
            println!("You don't seem to have any tables");
            exit(1);
        }

        let (name, sql, shall_run) = {
            let mut term = Term::default();
            let mut theme = FancyTheme::default();

            let mut promptuity = Promptuity::new(&mut term, &mut theme);
            let _ = promptuity.term().clear();

            promptuity
                .with_intro("Realtime")
                .begin()
                .expect("Failed to start interactive mode");

            let matrix = promptuity
                .prompt(
                    MultiSelect::new(
                        "Which tables do you want to have realtime enabled for?",
                        tables
                            .iter()
                            .map(|table| MultiSelectOption {
                                label: table.clone(),
                                value: table.clone(),
                                selected: realtime_tables.contains(table),
                                hint: None,
                            })
                            .collect(),
                    )
                    .with_required(false)
                    .with_hint("current state is reflected")
                    .as_mut(),
                )
                .unwrap();

            let mut to_add = Vec::new();
            let mut to_remove = Vec::new();

            for item in &matrix {
                if !realtime_tables.contains(&item) {
                    to_add.push(item.clone());
                }
            }

            for item in realtime_tables {
                if !matrix.contains(&item) {
                    to_remove.push(item.clone());
                }
            }

            if to_add.is_empty() && to_remove.is_empty() {
                println!("No changes to apply");
                exit(0);
            }

            let migration_name = "change_realtime";

            let sql = {
                let mut lines = Vec::new();

                if !to_add.is_empty() {
                    let line = format!(
                        r#"alter publication supabase_realtime add table {};"#,
                        to_add
                            .iter()
                            .map(|item| format!("{schema:?}.{item:?}"))
                            .collect::<Vec<String>>()
                            .join(", ")
                    );

                    lines.push(line);
                }

                if !to_remove.is_empty() {
                    let line = format!(
                        r#"alter publication supabase_realtime drop table {};"#,
                        to_remove
                            .iter()
                            .map(|item| format!("{schema:?}.{item:?}"))
                            .collect::<Vec<String>>()
                            .join(", ")
                    );

                    lines.push(line);
                }

                lines.join("\n")
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

            (migration_name, sql, shall_run)
        };

        SupabaseProject::create_migration(&name, &sql, shall_run)
            .await
            .expect("Failed to create migration");

        println!("Migration file created successfully!");
    }
}
