use std::collections::BTreeMap;
use std::io::Stderr;

use crate::patched::promptuity::{
    Promptuity,
    prompts::{MultiSelect, MultiSelectOption},
};
use anyhow::Context;

use crate::abstraction::{Column, Conditions, MaskKind, MaskRules, TableFilter, TableRef};

#[derive(Debug, Default)]
pub struct ScopeRequest {
    pub tables: Vec<String>,
    pub buckets: Vec<String>,
    pub conditions: Vec<String>,

    pub masks: Vec<String>,

    pub limit: Option<u64>,
    pub with_auth: bool,
    pub into_seed: bool,
    pub confirmed: bool,
    pub interactive: bool,
    pub salt: String,
}

pub struct Scope {
    tables: Vec<TableRef>,
    buckets: Vec<String>,
    conditions: Conditions,
    limit: Option<u64>,
    masks: MaskRules,
    with_auth: bool,
    files_allowed: bool,
    salt: String,
}

impl Scope {
    pub fn from_request(request: ScopeRequest) -> anyhow::Result<Self> {
        let tables = request
            .tables
            .iter()
            .map(|table| TableRef::parse(table))
            .collect::<anyhow::Result<Vec<_>>>()
            .with_context(|| styled_error!("Invalid `{}`", ("--tables", "command")))?;

        let conditions = Conditions::parse(&request.conditions)
            .with_context(|| styled_error!("Invalid `{}`", ("--where", "command")))?;

        let masks = MaskRules::parse(&request.masks).context("Invalid mask")?;

        if request.into_seed && !request.buckets.is_empty() {
            crate::styled_bail!(
                "Files can't be a part of a seed, drop `{}` or `{}`",
                ("--buckets", "command"),
                ("--out", "command")
            );
        }

        if !request.interactive && tables.is_empty() && request.buckets.is_empty() {
            crate::styled_bail!(
                "Nothing to clone, pass `{}` and/or `{}` when running without a terminal",
                ("--tables", "command"),
                ("--buckets", "command")
            );
        }

        if !request.interactive && !request.confirmed {
            crate::styled_bail!(
                "This command reads production data, pass `{}` to confirm it when running without a terminal",
                ("--confirm-prod", "command")
            );
        }

        Ok(Self {
            tables,
            buckets: request.buckets,
            conditions,
            limit: request.limit,
            masks,
            with_auth: request.with_auth,
            files_allowed: !request.into_seed,
            salt: request.salt,
        })
    }

    pub fn choose_interactively(
        &mut self,
        promptuity: &mut Promptuity<'_, Stderr>,
        tables: &[TableRef],
        buckets: &[String],
    ) -> anyhow::Result<()> {
        if !tables.is_empty() {
            let chosen = promptuity
                .prompt(
                    MultiSelect::new(
                        "Which tables do you want the data of?",
                        tables
                            .iter()
                            .map(|table| MultiSelectOption {
                                label: table.to_string(),
                                value: table.to_string(),
                                selected: false,
                                hint: None,
                            })
                            .collect(),
                    )
                    .with_required(false)
                    .as_mut(),
                )
                .context("Stopped")?;

            self.tables = tables
                .iter()
                .filter(|table| chosen.contains(&table.to_string()))
                .cloned()
                .collect();
        }

        if !buckets.is_empty() && self.files_allowed {
            self.buckets = promptuity
                .prompt(
                    MultiSelect::new(
                        "Which buckets do you want the files of?",
                        buckets
                            .iter()
                            .map(|bucket| MultiSelectOption {
                                label: bucket.clone(),
                                value: bucket.clone(),
                                selected: false,
                                hint: None,
                            })
                            .collect(),
                    )
                    .with_required(false)
                    .as_mut(),
                )
                .context("Stopped")?;
        }

        Ok(())
    }

    pub fn unmasked_personal_data(
        &self,
        columns: &BTreeMap<TableRef, Vec<Column>>,
    ) -> Vec<(TableRef, String)> {
        let mut suspects = Vec::new();

        for table in &self.tables {
            for column in columns.get(table).into_iter().flatten() {
                if MaskKind::looks_personal(&column.name)
                    && MaskKind::fits_type(&column.type_sql)
                    && !column.generated
                    && !self.masks.covers(table, &column.name)
                {
                    suspects.push((table.clone(), column.name.clone()));
                }
            }
        }

        suspects
    }

    pub fn mask_interactively(
        &mut self,
        promptuity: &mut Promptuity<'_, Stderr>,
        suspects: Vec<(TableRef, String)>,
    ) -> anyhow::Result<()> {
        let labels: Vec<String> = suspects
            .iter()
            .map(|(table, column)| format!("{table}.{column}"))
            .collect();

        let to_mask = promptuity
            .prompt(
                MultiSelect::new(
                    "These columns look like personal data, which of them should be masked?",
                    labels
                        .iter()
                        .map(|label| MultiSelectOption {
                            label: label.clone(),
                            value: label.clone(),
                            selected: true,
                            hint: None,
                        })
                        .collect(),
                )
                .with_required(false)
                .as_mut(),
            )
            .context("Stopped")?;

        for ((table, column), label) in suspects.into_iter().zip(labels) {
            if to_mask.contains(&label) {
                self.masks.insert(table, &column);
            }
        }

        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.tables.is_empty() && self.buckets.is_empty()
    }

    pub fn tables(&self) -> &[TableRef] {
        &self.tables
    }

    pub fn buckets(&self) -> &[String] {
        &self.buckets
    }

    pub fn includes_auth(&self) -> bool {
        self.with_auth && !self.tables.is_empty()
    }

    pub fn filter(&self, table: &TableRef) -> TableFilter {
        self.conditions.filter(table, self.limit)
    }

    pub fn masks(&self, table: &TableRef) -> BTreeMap<String, MaskKind> {
        self.masks.columns(table)
    }

    pub fn salt(&self) -> &str {
        &self.salt
    }
}
