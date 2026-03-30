# Docker Container File Serving — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Serve and preview files from inside Docker containers, both from host and from within containers.

**Architecture:** Introduce a `FileSource` async trait that abstracts filesystem access. `LocalSource` wraps `std::fs` (existing behavior). `DockerSource` shells out via `docker exec`. Server refactored to use `&dyn FileSource`, enabling Docker routes under `/docker/:container/*` alongside existing local routes. Polling watcher for Docker live reload.

**Tech Stack:** Rust, axum, async-trait, tokio-util (CancellationToken), Docker CLI (no SDK crate)

---

## File Map

| Action | File | Responsibility |
|--------|------|----------------|
| Create | `src/source/mod.rs` | FileSource trait, DirEntry, FileMeta, EntryType |
| Create | `src/source/local.rs` | LocalSource wrapping std::fs |
| Create | `src/source/docker.rs` | DockerSource using docker exec |
| Create | `src/docker.rs` | Container discovery, validation, CLI wrappers |
| Create | `src/docker_watcher.rs` | DockerPollWatcher with configurable interval |
| Create | `tests/source_test.rs` | Tests for LocalSource |
| Create | `tests/docker_test.rs` | Tests for DockerSource + docker module |
| Modify | `src/error.rs` | Add Docker error variants |
| Modify | `src/lib.rs` | Declare new modules |
| Modify | `src/server.rs` | Refactor to use FileSource, add Docker routes, multi-source AppState |
| Modify | `src/main.rs` | Add --host flag, docker subcommand |
| Modify | `Cargo.toml` | Add async-trait, tokio-util deps |
| Modify | `assets/index.html` | Docker containers section |
| Modify | `assets/app.js` | Fetch and render container list (NO innerHTML — safe DOM only) |
| Modify | `assets/style.css` | Docker section styling |
| Modify | `tests/server_test.rs` | Update for refactored create_router |
| Create | `book/src/usage/docker.md` | Docker usage documentation |
| Modify | `book/src/usage/serve.md` | Add --host flag docs |
| Modify | `book/src/concepts/design-decisions.md` | Docker design decisions |
| Modify | `book/src/architecture/overview.md` | FileSource + Docker modules |
| Modify | `book/src/SUMMARY.md` | Add Docker page |
| Modify | `book/src/roadmap.md` | Mark Docker as completed |
| Modify | `book/src/implementation/notes.md` | Implementation log entry |

---

## Task 1: Add Dependencies and Docker Error Variants

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/error.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Add new dependencies to Cargo.toml**

```toml
# Add under [dependencies]:
async-trait = "0.1"
tokio-util = { version = "0.7", features = ["rt"] }
```

- [ ] **Step 2: Add Docker error variants to PreviewError**

In `src/error.rs`, add five new variants:

```rust
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
```

- [ ] **Step 3: Run clippy and tests to verify no regressions**

Run: `cargo clippy -- -D warnings && cargo test`
Expected: All pass. New variants are unused but clippy allows dead_code on enum variants.

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml src/error.rs
git commit -m "Add async-trait, tokio-util deps and Docker error variants"
```

---

## Task 2: Create FileSource Trait and Types

**Files:**
- Create: `src/source/mod.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Create src/source/mod.rs with trait and types**

```rust
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
```

- [ ] **Step 2: Add source module to lib.rs**

```rust
pub mod error;
pub mod flags;
pub mod html;
pub mod markdown;
pub mod server;
pub mod source;
pub mod terminal;
pub mod watcher;

pub use error::PreviewError;
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check`
Expected: Compiles (local module declared but empty, will be created next).

- [ ] **Step 4: Commit**

```bash
git add src/source/mod.rs src/lib.rs
git commit -m "Add FileSource trait with DirEntry and FileMeta types"
```

---

## Task 3: Implement LocalSource

**Files:**
- Create: `src/source/local.rs`
- Create: `tests/source_test.rs`

- [ ] **Step 1: Write tests for LocalSource**

```rust
// tests/source_test.rs
use previewf::source::local::LocalSource;
use previewf::source::{EntryType, FileSource};

#[tokio::test]
async fn test_local_read_file() {
    let source = LocalSource::new("tests/fixtures").unwrap();
    let content = source.read_file("sample.md").await.unwrap();
    assert!(content.contains("Sample Document"));
}

#[tokio::test]
async fn test_local_read_file_not_found() {
    let source = LocalSource::new("tests/fixtures").unwrap();
    let result = source.read_file("nonexistent.md").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_local_list_dir_root() {
    let source = LocalSource::new("tests/fixtures").unwrap();
    let entries = source.list_dir("").await.unwrap();
    let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"sample.md"));
    assert!(names.contains(&"sample.html"));
}

#[tokio::test]
async fn test_local_list_dir_has_correct_types() {
    let source = LocalSource::new("tests/fixtures").unwrap();
    let entries = source.list_dir("").await.unwrap();
    for entry in &entries {
        if entry.name.ends_with(".md") || entry.name.ends_with(".html") {
            assert_eq!(entry.entry_type, EntryType::File);
        }
    }
}

#[tokio::test]
async fn test_local_stat_file() {
    let source = LocalSource::new("tests/fixtures").unwrap();
    let meta = source.stat("sample.md").await.unwrap();
    assert!(!meta.is_dir);
    assert!(meta.size > 0);
}

#[tokio::test]
async fn test_local_is_file() {
    let source = LocalSource::new("tests/fixtures").unwrap();
    assert!(source.is_file("sample.md").await);
    assert!(!source.is_file("nonexistent.md").await);
}

#[tokio::test]
async fn test_local_is_dir() {
    let source = LocalSource::new("tests/fixtures").unwrap();
    assert!(source.is_dir("").await);
    assert!(!source.is_dir("sample.md").await);
}

#[tokio::test]
async fn test_local_write_file() {
    let dir = tempfile::TempDir::new().unwrap();
    let source = LocalSource::new(dir.path()).unwrap();
    source.write_file("test.md", "# Hello\n").await.unwrap();
    let content = source.read_file("test.md").await.unwrap();
    assert_eq!(content, "# Hello\n");
}

#[tokio::test]
async fn test_local_path_traversal_rejected() {
    let source = LocalSource::new("tests/fixtures").unwrap();
    let result = source.read_file("../../Cargo.toml").await;
    assert!(result.is_err(), "path traversal must be rejected");
}

#[tokio::test]
async fn test_local_display_root() {
    let source = LocalSource::new("tests/fixtures").unwrap();
    let root = source.display_root();
    assert!(root.contains("fixtures"), "display root should contain the path");
}
```

- [ ] **Step 2: Run tests to see them fail**

Run: `cargo test --test source_test`
Expected: FAIL — `LocalSource` not yet implemented.

- [ ] **Step 3: Implement LocalSource**

```rust
// src/source/local.rs
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
    fn resolve(&self, path: &str) -> Result<PathBuf, PreviewError> {
        if path.is_empty() {
            return Ok(self.base.clone());
        }

        let joined = self.base.join(path);
        let canonical = std::fs::canonicalize(&joined)
            .map_err(|_| PreviewError::FileNotFound(joined.clone()))?;

        if canonical.starts_with(&self.base) {
            Ok(canonical)
        } else {
            Err(PreviewError::FileNotFound(joined))
        }
    }
}

#[async_trait]
impl FileSource for LocalSource {
    async fn read_file(&self, path: &str) -> Result<String, PreviewError> {
        let full = self.resolve(path)?;
        let content = tokio::task::spawn_blocking(move || std::fs::read_to_string(&full))
            .await
            .map_err(|e| PreviewError::Server(std::io::Error::new(std::io::ErrorKind::Other, e)))?
            .map_err(|_| PreviewError::FileNotFound(self.base.join(path)))?;
        Ok(content)
    }

    async fn list_dir(&self, path: &str) -> Result<Vec<DirEntry>, PreviewError> {
        let full = self.resolve(path)?;
        let entries = tokio::task::spawn_blocking(move || {
            let mut result = Vec::new();
            let read_dir = std::fs::read_dir(&full)
                .map_err(|_| PreviewError::FileNotFound(full.clone()))?;
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
        .map_err(|e| PreviewError::Server(std::io::Error::new(std::io::ErrorKind::Other, e)))??;
        Ok(entries)
    }

    async fn stat(&self, path: &str) -> Result<FileMeta, PreviewError> {
        let full = self.resolve(path)?;
        let meta = tokio::task::spawn_blocking(move || {
            let metadata = std::fs::metadata(&full)
                .map_err(|_| PreviewError::FileNotFound(full.clone()))?;
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
        .map_err(|e| PreviewError::Server(std::io::Error::new(std::io::ErrorKind::Other, e)))??;
        Ok(meta)
    }

    async fn write_file(&self, path: &str, content: &str) -> Result<(), PreviewError> {
        let full = self.resolve(path)?;
        let content = content.to_string();
        tokio::task::spawn_blocking(move || {
            std::fs::write(&full, &content)
                .map_err(PreviewError::Server)
        })
        .await
        .map_err(|e| PreviewError::Server(std::io::Error::new(std::io::ErrorKind::Other, e)))??;
        Ok(())
    }

    async fn is_file(&self, path: &str) -> bool {
        self.resolve(path).map(|p| p.is_file()).unwrap_or(false)
    }

    async fn is_dir(&self, path: &str) -> bool {
        self.resolve(path).map(|p| p.is_dir()).unwrap_or(false)
    }

    fn display_root(&self) -> String {
        self.base.display().to_string()
    }
}
```

- [ ] **Step 4: Run tests to see them pass**

Run: `cargo test --test source_test`
Expected: All PASS.

- [ ] **Step 5: Run full test suite + clippy**

Run: `cargo clippy -- -D warnings && cargo test`
Expected: All pass, no warnings.

- [ ] **Step 6: Commit**

```bash
git add src/source/local.rs tests/source_test.rs
git commit -m "Implement LocalSource wrapping std::fs with path traversal prevention"
```

---

## Task 4: Refactor Server to Use FileSource

This is the largest task — replace all direct `std::fs` calls in `server.rs` handlers with `FileSource` trait methods. The key change: `AppState` holds `Arc<dyn FileSource>` and all handlers call `state.source.method()` instead of `std::fs::*`.

**Files:**
- Modify: `src/server.rs`
- Modify: `tests/server_test.rs`

- [ ] **Step 1: Run existing server tests to establish baseline**

Run: `cargo test --test server_test`
Expected: All 20 tests PASS.

- [ ] **Step 2: Update AppState and ServerConfig**

In `src/server.rs`, update the configuration and state:

Add `host` to `ServerConfig` and `ServerBuilder`:

```rust
#[derive(Clone, Debug)]
pub struct ServerConfig {
    path: PathBuf,
    host: String,
    port: u16,
    live_reload: bool,
}

impl ServerConfig {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn host(&self) -> &str {
        &self.host
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn live_reload(&self) -> bool {
        self.live_reload
    }
}

pub struct ServerBuilder {
    path: PathBuf,
    host: String,
    port: u16,
    live_reload: bool,
}

impl ServerBuilder {
    pub fn new() -> Self {
        Self {
            path: PathBuf::from("."),
            host: "127.0.0.1".to_string(),
            port: 4567,
            live_reload: true,
        }
    }

    pub fn path<P: Into<PathBuf>>(mut self, path: P) -> Self {
        self.path = path.into();
        self
    }

    pub fn host<S: Into<String>>(mut self, host: S) -> Self {
        self.host = host.into();
        self
    }

    pub fn port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    pub fn live_reload(mut self, enabled: bool) -> Self {
        self.live_reload = enabled;
        self
    }

    pub fn build(self) -> Result<ServerConfig, PreviewError> {
        let path = std::fs::canonicalize(&self.path)
            .map_err(|_| PreviewError::FileNotFound(self.path.clone()))?;
        Ok(ServerConfig {
            path,
            host: self.host,
            port: self.port,
            live_reload: self.live_reload,
        })
    }
}
```

Update `AppState` to hold a `FileSource`:

```rust
use crate::source::local::LocalSource;
use crate::source::FileSource;

#[derive(Clone)]
struct AppState {
    config: ServerConfig,
    source: Arc<dyn FileSource>,
    reload_tx: broadcast::Sender<()>,
    file_locks: Arc<Mutex<HashMap<String, Arc<Mutex<()>>>>>,
    docker_available: bool,
}
```

- [ ] **Step 3: Update create_router and run to use LocalSource**

Update `create_router`:

```rust
pub fn create_router(config: ServerConfig) -> Router {
    create_router_with_reload(config, broadcast::channel::<()>(16).0)
}

fn create_router_with_reload(config: ServerConfig, reload_tx: broadcast::Sender<()>) -> Router {
    let source = Arc::new(
        LocalSource::new(config.path()).expect("LocalSource base path must exist")
    );
    let docker_available = check_docker_sync();

    let state = AppState {
        config,
        source,
        reload_tx,
        file_locks: Arc::new(Mutex::new(HashMap::new())),
        docker_available,
    };

    Router::new()
        .route("/", get(index_handler))
        .route("/browse/{*dirpath}", get(browse_handler))
        .route("/view/{*filepath}", get(view_handler))
        .route("/raw/{*filepath}", get(raw_handler))
        .route("/flags/{*filepath}", get(flags_handler))
        .route("/flag/{*filepath}", post(flag_handler))
        .route("/api/tree", get(tree_handler))
        .route("/ws", get(ws_handler))
        .route("/assets/{*filepath}", get(asset_handler))
        .with_state(state)
        .layer(middleware::from_fn(security_headers))
}

/// Quick synchronous check for Docker availability (called once at startup).
fn check_docker_sync() -> bool {
    std::process::Command::new("docker")
        .args(["version", "--format", "{{.Client.Version}}"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
```

Update `run` to use `config.host()`:

```rust
pub async fn run(config: ServerConfig) -> Result<(), PreviewError> {
    let (reload_tx, _) = broadcast::channel::<()>(16);

    if config.live_reload() {
        let watcher_path = config.path().to_path_buf();
        let tx = reload_tx.clone();
        tokio::spawn(async move {
            match crate::watcher::FileWatcher::new(watcher_path) {
                Ok((_fw, mut rx)) => loop {
                    match rx.recv().await {
                        Ok(_) => { let _ = tx.send(()); }
                        Err(broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(_) => break,
                    }
                },
                Err(e) => {
                    eprintln!("Warning: file watcher failed to start: {e}");
                }
            }
        });
    }

    let app = create_router_with_reload(config.clone(), reload_tx);
    let addr = format!("{}:{}", config.host(), config.port());
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    eprintln!(
        "previewf serving {} on http://{}:{}",
        config.path().display(),
        config.host(),
        config.port()
    );

    axum::serve(listener, app).await?;
    Ok(())
}
```

- [ ] **Step 4: Refactor handlers to use FileSource**

Convert `listing_response` to async, using `state.source`. Convert `view_handler`, `raw_handler`, `flags_handler`, `flag_handler` to use `state.source.read_file()` / `state.source.write_file()` instead of `std::fs` calls. Convert `tree_handler` to use an async `build_tree_async` function that calls `source.list_dir()`.

Remove the now-unused `resolve_path` function and `build_tree`/`build_tree_inner` functions.

The key pattern for each handler is replacing:
```rust
// Before:
let full_path = match resolve_path(state.config.path(), &filepath) {
    Some(p) => p,
    None => return not_found_response(&filepath),
};
let content = match std::fs::read_to_string(&full_path) { ... };

// After:
let content = match state.source.read_file(&filepath).await { ... };
```

And for flag_handler, the file_locks HashMap key changes from `PathBuf` to `String`:
```rust
// Before:
let file_lock = {
    let mut locks = state.file_locks.lock().await;
    locks.entry(full_path.clone()).or_insert_with(...)
};

// After:
let file_lock = {
    let mut locks = state.file_locks.lock().await;
    locks.entry(filepath.clone()).or_insert_with(...)
};
```

And for write:
```rust
// Before:
match std::fs::write(&full_path, &new_content) { ... }

// After:
match state.source.write_file(&filepath, &new_content).await { ... }
```

- [ ] **Step 5: Run all tests**

Run: `cargo test`
Expected: All existing tests pass.

- [ ] **Step 6: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: Clean.

- [ ] **Step 7: Commit**

```bash
git add src/server.rs
git commit -m "Refactor server handlers to use FileSource trait

Replace all direct std::fs calls with state.source methods.
Add --host field to ServerConfig/ServerBuilder.
No behavior change for existing local file serving."
```

---

## Task 5: Docker CLI Module

**Files:**
- Create: `src/docker.rs`
- Modify: `src/lib.rs`
- Create: `tests/docker_test.rs`

- [ ] **Step 1: Write tests for Docker CLI module**

```rust
// tests/docker_test.rs
use previewf::docker::{parse_container_list, validate_container_name};

#[test]
fn test_parse_container_list_single() {
    let output = "a1b2c3d4e5f6\tmy-app\tnode:20\tUp 2 hours\n";
    let containers = parse_container_list(output);
    assert_eq!(containers.len(), 1);
    assert_eq!(containers[0].name, "my-app");
    assert_eq!(containers[0].image, "node:20");
}

#[test]
fn test_parse_container_list_multiple() {
    let output = "a1b2c3\tapp\tnode:20\tUp 2h\nf6e5d4\tdb\tpostgres:16\tUp 1h\n";
    let containers = parse_container_list(output);
    assert_eq!(containers.len(), 2);
    assert_eq!(containers[0].name, "app");
    assert_eq!(containers[1].name, "db");
}

#[test]
fn test_parse_container_list_empty() {
    let containers = parse_container_list("");
    assert!(containers.is_empty());
}

#[test]
fn test_validate_container_name_valid() {
    assert!(validate_container_name("my-app").is_ok());
    assert!(validate_container_name("app_v2.1").is_ok());
    assert!(validate_container_name("a1b2c3d4").is_ok());
}

#[test]
fn test_validate_container_name_invalid() {
    assert!(validate_container_name("").is_err());
    assert!(validate_container_name("my;app").is_err());
    assert!(validate_container_name("$(whoami)").is_err());
    assert!(validate_container_name("app name").is_err());
}
```

- [ ] **Step 2: Run tests to see them fail**

Run: `cargo test --test docker_test`
Expected: FAIL — module doesn't exist.

- [ ] **Step 3: Implement docker.rs**

```rust
// src/docker.rs
use serde::Serialize;
use tokio::process::Command;

use crate::PreviewError;

/// Information about a running Docker container.
#[derive(Debug, Clone, Serialize)]
pub struct ContainerInfo {
    pub id: String,
    pub name: String,
    pub image: String,
    pub status: String,
}

/// Parse the tab-separated output of `docker ps --format`.
pub fn parse_container_list(output: &str) -> Vec<ContainerInfo> {
    output
        .lines()
        .filter(|line| !line.is_empty())
        .filter_map(|line| {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 4 {
                Some(ContainerInfo {
                    id: parts[0].to_string(),
                    name: parts[1].to_string(),
                    image: parts[2].to_string(),
                    status: parts[3].to_string(),
                })
            } else {
                None
            }
        })
        .collect()
}

/// Validate a container name or ID to prevent injection.
/// Docker names match `[a-zA-Z0-9][a-zA-Z0-9_.-]*`.
pub fn validate_container_name(name: &str) -> Result<(), PreviewError> {
    if name.is_empty() {
        return Err(PreviewError::ContainerNotFound(String::new()));
    }
    let valid = name.chars().enumerate().all(|(i, c)| {
        if i == 0 {
            c.is_ascii_alphanumeric()
        } else {
            c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-'
        }
    });
    if valid {
        Ok(())
    } else {
        Err(PreviewError::ContainerNotFound(format!(
            "Invalid container name: {name}"
        )))
    }
}

/// Check if Docker CLI is available.
pub async fn check_docker_available() -> Result<String, PreviewError> {
    let output = Command::new("docker")
        .args(["version", "--format", "{{.Client.Version}}"])
        .output()
        .await
        .map_err(|_| {
            PreviewError::DockerNotAvailable("Docker CLI not found. Is Docker installed?".into())
        })?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(PreviewError::DockerNotAvailable(
            "Docker CLI not responding".into(),
        ))
    }
}

/// List running Docker containers.
pub async fn list_containers() -> Result<Vec<ContainerInfo>, PreviewError> {
    let output = Command::new("docker")
        .args([
            "ps",
            "--format",
            "{{.ID}}\t{{.Names}}\t{{.Image}}\t{{.Status}}",
        ])
        .output()
        .await
        .map_err(|e| PreviewError::DockerExec(e.to_string()))?;

    if !output.status.success() {
        return Err(PreviewError::DockerExec(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(parse_container_list(&stdout))
}

/// Verify a container exists and is running.
pub async fn validate_container(name: &str) -> Result<(), PreviewError> {
    validate_container_name(name)?;

    let output = Command::new("docker")
        .args(["inspect", "--format", "{{.State.Running}}", name])
        .output()
        .await
        .map_err(|e| PreviewError::DockerExec(e.to_string()))?;

    if !output.status.success() {
        return Err(PreviewError::ContainerNotFound(name.to_string()));
    }

    let running = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if running == "true" {
        Ok(())
    } else {
        Err(PreviewError::ContainerNotRunning(name.to_string()))
    }
}

/// Check if a path exists inside a container.
pub async fn validate_container_path(container: &str, path: &str) -> Result<(), PreviewError> {
    let output = Command::new("docker")
        .args(["exec", container, "test", "-e", path])
        .output()
        .await
        .map_err(|e| PreviewError::DockerExec(e.to_string()))?;

    if output.status.success() {
        Ok(())
    } else {
        Err(PreviewError::ContainerPathNotFound {
            container: container.to_string(),
            path: path.to_string(),
        })
    }
}
```

- [ ] **Step 4: Add module to lib.rs**

```rust
pub mod docker;
pub mod error;
pub mod flags;
pub mod html;
pub mod markdown;
pub mod server;
pub mod source;
pub mod terminal;
pub mod watcher;

pub use error::PreviewError;
```

- [ ] **Step 5: Run tests**

Run: `cargo test --test docker_test`
Expected: All PASS (parse/validate tests don't need Docker installed).

- [ ] **Step 6: Commit**

```bash
git add src/docker.rs src/lib.rs tests/docker_test.rs
git commit -m "Add Docker CLI module with container discovery and validation"
```

---

## Task 6: Implement DockerSource

**Files:**
- Create: `src/source/docker.rs`
- Modify: `src/source/mod.rs`

- [ ] **Step 1: Add docker module to source/mod.rs**

```rust
pub mod docker;
pub mod local;
```

(Keep the rest of mod.rs unchanged.)

- [ ] **Step 2: Implement DockerSource**

```rust
// src/source/docker.rs
use async_trait::async_trait;
use tokio::process::Command;

use super::{DirEntry, EntryType, FileMeta, FileSource};
use crate::docker::validate_container_name;
use crate::PreviewError;

/// File source backed by a Docker container's filesystem.
///
/// All operations shell out via `docker exec` with separate arguments
/// (no shell interpolation) to prevent injection.
pub struct DockerSource {
    container: String,
    base_path: String,
}

impl DockerSource {
    pub fn new(container: String, base_path: String) -> Result<Self, PreviewError> {
        validate_container_name(&container)?;
        let base_path = normalize_container_path(&base_path);
        Ok(Self {
            container,
            base_path,
        })
    }

    pub fn container(&self) -> &str {
        &self.container
    }

    /// Resolve a relative path against the base, preventing traversal.
    fn resolve(&self, path: &str) -> Result<String, PreviewError> {
        let full = if path.is_empty() {
            self.base_path.clone()
        } else {
            let normalized = normalize_container_path(path);
            if self.base_path == "/" {
                normalized
            } else {
                format!("{}{}", self.base_path, normalized)
            }
        };

        // Reject any remaining ".." segments after normalization.
        if full.split('/').any(|seg| seg == "..") {
            return Err(PreviewError::ContainerPathNotFound {
                container: self.container.clone(),
                path: path.to_string(),
            });
        }

        // Verify the resolved path is under base_path.
        if !full.starts_with(&self.base_path) {
            return Err(PreviewError::ContainerPathNotFound {
                container: self.container.clone(),
                path: path.to_string(),
            });
        }

        Ok(full)
    }
}

/// Normalize a container path: ensure leading slash, collapse double slashes,
/// remove "." segments, reject ".." segments.
fn normalize_container_path(path: &str) -> String {
    let trimmed = path.trim_matches('/');
    if trimmed.is_empty() {
        return "/".to_string();
    }
    let cleaned: Vec<&str> = trimmed
        .split('/')
        .filter(|s| !s.is_empty() && *s != ".")
        .collect();
    if cleaned.iter().any(|s| *s == "..") {
        return "/".to_string();
    }
    format!("/{}", cleaned.join("/"))
}

#[async_trait]
impl FileSource for DockerSource {
    async fn read_file(&self, path: &str) -> Result<String, PreviewError> {
        let full = self.resolve(path)?;
        let output = Command::new("docker")
            .args(["exec", &self.container, "cat", &full])
            .output()
            .await
            .map_err(|e| PreviewError::DockerExec(e.to_string()))?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(PreviewError::ContainerPathNotFound {
                container: self.container.clone(),
                path: path.to_string(),
            })
        }
    }

    async fn list_dir(&self, path: &str) -> Result<Vec<DirEntry>, PreviewError> {
        let full = self.resolve(path)?;
        let output = Command::new("docker")
            .args(["exec", &self.container, "ls", "-1F", &full])
            .output()
            .await
            .map_err(|e| PreviewError::DockerExec(e.to_string()))?;

        if !output.status.success() {
            return Err(PreviewError::ContainerPathNotFound {
                container: self.container.clone(),
                path: path.to_string(),
            });
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut entries: Vec<DirEntry> = stdout
            .lines()
            .filter(|line| !line.is_empty())
            .filter_map(|line| {
                let name = line.trim();
                if name.starts_with('.') {
                    return None;
                }
                if let Some(dir_name) = name.strip_suffix('/') {
                    Some(DirEntry {
                        name: dir_name.to_string(),
                        entry_type: EntryType::Directory,
                    })
                } else {
                    // Strip other ls -F suffixes: * (executable), @ (symlink), etc.
                    let clean = name.trim_end_matches(|c| c == '*' || c == '@' || c == '|' || c == '=');
                    Some(DirEntry {
                        name: clean.to_string(),
                        entry_type: EntryType::File,
                    })
                }
            })
            .collect();

        entries.sort_by_key(|e| e.name.to_lowercase());
        Ok(entries)
    }

    async fn stat(&self, path: &str) -> Result<FileMeta, PreviewError> {
        let full = self.resolve(path)?;
        // stat -c works on Linux containers (most Docker containers are Linux).
        let output = Command::new("docker")
            .args(["exec", &self.container, "stat", "-c", "%Y %s %F", &full])
            .output()
            .await
            .map_err(|e| PreviewError::DockerExec(e.to_string()))?;

        if !output.status.success() {
            return Err(PreviewError::ContainerPathNotFound {
                container: self.container.clone(),
                path: path.to_string(),
            });
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let parts: Vec<&str> = stdout.trim().splitn(3, ' ').collect();
        if parts.len() < 3 {
            return Err(PreviewError::DockerExec(format!(
                "Unexpected stat output: {stdout}"
            )));
        }

        let mtime = parts[0].parse::<u64>().unwrap_or(0);
        let size = parts[1].parse::<u64>().unwrap_or(0);
        let is_dir = parts[2].contains("directory");

        Ok(FileMeta {
            mtime,
            size,
            is_dir,
        })
    }

    async fn write_file(&self, path: &str, content: &str) -> Result<(), PreviewError> {
        let full = self.resolve(path)?;
        let mut child = tokio::process::Command::new("docker")
            .args(["exec", "-i", &self.container, "sh", "-c", &format!("cat > '{full}'")])
            .stdin(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| PreviewError::DockerExec(e.to_string()))?;

        if let Some(mut stdin) = child.stdin.take() {
            use tokio::io::AsyncWriteExt;
            stdin
                .write_all(content.as_bytes())
                .await
                .map_err(|e| PreviewError::DockerExec(e.to_string()))?;
        }

        let status = child
            .wait()
            .await
            .map_err(|e| PreviewError::DockerExec(e.to_string()))?;

        if status.success() {
            Ok(())
        } else {
            Err(PreviewError::DockerExec(format!(
                "Failed to write {full} in container {}",
                self.container
            )))
        }
    }

    async fn is_file(&self, path: &str) -> bool {
        let Ok(full) = self.resolve(path) else {
            return false;
        };
        Command::new("docker")
            .args(["exec", &self.container, "test", "-f", &full])
            .output()
            .await
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    async fn is_dir(&self, path: &str) -> bool {
        let Ok(full) = self.resolve(path) else {
            return false;
        };
        Command::new("docker")
            .args(["exec", &self.container, "test", "-d", &full])
            .output()
            .await
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn display_root(&self) -> String {
        format!("{}:{}", self.container, self.base_path)
    }
}
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check`
Expected: Compiles.

- [ ] **Step 4: Commit**

```bash
git add src/source/docker.rs src/source/mod.rs
git commit -m "Implement DockerSource using docker exec for file operations"
```

---

## Task 7: DockerPollWatcher

**Files:**
- Create: `src/docker_watcher.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Implement DockerPollWatcher**

```rust
// src/docker_watcher.rs
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
        if !files.contains_key(&path) {
            let mtime = self
                .source
                .stat(&path)
                .await
                .map(|m| m.mtime)
                .unwrap_or(0);
            files.insert(path, mtime);
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
```

- [ ] **Step 2: Add module to lib.rs**

```rust
pub mod docker;
pub mod docker_watcher;
pub mod error;
pub mod flags;
pub mod html;
pub mod markdown;
pub mod server;
pub mod source;
pub mod terminal;
pub mod watcher;

pub use error::PreviewError;
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check`
Expected: Compiles.

- [ ] **Step 4: Commit**

```bash
git add src/docker_watcher.rs src/lib.rs
git commit -m "Add DockerPollWatcher for container file change detection"
```

---

## Task 8: Docker Routes in Server

**Files:**
- Modify: `src/server.rs`

- [ ] **Step 1: Add Docker routes, state fields, and handlers**

Add imports for docker modules. Extend `AppState` with `docker_sources` and `docker_reload_txs` fields. Add Docker route registrations to the router. Implement `docker_containers_handler`, `docker_index_handler`, `docker_browse_handler`, `docker_view_handler`, `docker_flags_handler`, `docker_flag_handler`, `docker_tree_handler`, `docker_ws_handler`.

Also add a `get_docker_source` helper that lazily creates and caches `DockerSource` instances per container, and a `build_docker_breadcrumbs` function that includes the whale emoji prefix.

Docker handlers follow the same pattern as local handlers but extract the container name from the URL path, look up or create a `DockerSource`, and delegate to the source for file operations. Docker routes use `/docker/{container}/` prefix for all paths.

The `create_docker_router` public function builds a router with the DockerSource as the primary source (used by `previewf docker serve` CLI).

- [ ] **Step 2: Run existing tests + clippy**

Run: `cargo clippy -- -D warnings && cargo test`
Expected: All existing tests pass.

- [ ] **Step 3: Commit**

```bash
git add src/server.rs
git commit -m "Add Docker routes with container browsing, preview, and flags

Routes namespaced under /docker/:container/ mirror existing local routes.
Per-container DockerSource caching and reload channels."
```

---

## Task 9: CLI — --host Flag and Docker Subcommand

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Update CLI with --host flag and docker subcommand**

Add `--host` flag to `Serve` variant. Add `Docker` variant with `DockerCommands` enum containing `Ls` and `Serve`. Add a `parse_duration` function for the `--poll-interval` flag.

The `Docker::Ls` handler calls `check_docker_available()` then `list_containers()` and prints a formatted table. The `Docker::Serve` handler validates the container and path, creates a `DockerSource`, starts a `DockerPollWatcher`, and runs a minimal server via `create_docker_router`.

- [ ] **Step 2: Run clippy and tests**

Run: `cargo clippy -- -D warnings && cargo test`
Expected: All pass.

- [ ] **Step 3: Commit**

```bash
git add src/main.rs
git commit -m "Add --host flag to serve and docker subcommand (ls, serve)"
```

---

## Task 10: UI — Docker Section on Index Page

**Files:**
- Modify: `assets/index.html`
- Modify: `assets/app.js`
- Modify: `assets/style.css`

- [ ] **Step 1: Add Docker section to index.html**

After the `<div class="listing-summary">{{summary}}</div>` closing div, add:

```html
                <div id="docker-section" class="docker-section" style="display:none">
                    <h2 class="docker-section-title">&#128051; Docker Containers</h2>
                    <div id="docker-list" class="docker-list"></div>
                    <button id="docker-refresh" class="docker-refresh">Refresh</button>
                </div>
```

- [ ] **Step 2: Add Docker container fetch logic to app.js (safe DOM only, NO innerHTML)**

At the end of `app.js`, add:

```javascript
// --- Docker container discovery ---
(function() {
    var section = document.getElementById('docker-section');
    var list = document.getElementById('docker-list');
    var refreshBtn = document.getElementById('docker-refresh');

    if (!section || !list) return;

    function clearChildren(el) {
        while (el.firstChild) el.removeChild(el.firstChild);
    }

    function renderContainers(containers) {
        clearChildren(list);
        if (containers.length === 0) {
            var empty = document.createElement('p');
            empty.className = 'docker-empty';
            empty.textContent = 'No running containers found.';
            list.appendChild(empty);
            return;
        }
        containers.forEach(function(c) {
            var a = document.createElement('a');
            a.className = 'file-entry docker-entry';
            a.href = '/docker/' + encodeURIComponent(c.name);

            var nameGroup = document.createElement('span');
            nameGroup.className = 'file-entry-name-group';

            var icon = document.createElement('span');
            icon.className = 'file-entry-icon docker-icon';
            icon.textContent = '\uD83D\uDC33'; // whale emoji

            var name = document.createElement('span');
            name.className = 'file-entry-name';
            name.textContent = c.name;

            nameGroup.appendChild(icon);
            nameGroup.appendChild(name);

            var badge = document.createElement('span');
            badge.className = 'file-entry-badge docker-badge';
            badge.textContent = c.image + ' \u00B7 ' + c.status;

            a.appendChild(nameGroup);
            a.appendChild(badge);
            list.appendChild(a);
        });
    }

    function fetchContainers() {
        fetch('/api/docker/containers')
            .then(function(r) {
                if (r.ok) return r.json();
                throw new Error('Docker not available');
            })
            .then(function(data) {
                section.style.display = '';
                renderContainers(data);
            })
            .catch(function() {
                section.style.display = 'none';
            });
    }

    if (refreshBtn) {
        refreshBtn.addEventListener('click', fetchContainers);
    }

    fetchContainers();
})();
```

- [ ] **Step 3: Add Docker section styles to style.css**

```css
/* Docker section */
.docker-section {
    margin-top: 2rem;
    padding-top: 1.5rem;
    border-top: 1px solid var(--border);
}
.docker-section-title {
    font-family: var(--font-heading);
    font-size: 1.25rem;
    font-weight: 700;
    margin-bottom: 1rem;
    color: var(--heading);
}
.docker-entry {
    border-left: 3px solid var(--accent) !important;
}
.docker-icon {
    font-size: 1.1em;
}
.docker-badge {
    font-size: 0.78em;
    color: var(--text-muted);
}
.docker-refresh {
    margin-top: 0.75rem;
    padding: 0.35rem 1rem;
    font-family: var(--font-sans);
    font-size: 0.85rem;
    background: var(--bg-alt);
    border: 1px solid var(--border);
    border-radius: 4px;
    cursor: pointer;
    color: var(--text);
}
.docker-refresh:hover {
    background: var(--border);
}
.docker-empty {
    color: var(--text-muted);
    font-style: italic;
    padding: 0.5rem 0;
}
```

- [ ] **Step 4: Run clippy and tests**

Run: `cargo clippy -- -D warnings && cargo test`
Expected: All pass.

- [ ] **Step 5: Commit**

```bash
git add assets/index.html assets/app.js assets/style.css
git commit -m "Add Docker containers section to index page UI"
```

---

## Task 11: Documentation

**Files:**
- Create: `book/src/usage/docker.md`
- Modify: `book/src/usage/serve.md`
- Modify: `book/src/concepts/design-decisions.md`
- Modify: `book/src/architecture/overview.md`
- Modify: `book/src/SUMMARY.md`
- Modify: `book/src/roadmap.md`
- Modify: `book/src/implementation/notes.md`

- [ ] **Step 1: Create book/src/usage/docker.md**

Write the Docker usage documentation covering:
- Overview of the two Docker modes (container-native, host-to-container)
- `previewf docker ls` command with example output
- `previewf docker serve` command with all options and examples
- Browser-based discovery via the index page
- Live reload behavior (polling watcher)
- Security (container name validation, path normalization)
- Troubleshooting (Docker not found, container not running, path not found)

Follow the existing usage/serve.md style: command syntax first, then features/routes, then how it works internally.

- [ ] **Step 2: Update book/src/usage/serve.md**

Add the `--host` flag to the command syntax section and options table. Add a "Container-Native Mode" section explaining:
```bash
previewf serve ./docs --host 0.0.0.0
```

- [ ] **Step 3: Update book/src/concepts/design-decisions.md**

Add three new decision sections:
- "Why a FileSource Trait" — abstraction choice, alternatives (temp-sync, FUSE)
- "Why Docker CLI over Docker Engine API" — simplicity, no extra crate
- "Why Polling over inotify for Docker" — host watchers can't see container changes

- [ ] **Step 4: Update book/src/architecture/overview.md**

Add new modules to Module Map and Module Responsibilities. Update the dependency graph. Add "Flow 4: Docker Container Preview" data flow diagram.

- [ ] **Step 5: Update book/src/SUMMARY.md**

Add Docker page under Usage:
```markdown
- [Docker Containers](usage/docker.md)
```

- [ ] **Step 6: Update book/src/roadmap.md**

Add "Docker Container File Serving" as a completed feature.

- [ ] **Step 7: Update book/src/implementation/notes.md**

Add a new PR entry documenting modules created, design decisions, test coverage, deviations from spec.

- [ ] **Step 8: Commit**

```bash
git add book/
git commit -m "Add Docker container file serving documentation

New usage/docker.md page, updated design decisions, architecture
overview, roadmap, and implementation notes."
```

---

## Task 12: Final Verification

- [ ] **Step 1: Run full test suite**

Run: `cargo test`
Expected: All tests pass.

- [ ] **Step 2: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: No warnings.

- [ ] **Step 3: Run format check**

Run: `cargo fmt --check`
Expected: No formatting issues.

- [ ] **Step 4: Manual smoke test (if Docker available)**

```bash
# Start a test container
docker run -d --name previewf-test -v $(pwd)/tests/fixtures:/docs alpine sleep 3600

# Test docker ls
cargo run -- docker ls

# Test docker serve
cargo run -- docker serve previewf-test /docs --port 4568

# Open http://localhost:4568 in browser, verify file listing and preview

# Cleanup
docker stop previewf-test && docker rm previewf-test
```

- [ ] **Step 5: Commit any final fixes**

If any issues found during verification, fix and commit.
