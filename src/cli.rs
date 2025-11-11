use clap::{Args, Parser, Subcommand};
use enum_variant_type::EnumVariantType;
use evt_trait_object::Variants;
use supercli::clap::create_help_styles;

use std::fmt::Debug;

use crate::commands::create::CreateCommands;
use crate::commands::manage::ManageCommands;

#[derive(Debug, Parser)]
#[command(
    name = "sbp",
    styles = create_help_styles()
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand, EnumVariantType, Variants)]
#[variants_trait(CliSubcommand)]
pub enum Commands {
    /// Generate completions for the given shell
    #[evt(derive(Debug, Args))]
    Completions {
        /// The shell to generate the completions for
        #[arg(value_enum)]
        shell: clap_complete_command::Shell,

        #[arg(short, long)]
        never_write: bool,
    },

    /// Create a resource of selected type
    #[evt(derive(Debug))]
    #[command(subcommand)]
    Create(CreateCommands),

    /// Manage a resource of selected type
    #[evt(derive(Debug))]
    #[command(subcommand)]
    Manage(ManageCommands),

    /// Stop any running Supabase project
    #[evt(derive(Debug, Args))]
    StopAny {},

    /// Upgrade this command to the latest version using cargo
    #[evt(derive(Debug, Args))]
    Upgrade {},

    /// Watch for sql files in a pointed directory and execute them as db queries on change, useful
    /// for storing rpcs in a repository
    #[evt(derive(Debug, Args))]
    Watch {
        #[arg()]
        directory: String,

        #[arg(short = 'I', long)]
        immediate: bool,
    },
}

#[async_trait::async_trait]
pub(crate) trait CliSubcommand: Debug + Send {
    #[cfg(debug_assertions)]
    async fn run(self: Box<Self>) -> anyhow::Result<()> {
        println!("Running command: {:#?}", self);

        Ok(())
    }

    #[cfg(not(debug_assertions))]
    async fn run(self: Box<Self>) -> anyhow::Result<()>;
}
