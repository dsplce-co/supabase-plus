use crate::cli::{CliSubcommand, Create};
use clap::{Args, Subcommand};
use enum_variant_type::EnumVariantType;
use evt_trait_object::Variants;

handle_subcommands!(Create);

#[derive(Debug, Subcommand, Clone, EnumVariantType, Variants)]
#[variants_trait(CliSubcommand)]
pub enum CreateCommands {
    /// Create a new bucket interactively, by creating a migration file inserting record into
    /// "storage"."buckets"
    #[evt(derive(Args, Debug))]
    Bucket {},
}

mod bucket;
