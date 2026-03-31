# Architecture Overview

previewf is structured as a single Rust crate with six modules, each responsible for a distinct subsystem. This chapter maps the modules, their responsibilities, and the data that flows between them.

## Module Map

```
src/
  main.rs            CLI entry point (clap parsing + subcommand dispatch)
  lib.rs             Public API re-exports
  error.rs           PreviewError enum (thiserror)
  flags.rs           Flag model, parsing, injection, extraction, formatting
  markdown.rs        Markdown-to-HTML (comrak + syntect + flag post-processing)
  terminal.rs        Markdown-to-terminal (termimad + flag formatting)
  server.rs          HTTP server (axum), routes, WebSocket, ServerBuilder
  watcher.rs         File watching (notify) with broadcast channel
  source/mod.rs      FileSource trait, DirEntry, FileMeta types
  source/local.rs    LocalSource (wraps std::fs)
  source/docker.rs   DockerSource (wraps docker exec)
  docker.rs          Container discovery, validation, CLI wrappers
  docker_watcher.rs  DockerPollWatcher (polling-based change detection)
```

## Module Responsibilities

### `main.rs` -- The Entry Point

Defines the CLI with clap's derive API. Parses arguments into a `Commands` enum, then dispatches to the appropriate subsystem. This file contains no business logic -- it is pure wiring.

**Depends on:** `lib.rs` (and transitively, all modules)

### `lib.rs` -- Public API

Declares all modules and re-exports key types:

```rust
pub mod error;
pub mod flags;
pub mod markdown;
pub mod server;
pub mod terminal;
pub mod watcher;

pub use error::PreviewError;
```

This file exists so that integration tests and `main.rs` can use `previewf::flags::extract_flags` syntax.

### `error.rs` -- Error Types

Defines `PreviewError`, the project's error enum:

```rust
#[derive(Debug, thiserror::Error)]
pub enum PreviewError {
    FileNotFound(PathBuf),
    NotMarkdown(PathBuf),
    FlagParse { line: usize, detail: String },
    Server(#[from] std::io::Error),
    Watcher(#[from] notify::Error),
}
```

**Depended on by:** all modules (they return `Result<_, PreviewError>`)

### `flags.rs` -- The Flag System

The core data model. Contains:

- `Flag` struct (id, line, text, comment)
- `FlagReport` struct (file, flags)
- `extract_flags(content) -> Vec<Flag>` -- regex-based parsing
- `inject_flag(content, line, comment) -> Result<String>` -- flag insertion
- `next_flag_id(content) -> u32` -- ID management
- `format_flags_text(report) -> String` -- human-readable output

**Depends on:** `error.rs`
**Depended on by:** `server.rs`, `markdown.rs`, `main.rs`

### `markdown.rs` -- Rendering Pipeline

Converts markdown content to styled HTML:

- `render_html(content) -> String` -- the public entry point
- `highlight_code_blocks(html) -> String` -- syntect post-processing
- `render_diff_block(code) -> String` -- git-style diff coloring
- `render_flag_spans(html) -> String` -- flag tag to styled span conversion

**Depends on:** (no internal dependencies, uses comrak and syntect directly)
**Depended on by:** `server.rs`

### `terminal.rs` -- Terminal Rendering

Converts markdown to ANSI-formatted terminal output:

- `render_terminal(content) -> String` -- the public entry point
- `prepare_flags_for_terminal(content) -> String` -- flag tag conversion

**Depends on:** (no internal dependencies, uses termimad directly)
**Depended on by:** `main.rs`

### `server.rs` -- Web Server

The HTTP layer. Contains:

- `ServerBuilder` -- builder pattern for configuration
- `ServerConfig` -- the built configuration
- `create_router(config) -> Router` -- creates the axum router (exposed for testing)
- `run(config) -> Result<()>` -- starts the server
- Route handlers: `index_handler`, `view_handler`, `raw_handler`, `flags_handler`, `flag_post_handler`, `ws_handler`, `asset_handler`
- `AppState` -- shared state (config + broadcast channel)

**Depends on:** `flags.rs`, `markdown.rs`, `error.rs`
**Depended on by:** `main.rs`

### `watcher.rs` -- File Watching

Monitors files/directories for changes and broadcasts notifications:

- `FileWatcher` struct (path, notify watcher, broadcast sender)
- `FileWatcher::new(path) -> (FileWatcher, Receiver)` -- create watcher + receiver pair
- `FileWatcher::watch() -> Result<()>` -- start watching
- `FileWatcher::subscribe() -> Receiver` -- get a new receiver

**Depends on:** `error.rs`
**Depended on by:** `server.rs` (for live reload integration)

### `source/mod.rs` -- FileSource Abstraction

Defines the `FileSource` async trait and shared types:

- `FileSource` trait -- `read_file`, `list_dir`, `stat`, `is_file`, `is_dir`
- `DirEntry` struct -- name, path, is_dir, size
- `FileMeta` struct -- path, modified time, size

**Depends on:** `error.rs`
**Depended on by:** `source/local.rs`, `source/docker.rs`, `server.rs`

### `source/local.rs` -- Local Filesystem Source

`LocalSource` implements `FileSource` over `std::fs`. A thin wrapper that translates `std::io::Error` into `PreviewError`.

**Depends on:** `source/mod.rs`, `error.rs`
**Depended on by:** `server.rs` (used as the default source for local serves)

### `source/docker.rs` -- Docker Container Source

`DockerSource` implements `FileSource` by shelling out to `docker exec`:

- `read_file` -- runs `docker exec <container> cat <path>`
- `list_dir` -- runs `docker exec <container> ls -la <path>`
- `stat` -- runs `docker exec <container> stat <path>`

**Depends on:** `source/mod.rs`, `docker.rs`, `error.rs`
**Depended on by:** `server.rs` (created on demand per container, cached in `AppState`)

### `docker.rs` -- Container Discovery and CLI Wrappers

Contains the Docker CLI interface for the host side:

- `list_containers() -> Result<Vec<ContainerInfo>>` -- runs `docker ps --format json`
- `validate_container_name(name)` -- regex check against `[a-zA-Z0-9_.-]+`
- `ContainerInfo` struct -- name, image, status

**Depends on:** `error.rs`
**Depended on by:** `source/docker.rs`, `server.rs`, `main.rs`

### `docker_watcher.rs` -- Docker Poll Watcher

`DockerPollWatcher` polls container file modification times on a configurable interval:

- `DockerPollWatcher::new(container, path, interval) -> (Watcher, Receiver)`
- `DockerPollWatcher::start() -> Result<()>` -- spawns the polling task
- Suspends automatically when no WebSocket clients hold a receiver

**Depends on:** `docker.rs`, `error.rs`
**Depended on by:** `server.rs` (created per container, one per active view session)

## Dependency Graph

```
                         main.rs
                    /    |    |    \
                   /     |    |     \
                  v      v    v      v
          server.rs  terminal.rs  flags.rs  docker.rs
          / | \ \                      |        |
         v  v  v  v                   v        v
 markdown.rs  watcher.rs  source/   error.rs  error.rs
              docker_watcher.rs
                  |
                  v
             source/docker.rs --> docker.rs
             source/local.rs  --> error.rs
```

Key observations:

- `error.rs` is at the bottom of the dependency graph -- everything depends on it
- `main.rs` is at the top -- it depends on everything but nothing depends on it
- `server.rs` is the most connected module -- it now also depends on `source/`, `docker.rs`, and `docker_watcher.rs`
- `source/mod.rs` defines the abstraction boundary; `source/local.rs` and `source/docker.rs` are the only modules that know about actual I/O backends
- `docker.rs` is shared between `source/docker.rs` (file reads) and `server.rs` (container discovery for the index page)

## Data Flow Overview

Three primary data flows exist in the system:

### Flow 1: Browser Preview

```
File on disk
    |
    v
std::fs::read_to_string  -->  raw markdown string
    |
    v
comrak::markdown_to_html  -->  raw HTML (with flag tags passed through)
    |
    v
highlight_code_blocks  -->  HTML with syntect-highlighted code
    |
    v
render_flag_spans  -->  HTML with styled flag <span> elements
    |
    v
Template substitution  -->  complete HTML page
    |
    v
axum response  -->  browser renders the page
```

### Flow 2: Flag Creation

```
Browser selection + comment
    |
    v
POST /flag/:path { comment, selected_text }
    |
    v
Read file from disk
    |
    v
Find line containing selected_text
    |
    v
inject_flag(content, line, comment)  -->  new content with <flag:N> tag
    |
    v
Write new content to disk
    |
    v
Broadcast reload notification
    |
    v
WebSocket  -->  browser reloads  -->  Flow 1 reruns
```

### Flow 3: Flag Export

```
File on disk
    |
    v
std::fs::read_to_string  -->  raw markdown string
    |
    v
extract_flags(content)  -->  Vec<Flag>
    |
    v
FlagReport { file, flags }
    |
    v
serde_json::to_string_pretty  -->  JSON string  -->  stdout
```

### Flow 4: Docker Container Preview

```
Browser GET /docker/my-app/view/docs/README.md
    |
    v
axum router extracts container="my-app", filepath="docs/README.md"
    |
    v
validate_container_name("my-app")  -->  ok
    |
    v
normalize_path("docs/README.md")   -->  check for .. traversal
    |
    v
get_docker_source("my-app")  -->  look up or create DockerSource in AppState
    |
    v
DockerSource::read_file("/docs/README.md")
    |
    v
docker exec my-app cat /docs/README.md  -->  raw markdown bytes
    |
    v
render_html(content)  -->  HTML with highlights and flag spans
    |
    v
document.html template  -->  substitute {{title}}, {{filepath}}, {{content}}
    |
    v
axum response  -->  browser renders the page
```

Live reload for this flow uses `DockerPollWatcher`. The watcher runs a background task that periodically calls `docker exec my-app stat /docs/README.md` and broadcasts a reload notification when the modification time changes.

## The Assets System

Static assets (HTML templates, CSS, JavaScript) are embedded in the binary at compile time using `rust-embed`:

```rust
#[derive(Embed)]
#[folder = "assets/"]
struct Assets;
```

At runtime, `Assets::get("style.css")` returns the file contents as a byte slice. This means:

- No external files to distribute alongside the binary
- No file-not-found errors for assets
- Templates and styles are version-locked to the binary

The assets are:

| File | Role |
|------|------|
| `document.html` | Template for the markdown document viewer |
| `index.html` | Template for the directory listing |
| `style.css` | All CSS (typography, themes, layout, animations) |
| `app.js` | Client-side JavaScript (theme toggle, flag UI, WebSocket) |

## Module Growth Strategy

The project follows a deliberate growth strategy:

**Phase 1 (current): Single files.** Each module is one `.rs` file. This works well when modules are under ~300 lines.

**Phase 2: Directory modules.** When a module exceeds ~300 lines, split it:

```
src/flags.rs  -->  src/flags/mod.rs
                   src/flags/parse.rs
                   src/flags/inject.rs
                   src/flags/export.rs
```

The `mod.rs` file re-exports everything, so external callers (`use previewf::flags::extract_flags`) do not need to change.

**Phase 3: Workspace.** When the project outgrows a single crate, extract into a Cargo workspace:

```
crates/previewf-core/      (flags, markdown, error)
crates/previewf-server/    (axum, websocket, watcher)
crates/previewf-cli/       (clap, terminal, main)
```

The trigger for each transition is developer pain, not arbitrary line counts. The 300-line threshold is a guideline, not a rule.

## Concurrency Model

previewf uses tokio as its async runtime. The concurrency model is straightforward:

- **The server** runs on tokio's multi-threaded runtime (`#[tokio::main]` with default settings)
- **Route handlers** are async functions that run on the tokio task pool
- **File I/O** uses synchronous `std::fs` operations (not tokio::fs), which is acceptable because file operations are fast for the file sizes involved and the tool is single-user
- **WebSocket connections** are managed as tokio tasks, one per client
- **The broadcast channel** (`tokio::sync::broadcast`) connects the file watcher and flag creation handler to WebSocket tasks
- **The file watcher** (notify) runs on its own OS thread (not a tokio task) and sends events through the broadcast channel

This model is simple and sufficient for a personal tool. There are no complex concurrency patterns, no shared mutable state (beyond the broadcast channel), and no lock contention.
