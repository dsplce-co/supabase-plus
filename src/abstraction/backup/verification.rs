use std::collections::BTreeMap;
use std::fmt::{self, Display};
use std::path::Path;

use anyhow::Context;
use cli_table::Cell;
use supercli::output::styling::apply_style;

use crate::abstraction::{Archive, Disposable, FileEntry, ObjectRef, PostgresImage, TableCount};
use crate::patched::throbberous::Throbber;
use crate::utils;

const TABLES: &str = r#"
select coalesce(json_agg(json_build_object(
    'table', format('%s.%s', n.nspname, c.relname),
    'rows', (xpath('/row/c/text()', query_to_xml(
        format('select count(*) as c from %I.%I', n.nspname, c.relname), false, true, ''
    )))[1]::text::bigint
)), '[]')
from pg_class c join pg_namespace n on n.oid = c.relnamespace
where c.relkind in ('r', 'p') and not c.relispartition
    and n.nspname !~ '^pg_' and n.nspname <> 'information_schema'
"#;

const OBJECTS: &str = r#"
select coalesce(json_agg(json_build_object(
    'bucket', bucket_id, 'name', name, 'size', coalesce((metadata->>'size')::bigint, 0)
)), '[]')
from storage.objects
"#;

struct Check {
    what: String,
    expected: u64,
    restored: Option<u64>,
}

impl Check {
    fn passes(&self) -> bool {
        self.restored == Some(self.expected)
    }
}

pub struct Verification {
    checks: Vec<Check>,
    mismatched: Vec<String>,
    restore_errors: Vec<String>,
}

impl Verification {
    pub async fn run(
        archive: &Archive,
        image: &PostgresImage,
        throbber: Option<&Throbber>,
    ) -> anyhow::Result<Self> {
        let (dir, manifest) = (archive.dir.as_path(), &archive.manifest);

        if let Some(throbber) = throbber {
            throbber
                .set_message(" restoring in a disposable database…")
                .await;
        }

        let database = image.disposable(dir).await?;
        let restore_errors = database.restore().await?;

        if !restore_errors.is_empty() {
            return Ok(Self {
                checks: Vec::new(),
                mismatched: Vec::new(),
                restore_errors,
            });
        }

        let tables: Vec<TableCount> =
            serde_json::from_str(&database.sql(Disposable::DATABASE, TABLES).await?)
                .context("Couldn't read the restored tables")?;
        let objects: Vec<ObjectRef> =
            serde_json::from_str(&database.sql(Disposable::DATABASE, OBJECTS).await?)
                .context("Couldn't read the restored storage objects")?;

        let mut checks: Vec<Check> = manifest
            .tables
            .iter()
            .map(|expected| Check {
                what: expected.table.clone(),
                expected: expected.rows,
                restored: tables
                    .iter()
                    .find(|restored| restored.table == expected.table)
                    .map(|restored| restored.rows),
            })
            .collect();

        let mut buckets: BTreeMap<&str, (u64, u64)> = BTreeMap::new();
        let mut mismatched = Vec::new();

        for object in &objects {
            buckets.entry(&object.bucket).or_default().0 += 1;
        }

        for entry in &manifest.files {
            let size = objects
                .iter()
                .find(|object| object.bucket == entry.bucket && object.name == entry.name)
                .map(|object| object.size);

            if entry.verified(dir, size).await {
                buckets.entry(&entry.bucket).or_default().1 += 1;
            } else {
                mismatched.push(format!("{}/{}", entry.bucket, entry.name));
            }
        }

        checks.extend(
            buckets
                .into_iter()
                .map(|(bucket, (expected, restored))| Check {
                    what: format!("files in {bucket}"),
                    expected,
                    restored: Some(restored),
                }),
        );

        Ok(Self {
            checks,
            mismatched,
            restore_errors,
        })
    }

    pub fn ok(&self) -> bool {
        self.restore_errors.is_empty()
            && self.mismatched.is_empty()
            && self.checks.iter().all(Check::passes)
    }
}

impl FileEntry {
    pub async fn verified(&self, dir: &Path, restored_size: Option<u64>) -> bool {
        let Ok(path) = Archive::path_of(dir, &self.bucket, &self.name) else {
            return false;
        };

        let Ok(metadata) = tokio::fs::metadata(path).await else {
            return false;
        };

        restored_size == Some(self.size) && metadata.len() == self.size
    }
}

impl Display for Verification {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if !self.checks.is_empty() {
            let header = ["check", "backed up", "restored", "result"]
                .map(|title| apply_style(title, "muted").cell())
                .into();

            let rows = self.checks.iter().map(|check| {
                let (status, style) = if check.passes() {
                    ("ok", "success")
                } else {
                    ("differs", "error")
                };

                vec![
                    check.what.as_str().cell(),
                    check.expected.cell(),
                    check
                        .restored
                        .map_or("missing".to_string(), |rows| rows.to_string())
                        .cell(),
                    apply_style(status, style).cell(),
                ]
            });

            writeln!(f, "{}", utils::table(header, rows))?;
        }

        for file in &self.mismatched {
            writeln!(
                f,
                "{}",
                apply_style(
                    format!("file doesn't match the restored data: {file}"),
                    "warning"
                )
            )?;
        }

        for error in self.restore_errors.iter().take(10) {
            writeln!(
                f,
                "{}",
                apply_style(format!("restore error: {error}"), "error")
            )?;
        }

        Ok(())
    }
}
