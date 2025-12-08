use std::io::Stderr;

use crate::patched::promptuity::{
    Promptuity,
    prompts::{MultiSelect, MultiSelectOption},
};
use anyhow::Context;

use crate::abstraction::Migration;

pub struct RealtimeChange {
    schema: String,
    to_add: Vec<String>,
    to_remove: Vec<String>,
}

impl Migration for RealtimeChange {
    fn sql(&self) -> String {
        let mut lines = Vec::new();

        if !self.to_add.is_empty() {
            let line = format!(
                r#"alter publication supabase_realtime add table {};"#,
                self.to_add
                    .iter()
                    .map(|item| format!("{:?}.{item:?}", self.schema))
                    .collect::<Vec<String>>()
                    .join(", ")
            );

            lines.push(line);
        }

        if !self.to_remove.is_empty() {
            let line = format!(
                r#"alter publication supabase_realtime drop table {};"#,
                self.to_remove
                    .iter()
                    .map(|item| format!("{:?}.{item:?}", self.schema))
                    .collect::<Vec<String>>()
                    .join(", ")
            );

            lines.push(line);
        }

        lines.join("\n")
    }

    fn migration_name(&self) -> String {
        format!("change_realtime")
    }
}

impl RealtimeChange {
    pub fn new_interactively(
        promptuity: &mut Promptuity<'_, Stderr>,
        schema: &str,
        tables: Vec<String>,
        enabled_for: Vec<String>,
        project_id: &str,
    ) -> anyhow::Result<Self> {
        promptuity
            .with_intro(format!("Realtime ({})", project_id))
            .begin()
            .context("Failed to start interactive mode")?;

        let matrix = promptuity
            .prompt(
                MultiSelect::new(
                    "Which tables do you want to have realtime enabled for?",
                    tables
                        .iter()
                        .map(|table| MultiSelectOption {
                            label: table.clone(),
                            value: table.clone(),
                            selected: enabled_for.contains(table),
                            hint: None,
                        })
                        .collect(),
                )
                .with_required(false)
                .with_hint("current state is reflected")
                .as_mut(),
            )
            .context("Stopped")?;

        let mut to_add = Vec::new();
        let mut to_remove = Vec::new();

        for item in &matrix {
            if !enabled_for.contains(&item) {
                to_add.push(item.clone());
            }
        }

        for item in enabled_for {
            if !matrix.contains(&item) {
                to_remove.push(item.clone());
            }
        }

        if to_add.is_empty() && to_remove.is_empty() {
            crate::styled_bail!("No changes to apply")
        }

        Ok(Self {
            schema: schema.to_string(),
            to_add,
            to_remove,
        })
    }
}
