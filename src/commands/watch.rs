use std::{fs::File, io::Read};

use crate::{
    abstraction::SupabaseProject,
    cli::{CliSubcommand, Watch},
};

use async_trait::*;
use notify_types::event::{CreateKind, ModifyKind};
use watchexec::Watchexec;
use watchexec_events::Tag;
use watchexec_signals::Signal;

#[async_trait]
impl CliSubcommand for Watch {
    async fn run(self: Box<Self>) {
        let wx = Watchexec::new(|mut action| {
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

                        println!("🔍 Change observed ({})", path.to_string_lossy());

                        let mut file = File::open(&path).unwrap();
                        let mut sql = String::new();

                        file.read_to_string(&mut sql).unwrap();

                        tokio::spawn(async move {
                            SupabaseProject::run_query(&sql).await;
                            println!("✅ Query run successfully")
                        });
                    }
                }
            }

            if action.signals().any(|sig| sig == Signal::Interrupt) {
                action.quit();
            }

            action
        })
        .unwrap();

        wx.config.pathset([self.directory]);

        wx.main().await.unwrap().unwrap();
    }
}
