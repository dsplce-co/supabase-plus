use super::prelude::*;
use duct::cmd;

#[async_trait]
impl CliSubcommand for Upgrade {
    async fn run(self: Box<Self>) -> anyhow::Result<()> {
        if let Err(error) = cmd!("cargo", "install", "supabase-plus").run() {
            crate::styled_bail!(
                "Something went wrong running cargo install\n> {}",
                (format!("{:?}", error), "dimmed")
            );
        }

        Ok(())
    }
}
