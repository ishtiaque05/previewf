use std::path::PathBuf;

use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::broadcast;

use crate::PreviewError;

pub struct FileWatcher {
    path: PathBuf,
    watcher: Option<RecommendedWatcher>,
    sender: broadcast::Sender<PathBuf>,
}

impl FileWatcher {
    /// Create a new file watcher for the given path.
    /// Returns the watcher and a receiver for change notifications.
    pub fn new(path: PathBuf) -> Result<(Self, broadcast::Receiver<PathBuf>), PreviewError> {
        let (sender, receiver) = broadcast::channel(100);

        let watcher = FileWatcher {
            path,
            watcher: None,
            sender,
        };

        Ok((watcher, receiver))
    }

    /// Start watching for file changes.
    pub fn watch(&mut self) -> Result<(), PreviewError> {
        let sender = self.sender.clone();

        let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                if event.kind.is_modify() || event.kind.is_create() {
                    for path in event.paths {
                        let _ = sender.send(path);
                    }
                }
            }
        })
        .map_err(PreviewError::Watcher)?;

        let mode = if self.path.is_dir() {
            RecursiveMode::Recursive
        } else {
            RecursiveMode::NonRecursive
        };

        watcher
            .watch(&self.path, mode)
            .map_err(PreviewError::Watcher)?;

        self.watcher = Some(watcher);
        Ok(())
    }

    /// Get a new receiver for change notifications.
    pub fn subscribe(&self) -> broadcast::Receiver<PathBuf> {
        self.sender.subscribe()
    }
}
