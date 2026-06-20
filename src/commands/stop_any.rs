use super::prelude::*;
use crate::abstraction::{SupabaseProject, SupabaseRuntime};

#[async_trait]
impl CliSubcommand for StopAny {
    async fn run(self: Box<Self>) -> anyhow::Result<()> {
        let projects = SupabaseProject::running().await?;

        for project in projects {
            supercli::styled!("Detected project `{}` running", (project.id(), "id"));
        }

        SupabaseRuntime::passthrough("stop --all")?;

        Ok(())
    }
}
