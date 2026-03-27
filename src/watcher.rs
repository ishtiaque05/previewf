use std::path::PathBuf;

use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::broadcast;

use crate::PreviewError;

pub struct FileWatcher {
    _watcher: RecommendedWatcher,
    sender: broadcast::Sender<PathBuf>,
}

impl FileWatcher {
    /// Create and start a file watcher for the given path.
    /// Returns the watcher and a receiver for change notifications.
    pub fn new(path: PathBuf) -> Result<(Self, broadcast::Receiver<PathBuf>), PreviewError> {
        let (sender, receiver) = broadcast::channel(100);
        let tx = sender.clone();

        let mut watcher =
            notify::recommended_watcher(move |res: Result<Event, notify::Error>| match res {
                Ok(event) => {
                    if event.kind.is_modify() || event.kind.is_create() {
                        for path in event.paths {
                            let _ = tx.send(path);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Warning: file watcher error: {e}");
                }
            })
            .map_err(PreviewError::Watcher)?;

        let mode = if path.is_dir() {
            RecursiveMode::Recursive
        } else {
            RecursiveMode::NonRecursive
        };

        watcher.watch(&path, mode).map_err(PreviewError::Watcher)?;

        Ok((
            Self {
                _watcher: watcher,
                sender,
            },
            receiver,
        ))
    }

    /// Get a new receiver for change notifications.
    pub fn subscribe(&self) -> broadcast::Receiver<PathBuf> {
        self.sender.subscribe()
    }
}
