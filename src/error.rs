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
}
