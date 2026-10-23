use std::io::IsTerminal;

use super::prelude::*;
use crate::abstraction::{
    OutputFormat, RemoteConnection, RemoteSource, Report, Scope, ScopeRequest, Seed, Snapshot,
    SupabaseProject, TableRef,
};
use crate::config::Config;
use crate::patched::throbberous::Throbber;

use anyhow::Context;

#[async_trait]
impl CliSubcommand for CloneProduction {
    async fn run(self: Box<Self>) -> anyhow::Result<()> {
        let format = if self.json {
            OutputFormat::Json
        } else {
            OutputFormat::Human
        };

        let interactive = std::io::stdin().is_terminal();

        let outcome = async {
            let project = SupabaseProject::discover()?;
            let config = project.config()?;

            let seed = self.out.as_deref().map(Seed::new).transpose()?;

            if seed.is_none() {
                project.runtime().validate().await?;
            }

            let mut scope =
                Scope::from_request(self.scope_request(&project, &config, interactive))?;
            let production = RemoteConnection::from_source(self.source()).await?;

            let catalog = production.catalog();
            let choosing = scope.is_empty();

            let (tables, buckets) = if choosing {
                (catalog.user_tables().await?, catalog.buckets().await?)
            } else {
                (scope.tables().to_vec(), Vec::new())
            };

            let columns = catalog.columns(&tables).await?;
            let suspects = scope.unmasked_personal_data(&columns);

            if !interactive {
                warn_about_unmasked(&suspects, format);
            } else if choosing || !suspects.is_empty() {
                use_promptuity!(promptuity => {
                    promptuity
                        .with_intro(format!("Clone from production ({})", project.id()))
                        .begin()
                        .context("Failed to start interactive mode")?;

                    if choosing {
                        scope
                            .choose_interactively(&mut promptuity, &tables, &buckets)
                            .unwrap_or_else(|_| exit(0));
                    }

                    let suspects = scope.unmasked_personal_data(&columns);

                    if !suspects.is_empty() {
                        scope
                            .mask_interactively(&mut promptuity, suspects)
                            .unwrap_or_else(|_| exit(0));
                    }

                    let _ = promptuity.finish();
                });
            }

            if scope.is_empty() {
                crate::styled_bail!("Nothing selected, nothing to clone");
            }

            let snapshot = Snapshot::take(&production, &scope).await?;
            let estimate = snapshot.estimate().await?;

            if !self.confirm_prod {
                use_promptuity!(promptuity => {
                    promptuity
                        .with_intro(format!("Production data ({})", project.id()))
                        .begin()
                        .context("Failed to start interactive mode")?;

                    let _ = promptuity.warn(&estimate);

                    let shall_continue = promptuity
                        .prompt(Confirm::new("Do you want to continue?").with_default(false))
                        .unwrap_or_else(|_| exit(0));

                    let _ = promptuity.finish();

                    if !shall_continue {
                        exit(0);
                    }
                });
            } else if format == OutputFormat::Human {
                supercli::styled!("<warning> {}", (estimate.to_string(), "warning"));
            }

            let throbber = (format == OutputFormat::Human).then(Throbber::new);

            if let Some(throbber) = &throbber {
                throbber.set_message(" copying from production…").await;
                throbber.start().await;
            }

            let report = match &seed {
                Some(seed) => seed.write(&snapshot, throbber.as_ref()).await,
                None => project.local().import(&snapshot, throbber.as_ref()).await,
            };

            if let Some(throbber) = &throbber {
                match &report {
                    Ok(_) => throbber.stop_success(" copied from production").await,
                    Err(_) => throbber.stop_err(" copying failed").await,
                }
            }

            report
        };

        let outcome = outcome.await;

        if format == OutputFormat::Json {
            let report = outcome.unwrap_or_else(Report::failed).conclude();

            println!("{}", report.to_json());

            if !report.ok {
                exit(1);
            }

            return Ok(());
        }

        let report = outcome?.conclude();

        print_summary(&report);

        if let Some(error) = report.error {
            anyhow::bail!(error);
        }

        match &self.out {
            Some(path) => supercli::styled!(
                "{} Seed written to `{}`",
                ("✔", "success_symbol"),
                (path.to_string_lossy(), "file_path")
            ),
            None => supercli::success!(" Production data cloned successfully!"),
        }

        Ok(())
    }
}

impl CloneProduction {
    fn source(&self) -> RemoteSource {
        RemoteSource {
            db_url: self.db_url.clone(),
            project_url: self.project_url.clone(),
            service_role_key: self.service_role_key.clone(),
        }
    }

    fn scope_request(
        &self,
        project: &SupabaseProject,
        config: &Config,
        interactive: bool,
    ) -> ScopeRequest {
        ScopeRequest {
            tables: self.tables.clone(),
            buckets: self.buckets.clone(),
            conditions: self.wheres.clone(),
            masks: [self.mask.clone(), config.clone.mask.clone()].concat(),
            limit: self.limit,
            with_auth: !self.no_auth,
            into_seed: self.out.is_some(),
            confirmed: self.confirm_prod,
            interactive,
            salt: config
                .clone
                .salt
                .clone()
                .unwrap_or_else(|| format!("sbp-clone:{}", project.id())),
        }
    }
}

fn warn_about_unmasked(suspects: &[(TableRef, String)], format: OutputFormat) {
    if suspects.is_empty() {
        return;
    }

    let columns = suspects
        .iter()
        .map(|(table, column)| format!("{table}.{column}"))
        .collect::<Vec<_>>()
        .join(", ");

    match format {
        OutputFormat::Human => supercli::styled!(
            "<warning> These columns look like personal data and aren't masked: {}\nPass them to `{}` (or list them in `{}`) to mask them",
            (columns, "property"),
            ("--mask", "command"),
            ("sbp.toml", "file_path")
        ),
        OutputFormat::Json => eprintln!(
            "These columns look like personal data and aren't masked: {columns}\nPass them to `--mask` (or list them in `sbp.toml`) to mask them"
        ),
    }
}

fn print_summary(report: &Report) {
    for table in &report.tables {
        if table.masked.is_empty() {
            supercli::styled!(
                "`{}`: {} rows",
                (&table.table, "id"),
                (table.rows.to_string(), "number")
            );
        } else {
            supercli::styled!(
                "`{}`: {} rows, masked {}",
                (&table.table, "id"),
                (table.rows.to_string(), "number"),
                (table.masked.join(", "), "property")
            );
        }
    }

    for bucket in &report.storage {
        supercli::styled!(
            "Bucket `{}`: {} files, {}",
            (&bucket.bucket, "id"),
            (bucket.files.to_string(), "number"),
            (human_bytes(bucket.bytes), "number")
        );
    }

    let discrepancies = &report.discrepancies;

    for name in &discrepancies.removed_since_snapshot {
        supercli::styled!(
            "<warning> Removed in production since the snapshot: `{}`",
            (name, "file_path")
        );
    }

    for name in &discrepancies.added_since_snapshot {
        supercli::styled!(
            "<warning> Added in production since the snapshot, not copied: `{}`",
            (name, "file_path")
        );
    }

    for failure in &discrepancies.failed {
        supercli::styled!(
            "<error> Couldn't copy `{}`\n> {}",
            (&failure.object, "file_path"),
            (&failure.error, "muted")
        );
    }
}
