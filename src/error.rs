use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum PreviewError {
    #[error("File not found: {0}")]
    FileNotFound(PathBuf),

    #[error("Not a markdown file: {0}")]
    NotMarkdown(PathBuf),

    #[error("Invalid flag syntax at line {line}: {detail}")]
    FlagParse { line: usize, detail: String },

    #[error("Server error: {0}")]
    Server(#[from] std::io::Error),

    #[error("Watch error: {0}")]
    Watcher(#[from] notify::Error),

    #[error("Docker not available: {0}")]
    DockerNotAvailable(String),

    #[error("Container not found: {0}")]
    ContainerNotFound(String),

    #[error("Container not running: {0}")]
    ContainerNotRunning(String),

    #[error("Docker command failed: {0}")]
    DockerExec(String),

    #[error("Path not found in container {container}:{path}")]
    ContainerPathNotFound { container: String, path: String },
}
