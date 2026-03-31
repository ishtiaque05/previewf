use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{broadcast, Mutex};
use tokio_util::sync::CancellationToken;

use crate::source::docker::DockerSource;
use crate::source::FileSource;

/// Polls Docker container files for changes by comparing mtimes.
pub struct DockerPollWatcher {
    source: Arc<DockerSource>,
    interval: Duration,
    tracked_files: Arc<Mutex<HashMap<String, u64>>>,
    reload_tx: broadcast::Sender<()>,
    cancel: CancellationToken,
}

impl DockerPollWatcher {
    pub fn new(
        source: Arc<DockerSource>,
        interval: Duration,
        reload_tx: broadcast::Sender<()>,
    ) -> Self {
        Self {
            source,
            interval,
            tracked_files: Arc::new(Mutex::new(HashMap::new())),
            reload_tx,
            cancel: CancellationToken::new(),
        }
    }

    /// Register a file path to be watched.
    pub async fn track(&self, path: String) {
        let mut files = self.tracked_files.lock().await;
        if let std::collections::hash_map::Entry::Vacant(e) = files.entry(path) {
            let mtime = self
                .source
                .stat(e.key())
                .await
                .map(|m| m.mtime)
                .unwrap_or(0);
            e.insert(mtime);
        }
    }

    /// Unregister a file path.
    pub async fn untrack(&self, path: &str) {
        let mut files = self.tracked_files.lock().await;
        files.remove(path);
    }

    /// Returns true if there are no tracked files.
    pub async fn is_empty(&self) -> bool {
        self.tracked_files.lock().await.is_empty()
    }

    /// Get a cancellation token for shutting down the watcher.
    pub fn cancel_token(&self) -> CancellationToken {
        self.cancel.clone()
    }

    /// Start the polling loop. Runs until cancelled.
    pub async fn run(&self) {
        loop {
            tokio::select! {
                _ = self.cancel.cancelled() => break,
                _ = tokio::time::sleep(self.interval) => {
                    self.poll_once().await;
                }
            }
        }
    }

    /// Run a single poll cycle: check mtimes of all tracked files.
    async fn poll_once(&self) {
        let files: Vec<(String, u64)> = {
            let tracked = self.tracked_files.lock().await;
            tracked.iter().map(|(k, v)| (k.clone(), *v)).collect()
        };

        if files.is_empty() {
            return;
        }

        let mut changed = false;
        let mut updates: Vec<(String, u64)> = Vec::new();

        for (path, old_mtime) in &files {
            match self.source.stat(path).await {
                Ok(meta) => {
                    if meta.mtime != *old_mtime {
                        changed = true;
                        updates.push((path.clone(), meta.mtime));
                    }
                }
                Err(_) => {
                    changed = true;
                }
            }
        }

        if changed {
            let mut tracked = self.tracked_files.lock().await;
            for (path, mtime) in updates {
                tracked.insert(path, mtime);
            }
            let _ = self.reload_tx.send(());
        }
    }
}
