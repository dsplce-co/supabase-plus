use std::{path::PathBuf, sync::Arc};

use super::prelude::*;
use crate::abstraction::{CodeWatch, SupabaseProject};

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

                let mut file = File::open(path.to_str().unwrap()).await.unwrap();
                let mut sql = String::new();

                file.read_to_string(&mut sql).await.unwrap();

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
            .build(&self.directory, ExecuteEvent::watched);

        supercli::styled!(
            "👁️  Starting sql watch to reflect in project `{}`…\n",
            (project.id(), "id")
        );

        if self.immediate {
            let paths = glob::glob(&format!("{}/**/*.sql", self.directory))
                .expect("Invalid directory path")
                .filter_map(Result::ok);

            for path in paths {
                let path = Arc::new(path);
                queuer.send(ExecuteEvent::immediate(path)).await.unwrap();
            }
        }

        codewatch.run().await.unwrap().unwrap();

        Ok(())
    }
}
