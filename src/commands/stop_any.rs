use super::prelude::*;
use crate::abstraction::SupabaseProject;

#[async_trait]
impl CliSubcommand for StopAny {
    async fn run(self: Box<Self>) -> anyhow::Result<()> {
        let projects = SupabaseProject::running().await;

        if projects.is_empty() {
            println!("No projects running");
            exit(1)
        }

        for project in projects {
            println!("Detected project {:?} running", project.id());
            project.stop().await?;
        }

        Ok(())
    }
}
