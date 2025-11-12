use std::io::Stderr;

use anyhow::Context;
use promptuity::{
    Promptuity,
    prompts::{Confirm, Input, Select, SelectOption},
};

use crate::abstraction::Migration;

pub struct NewBucket {
    pub name: String,
    pub public: bool,
    pub mime_types: Vec<String>,
}

impl Migration for NewBucket {
    fn sql(&self) -> String {
        format!(
            r#"INSERT INTO storage.buckets (id, name, public, allowed_mime_types) VALUES ('{0}', '{0}', {1}, '{{{2}}}'::text[]);"#,
            self.name,
            self.public,
            self.mime_types.join(", ")
        )
    }

    fn migration_name(&self) -> String {
        format!("create_{}_bucket", self.name)
    }
}

impl NewBucket {
    pub fn new_interactively(
        promptuity: &mut Promptuity<'_, Stderr>,
        project_id: &str,
    ) -> anyhow::Result<Self> {
        let intro = format!("Creating bucket ({})", project_id);

        promptuity
            .with_intro(&intro)
            .begin()
            .expect("You don't seem to be in an interactive mode");

        let name = promptuity
            .prompt(
                Input::new("Please enter a slug for your new bucket").with_placeholder("cabinets"),
            )
            .ok()
            .context("Stopped")?;

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
            .context("Stopped")?;

        let mime_type_limitation = promptuity
            .prompt(
                Confirm::new("Would you also like to limit accepted mime types?")
                    .with_default(false),
            )
            .context("Stopped")?;

        let mut mime_types: Vec<String> = vec![];

        if mime_type_limitation {
            loop {
                let hint = format!("{}", mime_types.clone().join(", "));

                let ext = promptuity
                    .prompt(
                        Input::new("Please enter a file extension and we will guess the mime type")
                            .with_placeholder("jpg")
                            .with_hint(&hint),
                    )
                    .context("Stopped")?;

                let Some(mime_type) = mime_guess::from_path(format!("_.{}", ext)).first() else {
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
                    .context("Stopped")?;

                if !will_add_more {
                    break;
                }
            }
        }

        Ok(Self {
            name,
            public,
            mime_types,
        })
    }
}
