use anyhow::Context;
use bytes::Bytes;
use futures_util::{SinkExt, StreamExt, pin_mut};
use tokio_postgres::Client;

use crate::abstraction::{
    Catalog, CopyItem, FileTransfer, Report, Snapshot, StorageApi, SupabaseProject, TableRef,
};
use crate::patched::throbberous::Throbber;

pub struct LocalEnvironment<'a> {
    pub project: &'a SupabaseProject,
}

impl LocalEnvironment<'_> {
    pub async fn import(
        &self,
        snapshot: &Snapshot<'_>,
        throbber: Option<&Throbber>,
    ) -> anyhow::Result<Report> {
        let local = self.project.runtime().client().await?;
        let catalog = Catalog::new(&local);

        let auth_tables: Vec<TableRef> = snapshot
            .items()
            .iter()
            .map(|item| item.table().clone())
            .filter(TableRef::is_auth)
            .collect();

        let local_columns = catalog.columns(&auth_tables).await?;

        let items: Vec<CopyItem> = snapshot
            .items()
            .iter()
            .map(|item| {
                if item.table().is_auth() {
                    item.clone().retain_columns(
                        local_columns
                            .get(item.table())
                            .map(Vec::as_slice)
                            .unwrap_or_default(),
                    )
                } else {
                    item.clone()
                }
            })
            .collect();

        let files = self.prepare_files(&catalog, snapshot).await?;

        let mut report = Report::default();

        let rows = self
            .replace_rows(&local, snapshot, &items, throbber)
            .await?;

        for (item, rows) in items.iter().zip(rows) {
            report.record_table(item, rows);
        }

        snapshot.release().await;

        if let Some(files) = files
            && let Err(error) = files.run(snapshot, &mut report, throbber).await
        {
            report.error = Some(format!(
                "The data has been copied, but copying files failed: {error:#}"
            ));
        }

        Ok(report)
    }

    async fn prepare_files(
        &self,
        catalog: &Catalog<'_>,
        snapshot: &Snapshot<'_>,
    ) -> anyhow::Result<Option<FileTransfer>> {
        if snapshot.buckets().is_empty() {
            return Ok(None);
        }

        let missing = catalog.missing_buckets(snapshot.buckets()).await?;

        if !missing.is_empty() {
            crate::styled_bail!(
                "Local project has no bucket named `{}`, create it first with `{}`",
                (missing.join(", "), "id"),
                ("sbp create bucket", "command")
            );
        }

        Ok(Some(FileTransfer::new(
            snapshot.connection().storage()?,
            StorageApi::local(self.project).await?,
        )))
    }

    async fn replace_rows(
        &self,
        local: &Client,
        snapshot: &Snapshot<'_>,
        items: &[CopyItem],
        throbber: Option<&Throbber>,
    ) -> anyhow::Result<Vec<u64>> {
        local.batch_execute("BEGIN").await?;

        match self.load(local, snapshot, items, throbber).await {
            Ok(counts) => {
                local.batch_execute("COMMIT").await?;
                Ok(counts)
            }
            Err(error) => {
                let _ = local.batch_execute("ROLLBACK").await;
                Err(error)
            }
        }
    }

    async fn load(
        &self,
        local: &Client,
        snapshot: &Snapshot<'_>,
        items: &[CopyItem],
        throbber: Option<&Throbber>,
    ) -> anyhow::Result<Vec<u64>> {
        local
            .batch_execute("SET LOCAL session_replication_role = replica")
            .await
            .context(
                "The local database doesn't let this role disable triggers, can't load data safely",
            )?;

        let mut to_empty = Vec::new();

        if snapshot.includes_auth() {
            to_empty.extend(Catalog::new(local).auth_dependents().await?);
            to_empty.push(TableRef::auth_users());
        }

        to_empty.extend(snapshot.tables().iter().rev().cloned());

        for table in to_empty {
            local
                .batch_execute(&format!("DELETE FROM {}", table.quoted()))
                .await
                .with_context(|| {
                    styled_error!("Couldn't empty local `{}`", (table.to_string(), "id"))
                })?;
        }

        let production = snapshot.connection().client();
        let mut counts = Vec::new();

        for item in items {
            if let Some(throbber) = throbber {
                throbber
                    .set_message(format!(" copying `{}`…", item.table()))
                    .await;
            }

            let rows = production
                .copy_out(item.copy_out_sql().as_str())
                .await
                .with_context(|| {
                    styled_error!(
                        "Couldn't read `{}` from production",
                        (item.table().to_string(), "id")
                    )
                })?;
            pin_mut!(rows);

            let sink = local
                .copy_in::<_, Bytes>(item.copy_in_sql().as_str())
                .await
                .with_context(|| {
                    styled_error!(
                        "Couldn't write `{}` locally",
                        (item.table().to_string(), "id")
                    )
                })?;
            pin_mut!(sink);

            while let Some(chunk) = rows.next().await {
                sink.send(chunk?).await?;
            }

            counts.push(sink.finish().await.with_context(|| {
                styled_error!("Copying `{}` failed", (item.table().to_string(), "id"))
            })?);
        }

        let tables: Vec<TableRef> = items.iter().map(|item| item.table().clone()).collect();

        local
            .batch_execute(&TableRef::reset_sequences_sql(&tables))
            .await
            .context("Couldn't reset sequences")?;

        Ok(counts)
    }
}
