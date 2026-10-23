use std::fmt;

use anyhow::Context;

use crate::abstraction::{CopyItem, ObjectRef, RemoteConnection, Scope, TableRef, load_order};
use crate::utils::human_bytes;

pub struct Snapshot<'a> {
    connection: &'a RemoteConnection,
    items: Vec<CopyItem>,
    tables: Vec<TableRef>,
    includes_auth: bool,
    buckets: Vec<String>,
    objects: Vec<ObjectRef>,
}

pub struct Estimate {
    pub tables: usize,
    pub rows: u64,
    pub bytes: u64,
    pub files: usize,
    pub files_bytes: u64,
}

impl<'a> Snapshot<'a> {
    pub async fn take(connection: &'a RemoteConnection, scope: &Scope) -> anyhow::Result<Self> {
        connection
            .client()
            .batch_execute("BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY")
            .await
            .context("Couldn't start a read-only snapshot in production")?;

        let catalog = connection.catalog();

        let references = if scope.tables().is_empty() {
            Vec::new()
        } else {
            catalog.references(scope.tables()).await?
        };

        let tables = load_order(scope.tables(), &references);

        let mut wanted = tables.clone();

        if scope.includes_auth() {
            wanted.extend([TableRef::auth_users(), TableRef::auth_identities()]);
        }

        let columns = catalog.columns(&wanted).await?;
        let columns_of =
            |table: &TableRef| columns.get(table).map(Vec::as_slice).unwrap_or_default();

        let mut items = Vec::new();

        if scope.includes_auth() {
            items.push(CopyItem::auth_users(
                columns_of(&TableRef::auth_users()),
                scope.salt(),
            )?);
            items.push(CopyItem::auth_identities(
                columns_of(&TableRef::auth_identities()),
                scope.salt(),
            )?);
        }

        for table in &tables {
            items.push(
                CopyItem::new(table, columns_of(table))?
                    .with_filter(scope.filter(table))
                    .with_masks(&scope.masks(table), scope.salt())?,
            );
        }

        let objects = if scope.buckets().is_empty() {
            Vec::new()
        } else {
            let missing = catalog.missing_buckets(scope.buckets()).await?;

            if !missing.is_empty() {
                crate::styled_bail!(
                    "Production has no bucket named `{}`",
                    (missing.join(", "), "id")
                );
            }

            ObjectRef::list(connection.client(), scope.buckets()).await?
        };

        Ok(Self {
            connection,
            items,
            tables,
            includes_auth: scope.includes_auth(),
            buckets: scope.buckets().to_vec(),
            objects,
        })
    }

    pub async fn estimate(&self) -> anyhow::Result<Estimate> {
        let catalog = self.connection.catalog();

        let (mut rows, mut bytes) = (0, 0);

        for item in &self.items {
            let size = catalog.estimate(&item.select_sql()).await?;

            rows += size.rows;
            bytes += size.bytes;
        }

        Ok(Estimate {
            tables: self.items.len(),
            rows,
            bytes,
            files: self.objects.len(),
            files_bytes: self.objects.iter().map(|object| object.size).sum(),
        })
    }

    pub async fn release(&self) {
        let _ = self.connection.client().batch_execute("COMMIT").await;
    }

    pub fn connection(&self) -> &RemoteConnection {
        self.connection
    }

    pub fn items(&self) -> &[CopyItem] {
        &self.items
    }

    pub fn tables(&self) -> &[TableRef] {
        &self.tables
    }

    pub fn includes_auth(&self) -> bool {
        self.includes_auth
    }

    pub fn buckets(&self) -> &[String] {
        &self.buckets
    }

    pub fn objects(&self) -> &[ObjectRef] {
        &self.objects
    }
}

impl fmt::Display for Estimate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "This copies PRODUCTION data (read-only, nothing is written to production)"
        )?;
        writeln!(
            f,
            "Tables: {} (≈ {} rows, ≈ {} over the wire)",
            self.tables,
            self.rows,
            human_bytes(self.bytes)
        )?;

        if self.files > 0 {
            writeln!(
                f,
                "Files: {} ({})",
                self.files,
                human_bytes(self.files_bytes)
            )?;
        }

        write!(
            f,
            "It counts towards the egress limit of the production plan"
        )
    }
}
