use super::prelude::*;
use crate::abstraction::{Identity, PolicyReport, SupabaseProject};

#[async_trait]
impl CliSubcommand for Policy {
    async fn run(self: Box<Self>) -> anyhow::Result<()> {
        let project = SupabaseProject::from_cwd().await?;
        let identities = Identity::load(&project, self.user.as_deref()).await?;

        let report = PolicyReport::load(
            &project,
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
