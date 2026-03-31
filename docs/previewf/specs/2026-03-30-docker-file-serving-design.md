# Docker Container File Serving

**Date:** 2026-03-30
**Status:** Draft

## Overview

Add the ability to serve and preview files from inside Docker containers. Two modes:

1. **Container-native** — previewf installed inside a container, serves via port mapping to host browser
2. **Host-to-container** — previewf runs on the host, reads files from running containers via Docker CLI

## CLI Interface

### Container-native mode

Add `--host` to the existing `serve` command:

```
previewf serve ./docs --host 0.0.0.0 --port 4567
```

Default remains `127.0.0.1`. Users running inside a container with `-p 4567:4567` add `--host 0.0.0.0`.

### Host-to-container mode

New `docker` subcommand with nested commands:

```
previewf docker ls                                              # list running containers
previewf docker serve <container> [path] [--port] [--poll-interval]  # serve files from container
```

- `<container>` — name or ID (partial IDs accepted, matching Docker CLI behavior)
- `[path]` — path inside container, defaults to `/`
- `--port` — defaults to `4567`
- `--poll-interval` — defaults to `2s`, controls file change detection frequency

Example:

```
$ previewf docker ls
CONTAINER ID   NAME          IMAGE           STATUS
a1b2c3d4e5f6   my-app        node:20         Up 2 hours
f6e5d4c3b2a1   docs-builder  python:3.12     Up 30 minutes

$ previewf docker serve my-app /app/docs
previewf serving my-app:/app/docs on http://localhost:4567
```

## Core Abstraction: FileSource Trait

An async trait decoupling the server from the filesystem:

```rust
#[async_trait]
pub trait FileSource: Send + Sync {
    async fn read_file(&self, path: &str) -> Result<String, PreviewError>;
    async fn list_dir(&self, path: &str) -> Result<Vec<DirEntry>, PreviewError>;
    async fn stat(&self, path: &str) -> Result<FileMeta, PreviewError>;
    async fn write_file(&self, path: &str, content: &str) -> Result<(), PreviewError>;
    fn display_root(&self) -> String;
}

pub struct DirEntry {
    pub name: String,
    pub entry_type: EntryType, // File, Directory
}

pub struct FileMeta {
    pub mtime: u64,
    pub size: u64,
    pub is_dir: bool,
}
```

### LocalSource

Wraps `std::fs` calls. Extracted from current `server.rs` logic. Uses `tokio::task::spawn_blocking` to avoid blocking the async runtime. Path traversal prevention via canonicalize + starts_with check.

### DockerSource

Holds container name/ID. Each method shells out via `tokio::process::Command`:

- `read_file` → `docker exec <ctr> cat <path>`
- `list_dir` → `docker exec <ctr> ls -1F <path>` (F appends `/` to dirs)
- `stat` → `docker exec <ctr> stat -c '%Y %s %F' <path>`
- `write_file` → `docker exec -i <ctr> sh -c 'cat > <path>'` with content on stdin

All calls use `Command` with separate args — no shell interpolation, no injection vector.

## Multi-Source Server Architecture

The server supports local files and any number of containers simultaneously.

### AppState

```rust
struct AppState {
    config: ServerConfig,
    local_source: Arc<LocalSource>,
    docker_sources: Arc<Mutex<HashMap<String, Arc<DockerSource>>>>,
    local_reload_tx: broadcast::Sender<()>,
    docker_reload_txs: Arc<Mutex<HashMap<String, broadcast::Sender<()>>>>,
    file_locks: Arc<Mutex<HashMap<String, Arc<Mutex<()>>>>>,
    docker_available: bool,
}
```

`docker_sources` is a lazy cache — a `DockerSource` is created on first access to a container and reused for subsequent requests. If a container stops, the next request returns an error and evicts it from the cache.

### Docker Availability

At startup, `previewf serve` runs `docker version` once. If it succeeds, `docker_available = true` and the index page shows the containers section. If Docker isn't installed, the section is hidden and `/api/docker/*` routes return 503.

### Route Structure

Existing local routes are unchanged:

```
GET /                          → index (local files + docker section)
GET /browse/*path              → local directory
GET /view/*path                → local file preview
GET /raw/*path                 → local raw HTML
GET /flags/*path               → local flags JSON
POST /flag/*path               → local flag injection
GET /api/tree                  → local tree JSON
GET /ws                        → local reload WebSocket
```

New Docker routes namespaced under `/docker`:

```
GET /api/docker/containers                → list running containers as JSON
GET /docker/:container/                   → container root listing
GET /docker/:container/browse/*path       → browse container subdirectory
GET /docker/:container/view/*path         → preview container file
GET /docker/:container/flags/*path        → flags from container file
POST /docker/:container/flag/*path        → inject flag into container file
GET /docker/:container/api/tree           → container tree JSON
GET /docker/:container/ws                 → container reload WebSocket
```

### Handler Reuse

View/browse/flags logic is identical regardless of source. Shared handler functions take `&dyn FileSource`:

```rust
async fn render_listing(source: &dyn FileSource, path: &str, ...) -> Response { ... }
async fn render_preview(source: &dyn FileSource, path: &str, ...) -> Response { ... }

// Local handlers
async fn view_handler(State(state): State<AppState>, ...) -> Response {
    render_preview(&*state.local_source, &filepath, ...).await
}

// Docker handlers
async fn docker_view_handler(State(state): State<AppState>, ...) -> Response {
    let source = get_or_create_docker_source(&state, &container).await?;
    render_preview(&*source, &filepath, ...).await
}
```

## Docker Discovery & Connection

### Container Discovery

`list_containers()` shells out to `docker ps --format '{{.ID}}\t{{.Names}}\t{{.Image}}\t{{.Status}}'` and parses tab-separated output.

```rust
pub struct ContainerInfo {
    pub id: String,
    pub name: String,
    pub image: String,
    pub status: String,
}
```

### Container Validation

Before starting the server in `previewf docker serve` mode, three checks run:

1. `docker version --format '{{.Client.Version}}'` — Docker CLI available
2. `docker inspect --format '{{.State.Running}}' <container>` — container exists and is running
3. `docker exec <ctr> test -e <path>` — path exists inside container

All three fail fast with actionable error messages.

### Browser-Based Discovery

The index page at `/` includes a Docker containers section (when Docker is available) populated via `/api/docker/containers`. Users click a container to browse its filesystem — no CLI interaction needed.

## Polling Watcher for Docker

### DockerPollWatcher

```rust
pub struct DockerPollWatcher {
    container: String,
    interval: Duration,
    tracked_files: Arc<Mutex<HashMap<String, u64>>>,
    cancel: CancellationToken,
}
```

### What Gets Polled

Only files the user is actively viewing via WebSocket connections. Paths are registered when a browser client connects and unregistered when the WebSocket closes.

### Poll Cycle

Every `poll_interval` (default 2s):

1. Batch stat all tracked files: `docker exec <ctr> stat -c '%Y' file1 file2 file3`
2. Compare mtimes against stored values
3. If any changed, update stored mtime, send on `reload_tx`
4. If `docker exec` fails (container stopped), emit "container_stopped" event and stop polling

Batching into a single `docker exec` call keeps overhead to one process spawn per cycle regardless of file count.

### Lifecycle

- Created lazily when the first file in a container is viewed via WebSocket
- Shut down via `CancellationToken` when the last WebSocket for that container closes
- Container stop detected on next poll — sends "container_stopped" to connected clients, cleans up

### Per-Container Reload Channels

Each container gets its own `broadcast::Sender<()>` so changes in container A don't trigger reloads in tabs viewing container B.

## UI Changes

### Index Page — Docker Section

Conditional section below the local file listing:

```
┌──────────────────────────────────────────────────┐
│  📁 Local Files                                   │
│  ├── docs/                                        │
│  ├── README.md                                    │
│  └── spec.md                                      │
│                                                    │
│  🐳 Docker Containers                             │
│  ├── my-app (node:20) — Up 2h                    │
│  ├── docs-builder (python:3.12) — Up 30m         │
│  └── redis (redis:7) — Up 2h                     │
│                                                    │
│  [Refresh]                                         │
└──────────────────────────────────────────────────┘
```

### Breadcrumbs in Docker Mode

```
root / 🐳 my-app / app / docs / README.md
```

Container name segment links to `/docker/my-app/`. The whale prefix distinguishes container browsing from local browsing.

### Container Stopped Banner

If a container stops while viewing, the WebSocket receives "container_stopped" and the UI shows:

```
⚠ Container "my-app" is no longer running. File content may be stale.
```

### Template Reuse

Docker file browser reuses the same templates and CSS as local file browsing. Differences are limited to breadcrumbs (container name) and a small Docker mode badge.

## Module Structure

```
src/
├── source/
│   ├── mod.rs             # FileSource trait, DirEntry, FileMeta, EntryType
│   ├── local.rs           # LocalSource (extracted from server.rs)
│   └── docker.rs          # DockerSource
├── docker.rs              # Container discovery, validation, CLI wrappers
├── docker_watcher.rs      # DockerPollWatcher
├── server.rs              # Refactored handlers using &dyn FileSource
├── watcher.rs             # Existing FileWatcher (unchanged)
├── flags.rs               # Unchanged
├── markdown.rs            # Unchanged
├── terminal.rs            # Unchanged
├── error.rs               # New Docker error variants
├── html.rs                # Unchanged
└── lib.rs                 # New module declarations
```

## Error Handling

New `PreviewError` variants:

```rust
#[error("Docker not available: {0}")]
DockerNotAvailable(String),

#[error("Container not found: {0}")]
ContainerNotFound(String),

#[error("Container not running: {0}")]
ContainerNotRunning(String),

#[error("Docker command failed: {0}")]
DockerExec(String),

#[error("Path not found in container: {container}:{path}")]
ContainerPathNotFound { container: String, path: String },
```

## Security

- **Path traversal in Docker mode:** Normalize paths (resolve `..`, strip leading `/`, reject `..` segments after normalization) before passing to `docker exec`. No `std::fs::canonicalize` available for container paths.
- **Container name injection:** Validate names match `[a-zA-Z0-9][a-zA-Z0-9_.-]*` before use in any command.
- **No shell interpolation:** All `docker exec` calls use `tokio::process::Command` with separate args.

## Dependencies

New crate additions:

- `async-trait` — for async `FileSource` trait (or use RPITIT with Rust 1.75+ MSRV)
- `tokio-util` — for `CancellationToken` in `DockerPollWatcher`

No Docker SDK crate. Docker interaction is exclusively via CLI.

## Scope

### In scope
- `--host` flag on `serve` for container-native mode
- `previewf docker ls` and `previewf docker serve` commands
- `FileSource` trait with `LocalSource` and `DockerSource`
- Multi-source server (local + containers in one instance)
- Browser-based Docker discovery on index page
- `DockerPollWatcher` with configurable interval
- Per-container WebSocket reload channels
- Docker route namespace (`/docker/:container/...`)
- Security: path normalization, name validation, no shell interpolation

### Out of scope
- Interactive CLI discovery (prompt-based container selection)
- Podman / other container runtimes
- Docker Engine API (socket-based)
- FUSE mounting
- Container log viewing
- Container exec / terminal
