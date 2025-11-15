use crate::abstraction::{Migration, MigrationStatus, NO_DOCKER, SupabaseRuntime, containers};
use crate::errors::NoWay;

use std::collections::HashSet;
use std::path::PathBuf;
use std::{fs::File, io::Write};

use anyhow::Context;
use bollard::query_parameters::KillContainerOptions;
use bollard::{Docker, secret::ContainerSummary};
use chrono::Utc;
use regex::Regex;

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct SupabaseProject {
    pub(crate) project_id: String,
    pub(crate) root: Option<PathBuf>,
}

#[derive(thiserror::Error, Debug)]
pub enum DbDiffError {
    #[error("data store disconnected")]
    Terminated,

    #[error("{0}")]
    Failed(String),

    #[error("{0}")]
    Os(#[from] anyhow::Error),
}

impl SupabaseProject {
    pub async fn from_cwd() -> anyhow::Result<Self> {
        let root = std::env::current_dir().context(
            "Failed to get current directory to indicate the project, make sure the current directory exists (sic!) and you have the necessary permissions"
        )?;

        let root = Self::find_root(root).with_context(|| {
            styled_error!(
                "Could not find Supabase project root, no `{}` in cwd or any parent directory",
                ("supabase/config.toml", "file_path")
            )
        })?;

        let config_path = root.join("supabase/config.toml");

        let config = std::fs::read_to_string(config_path)
            .no_way_because("The config file has been found earlier");

        let project_id = config
            .lines()
            .find(|line| line.starts_with("project_id"))
            .with_context(|| styled_error!(
                "Failed to find `{}` in `{}`, make sure the `{}` has no syntax errors and contains a `{}`",
                ("project_id", "property"),
                ("config.toml", "file_path"),
                ("config.toml", "file_path"),
                ("project_id", "property")
            ))?
            .splitn(2, '=')
            .nth(1)
            .with_context(|| styled_error!(
                "Failed to parse `{}` in `{}`, make sure the `{}` kv pair has no syntax errors",
                ("project_id", "property"),
                ("config.toml", "file_path"),
                ("project_id", "property")
            ))?
            .trim()
            .to_string();

        let project_id: String = serde_json::from_str(&project_id).with_context(|| {
            styled_error!(
                "Failed to parse `{}` in `{}`, make sure the `{}` value has no syntax errors",
                ("project_id", "property"),
                ("config.toml", "file_path"),
                ("project_id", "property")
            )
        })?;

        let result = Self {
            project_id,
            root: Some(root),
        };

        result.runtime().validate().await?;

        Ok(result)
    }

    fn find_root(path: PathBuf) -> Option<PathBuf> {
        let config_path = path.join("supabase/config.toml");

        if config_path.exists() {
            return Some(path);
        }

        let Some(parent) = path.parent() else {
            return None;
        };

        Self::find_root(parent.to_path_buf())
    }

    pub fn runtime(&self) -> SupabaseRuntime<'_> {
        SupabaseRuntime { project: &self }
    }

    pub fn migrations_dir(&self) -> PathBuf {
        let path = self
            .root
            .clone()
            .no_way_because("root should be provided for root dependent commands")
            .join("supabase/migrations");

        path
    }

    pub async fn create_migration<T: Migration>(
        &self,
        migration: T,
        run_after: bool,
        mark_after: bool,
    ) -> anyhow::Result<()> {
        let name = migration.migration_name();
        let sql = migration.sql();

        let timecode = Utc::now().format("%Y%m%d%H%M%S").to_string();
        let migrations_dir = self.migrations_dir();

        if !migrations_dir.exists() {
            std::fs::create_dir(&migrations_dir).context(styled_error!(
                "You don't seem to have a `{}` directory, we tried creating it but we failed",
                ("supabase/migrations", "file_path")
            ))?;
        }

        if migrations_dir.is_file() {
            crate::styled_bail!(
                "`{}` is a file, not a directory",
                ("supabase/migrations", "file_path")
            );
        }

        let file_path = migrations_dir.join(format!("{timecode}_{name}.sql"));

        let mut file = File::create(file_path).with_context(|| {
            let os_error = anyhow::anyhow!(std::io::Error::last_os_error()).to_string();

            styled_error!(
                "Failed to create migration file at `{}`\n> {}",
                (migrations_dir.display().to_string(), "file_path"),
                (os_error, "dimmed")
            )
        })?;

        file.write_all(sql.as_bytes())
            .context("Failed to write to the migration file")?;

        file.sync_all()
            .context("Failed to sync newly created migration file")?;

        if run_after {
            self.runtime().sql(&sql).await?;
        }

        if mark_after {
            self.mark_timecode(&timecode, MigrationStatus::Applied, false)
                .await?;
        }

        Ok(())
    }

    pub async fn mark_timecode(
        &self,
        timecode: &str,
        status: MigrationStatus,
        linked: bool,
    ) -> anyhow::Result<()> {
        self.runtime()
            .command(&format!(
                "migration repair --{} --status {} {}",
                if linked { "linked" } else { "local" },
                &status.to_string(),
                timecode
            ))
            .await?;

        Ok(())
    }

    pub async fn db_diff(&self, schema: &str) -> anyhow::Result<Option<String>, DbDiffError> {
        self.kill_shadow_db().await?;

        let command = format!("db diff --schema {}", schema);

        tokio::select! {
            output = self.runtime().command_silent(&command) => {
                let Ok(output) = output else {
                    return Err(DbDiffError::Os(output.unwrap_err()))
                };

                if output.status.success() {
                    let value = String::from_utf8_lossy(&output.stdout);

                    return Ok(if value.is_empty() {
                        None
                    } else {
                        Some(value.to_string())
                    })
                }

                let error = String::from_utf8_lossy(&output.stderr);

                return Err(DbDiffError::Failed(error.to_string()))
            }
            _ = tokio::signal::ctrl_c() => {
                self.kill_shadow_db().await?;
                Err(DbDiffError::Terminated)
            }
        }
    }

    pub async fn kill_shadow_db(&self) -> anyhow::Result<()> {
        let maybe_shadow_db = containers::shadow_db()
            .await?
            .and_then(|container| container.id);

        if let Some(shadow_db) = maybe_shadow_db {
            let docker =
                Docker::connect_with_socket_defaults().with_context(|| NO_DOCKER.clone())?;

            docker
                .kill_container(&shadow_db, None::<KillContainerOptions>)
                .await?;
        }

        Ok(())
    }

    pub async fn running() -> anyhow::Result<HashSet<SupabaseProject>> {
        let containers = containers::supabase().await?;

        let mut projects = Vec::new();

        for container in containers {
            let Some(slug) = TryInto::<SupabaseProject>::try_into(container).ok() else {
                continue;
            };

            if projects.contains(&slug) {
                continue;
            }

            projects.push(slug);
        }

        Ok(projects.into_iter().collect())
    }

    pub async fn stop(&self) -> anyhow::Result<()> {
        self.runtime().stop().await?;

        Ok(())
    }

    pub fn id(&self) -> &str {
        &self.project_id
    }

    pub async fn migrations_table(&self, linked: bool) -> anyhow::Result<Vec<(String, bool)>> {
        let path = self.migrations_dir();

        let mut entries = tokio::fs::read_dir(&path).await.with_context(|| {
            let os_error = anyhow::anyhow!(std::io::Error::last_os_error()).to_string();

            styled_error!(
                "Couldn't find `{}` directory\n> {}",
                (path.display().to_string(), "file_path"),
                (os_error, "dimmed")
            )
        })?;

        let mut migrations_from_files = Vec::new();

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            let name = path.file_name().unwrap().to_str().unwrap().to_string();
            let regex = Regex::new(r"^\d{14}_").no_way_because("the regex is 'static");
            let is_migration = regex.is_match(&name);

            if !is_migration {
                continue;
            }

            let timecode = name
                .splitn(2, '_')
                .next()
                .no_way_because("the regex already validated `_` presence")
                .parse::<u64>()
                .no_way_because("the regex already validated timecode format");

            migrations_from_files.push(timecode);
        }

        migrations_from_files.sort_by(|prev, next| prev.cmp(next).reverse());

        let cmd = format!(
            "migration list --{} | awk '{{ print $3 }}' | grep '^2' | cat",
            if linked { "linked" } else { "local" }
        );

        let run = self.runtime().command_silent(&cmd).await?;

        if !run.status.success() {
            Err::<(), _>(String::from_utf8_lossy(&run.stderr)).no_way_because(
                "the command's valid and generic, also checked if Supabase's running",
            );
        }

        let output = run.stdout;
        let buffer = String::from_utf8_lossy(&output);

        let applied_migrations: Vec<String> = buffer
            .split('\n')
            .into_iter()
            .map(|value| value.to_string())
            .collect();

        let mut results = Vec::<(String, bool)>::new();

        for migration in migrations_from_files {
            results.push((
                migration.to_string(),
                applied_migrations.contains(&migration.to_string()),
            ));
        }

        Ok(results)
    }

    pub async fn tables(&self, schema: &str) -> anyhow::Result<Vec<String>> {
        let result = self
            .runtime()
            .query(
                "select tablename from pg_tables where schemaname = $1",
                &[&schema],
            )
            .await
            .with_context(|| {
                styled_error!("Couldn't fetch tables for `{}` schema", (schema, "id"))
            })?;

        Ok(result.into_iter().map(|row| row.get(0)).collect())
    }

    pub async fn realtime_tables(&self, schema: &str) -> anyhow::Result<Vec<String>> {
        let result = self
            .runtime()
            .query(
                "select tablename from pg_publication_tables where schemaname = $1 and pubname = 'supabase_realtime'",
                &[&schema]
            )
            .await
            .with_context(|| {
                styled_error!("Couldn't fetch tables for `{}` schema", (schema, "id"))
            })?;

        Ok(result.into_iter().map(|row| row.get(0)).collect())
    }
}

impl TryInto<SupabaseProject> for ContainerSummary {
    type Error = String;

    fn try_into(self) -> Result<SupabaseProject, Self::Error> {
        for name in self.names.unwrap_or_default() {
            let re = Regex::new(r"^/supabase(_[^_]+)*_").unwrap();
            let slug = re.replace(&name, "").to_string();

            if slug.is_empty() {
                continue;
            }

            return Ok(SupabaseProject {
                project_id: slug.to_string(),
                root: None,
            });
        }

        Err("No valid project slug found".to_string())
    }
}
