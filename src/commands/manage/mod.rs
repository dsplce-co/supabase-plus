use crate::cli::{CliSubcommand, Manage};
use clap::{Args, Subcommand};
use enum_variant_type::EnumVariantType;
use evt_trait_object::Variants;

handle_subcommands!(Manage);

#[derive(Debug, Subcommand, Clone, EnumVariantType, Variants)]
#[variants_trait(CliSubcommand)]
pub enum ManageCommands {
    /// Toggle realtime on/off on selected tables and generate relevant migrations
    #[evt(derive(Args, Debug))]
    Realtime {
        #[arg(long, default_value = "public")]
        schema: String,
    },

    /// Manage the register of run migrations
    #[evt(derive(Args, Debug))]
    Migrations {
        /// Whether to manage the register for the linked cloud project
        #[arg(long)]
        linked: bool,
    },
}

mod migrations;
mod realtime;
