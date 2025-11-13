use super::prelude::*;
use crate::abstraction::SupabaseProject;

#[async_trait]
impl CliSubcommand for StopAny {
    async fn run(self: Box<Self>) -> anyhow::Result<()> {
        let projects = SupabaseProject::running().await?;

        if projects.is_empty() {
            crate::styled_bail!("No projects running");
        }

        for project in projects {
            supercli::styled!("Detected project `{}` running", (project.id(), "id"));
            project.stop().await?;
        }

        Ok(())
    }
}
