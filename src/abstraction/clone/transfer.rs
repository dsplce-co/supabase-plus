use std::collections::BTreeSet;

use crate::abstraction::{
    BucketReport, Failure, ObjectRef, Report, Snapshot, StorageApi, Transferred,
};
use crate::patched::throbberous::Throbber;

pub struct FileTransfer {
    production: StorageApi,
    local: StorageApi,
}

impl FileTransfer {
    pub fn new(production: StorageApi, local: StorageApi) -> Self {
        Self { production, local }
    }

    pub async fn run(
        &self,
        snapshot: &Snapshot<'_>,
        report: &mut Report,
        throbber: Option<&Throbber>,
    ) -> anyhow::Result<()> {
        report.storage = snapshot
            .buckets()
            .iter()
            .map(|bucket| BucketReport {
                bucket: bucket.clone(),
                ..BucketReport::default()
            })
            .collect();

        let total = snapshot.objects().len();

        for (index, object) in snapshot.objects().iter().enumerate() {
            if let Some(throbber) = throbber {
                throbber
                    .set_message(format!(" copying files {}/{total}…", index + 1))
                    .await;
            }

            match self.production.copy_to(&self.local, object).await {
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

        let known: BTreeSet<String> = snapshot.objects().iter().map(ObjectRef::display).collect();

        report.discrepancies.added_since_snapshot =
            ObjectRef::list(snapshot.connection().client(), snapshot.buckets())
                .await?
                .iter()
                .map(ObjectRef::display)
                .filter(|name| !known.contains(name))
                .collect();

        Ok(())
    }
}
