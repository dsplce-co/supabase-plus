use regex::Regex;
use serde::Serialize;

use crate::abstraction::CopyItem;
use crate::errors::NoWay;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Human,
    Json,
}

#[derive(Debug, Default, Serialize)]
pub struct Report {
    pub ok: bool,
    pub error: Option<String>,
    pub tables: Vec<TableReport>,
    pub storage: Vec<BucketReport>,
    pub discrepancies: Discrepancies,
}

#[derive(Debug, Serialize)]
pub struct TableReport {
    pub table: String,
    pub rows: u64,
    pub masked: Vec<String>,
}

#[derive(Debug, Default, Serialize)]
pub struct BucketReport {
    pub bucket: String,
    pub files: u64,
    pub bytes: u64,
}

#[derive(Debug, Default, Serialize)]
pub struct Discrepancies {
    pub added_since_snapshot: Vec<String>,
    pub removed_since_snapshot: Vec<String>,
    pub failed: Vec<Failure>,
}

#[derive(Debug, Serialize)]
pub struct Failure {
    pub object: String,
    pub error: String,
}

impl Report {
    pub fn failed(error: anyhow::Error) -> Self {
        let styles = Regex::new(r"\x1b\[[0-9;]*m").no_way_because("the regex is 'static");

        Self {
            error: Some(styles.replace_all(&format!("{error:#}"), "").to_string()),
            ..Self::default()
        }
    }

    pub fn record_table(&mut self, item: &CopyItem, rows: u64) {
        self.tables.push(TableReport {
            table: item.table().to_string(),
            rows,
            masked: item.masked_columns(),
        });
    }

    pub fn conclude(mut self) -> Self {
        let failed = self.discrepancies.failed.len();

        if failed > 0 && self.error.is_none() {
            self.error = Some(format!("{failed} file(s) couldn't be transferred"));
        }

        self.ok = self.error.is_none();

        self
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).no_way_because("the report consists of plain values")
    }
}
