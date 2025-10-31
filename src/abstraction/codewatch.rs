use derive_setters::Setters;
use futures_channel::mpsc::{Receiver, Sender, channel};
use futures_util::{SinkExt, StreamExt, pin_mut};
use notify_types::event::{CreateKind, ModifyKind};
use std::{collections::HashSet, path::PathBuf, sync::Arc, time::Duration};
use tokio::time::sleep;
use watchexec::Watchexec;
use watchexec_events::Tag;
use watchexec_signals::Signal;

#[derive(Setters, Default)]
#[setters(strip_option)]
pub struct CodeWatch<T> {
    #[setters(skip)]
    watcher: Option<Arc<Watchexec>>,

    #[setters(skip)]
    dedup: Option<(Sender<Arc<PathBuf>>, Receiver<Arc<PathBuf>>)>,

    #[setters(into)]
    extension: Option<String>,

    queuer: Option<Sender<T>>,

    ctor: Option<WatcherCtor<T>>,
}

type WatcherCtor<T> = Arc<dyn Fn(Arc<PathBuf>) -> T + Send + Sync + 'static>;

impl<T: Send + Sync + 'static + Eq + std::hash::Hash> CodeWatch<T> {
    pub fn build(
        mut self,
        path: &str,
        ctor: impl Fn(Arc<PathBuf>) -> T + Send + Sync + 'static + Clone,
    ) -> Self {
        self.dedup = Some(channel::<Arc<PathBuf>>(1024));
        self.ctor = Some(Arc::new(ctor));

        let maybe_expected_extension = Arc::new(self.extension.clone());

        let watcher = Watchexec::new({
            let dedup_queuer = self.dedup.as_ref().unwrap().0.clone();
            let maybe_expected_extension = maybe_expected_extension.clone();

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

                            if let Some(expected_extension) = maybe_expected_extension.as_ref() {
                                if !extension.to_string_lossy().ends_with(expected_extension) {
                                    continue;
                                }
                            }

                            let mut dedup_queuer = dedup_queuer.clone();
                            let path = Arc::new(path.to_owned());

                            tokio::spawn(async move {
                                dedup_queuer.send(path).await.unwrap();
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

        watcher.config.pathset([path]);
        self.watcher = Some(watcher);

        self
    }

    pub fn run(mut self) -> tokio::task::JoinHandle<Result<(), watchexec::error::CriticalError>> {
        tokio::spawn({
            let ctor: WatcherCtor<T> =
                Arc::clone(&self.ctor.expect("You should call `build` method first"));

            let mut dedup_queue = self.dedup.unwrap().1;

            async move {
                loop {
                    let deadline = sleep(Duration::from_millis(16));

                    let mut batch = Vec::with_capacity(1024);

                    let batch_fut = dedup_queue
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
                        .map(|path| ctor(path))
                        .collect::<HashSet<_>>();

                    if let Some(queuer) = &mut self.queuer {
                        for item in deduped {
                            queuer.send(item).await.unwrap();
                        }
                    }
                }
            }
        });

        self.watcher
            .expect("You should call `build` method first")
            .main()
    }
}
