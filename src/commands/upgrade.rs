use crate::cli::{CliSubcommand, Upgrade};

use async_trait::*;

#[async_trait]
impl CliSubcommand for Upgrade {
    async fn run(self: Box<Self>) {
        cmd!("cargo install supabase-plus")
            .run()
            .expect("Something went wrong running cargo install");
    }
}
