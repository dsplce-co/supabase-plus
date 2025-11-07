use std::collections::HashSet;
use std::path::Path;
use std::{fs::File, io::Write};

use anyhow::Context;
use bollard::{Docker, query_parameters::ListContainersOptions, secret::ContainerSummary};
use chrono::Utc;
use regex::Regex;
use tokio::process::Command;
use tokio_postgres::{Client, NoTls};

use crate::abstraction::Migration;

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct SupabaseProject(String);

impl SupabaseProject {
    pub async fn create_migration<T: Migration>(
        migration: T,
        run_immediately: bool,
    ) -> anyhow::Result<()> {
        let name = migration.migration_name();
        let sql = migration.sql();

        let timecode = Utc::now().format("%Y%m%d%H%M%S").to_string();

        let mut file = File::create(format!("supabase/migrations/{timecode}_{name}.sql"))
            .expect("Failed to create migration file");

        file.write_all(sql.as_bytes())
            .expect("Failed to write to migration file");

        file.sync_all().expect("Failed to sync migration file");

        if run_immediately {
            SupabaseProject::execute_sql(&sql)
                .await
                .expect("Failed to run query");

            SupabaseProject::mark_timecode(&timecode, MigrationStatus::Applied, false);
        }

        Ok(())
    }

    pub fn mark_timecode(timecode: &str, status: MigrationStatus, linked: bool) {
        cmd!(
            "npx --yes supabase@latest migration repair --{} --status {} {}",
            if linked { "linked" } else { "local" },
            &status.to_string(),
            timecode
        )
        .run()
        .expect("Failed to mark migration");
    }

    pub async fn sql_client() -> anyhow::Result<Client> {
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

    pub async fn execute_sql(sql: &str) -> anyhow::Result<()> {
        let client = Self::sql_client().await?;

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
        let Self(project_id) = self;

        cmd!("npx --yes supabase@latest stop --project-id={}", project_id)
            .run()
            .unwrap();
    }

    pub fn id(&self) -> &str {
        &self.0
    }

    pub async fn migrations_table(linked: bool) -> anyhow::Result<Vec<(String, bool)>> {
        let path = Path::new("supabase/migrations");
        let mut entries = tokio::fs::read_dir(path).await?;

        let mut migrations_from_files = Vec::new();

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            let name = path.file_name().unwrap().to_str().unwrap().to_string();
            let regex = Regex::new(r"^\d{14}_").unwrap();
            let is_migration = regex.is_match(&name);

            if !is_migration {
                continue;
            }

            let timecode = name.split('_').next().unwrap().parse::<u64>().unwrap();

            migrations_from_files.push(timecode);
        }

        let cmd = format!(
            "npx --yes supabase@latest migration list --{} | awk '{{ print $3 }}' | grep '^2'",
            if linked { "linked" } else { "local" }
        );

        let run = Command::new("sh").arg("-c").arg(cmd).output().await?;

        if !run.status.success() {
            eprintln!("{}", String::from_utf8_lossy(&run.stderr));

            anyhow::bail!("supabase-cli error");
        }

        let output = run.stdout;

        let buffer = String::from_utf8(output).unwrap();

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

    pub async fn tables(schema: &str) -> anyhow::Result<Vec<String>> {
        let client = Self::sql_client().await?;

        let result = client
            .query(
                "select tablename from pg_tables where schemaname = $1",
                &[&schema],
            )
            .await
            .expect(&format!("Couldn't fetch tables for '{schema}' schema"));

        Ok(result.into_iter().map(|row| row.get(0)).collect())
    }

    pub async fn realtime_tables(schema: &str) -> anyhow::Result<Vec<String>> {
        let client = Self::sql_client().await?;

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

            return Ok(SupabaseProject(slug.to_string()));
        }

        Err("No valid project slug found".to_string())
    }
}

#[derive(strum_macros::Display)]
pub enum MigrationStatus {
    #[strum(to_string = "applied")]
    Applied,

    #[strum(to_string = "reverted")]
    Reverted,
}
