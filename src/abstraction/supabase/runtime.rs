use std::process::Output;

use anyhow::Context;
use tokio::process::Command;
use tokio_postgres::{Client, NoTls, Row, ToStatement, types::ToSql};

use crate::abstraction::SupabaseProject;

pub struct SupabaseRuntime<'a> {
    pub project: &'a SupabaseProject,
}

impl SupabaseRuntime<'_> {
    pub async fn validate(&self) -> anyhow::Result<()> {
        let project_id = self.project.id();
        let running_projects = SupabaseProject::running().await;

        if running_projects.len() > 1 {
            anyhow::bail!(
                "You have multiple projects running which is an unhealthy quantum state for local Supabase, stop all with `sbp stop-any` and then start the project via `supabase start`"
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

    async fn sql_client(&self) -> Result<Client, tokio_postgres::Error> {
        let (client, connection) = tokio_postgres::connect(
            "postgresql://postgres:postgres@127.0.0.1:54322/postgres",
            NoTls,
        )
        .await?;

        tokio::spawn(async move {
            if let Err(error) = connection.await {
                eprintln!("Connection error: {}", error);
            }
        });

        Ok(client)
    }

    pub async fn sql(self, sql: &str) -> anyhow::Result<()> {
        self.validate().await?;
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

    pub async fn query<T>(
        self,
        statement: &T,
        params: &[&(dyn ToSql + Sync)],
    ) -> anyhow::Result<Vec<Row>>
    where
        T: ?Sized + ToStatement,
    {
        self.validate().await?;
        let client: Client = self.sql_client().await?;

        client.query(statement, params).await.context("SQL error")
    }

    pub async fn command(self, command: &str) -> anyhow::Result<()> {
        self.validate().await?;

        if let Err(error) = cmd!(command).run() {
            anyhow::bail!("Command execution failed:\n> {:?}", error);
        }

        Ok(())
    }

    pub async fn command_silent(self, command: &str) -> anyhow::Result<Output> {
        self.validate().await.context("SQL error")?;

        Ok(Command::new("sh").arg("-c").arg(command).output().await?)
    }
}
