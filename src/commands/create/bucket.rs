use std::{process::exit};

use crate::{abstraction::SupabaseProject, cli::CliSubcommand, commands::create::Bucket};
use async_trait::async_trait;
use promptuity::{
    Promptuity, Term,
    prompts::{Confirm, Input, Select, SelectOption},
    themes::FancyTheme,
};

#[async_trait]
impl CliSubcommand for Bucket {
    async fn run(self: Box<Self>) {
        let mut term = Term::default();
        let mut theme = FancyTheme::default();

        let (migration_name, sql, shall_run) = {
            let mut promptuity = Promptuity::new(&mut term, &mut theme);
            let _ = promptuity.term().clear();

            promptuity
                .with_intro("Creating bucket")
                .begin()
                .expect("Failed to start interactive mode");

            let name = promptuity
                .prompt(
                    Input::new("Please enter a slug for your new bucket")
                        .with_placeholder("cabinets"),
                )
                .ok()
                .unwrap_or_else(|| exit(0));

            let public = promptuity
                .prompt(
                    Select::new(
                        "Set the visibility of your bucket",
                        vec![
                            SelectOption::new("Public", true),
                            SelectOption::new("Private", false),
                        ],
                    )
                    .with_page_size(2),
                )
                .unwrap_or_else(|_| exit(0));

            let mime_type_limitation = promptuity
                .prompt(
                    Confirm::new("Would you also like to limit accepted mime types?")
                        .with_default(false),
                )
                .unwrap_or_else(|_| exit(0));

            let mut mime_types: Vec<String> = vec![];

            if mime_type_limitation {
                loop {
                    let hint = format!("{}", mime_types.clone().join(", "));

                    let ext = promptuity
                        .prompt(
                            Input::new(
                                "Please enter a file extension and we will guess the mime type",
                            )
                            .with_placeholder("jpg")
                            .with_hint(&hint),
                        )
                        .unwrap_or_else(|_| exit(0));

                    let Some(mime_type) = mime_guess::from_path(format!("_.{}", ext)).first()
                    else {
                        continue;
                    };

                    mime_types.push(mime_type.to_string());

                    let hint = format!("[{}]", mime_types.clone().join(", "));
                    let will_add_more = promptuity
                        .prompt(
                            Confirm::new("Would you like to add another mime type?")
                                .with_default(false)
                                .with_hint(hint),
                        )
                        .unwrap_or_else(|_| exit(0));

                    if !will_add_more {
                        break;
                    }
                }
            }

            let sql = format!(
                r#"INSERT INTO storage.buckets (id, name, public, allowed_mime_types) VALUES ('{0}', '{0}', {1}, '{{{2}}}'::text[]);"#,
                name,
                public,
                mime_types.join(", ")
            );

            let migration_name = format!("create_{name}_bucket");

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

        SupabaseProject::create_migration(&migration_name, &sql, shall_run).await.expect("Failed to create migration");

        println!("Migration file created successfully!");
    }
}
