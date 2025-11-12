use std::process::Output;

use anyhow::Context;
use tokio::process::Command;
use tokio_postgres::{Client, NoTls, Row, ToStatement, types::ToSql};

use crate::{abstraction::SupabaseProject, utils::escape_for_sh_double_quotes};

pub struct SupabaseRuntime<'a> {
    pub project: &'a SupabaseProject,
}

impl SupabaseRuntime<'_> {
    pub async fn validate(&self) -> anyhow::Result<()> {
        let project_id = self.project.id();
        let running_projects = SupabaseProject::running().await;

        if running_projects.len() > 1 {
            crate::styled_bail!(
                "You have multiple projects running which is an unhealthy quantum state for local Supabase, stop all with `{}` and then start the project via `{}`",
                ("sbp stop-any", "command"),
                ("supabase start", "command")
            );
        }

        let Some(running_project) = running_projects.iter().next() else {
            crate::styled_bail!(
                "You don't have a project running, you can start `{}` by running `{}` in the current directory",
                (project_id, "id"),
                ("supabase start", "command")
            );
        };

        let running_project_id = running_project.id();

        if project_id != running_project_id {
            crate::styled_bail!(
                "Currently running project is `{}` but you're in the directory of `{}` project.

It's not unambiguous for which of these projects you want to conduct the operation.

1. If you'd like to run it for `{}`, first stop `{}` with `{}` and then run `{}` in cwd.
2. If you'd like to run it for `{}`, first navigate to its directory.

Then re-run the command.",
                (running_project_id, "id"),
                (project_id, "id"),
                (project_id, "id"),
                (running_project_id, "id"),
                ("sbp stop-any", "command"),
                ("supabase start", "command"),
                (running_project_id, "id")
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
                supercli::error!("Connection error: {}", &error.to_string());
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

        let full_command = format!(
            "sh -c \"npx --yes supabase@latest {}\"",
            escape_for_sh_double_quotes(command)
        );

        if let Err(error) = cmd!(&full_command).run() {
            crate::styled_bail!(
                "Command execution failed:\n> {}",
                (&format!("{:?}", error), "muted")
            );
        }

        Ok(())
    }

    pub async fn command_silent(self, command: &str) -> anyhow::Result<Output> {
        self.validate().await?;

        let full_command = format!("npx --yes supabase@latest {}", command);

        Ok(Command::new("sh")
            .arg("-c")
            .arg(&full_command)
            .output()
            .await?)
    }

    pub async fn stop(self) -> anyhow::Result<()> {
        let SupabaseProject { project_id, root } = self.project;

        if root.is_some() {
            self.validate().await?;
        }

        self.command(&format!("stop --project-id={project_id}",))
            .await?;

        Ok(())
    }
}
