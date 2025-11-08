#[derive(strum_macros::Display)]
pub enum MigrationStatus {
    #[strum(to_string = "applied")]
    Applied,

    #[strum(to_string = "reverted")]
    Reverted,
}

pub mod project;
pub use project::*;
