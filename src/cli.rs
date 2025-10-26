use clap::{Args, Parser, Subcommand};
use enum_variant_type::EnumVariantType;
use evt_trait_object::Variants;
use std::fmt::Debug;

use crate::commands::create::CreateCommands;

#[derive(Debug, Parser)]
#[command(name = "spb")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand, EnumVariantType, Variants)]
#[variants_trait(CliSubcommand)]
pub enum Commands {
    /// Stop any running Supabase project
    #[evt(derive(Debug, Args))]
    StopAny {},

    /// Upgrade this command to the latest version using cargo
    #[evt(derive(Debug, Args))]
    Upgrade {},

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
}

#[async_trait::async_trait]
pub(crate) trait CliSubcommand: Debug + Send {
    #[cfg(debug_assertions)]
    async fn run(self: Box<Self>) {
        println!("Running command: {:#?}", self);
    }

    #[cfg(not(debug_assertions))]
    async fn run(self: Box<Self>);
}
