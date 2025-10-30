use std::{collections::HashSet, fs::File, io::Read, path::PathBuf, sync::Arc, time::Duration};

use crate::{
    abstraction::SupabaseProject,
    cli::{CliSubcommand, Watch},
};

use async_trait::*;
use futures_util::{StreamExt, pin_mut, sink::SinkExt};
use notify_types::event::{CreateKind, ModifyKind};
use tokio::time::sleep;
use watchexec::Watchexec;
use watchexec_events::Tag;
use watchexec_signals::Signal;

#[async_trait]
impl CliSubcommand for Watch {
    async fn run(self: Box<Self>) {
        let (sender, mut receiver) = futures_channel::mpsc::channel::<(Arc<PathBuf>, bool)>(1024);

        let (dedup_sender, mut dedup_receiver) =
            futures_channel::mpsc::channel::<(Arc<PathBuf>, bool)>(1024);

        let wx = Watchexec::new({
            let debounced_sender = dedup_sender.clone();

            move |mut action| {
                for event in action.events.iter() {
                    for tag in &event.tags {
                        let Tag::FileEventKind(kind) = tag else {
                            continue;
                        };

                        if !matches!(
                            kind,
                            notify_types::event::EventKind::Create(CreateKind::File)
                                | notify_types::event::EventKind::Modify(ModifyKind::Data(_))
                        ) {
                            continue;
                        };

                        for (path, file_type) in event.paths() {
                            if !matches!(file_type, Some(watchexec_events::FileType::File)) {
                                continue;
                            }

                            let Some(extension) = path.extension() else {
                                continue;
                            };

                            if !extension.to_string_lossy().ends_with("sql") {
                                continue;
                            }

                            let mut debounced_sender = debounced_sender.clone();
                            let path = Arc::new(path.to_owned());

                            tokio::spawn(async move {
                                debounced_sender.send((path, false)).await.unwrap();
                            });
                        }
                    }
                }

                if action.signals().any(|sig| sig == Signal::Interrupt) {
                    action.quit();
                }

                action
            }
        })
        .unwrap();

        wx.config.pathset([self.directory.clone()]);

        tokio::spawn({
            let mut sender = sender.clone();

            async move {
                loop {
                    let deadline = sleep(Duration::from_millis(16));

                    let mut batch = Vec::with_capacity(1024);

                    let batch_fut = dedup_receiver
                        .by_ref()
                        .take(1024)
                        .take_until(deadline)
                        .collect::<Vec<_>>();

                    pin_mut!(batch_fut);

                    tokio::select! {
                        mut rest = batch_fut => {
                            batch.extend(rest.drain(..));
                        }
                        _ = tokio::signal::ctrl_c() => {
                            eprintln!(" terminated watcher");
                            break;
                        }
                    }

                    if batch.is_empty() {
                        continue;
                    }

                    let deduped = batch
                        .into_iter()
                        .map(move |(path, _)| path.to_string_lossy().to_string())
                        .collect::<HashSet<_>>();

                    for item in deduped {
                        sender.send((Arc::new(item.into()), false)).await.unwrap();
                    }
                }
            }
        });

        tokio::spawn(async move {
            while let Some((path, immediate_run)) = receiver.next().await {
                if immediate_run {
                    println!("🛫 Executing file immediately ({})", path.to_string_lossy());
                } else {
                    println!("🔍 Change observed ({})", path.to_string_lossy());
                }

                let mut file = File::open(path.to_str().unwrap()).unwrap();
                let mut sql = String::new();

                file.read_to_string(&mut sql).unwrap();

                match SupabaseProject::run_query(&sql).await {
                    Err(err) => eprintln!("❌ E{}\n", err),
                    _ => println!("✅ Query run successfully\n"),
                }
            }
        });

        if self.immediate {
            glob::glob(&format!("{}/**/*.sql", self.directory))
                .unwrap()
                .filter_map(|entry| entry.ok())
                .for_each(|path| {
                    let path = Arc::new(path);
                    let mut sender = sender.clone();

                    tokio::spawn(async move {
                        sender.send((path, true)).await.unwrap();
                    });
                });
        }

        wx.main().await.unwrap().unwrap();
    }
}
