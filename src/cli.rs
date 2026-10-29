use clap::{Args, Parser, Subcommand};
use enum_variant_type::EnumVariantType;
use evt_trait_object::Variants;

use std::fmt::Debug;

use crate::commands::create::CreateCommands;
use crate::commands::db::DbCommands;
use crate::commands::manage::ManageCommands;

#[derive(Debug, Parser)]
#[command(
    name = "sbp",
    version,
    styles = supercli::clap::create_minimal_help_styles()
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

    /// Show which Supabase project is currently running
    #[evt(derive(Debug, Args))]
    Running {},

    /// Bring real production state (table data, auth users and storage files) into the local
    /// Supabase, with personal data masked. Read-only towards production, schema is left to
    /// migrations
    #[evt(derive(Debug, Args))]
    #[command(name = "clone")]
    CloneProduction {
        /// Tables to copy data of, comma separated, `schema.table` or `table` for `public`
        #[arg(long, value_delimiter = ',')]
        tables: Vec<String>,

        /// Storage buckets to copy files of, comma separated
        #[arg(long, value_delimiter = ',')]
        buckets: Vec<String>,

        /// Row filter for a table formatted `table:condition`, repeatable
        #[arg(long = "where", value_name = "TABLE:CONDITION")]
        wheres: Vec<String>,

        /// Max rows to take from each table
        #[arg(long)]
        limit: Option<u64>,

        /// Columns to replace with fake but consistent values, `table.column`, comma separated
        #[arg(long, value_delimiter = ',', value_name = "TABLE.COLUMN")]
        mask: Vec<String>,

        /// Don't copy auth users even though tables are copied
        #[arg(long)]
        no_auth: bool,

        /// Write the data to a seed file instead of the local database
        #[arg(long, value_name = "FILE")]
        out: Option<std::path::PathBuf>,

        /// Acknowledge that the data comes from production, required without a terminal
        #[arg(long)]
        confirm_prod: bool,

        /// Print the result as JSON to stdout
        #[arg(long)]
        json: bool,

        /// Connection string of the database to read from, session mode pooler or direct
        #[arg(long, env = "SUPABASE_DB_URL", hide_env_values = true)]
        db_url: Option<String>,

        /// Project API url, needed for storage files
        #[arg(long, env = "SUPABASE_URL")]
        project_url: Option<String>,

        /// Service role key of the project, needed for storage files
        #[arg(long, env = "SUPABASE_SERVICE_ROLE_KEY", hide_env_values = true)]
        service_role_key: Option<String>,
    },

    /// Show what `anon`, `authenticated` and `service_role` can access and generate a migration
    /// granting what they are missing
    #[evt(derive(Debug, Args))]
    Grants {
        #[arg(long, default_value = "public")]
        schema: String,

        /// Generate a migration with the missing grants
        #[arg(long)]
        fix: bool,
    },

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

    /// Set of commands for direct database operations
    #[evt(derive(Debug))]
    #[command(subcommand)]
    Db(DbCommands),
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
