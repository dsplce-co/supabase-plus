use std::fmt;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use anyhow::Context;
use tokio::process::{Child, Command};

use crate::abstraction::{Archive, DatabaseUrl, NO_DOCKER, SupabaseProject, containers};

const PASSWORD: &str = "restore";
const DUMP: &str = "/tmp/database.dump";

pub struct PostgresImage {
    tag: String,
}

impl PostgresImage {
    pub fn resolve(project: &SupabaseProject) -> anyhow::Result<Self> {
        let tag = std::fs::read_to_string(project.root().join("supabase/.temp/postgres-version"))
            .map(|tag| tag.trim().to_string())
            .ok()
            .filter(|tag| !tag.is_empty())
            .with_context(|| {
                styled_error!(
                    "Couldn't tell which Postgres version production runs, run `{}` first",
                    ("supabase link", "command")
                )
            })?;

        if !tag.starts_with(|char: char| char.is_ascii_digit())
            || !tag.chars().all(|char| char.is_ascii_digit() || char == '.')
        {
            crate::styled_bail!(
                "`{}` isn't a Postgres image version like `{}`",
                (tag, "id"),
                ("15.8.1.069", "property")
            );
        }

        Ok(Self { tag })
    }

    pub async fn pull(&self) -> anyhow::Result<()> {
        let mut inspect = Command::new("docker");

        inspect.args(["image", "inspect"]).arg(self.to_string());

        if inspect
            .output()
            .await
            .with_context(|| NO_DOCKER.clone())?
            .status
            .success()
        {
            return Ok(());
        }

        let mut command = Command::new("docker");

        command.args(["pull", "--quiet"]).arg(self.to_string());

        containers::execute(command)
            .await
            .context("Couldn't pull the Postgres image")?;

        Ok(())
    }

    pub async fn dump(
        &self,
        database: &DatabaseUrl,
        snapshot: &str,
        dir: &Path,
    ) -> anyhow::Result<()> {
        let dump = [
            "pg_dump".to_string(),
            "-Fc".to_string(),
            format!("--snapshot={snapshot}"),
        ];

        let roles = [
            "pg_dumpall".to_string(),
            "--roles-only".to_string(),
            "--no-role-passwords".to_string(),
        ];

        self.export(database, &dump, &dir.join(Archive::DUMP))
            .await
            .context("Couldn't dump the production database")?;

        self.export(database, &roles, &dir.join(Archive::ROLES))
            .await
            .context("Couldn't dump the roles of the production database")
    }

    pub async fn disposable(&self, dir: &Path) -> anyhow::Result<Disposable> {
        let name = format!("sbp-restore-{}", std::process::id());

        let mut command = Command::new("docker");

        command
            .args(["run", "--rm", "--name", &name])
            .args(["-e", &format!("POSTGRES_PASSWORD={PASSWORD}")])
            .arg(self.to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true);

        let mut container = Disposable {
            name,
            process: command.spawn().with_context(|| NO_DOCKER.clone())?,
        };

        for _ in 0..120 {
            if container.process.try_wait()?.is_some() {
                break;
            }

            if container.sql("postgres", "select 1").await.is_ok() {
                let mut copy = Command::new("docker");

                copy.arg("cp")
                    .arg(dir.join(Archive::DUMP))
                    .arg(format!("{}:{DUMP}", container.name));

                containers::execute(copy)
                    .await
                    .context("Couldn't copy the dump to the disposable database")?;

                return Ok(container);
            }

            tokio::time::sleep(Duration::from_secs(1)).await;
        }

        crate::styled_bail!("The disposable database didn't start");
    }

    async fn export(
        &self,
        database: &DatabaseUrl,
        program: &[String],
        file: &Path,
    ) -> anyhow::Result<()> {
        let mut command = Command::new("docker");

        command
            .args(["run", "--rm"])
            .stdout(Stdio::from(std::fs::File::create(file)?))
            .stderr(Stdio::piped());

        for (key, value) in database.environment() {
            command.args(["-e", key]).env(key, value);
        }

        command.arg(self.to_string()).args(program);

        let output = command
            .spawn()
            .with_context(|| NO_DOCKER.clone())?
            .wait_with_output()
            .await?;

        containers::checked(output)?;

        Ok(())
    }
}

impl fmt::Display for PostgresImage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "public.ecr.aws/supabase/postgres:{}", self.tag)
    }
}

pub struct Disposable {
    name: String,
    process: Child,
}

impl Disposable {
    pub const DATABASE: &'static str = "restore_check";

    pub async fn sql(&self, database: &str, sql: &str) -> anyhow::Result<String> {
        let mut command = self.exec("psql");

        command
            .args(["-U", "supabase_admin", "-d", database])
            .args(["-v", "ON_ERROR_STOP=1", "-tAc", sql]);

        let output = containers::execute(command).await?;

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    pub async fn restore(&self) -> anyhow::Result<Vec<String>> {
        self.sql(
            "postgres",
            &format!("create database {} template template0", Self::DATABASE),
        )
        .await?;

        let mut command = self.exec("pg_restore");

        command
            .args(["-U", "supabase_admin", "-d", Self::DATABASE])
            .args(["--no-owner", "--no-privileges"])
            .arg(DUMP);

        let output = command.output().await.with_context(|| NO_DOCKER.clone())?;
        let stderr = String::from_utf8_lossy(&output.stderr);

        let mut errors: Vec<String> = stderr
            .lines()
            .filter_map(|line| line.strip_prefix("pg_restore: error: "))
            .map(str::to_string)
            .collect();

        if !output.status.success() && errors.is_empty() {
            errors.push(stderr.trim().to_string());
        }

        Ok(errors)
    }

    fn exec(&self, program: &str) -> Command {
        let mut command = Command::new("docker");

        command
            .args(["exec", "-e", &format!("PGPASSWORD={PASSWORD}"), &self.name])
            .arg(program)
            .args(["-h", "localhost"]);

        command
    }
}

impl Drop for Disposable {
    fn drop(&mut self) {
        let _ = std::process::Command::new("docker")
            .args(["rm", "-f", &self.name])
            .output();
    }
}
