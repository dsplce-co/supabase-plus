#[cfg(not(feature = "default"))]
pub use promptuity;

#[cfg(feature = "default")]
pub use dsplce_co_promptuity as promptuity;

#[cfg(not(feature = "default"))]
pub use throbberous;

#[cfg(feature = "default")]
pub use dsplce_co_throbberous as throbberous;
