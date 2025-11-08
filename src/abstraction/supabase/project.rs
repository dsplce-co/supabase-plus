use crate::abstraction::{Migration, MigrationStatus};
use crate::errors::NoWay;

use std::collections::HashSet;
use std::path::PathBuf;
use std::{fs::File, io::Write};

use anyhow::{Context, bail};
use bollard::{Docker, query_parameters::ListContainersOptions, secret::ContainerSummary};
use chrono::Utc;
use regex::Regex;
use tokio::process::Command;
use tokio_postgres::{Client, NoTls};

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct SupabaseProject {
    pub(crate) project_id: String,
    pub(crate) root: Option<PathBuf>,
}

impl SupabaseProject {
    pub async fn from_cwd() -> anyhow::Result<Self> {
        let root = std::env::current_dir()
            .with_context(|| "Failed to get current directory to indicate the project, make sure the current directory exists (sic!) and you have the necessary permissions")?;

        let root = Self::find_root(root).with_context(|| "Could not find Supabase project root, no `supabase/config.toml` in cwd or any parent directory")?;
        let config_path = root.join("supabase/config.toml");

        let config = std::fs::read_to_string(config_path)
            .no_way_because("The config file has been found earlier");

        let project_id = config
            .lines()
            .find(|line| line.starts_with("project_id"))
            .with_context(|| "Failed to find project_id in config.toml, make sure the config.toml has no syntax errors and contains a project_id")?
            .splitn(2, '=')
            .nth(1)
            .with_context(|| "Failed to parse project_id in config.toml, make sure the project_id kv pair has no syntax errors")?
            .trim()
            .to_string();

        let project_id: String = serde_json::from_str(&project_id).with_context(
            || "Failed to parse project_id in config.toml, make sure the project_id value has no syntax errors",
        )?;

        Self::check_for_ambiguity(&project_id).await?;

        Ok(Self {
            project_id,
            root: Some(root),
        })
    }

    async fn check_for_ambiguity(project_id: &str) -> anyhow::Result<()> {
        let running_projects = Self::running().await;

        if running_projects.len() > 1 {
            anyhow::bail!(
                "You have multiple projects running which is an unhealthy quantum state for local Supabase, stop all with `sbp stop-any` and then start the project regularly"
            );
        }

        let Some(running_project) = running_projects.iter().next() else {
            anyhow::bail!(
                "You don't have a project running, you can start `{project_id}` by running `supabase start` in the current directory"
            );
        };

        let running_project_id = running_project.id();

        if project_id != running_project_id {
            anyhow::bail!(
                    "Currently running project is `{running_project_id}` but you're in the directory of `{project_id}` project.

It's not unambiguous for which of these projects you want to conduct the operation.

1. If you'd like to run it for `{project_id}`, first stop `{running_project_id}` with `sbp stop-any` and then run `supabase start` in cwd.
2. If you'd like to run it for `{running_project_id}`, first navigate to its directory.

Then re-run the command."
            );
        }

        Ok(())
    }

    async fn not_ambigious(&self) -> anyhow::Result<()> {
        Self::check_for_ambiguity(self.id()).await
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
        run_immediately: bool,
    ) -> anyhow::Result<()> {
        let name = migration.migration_name();
        let sql = migration.sql();

        let timecode = Utc::now().format("%Y%m%d%H%M%S").to_string();

        // TODO: This needs to be done in project root
        let migrations_dir = self.migrations_dir();

        if !migrations_dir.exists() {
            std::fs::create_dir(&migrations_dir)
                .context("You don't seem to have a `supabase/migrations` directory, we tried creating it but we failed")?;
        }

        if migrations_dir.is_file() {
            anyhow::bail!("`supabase/migrations` is a file, not a directory");
        }

        let file_path = migrations_dir.join(format!("{timecode}_{name}.sql"));

        let mut file = File::create(file_path).with_context(|| {
            anyhow::anyhow!(
                "Failed to create migration file at `{}`\n> {}",
                migrations_dir.display(),
                std::io::Error::last_os_error()
            )
        })?;

        file.write_all(sql.as_bytes())
            .context("Failed to write to the migration file")?;

        file.sync_all()
            .context("Failed to sync newly created migration file")?;

        if run_immediately {
            self.execute_sql(&sql).await?;
            self.mark_timecode(&timecode, MigrationStatus::Applied, false)?;
        }

        Ok(())
    }

    pub fn mark_timecode(
        &self,
        timecode: &str,
        status: MigrationStatus,
        linked: bool,
    ) -> anyhow::Result<()> {
        let cmd = cmd!(
            "npx --yes supabase@latest migration repair --{} --status {} {}",
            if linked { "linked" } else { "local" },
            &status.to_string(),
            timecode
        )
        .run();

        if let Err(error) = cmd {
            bail!(
                "Failed to mark migration as {}\n> {:#?}",
                status.to_string(),
                error
            )
        }

        Ok(())
    }

    pub async fn sql_client(&self) -> anyhow::Result<Client> {
        self.not_ambigious().await?;

        let (client, connection) = tokio_postgres::connect(
            "postgresql://postgres:postgres@127.0.0.1:54322/postgres",
            NoTls,
        )
        .await
        .with_context(|| "Couldn't connect to the database")?;

        tokio::spawn(async move {
            if let Err(error) = connection.await {
                eprintln!("Connection error: {}", error);
            }
        });

        Ok(client)
    }

    pub async fn execute_sql(&self, sql: &str) -> anyhow::Result<()> {
        let client = self.sql_client().await?;

        let result = client.batch_execute(sql).await;

        if let Some(error) = result
            .as_ref()
            .err()
            .map(|error| error.as_db_error())
            .and_then(|option| option)
        {
            let mut message = format!("{}: {}\n\t", error.code().code(), error.message());

            if let Some(position) = error.position() {
                message.push_str(&format!("at char {:?}", position));
            }

            if let Some(where_) = error.where_() {
                message.push_str(" ");
                message.push_str(&where_.replace("\n", " "));
            }

            anyhow::bail!(message);
        }

        Ok(())
    }

    pub async fn running() -> HashSet<SupabaseProject> {
        let containers = Self::get_supabase_containers().await;

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

        projects.into_iter().collect()
    }

    // TODO: Move to another namespace
    async fn get_supabase_containers() -> Vec<ContainerSummary> {
        let docker = Docker::connect_with_socket_defaults().unwrap();

        docker
            .list_containers(None::<ListContainersOptions>)
            .await
            .unwrap()
            .into_iter()
            .filter(|container| {
                container
                    .names
                    .as_ref()
                    .map(|indeed_names| {
                        indeed_names
                            .iter()
                            .any(|name| name.starts_with("/supabase_"))
                    })
                    .unwrap_or_default()
            })
            .collect()
    }

    pub fn stop(&self) {
        let Self { project_id, .. } = self;

        cmd!("npx --yes supabase@latest stop --project-id={}", project_id)
            .run()
            .unwrap();
    }

    pub fn id(&self) -> &str {
        &self.project_id
    }

    pub async fn migrations_table(&self, linked: bool) -> anyhow::Result<Vec<(String, bool)>> {
        let path = self.migrations_dir();

        let mut entries = tokio::fs::read_dir(&path).await.with_context(|| {
            anyhow::anyhow!(
                "Couldn't find `{}` directory\n> {}",
                path.display(),
                std::io::Error::last_os_error()
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

        let cmd = format!(
            "npx --yes supabase@latest migration list --{} | awk '{{ print $3 }}' | grep '^2' | cat",
            if linked { "linked" } else { "local" }
        );

        self.not_ambigious().await?;

        let run = Command::new("sh").arg("-c").arg(cmd).output().await?;

        if !run.status.success() {
            eprintln!("{}", String::from_utf8_lossy(&run.stderr));

            anyhow::bail!("supabase-cli error");
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
        let client = self.sql_client().await?;

        let result = client
            .query(
                "select tablename from pg_tables where schemaname = $1",
                &[&schema],
            )
            .await
            .with_context(|| format!("Couldn't fetch tables for '{schema}' schema"))?;

        Ok(result.into_iter().map(|row| row.get(0)).collect())
    }

    pub async fn realtime_tables(&self, schema: &str) -> anyhow::Result<Vec<String>> {
        let client = self.sql_client().await?;

        let result = client.query(
            "select tablename from pg_publication_tables where schemaname = $1 and pubname = 'supabase_realtime'",
            &[&schema]
        )
            .await
            .expect(&format!("Couldn't fetch realtime tables for '{schema}' schema"));

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
