use std::path::Path;

use anyhow::Context;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct Manifest {
    pub created_at: String,
    pub postgres_image: String,
    pub tables: Vec<TableCount>,
    pub files: Vec<FileEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TableCount {
    pub table: String,
    pub rows: u64,
}

#[derive(Debug, Serialize)]
pub struct FileEntry {
    pub bucket: String,
    pub name: String,
    pub size: u64,
    pub sha256: String,
}

impl Manifest {
    pub const FILE: &'static str = "manifest.json";

    pub fn write(&self, dir: &Path) -> anyhow::Result<()> {
        let path = dir.join(Self::FILE);

        std::fs::write(&path, serde_json::to_string_pretty(self)?).with_context(|| {
            styled_error!("Couldn't write `{}`", (path.to_string_lossy(), "file_path"))
        })
    }
}
