use super::prelude::*;

#[async_trait]
impl CliSubcommand for Upgrade {
    async fn run(self: Box<Self>) -> anyhow::Result<()> {
        if let Err(error) = cmd!("cargo install supabase-plus").run() {
            anyhow::bail!("Something went wrong running cargo install\n> {:?}", error);
        }

        Ok(())
    }
}
