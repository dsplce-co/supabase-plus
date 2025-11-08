#[macro_export]
macro_rules! handle_subcommands {
    ($super:ident) => {
        #[async_trait::async_trait]
        impl crate::cli::CliSubcommand for $super {
            async fn run(self: Box<Self>) -> anyhow::Result<()> {
                self.0.to_object().run().await?;

                Ok(())
            }
        }
    };
}
