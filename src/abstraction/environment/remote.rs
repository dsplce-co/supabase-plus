use anyhow::Context;
use tokio_postgres::Client;

use crate::abstraction::{Catalog, DatabaseUrl, StorageApi};

#[derive(Debug, Default)]
pub struct RemoteSource {
    pub db_url: Option<String>,
    pub project_url: Option<String>,
    pub service_role_key: Option<String>,
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
            DatabaseUrl::parse(&db_url).context("Refusing to clone from this database")?;

        Ok(Self {
            client: database.connect().await?,
            project_url: source.project_url,
            service_role_key: source.service_role_key,
        })
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
