use std::collections::HashSet;

use anyhow::Context;
use bollard::{Docker, query_parameters::ListContainersOptions, secret::ContainerSummary};
use regex::Regex;
use tokio_postgres::NoTls;

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct SupabaseProject(String);

impl SupabaseProject {
    pub async fn run_query(sql: &str) -> anyhow::Result<()> {
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
