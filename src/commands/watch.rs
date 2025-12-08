use std::{path::PathBuf, sync::Arc};

use super::prelude::*;
use crate::{
    abstraction::{CodeWatch, SupabaseProject},
    errors::NoWay,
};

use anyhow::Context;
use futures_channel::mpsc::Sender;
use futures_util::{StreamExt, sink::SinkExt};
use tokio::{fs::File, io::AsyncReadExt};
pub struct SqlFileExecutor;

impl SqlFileExecutor {
    pub fn start(project: SupabaseProject) -> Sender<ExecuteEvent> {
        let (execute_queuer, mut execute_queue) =
            futures_channel::mpsc::channel::<ExecuteEvent>(1024);

        let project = Arc::new(project);

        tokio::spawn(async move {
            let project = Arc::clone(&project);

            while let Some(ExecuteEvent {
                path,
                immediate_run,
            }) = execute_queue.next().await
            {
                if immediate_run {
                    supercli::styled!(
                        "🛫 Executing file immediately ({})",
                        (path.to_string_lossy(), "file_path")
                    );
                } else {
                    supercli::styled!(
                        "🔍 Change observed ({})",
                        (path.to_string_lossy(), "file_path")
                    );
                }

                let file = File::open(path.as_path()).await;

                let Some(mut file) = file.ok() else {
                    let message = styled_error!(
                        "Failed to open the file at {}",
                        (path.to_string_lossy(), "file_path")
                    );

                    println!("{}", message);

                    continue;
                };

                let mut sql = String::new();

                let result = file.read_to_string(&mut sql).await;

                if result.is_err() {
                    let message = styled_error!(
                        "Make sure the file at {} is a valid UTF-8 file",
                        (path.to_string_lossy(), "file_path")
                    );

                    println!("{}", message);

                    continue;
                }

                match project.runtime().sql(&sql).await {
                    Err(err) => supercli::error!(&format!("Error: {}\n", err)),
                    _ => supercli::success!("Query run successfully\n"),
                }
            }
        });

        execute_queuer
    }
}

#[derive(Default, PartialEq, Eq, Hash)]
pub struct ExecuteEvent {
    path: Arc<PathBuf>,
    immediate_run: bool,
}

impl ExecuteEvent {
    pub fn immediate(path: Arc<PathBuf>) -> Self {
        Self {
            path,
            immediate_run: true,
        }
    }

    pub fn watched(path: Arc<PathBuf>) -> Self {
        Self {
            path,
            immediate_run: false,
        }
    }
}

#[async_trait]
impl CliSubcommand for Watch {
    async fn run(self: Box<Self>) -> anyhow::Result<()> {
        let project = SupabaseProject::from_cwd().await?;

        let mut queuer = SqlFileExecutor::start(project.clone());

        let codewatch = CodeWatch::default()
            .extension("sql")
            .queuer(queuer.clone())
            .build(&self.directory, ExecuteEvent::watched)?;

        supercli::styled!(
            "👁️  Starting sql watch to reflect in project `{}`…\n",
            (project.id(), "id")
        );

        if self.immediate {
            let paths = glob::glob(&format!("{}/**/*.sql", self.directory))
                .context("Invalid directory path passed")?
                .filter_map(Result::ok);

            for path in paths {
                let path = Arc::new(path);

                queuer
                    .send(ExecuteEvent::immediate(path))
                    .await
                    .no_way_because("the receiver should still be alive by design");
            }
        }

        codewatch
            .run()
            .await
            .no_way_because("all the errors inside of tasks should be handled")
            .context("the file watcher has failed")?;

        Ok(())
    }
}
