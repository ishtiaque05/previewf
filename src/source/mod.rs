pub mod docker;
pub mod local;

use async_trait::async_trait;
use serde::Serialize;

use crate::PreviewError;

/// The type of a directory entry.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum EntryType {
    File,
    Directory,
}

/// A single entry returned by `list_dir`.
#[derive(Debug, Clone, Serialize)]
pub struct DirEntry {
    pub name: String,
    pub entry_type: EntryType,
}

/// Metadata for a file or directory.
#[derive(Debug, Clone)]
pub struct FileMeta {
    /// Last modification time as a Unix timestamp (seconds).
    pub mtime: u64,
    /// Size in bytes.
    pub size: u64,
    /// Whether the path is a directory.
    pub is_dir: bool,
}

/// Abstraction over a filesystem — local or Docker container.
///
/// Every handler in the server reads/writes files through this trait,
/// which allows the same rendering logic for both local and container paths.
#[async_trait]
pub trait FileSource: Send + Sync {
    /// Read a file's contents as a UTF-8 string.
    async fn read_file(&self, path: &str) -> Result<String, PreviewError>;

    /// List entries in a directory.
    async fn list_dir(&self, path: &str) -> Result<Vec<DirEntry>, PreviewError>;

    /// Get metadata for a path.
    async fn stat(&self, path: &str) -> Result<FileMeta, PreviewError>;

    /// Write content to a file (used for flag injection).
    async fn write_file(&self, path: &str, content: &str) -> Result<(), PreviewError>;

    /// Check if a path is a file.
    async fn is_file(&self, path: &str) -> bool;

    /// Check if a path is a directory.
    async fn is_dir(&self, path: &str) -> bool;

    /// Human-readable root for display (e.g. "/Users/me/docs" or "my-app:/app/docs").
    fn display_root(&self) -> String;
}
