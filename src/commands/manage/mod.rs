use crate::cli::{CliSubcommand, Manage};
use clap::{Args, Subcommand};
use enum_variant_type::EnumVariantType;
use evt_trait_object::Variants;

handle_subcommands!(Manage);

#[derive(Debug, Subcommand, Clone, EnumVariantType, Variants)]
#[variants_trait(CliSubcommand)]
pub enum ManageCommands {
    /// Manage realtime in tables
    #[evt(derive(Args, Debug))]
    Realtime {
        #[arg(long, default_value = "public")]
        schema: String,
    },
}

mod realtime;
