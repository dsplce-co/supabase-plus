use std::io::IsTerminal;

use super::prelude::*;
use crate::abstraction::{Identity, PolicyReport, RemoteSource, SupabaseProject};

use anyhow::Context;

#[async_trait]
impl CliSubcommand for Policy {
    async fn run(self: Box<Self>) -> anyhow::Result<()> {
        let project = SupabaseProject::discover()?;
        let remote = RemoteSource::requested(&project, self.linked, self.db_url.clone())?;

        let is_remote = remote.is_some();
        let mut client = project.database(remote).await?;

        if is_remote {
            if !std::io::stdin().is_terminal() {
                crate::styled_bail!(
                    "Evaluating policies on a remote database needs your confirmation, run it in a terminal"
                );
            }

            let confirmed = use_promptuity!(promptuity => {
                promptuity
                    .with_intro(format!("Evaluating policies remotely ({})", project.id()))
                    .begin()
                    .context("Failed to start interactive mode")?;

                let _ = promptuity.warn(
                    "To count the rows each policy lets through this opens a temporary view in a session of the remote database. The transaction is read-only, limited to 30 seconds per statement and always rolled back, nothing is stored.",
                );

                let confirmed = promptuity
                    .prompt(Confirm::new("Do you want to continue?").with_default(false))
                    .unwrap_or_else(|_| exit(0));

                let _ = promptuity.finish();

                confirmed
            });

            if !confirmed {
                return Ok(());
            }
        }

        let identities = Identity::load(&client, self.user.as_deref()).await?;

        let report = PolicyReport::load(
            &mut client,
            &self.schema,
            self.table.as_deref(),
            self.name.as_deref(),
            identities,
        )
        .await?;

        if report.rows.is_empty() {
            crate::styled_bail!(
                "Couldn't find any policies in `{}` schema matching the given filters",
                (&self.schema, "id")
            );
        }

        println!("{report}");

        for (row, identity, message) in report.failures() {
            supercli::warning!(&format!(
                "Couldn't evaluate `{}` on {} for {identity}: {message}",
                row.name, row.table
            ));
        }

        Ok(())
    }
}
