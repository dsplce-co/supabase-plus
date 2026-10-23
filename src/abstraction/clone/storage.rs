use anyhow::Context;
use reqwest::{Body, Client as Http, StatusCode};
use tokio_postgres::Client;

use crate::abstraction::SupabaseProject;

#[derive(Debug, Clone)]
pub struct ObjectRef {
    pub bucket: String,
    pub name: String,
    pub size: u64,
}

impl ObjectRef {
    pub async fn list(client: &Client, buckets: &[String]) -> anyhow::Result<Vec<Self>> {
        let rows = client
            .query(
                "select bucket_id::text, name::text, coalesce((metadata->>'size')::bigint, 0)
                 from storage.objects where bucket_id = any($1) order by 1, 2",
                &[&buckets.to_vec()],
            )
            .await
            .context("Couldn't list storage objects")?;

        Ok(rows
            .into_iter()
            .map(|row| Self {
                bucket: row.get(0),
                name: row.get(1),
                size: row.get::<_, i64>(2).max(0) as u64,
            })
            .collect())
    }

    pub fn display(&self) -> String {
        format!("{}/{}", self.bucket, self.name)
    }
}

pub enum Transferred {
    Done(u64),

    Gone,
}

pub struct StorageApi {
    base: String,
    key: String,
    http: Http,
}

impl StorageApi {
    pub fn new(base: &str, key: &str) -> Self {
        Self {
            base: base.trim_end_matches('/').to_string(),
            key: key.to_string(),
            http: Http::new(),
        }
    }

    pub async fn local(project: &SupabaseProject) -> anyhow::Result<Self> {
        let output = project.runtime().command_silent("status -o env").await?;
        let text = String::from_utf8_lossy(&output.stdout);

        let setting = |names: &[&str]| {
            names.iter().find_map(|name| {
                text.lines()
                    .filter_map(|line| line.split_once('='))
                    .find(|(key, _)| key.trim() == *name)
                    .map(|(_, value)| value.trim().trim_matches('"').to_string())
            })
        };

        let (Some(url), Some(key)) = (
            setting(&["API_URL"]),
            setting(&["SERVICE_ROLE_KEY", "SECRET_KEY"]),
        ) else {
            crate::styled_bail!(
                "Couldn't read the local API url and service role key from `{}`",
                ("supabase status", "command")
            );
        };

        Ok(Self::new(&url, &key))
    }

    pub async fn copy_to(
        &self,
        target: &StorageApi,
        object: &ObjectRef,
    ) -> anyhow::Result<Transferred> {
        let response = self
            .authorize(self.http.get(self.object_url("authenticated/", object)))
            .send()
            .await
            .context("Download request failed")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();

            if status == StatusCode::NOT_FOUND
                || (status == StatusCode::BAD_REQUEST && body.contains("not_found"))
            {
                return Ok(Transferred::Gone);
            }

            anyhow::bail!("Download refused ({status}): {body}");
        }

        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("application/octet-stream")
            .to_string();

        let length = response.content_length();

        let mut upload = target
            .authorize(target.http.post(target.object_url("", object)))
            .header("x-upsert", "true")
            .header(reqwest::header::CONTENT_TYPE, content_type);

        if let Some(length) = length {
            upload = upload.header(reqwest::header::CONTENT_LENGTH, length);
        }

        let upload = upload
            .body(Body::wrap_stream(response.bytes_stream()))
            .send()
            .await
            .context("Upload request failed")?;

        if !upload.status().is_success() {
            let status = upload.status();

            anyhow::bail!(
                "Upload refused ({status}): {}",
                upload.text().await.unwrap_or_default()
            );
        }

        Ok(Transferred::Done(length.unwrap_or(object.size)))
    }

    fn object_url(&self, kind: &str, object: &ObjectRef) -> String {
        let path = object
            .name
            .split('/')
            .map(|segment| urlencoding::encode(segment).into_owned())
            .collect::<Vec<_>>()
            .join("/");

        format!(
            "{}/storage/v1/object/{kind}{}/{path}",
            self.base,
            urlencoding::encode(&object.bucket)
        )
    }

    fn authorize(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        let request = request.header("apikey", &self.key);

        if self.key.starts_with("eyJ") {
            request.bearer_auth(&self.key)
        } else {
            request
        }
    }
}
