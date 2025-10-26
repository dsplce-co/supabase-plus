use std::process::exit;

use crate::{
    abstraction::SupabaseProject,
    cli::{CliSubcommand, StopAny},
};

use async_trait::*;

#[async_trait]
impl CliSubcommand for StopAny {
    async fn run(self: Box<Self>) {
        let projects = SupabaseProject::running().await;

        if projects.is_empty() {
            println!("No projects running");
            exit(1)
        }

        for project in projects {
            println!("Detected project {:?} running", project.id());
            project.stop();
        }
    }
}
