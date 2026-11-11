use std::collections::BTreeSet;

use async_trait::async_trait;
use tokio_postgres::Client;

use crate::abstraction::{BucketReport, Failure, ObjectRef, Report, StorageApi, Transferred};
use crate::patched::throbberous::Throbber;

#[async_trait]
pub trait Destination {
    async fn receive(
        &mut self,
        production: &StorageApi,
        object: &ObjectRef,
    ) -> anyhow::Result<Transferred>;
}

#[async_trait]
impl Destination for StorageApi {
    async fn receive(
        &mut self,
        production: &StorageApi,
        object: &ObjectRef,
    ) -> anyhow::Result<Transferred> {
        production.copy_to(self, object).await
    }
}

pub struct FileTransfer<D> {
    production: StorageApi,
    destination: D,
}

impl<D: Destination> FileTransfer<D> {
    pub fn new(production: StorageApi, destination: D) -> Self {
        Self {
            production,
            destination,
        }
    }

    pub fn into_destination(self) -> D {
        self.destination
    }

    pub async fn run(
        &mut self,
        buckets: &[String],
        objects: &[ObjectRef],
        production: &Client,
        report: &mut Report,
        throbber: Option<&Throbber>,
    ) -> anyhow::Result<()> {
        report.storage = buckets
            .iter()
            .map(|bucket| BucketReport {
                bucket: bucket.clone(),
                ..BucketReport::default()
            })
            .collect();

        let total = objects.len();

        for (index, object) in objects.iter().enumerate() {
            if let Some(throbber) = throbber {
                throbber
                    .set_message(format!(" copying files {}/{total}…", index + 1))
                    .await;
            }

            match self.destination.receive(&self.production, object).await {
                Ok(Transferred::Done(bytes)) => {
                    if let Some(bucket) = report
                        .storage
                        .iter_mut()
                        .find(|bucket| bucket.bucket == object.bucket)
                    {
                        bucket.files += 1;
                        bucket.bytes += bytes;
                    }
                }
                Ok(Transferred::Gone) => report
                    .discrepancies
                    .removed_since_snapshot
                    .push(object.display()),
                Err(error) => report.discrepancies.failed.push(Failure {
                    object: object.display(),
                    error: format!("{error:#}"),
                }),
            }
        }

        let known: BTreeSet<String> = objects.iter().map(ObjectRef::display).collect();

        report.discrepancies.added_since_snapshot = ObjectRef::list(production, buckets)
            .await?
            .iter()
            .map(ObjectRef::display)
            .filter(|name| !known.contains(name))
            .collect();

        Ok(())
    }
}
