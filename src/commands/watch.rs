use std::{path::PathBuf, sync::Arc};

use crate::{
    abstraction::{CodeWatch, SupabaseProject},
    cli::{CliSubcommand, Watch},
};

use async_trait::*;
use futures_channel::mpsc::Sender;
use futures_util::{StreamExt, sink::SinkExt};
use tokio::{fs::File, io::AsyncReadExt};

pub struct SqlFileExecutor;

impl SqlFileExecutor {
    pub fn start() -> Sender<ExecuteEvent> {
        let (execute_queuer, mut execute_queue) =
            futures_channel::mpsc::channel::<ExecuteEvent>(1024);

        tokio::spawn(async move {
            while let Some(ExecuteEvent {
                path,
                immediate_run,
            }) = execute_queue.next().await
            {
                if immediate_run {
                    println!("🛫 Executing file immediately ({})", path.to_string_lossy());
                } else {
                    println!("🔍 Change observed ({})", path.to_string_lossy());
                }

                let mut file = File::open(path.to_str().unwrap()).await.unwrap();
                let mut sql = String::new();

                file.read_to_string(&mut sql).await.unwrap();

                match SupabaseProject::run_query(&sql).await {
                    Err(err) => eprintln!("❌ E{}\n", err),
                    _ => println!("✅ Query run successfully\n"),
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
    async fn run(self: Box<Self>) {
        let mut queuer = SqlFileExecutor::start();

        let codewatch = CodeWatch::default()
            .extension("sql")
            .queuer(queuer.clone())
            .build(&self.directory, ExecuteEvent::watched);

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
    }
}
