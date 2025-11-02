pub trait Migration {
    fn sql(&self) -> String;
    fn migration_name(&self) -> String;
}

impl Migration for (String, String) {
    fn sql(&self) -> String {
        self.0.clone()
    }

    fn migration_name(&self) -> String {
        self.1.clone()
    }
}

pub mod bucket;
pub use bucket::*;

pub mod realtime;
pub use realtime::*;
