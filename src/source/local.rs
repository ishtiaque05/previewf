use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use async_trait::async_trait;

use super::{DirEntry, EntryType, FileMeta, FileSource};
use crate::PreviewError;

/// File source backed by the local filesystem.
pub struct LocalSource {
    base: PathBuf,
}

impl LocalSource {
    /// Create a new LocalSource rooted at `base`.
    /// The path is canonicalized at creation time.
    pub fn new<P: AsRef<Path>>(base: P) -> Result<Self, PreviewError> {
        let base = std::fs::canonicalize(base.as_ref())
            .map_err(|_| PreviewError::FileNotFound(base.as_ref().to_path_buf()))?;
        Ok(Self { base })
    }

    pub fn base(&self) -> &Path {
        &self.base
    }

    /// Resolve a relative path against the base, preventing traversal.
    ///
    /// For existing paths, canonicalize is used directly.
    /// For non-existing paths (e.g. write targets), the parent directory
    /// is canonicalized and the filename is appended.
    fn resolve(&self, path: &str) -> Result<PathBuf, PreviewError> {
        if path.is_empty() {
            return Ok(self.base.clone());
        }

        let joined = self.base.join(path);

        // Try canonicalizing the full path first (works for existing files).
        if let Ok(canonical) = std::fs::canonicalize(&joined) {
            return if canonical.starts_with(&self.base) {
                Ok(canonical)
            } else {
                Err(PreviewError::FileNotFound(joined))
            };
        }

        // Path doesn't exist yet — canonicalize the parent to check traversal,
        // then append the file name. This supports write_file for new files.
        if let Some(parent) = joined.parent() {
            if let Ok(canonical_parent) = std::fs::canonicalize(parent) {
                if canonical_parent.starts_with(&self.base) {
                    if let Some(file_name) = joined.file_name() {
                        return Ok(canonical_parent.join(file_name));
                    }
                }
            }
        }

        Err(PreviewError::FileNotFound(joined))
    }
}

#[async_trait]
impl FileSource for LocalSource {
    async fn read_file(&self, path: &str) -> Result<String, PreviewError> {
        let full = self.resolve(path)?;
        let path_for_err = self.base.join(path);
        let content = tokio::task::spawn_blocking(move || std::fs::read_to_string(&full))
            .await
            .map_err(|e| PreviewError::Server(std::io::Error::other(e)))?
            .map_err(|_| PreviewError::FileNotFound(path_for_err))?;
        Ok(content)
    }

    async fn list_dir(&self, path: &str) -> Result<Vec<DirEntry>, PreviewError> {
        let full = self.resolve(path)?;
        let entries = tokio::task::spawn_blocking(move || {
            let mut result = Vec::new();
            let read_dir =
                std::fs::read_dir(&full).map_err(|_| PreviewError::FileNotFound(full.clone()))?;
            for entry in read_dir.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with('.') {
                    continue;
                }
                let entry_type = match entry.file_type() {
                    Ok(ft) if ft.is_dir() => EntryType::Directory,
                    _ => EntryType::File,
                };
                result.push(DirEntry { name, entry_type });
            }
            result.sort_by_key(|e| e.name.to_lowercase());
            Ok::<Vec<DirEntry>, PreviewError>(result)
        })
        .await
        .map_err(|e| PreviewError::Server(std::io::Error::other(e)))??;
        Ok(entries)
    }

    async fn stat(&self, path: &str) -> Result<FileMeta, PreviewError> {
        let full = self.resolve(path)?;
        let meta = tokio::task::spawn_blocking(move || {
            let metadata =
                std::fs::metadata(&full).map_err(|_| PreviewError::FileNotFound(full.clone()))?;
            let mtime = metadata
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            Ok::<FileMeta, PreviewError>(FileMeta {
                mtime,
                size: metadata.len(),
                is_dir: metadata.is_dir(),
            })
        })
        .await
        .map_err(|e| PreviewError::Server(std::io::Error::other(e)))??;
        Ok(meta)
    }

    async fn write_file(&self, path: &str, content: &str) -> Result<(), PreviewError> {
        let full = self.resolve(path)?;
        let content = content.to_string();
        tokio::task::spawn_blocking(move || {
            std::fs::write(&full, &content).map_err(PreviewError::Server)
        })
        .await
        .map_err(|e| PreviewError::Server(std::io::Error::other(e)))??;
        Ok(())
    }

    async fn is_file(&self, path: &str) -> bool {
        let Ok(p) = self.resolve(path) else {
            return false;
        };
        tokio::task::spawn_blocking(move || p.is_file())
            .await
            .unwrap_or(false)
    }

    async fn is_dir(&self, path: &str) -> bool {
        let Ok(p) = self.resolve(path) else {
            return false;
        };
        tokio::task::spawn_blocking(move || p.is_dir())
            .await
            .unwrap_or(false)
    }

    fn display_root(&self) -> String {
        self.base.display().to_string()
    }
}
