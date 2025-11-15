use crate::cli::{CliSubcommand, Db};
use clap::{Args, Subcommand};
use enum_variant_type::EnumVariantType;
use evt_trait_object::Variants;

handle_subcommands!(Db);

#[derive(Debug, Subcommand, Clone, EnumVariantType, Variants)]
#[variants_trait(CliSubcommand)]
pub enum DbCommands {
    /// Creates new migration containing all changes made to local schema
    #[evt(derive(Debug, Args))]
    Commit {
        #[arg(long, short, default_value = "public")]
        schema: String,
    },
}

mod commit;
