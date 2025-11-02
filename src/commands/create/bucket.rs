use crate::commands::prelude::*;

use crate::{
    abstraction::{NewBucket, SupabaseProject},
    commands::create::Bucket,
};

#[async_trait]
impl CliSubcommand for Bucket {
    async fn run(self: Box<Self>) {
        let (bucket, shall_run) = use_promptuity!(promptuity => {
            let Ok(bucket) = NewBucket::new_interactively(&mut promptuity) else {
                exit(0)
            };

            let shall_run = promptuity
                .prompt(
                    Confirm::new(
                        "Would you like to run this migration immediately and set it to applied?",
                    )
                    .with_default(true),
                )
                .unwrap_or_else(|_| exit(0));

            let _ = promptuity.finish();

            (bucket, shall_run)
        });

        SupabaseProject::create_migration(bucket, shall_run)
            .await
            .expect("Failed to create migration");

        println!("Migration file created successfully!");
    }
}
