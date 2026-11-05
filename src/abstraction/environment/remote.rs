use anyhow::Context;
use tokio_postgres::{Client, config::Host};

use crate::abstraction::{Catalog, DatabaseUrl, StorageApi, SupabaseProject};

#[derive(Debug, Default)]
pub struct RemoteSource {
    pub db_url: Option<String>,
    pub project_url: Option<String>,
    pub service_role_key: Option<String>,
}

impl RemoteSource {
    pub fn requested(
        project: &SupabaseProject,
        linked: bool,
        db_url: Option<String>,
    ) -> anyhow::Result<Option<Self>> {
        if linked {
            return Self::linked(project, None).map(Some);
        }

        Ok(db_url.map(|db_url| Self {
            db_url: Some(db_url),
            ..Self::default()
        }))
    }

    /// Files under `supabase/.temp` aren't trusted blindly, as one planted in a cloned repo
    /// could otherwise send the password or the service role key to a foreign host
    pub fn linked(
        project: &SupabaseProject,
        service_role_key: Option<String>,
    ) -> anyhow::Result<Self> {
        let read = |name: &str| {
            std::fs::read_to_string(project.root().join("supabase/.temp").join(name))
                .ok()
                .map(|value| value.trim().to_string())
        };

        let reference = read("project-ref")
            .filter(|reference| {
                !reference.is_empty()
                    && reference
                        .chars()
                        .all(|char| char.is_ascii_lowercase() || char.is_ascii_digit())
            })
            .with_context(|| {
                crate::styled_error!(
                    "This project isn't linked, run `{}` first",
                    ("supabase link", "command")
                )
            })?;

        let pooler: tokio_postgres::Config = read("pooler-url")
            .with_context(|| {
                crate::styled_error!(
                    "The linked project has no pooler url, run `{}` again",
                    ("supabase link", "command")
                )
            })?
            .parse()
            .context("The pooler url of the linked project is not valid")?;

        let host = match pooler.get_hosts() {
            [Host::Tcp(host)]
                if (host.ends_with(".supabase.com") || host.ends_with(".supabase.co"))
                    && host.chars().all(|char| {
                        char.is_ascii_lowercase()
                            || char.is_ascii_digit()
                            || char == '.'
                            || char == '-'
                    }) =>
            {
                host
            }
            _ => crate::styled_bail!(
                "The pooler url of the linked project doesn't point to Supabase, refusing to send the password there"
            ),
        };

        let user = format!("postgres.{reference}");

        if pooler.get_user() != Some(&user) {
            crate::styled_bail!(
                "The pooler url of the linked project doesn't belong to `{}`",
                (&reference, "id")
            );
        }

        let Some(password) = std::env::var("SUPABASE_DB_PASSWORD")
            .ok()
            .filter(|password| !password.is_empty())
        else {
            crate::styled_bail!(
                "The database password of the linked project is missing, set `{}`",
                ("SUPABASE_DB_PASSWORD", "property")
            );
        };

        Ok(Self {
            db_url: Some(format!(
                "postgresql://{user}:{}@{host}:5432/postgres",
                urlencoding::encode(&password)
            )),
            project_url: Some(format!("https://{reference}.supabase.co")),
            service_role_key,
        })
    }
}

pub struct RemoteConnection {
    client: Client,
    project_url: Option<String>,
    service_role_key: Option<String>,
}

impl RemoteConnection {
    pub async fn from_source(source: RemoteSource) -> anyhow::Result<Self> {
        let Some(db_url) = source.db_url else {
            crate::styled_bail!(
                "Production database url is missing, pass `{}` or set `{}`",
                ("--db-url", "command"),
                ("SUPABASE_DB_URL", "property")
            );
        };

        let database =
            DatabaseUrl::parse(&db_url).context("Refusing to connect to this database")?;

        Ok(Self {
            client: database.connect().await?,
            project_url: source.project_url,
            service_role_key: source.service_role_key,
        })
    }

    pub fn into_client(self) -> Client {
        self.client
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    pub fn catalog(&self) -> Catalog<'_> {
        Catalog::new(&self.client)
    }

    pub fn storage(&self) -> anyhow::Result<StorageApi> {
        let (Some(url), Some(key)) = (&self.project_url, &self.service_role_key) else {
            crate::styled_bail!(
                "Files need the production project's api url and service role key, pass `{}` and `{}` (or set `{}` and `{}`)",
                ("--project-url", "command"),
                ("--service-role-key", "command"),
                ("SUPABASE_URL", "property"),
                ("SUPABASE_SERVICE_ROLE_KEY", "property")
            );
        };

        Ok(StorageApi::new(url, key))
    }
}
