use std::path::{Path, PathBuf};

use anyhow::Context;
use async_trait::async_trait;
use futures_util::StreamExt;
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;

use crate::abstraction::{
    Catalog, Destination, FileEntry, FileTransfer, Manifest, ObjectRef, PostgresImage,
    RemoteConnection, Report, StorageApi, TableCount, TableRef, Transferred,
};
use crate::patched::throbberous::Throbber;

pub struct Archive {
    pub dir: PathBuf,
    pub manifest: Manifest,
}

impl Archive {
    pub const DUMP: &'static str = "database.dump";
    pub const ROLES: &'static str = "roles.sql";

    pub fn create(dir: &Path, image: &PostgresImage) -> anyhow::Result<Self> {
        let dir = std::path::absolute(dir).context("Couldn't resolve the backup directory")?;

        if dir.is_dir()
            && std::fs::read_dir(&dir)
                .context("Couldn't read the backup directory")?
                .next()
                .is_some()
        {
            crate::styled_bail!(
                "`{}` isn't empty, backups go to a directory of their own",
                (dir.to_string_lossy(), "file_path")
            );
        }

        let anchor = dir
            .ancestors()
            .find(|ancestor| ancestor.exists())
            .context("Couldn't resolve the backup directory")?
            .to_path_buf();

        let existing = anchor.canonicalize()?;
        let dir = existing.join(dir.strip_prefix(&anchor)?);

        let git = |args: &[&str]| {
            std::process::Command::new("git")
                .arg("-C")
                .arg(&existing)
                .args(args)
                .output()
        };

        let in_repository = git(&["rev-parse", "--is-inside-work-tree"])
            .is_ok_and(|output| output.stdout.starts_with(b"true"));

        let ignored = git(&["check-ignore", "-q", &dir.join("").to_string_lossy()])
            .is_ok_and(|output| output.status.success());

        if in_repository && !ignored {
            crate::styled_bail!(
                "`{}` is inside a git repository and isn't ignored, a backup holds unmasked production data, put it elsewhere or add it to `{}`",
                (dir.to_string_lossy(), "file_path"),
                (".gitignore", "file_path")
            );
        }

        Ok(Self {
            dir,
            manifest: Manifest {
                created_at: chrono::Utc::now().to_rfc3339(),
                postgres_image: image.to_string(),
                tables: Vec::new(),
                files: Vec::new(),
            },
        })
    }

    pub fn path_of(dir: &Path, bucket: &str, name: &str) -> anyhow::Result<PathBuf> {
        let mut path = dir.join("storage");

        for segment in std::iter::once(bucket).chain(name.split('/')) {
            if segment.is_empty()
                || segment == "."
                || segment == ".."
                || segment.contains(['\\', '\0'])
            {
                crate::styled_bail!(
                    "`{}` isn't a name which is safe to write to disk",
                    (format!("{bucket}/{name}"), "file_path")
                );
            }

            path.push(segment);
        }

        Ok(path)
    }

    pub async fn take(
        &mut self,
        production: &RemoteConnection,
        image: &PostgresImage,
        throbber: Option<&Throbber>,
    ) -> anyhow::Result<Report> {
        if let Some(throbber) = throbber {
            throbber.set_message(" pulling the Postgres image…").await;
        }

        image.pull().await?;

        std::fs::create_dir_all(&self.dir).context("Couldn't create the backup directory")?;

        let client = production.client();

        client
            .batch_execute("BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY")
            .await
            .context("Couldn't start a read-only snapshot in production")?;

        let snapshot: String = client
            .query_one("select pg_export_snapshot()", &[])
            .await
            .context("Couldn't export a snapshot of production")?
            .get(0);

        let catalog = Catalog::new(client);

        let mut tables = catalog.user_tables().await?;
        tables.extend([TableRef::auth_users(), TableRef::new("storage", "objects")]);

        for table in &tables {
            let rows: i64 = client
                .query_one(&format!("select count(*) from {}", table.quoted()), &[])
                .await
                .with_context(|| {
                    styled_error!("Couldn't count rows of `{}`", (table.to_string(), "id"))
                })?
                .get(0);

            self.manifest.tables.push(TableCount {
                table: table.to_string(),
                rows: rows.max(0) as u64,
            });
        }

        let buckets = catalog.buckets().await?;
        let objects = ObjectRef::list(client, &buckets).await?;

        let storage = (!objects.is_empty())
            .then(|| production.storage())
            .transpose()?;

        if let Some(throbber) = throbber {
            throbber.set_message(" dumping the database…").await;
        }

        let dumped = image
            .dump(production.database(), &snapshot, &self.dir)
            .await;

        let _ = client.batch_execute("COMMIT").await;

        dumped?;

        let mut report = Report::default();

        if let Some(storage) = storage {
            let mut transfer = FileTransfer::new(storage, FileSink::new(&self.dir));

            if let Err(error) = transfer
                .run(&buckets, &objects, client, &mut report, throbber)
                .await
            {
                report.error = Some(format!(
                    "The database has been dumped, but saving files failed: {error:#}"
                ));
            }

            self.manifest.files = transfer.into_destination().files;
        }

        self.manifest.write(&self.dir)?;

        Ok(report.conclude())
    }
}

pub struct FileSink {
    dir: PathBuf,
    files: Vec<FileEntry>,
}

impl FileSink {
    fn new(dir: &Path) -> Self {
        Self {
            dir: dir.to_path_buf(),
            files: Vec::new(),
        }
    }

    async fn write(path: &Path, response: reqwest::Response) -> anyhow::Result<(u64, String)> {
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let mut file = tokio::fs::File::create(path).await?;
        let mut hasher = Sha256::new();
        let mut size = 0;
        let mut stream = response.bytes_stream();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;

            hasher.update(&chunk);
            file.write_all(&chunk).await?;
            size += chunk.len() as u64;
        }

        file.flush().await?;

        Ok((size, format!("{:x}", hasher.finalize())))
    }
}

#[async_trait]
impl Destination for FileSink {
    async fn receive(
        &mut self,
        production: &StorageApi,
        object: &ObjectRef,
    ) -> anyhow::Result<Transferred> {
        let path = Archive::path_of(&self.dir, &object.bucket, &object.name)?;

        let Some(response) = production.fetch(object).await? else {
            return Ok(Transferred::Gone);
        };

        match Self::write(&path, response).await {
            Ok((size, sha256)) => {
                self.files.push(FileEntry {
                    bucket: object.bucket.clone(),
                    name: object.name.clone(),
                    size,
                    sha256,
                });

                Ok(Transferred::Done(size))
            }
            Err(error) => {
                let _ = tokio::fs::remove_file(&path).await;

                Err(error)
            }
        }
    }
}
