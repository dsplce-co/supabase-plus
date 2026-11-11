use std::io::IsTerminal;

use super::clone::print_summary;
use super::prelude::*;
use crate::abstraction::{
    Archive, PostgresImage, RemoteConnection, RemoteSource, SupabaseProject, Verification,
};
use crate::patched::throbberous::Throbber;

use anyhow::Context;

#[async_trait]
impl CliSubcommand for Backup {
    async fn run(self: Box<Self>) -> anyhow::Result<()> {
        let project = SupabaseProject::discover()?;
        let image = PostgresImage::resolve(&project)?;
        let mut archive = Archive::create(&self.out, &image)?;

        let production = RemoteConnection::from_source(self.source(&project)?).await?;

        self.confirm(&project)?;

        let throbber = Throbber::new();
        throbber.set_message(" backing up production…").await;
        throbber.start().await;

        let report = archive.take(&production, &image, Some(&throbber)).await;

        match &report {
            Ok(report) if report.ok => throbber.stop_success(" backed up").await,
            _ => throbber.stop_err(" backup failed").await,
        }

        let report = report?;

        print_summary(&report);

        if let Some(error) = report.error {
            anyhow::bail!(error);
        }

        supercli::styled!(
            "{} Backup written to `{}`",
            ("✔", "success_symbol"),
            (archive.dir.to_string_lossy(), "file_path")
        );

        if self.no_verify {
            return Ok(());
        }

        let throbber = Throbber::new();
        throbber.start().await;

        let verification = Verification::run(&archive, &image, Some(&throbber)).await;

        match &verification {
            Ok(verification) if verification.ok() => {
                throbber.stop_success(" restored the backup").await
            }
            _ => throbber.stop_err(" restoring the backup failed").await,
        }

        let verification = verification?;

        println!("{verification}");

        if !verification.ok() {
            crate::styled_bail!("The restored backup doesn't match what was backed up");
        }

        supercli::success!(" The backup restores correctly!");

        Ok(())
    }
}

impl Backup {
    fn source(&self, project: &SupabaseProject) -> anyhow::Result<RemoteSource> {
        if self.linked {
            return RemoteSource::linked(project, self.service_role_key.clone());
        }

        Ok(RemoteSource {
            db_url: self.db_url.clone(),
            project_url: self.project_url.clone(),
            service_role_key: self.service_role_key.clone(),
        })
    }

    fn confirm(&self, project: &SupabaseProject) -> anyhow::Result<()> {
        if self.confirm_prod {
            return Ok(());
        }

        if !std::io::stdin().is_terminal() {
            crate::styled_bail!(
                "This command reads production data, pass `{}` to confirm it when running without a terminal",
                ("--confirm-prod", "command")
            );
        }

        use_promptuity!(promptuity => {
            promptuity
                .with_intro(format!("Production data ({})", project.id()))
                .begin()
                .context("Failed to start interactive mode")?;

            let _ = promptuity.warn(format!(
                "The backup holds unmasked production data and goes to `{}`",
                self.out.display()
            ));

            let shall_continue = promptuity
                .prompt(Confirm::new("Do you want to continue?").with_default(false))
                .unwrap_or_else(|_| exit(0));

            let _ = promptuity.finish();

            if !shall_continue {
                exit(0);
            }
        });

        Ok(())
    }
}
