use std::{fs::File, io::Write, process::exit};

use crate::{cli::CliSubcommand, commands::create::Bucket};
use async_trait::*;
use chrono::Utc;
use promptuity::{
    Prompt, Promptuity, Term,
    prompts::{Confirm, Input, Select, SelectOption},
    themes::FancyTheme,
};
use tokio_postgres::NoTls;

#[async_trait]
impl CliSubcommand for Bucket {
    async fn run(self: Box<Self>) {
        let mut term = Term::default();
        let mut theme = FancyTheme::default();

        let (sql, timecode) = {
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

            let timecode = Utc::now().format("%Y%m%d%H%M%S").to_string();
            let migration_name = format!("{timecode}_create_{name}_bucket");

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

        let (client, connection) = tokio_postgres::connect(
            "postgresql://postgres:postgres@127.0.0.1:54322/postgres",
            NoTls,
        )
        .await
        .expect("Couldn't connect to the database");

        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("connection error: {}", e);
            }
        });

        client
            .query(&sql, &[])
            .await
            .expect("Couldn't execute SQL query");

        cmd!(
            "npx --yes supabase@latest migration repair --local --status applied {}",
            &timecode
        )
        .run()
        .unwrap();

        println!("Migration file created successfully, migration applied locally!");
    }
}
//
