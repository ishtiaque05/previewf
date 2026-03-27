# Entry Points

This chapter traces how user commands enter the system and get dispatched to the appropriate subsystem. previewf has one binary entry point (`main`) that branches into three subcommands, each reaching a different set of modules.

## The CLI Definition

The CLI is defined in `src/main.rs` using clap's derive API:

```rust
#[derive(Parser)]
#[command(name = "previewf", version, about = "Preview and annotate markdown files")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Serve files on localhost for browser preview
    Serve {
        path: PathBuf,
        #[arg(short, long, default_value_t = 3000)]
        port: u16,
    },

    /// View a markdown file in the terminal
    View {
        path: PathBuf,
    },

    /// Extract flags from a markdown file
    Flags {
        path: PathBuf,
        #[arg(long)]
        json: bool,
    },
}
```

clap handles all argument parsing, validation, and help text generation. The `#[command]` and `#[arg]` attributes configure behavior declaratively. When parsing fails (wrong subcommand, missing argument, invalid type), clap prints an error message and exits before our code runs.

## The Dispatch

The `main` function parses the CLI arguments and dispatches to a handler based on the matched subcommand:

```rust
#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Serve { path, port } => {
            // ... server setup and run
        }
        Commands::View { path } => {
            // ... file read and terminal render
        }
        Commands::Flags { path, json } => {
            // ... file read and flag export
        }
    }
}
```

The `#[tokio::main]` attribute initializes the tokio async runtime before `main` runs. This is necessary for the `serve` command (which runs an async web server) but is technically unnecessary for `view` and `flags` (which are synchronous). We accept the minor overhead because having a single entry point is simpler than conditionally initializing the runtime.

## Entry Point 1: `serve`

The `serve` command has the most complex initialization path. Here is the complete trace:

```
User runs: previewf serve ./docs/ --port 8080
    |
    v
clap parses: Commands::Serve { path: "./docs/", port: 8080 }
    |
    v
main.rs: match Commands::Serve { path, port }
    |
    v
ServerBuilder::new()
    .path(&path)          // PathBuf: "./docs/"
    .port(port)           // u16: 8080
    .live_reload(true)    // bool: true (hardcoded default)
    .build()?             // -> Result<ServerConfig, PreviewError>
    |
    v
previewf::server::run(config).await?
    |
    v
server.rs: run(config)
    |
    +-- create_router(config)
    |       |
    |       +-- broadcast::channel(100)  -- create reload channel
    |       +-- AppState { config, reload_tx }
    |       +-- Router::new()
    |       |       .route("/", get(index_handler))
    |       |       .route("/view/{*filepath}", get(view_handler))
    |       |       .route("/raw/{*filepath}", get(raw_handler))
    |       |       .route("/flags/{*filepath}", get(flags_handler))
    |       |       .route("/flag/{*filepath}", post(flag_post_handler))
    |       |       .route("/ws", get(ws_handler))
    |       |       .route("/assets/{*filepath}", get(asset_handler))
    |       |       .with_state(state)
    |       |
    |       v
    |       axum::Router (fully configured)
    |
    +-- TcpListener::bind("0.0.0.0:8080").await
    |
    +-- println!("Serving ./docs/ on http://localhost:8080")
    |
    +-- axum::serve(listener, app).await
            |
            v
            Server is now running, handling requests
            (blocks until ctrl-c or error)
```

### What `build()` Validates

The `ServerBuilder::build()` method checks:

1. **Path is provided.** If `path` is `None` (which cannot happen from the CLI since it is a required argument, but can happen from programmatic use), it returns `PreviewError::FileNotFound`.

The path is not validated for existence at build time. This is deliberate -- the path is validated when requests come in, allowing the served directory to be created after the server starts.

### What `create_router` Sets Up

The router is the central nervous system of the server. Each route is a function that takes the shared state and request data as extractors and returns a response. The routes are:

| Route | Handler | Modules Used |
|-------|---------|-------------|
| `/` | `index_handler` | `flags.rs` (for flag counts) |
| `/view/{*filepath}` | `view_handler` | `markdown.rs` (for rendering) |
| `/raw/{*filepath}` | `raw_handler` | (none -- raw file serving) |
| `/flags/{*filepath}` | `flags_handler` | `flags.rs` (for extraction) |
| `/flag/{*filepath}` | `flag_post_handler` | `flags.rs` (for injection) |
| `/ws` | `ws_handler` | (broadcast channel) |
| `/assets/{*filepath}` | `asset_handler` | (rust-embed) |

The `{*filepath}` syntax in routes is axum's wildcard path parameter. It captures everything after the prefix, including slashes. This allows paths like `/view/subdir/file.md`.

## Entry Point 2: `view`

The `view` command is the simplest path through the system:

```
User runs: previewf view ./plan.md
    |
    v
clap parses: Commands::View { path: "./plan.md" }
    |
    v
main.rs: match Commands::View { path }
    |
    v
std::fs::read_to_string(&path)
    .with_context(|| format!("Cannot read file: {}", path.display()))?
    |
    v
previewf::terminal::render_terminal(&content)
    |
    +-- prepare_flags_for_terminal(&content)
    |       regex: <flag:N>Comment: text</flag> -> **[FLAG #N:** text**]**
    |
    +-- MadSkin::default().term_text(&prepared)
    |       termimad renders markdown to ANSI string
    |
    v
print!("{}", rendered)
    |
    v
Program exits with status 0
```

The entire `view` path is synchronous. It reads a file, transforms it, prints it, and exits. No server, no watcher, no async. The tokio runtime is initialized (because `main` is `#[tokio::main]`) but not used.

### Error Path

If the file does not exist:

```
std::fs::read_to_string("./missing.md")
    |
    v
Err(io::Error { kind: NotFound })
    |
    v
anyhow::Context wraps it: "Cannot read file: ./missing.md"
    |
    v
main returns Err, anyhow prints: "Error: Cannot read file: ./missing.md"
    |
    v
Program exits with status 1
```

## Entry Point 3: `flags`

The `flags` command has two output modes controlled by the `--json` flag:

```
User runs: previewf flags ./plan.md --json
    |
    v
clap parses: Commands::Flags { path: "./plan.md", json: true }
    |
    v
main.rs: match Commands::Flags { path, json }
    |
    v
std::fs::read_to_string(&path)
    .with_context(|| ...)?
    |
    v
previewf::flags::extract_flags(&content)  -> Vec<Flag>
    |
    v
FlagReport { file: "plan.md", flags }
    |
    +-- if json == true:
    |       serde_json::to_string_pretty(&report)?
    |       println!("{}", json_str)
    |
    +-- if json == false:
            previewf::flags::format_flags_text(&report)
            print!("{}", text)
    |
    v
Program exits with status 0
```

Like `view`, this is a synchronous path. File read, regex scan, format, print, exit.

## How Subcommands Reach Modules

A summary of which modules each subcommand touches:

```
                    main.rs
                      |
         +------------+------------+
         |            |            |
       serve         view        flags
         |            |            |
    server.rs    terminal.rs   flags.rs
    /    |   \                     |
   /     |    \                    |
flags  markdown watcher         error
  |      |       |
error  error   error
```

### Module usage per subcommand:

| Module | `serve` | `view` | `flags` |
|--------|---------|--------|---------|
| `server.rs` | Yes | No | No |
| `markdown.rs` | Yes (via server) | No | No |
| `flags.rs` | Yes (via server) | No | Yes |
| `terminal.rs` | No | Yes | No |
| `watcher.rs` | Yes (via server) | No | No |
| `error.rs` | Yes | Yes (via anyhow) | Yes (via anyhow) |

This table shows clear separation of concerns. The `view` command never touches the server or markdown modules. The `flags` command never touches the server, markdown, terminal, or watcher modules.

## HTTP Entry Points

Once the server is running, HTTP requests are additional entry points into the system. Each route handler is an entry point:

### `GET /` -- Index

```
Browser: GET http://localhost:3000/
    |
    v
axum router matches: "/"  -->  index_handler
    |
    v
index_handler(State(state))
    |
    +-- if state.config.path.is_file():
    |       Redirect::to("/view/<filename>")
    |
    +-- else (directory):
            read_dir -> for each .md: extract_flags for count
            load index.html template from Assets
            substitute {{directory}}, {{file_entries}}, {{summary}}
            return Html(page)
```

### `GET /view/:path` -- Document Viewer

```
Browser: GET http://localhost:3000/view/plan.md
    |
    v
axum router matches: "/view/{*filepath}"  -->  view_handler
    |
    v
view_handler(State(state), Path(filepath))
    |
    v
resolve_path(base, "plan.md")  -->  full filesystem path
    |
    v
std::fs::read_to_string(full_path)  -->  markdown content
    |
    v
render_html(&content)  -->  styled HTML
    |
    v
load document.html template from Assets
    |
    v
substitute {{title}}, {{filepath}}, {{content}}
    |
    v
return Html(page) with status 200
```

### `POST /flag/:path` -- Flag Creation

```
Browser: POST http://localhost:3000/flag/plan.md
         Body: { "comment": "needs work", "selected_text": "the timeline" }
    |
    v
axum router matches: "/flag/{*filepath}"  -->  flag_post_handler
    |
    v
flag_post_handler(State(state), Path(filepath), Json(payload))
    |
    v
resolve_path + read file  -->  content
    |
    v
Find line containing "the timeline"  -->  line number
    |
    v
inject_flag(content, line, "needs work")  -->  new content
    |
    v
std::fs::write(path, new_content)  -->  file updated
    |
    v
state.reload_tx.send(())  -->  broadcast to WebSocket clients
    |
    v
return StatusCode::OK
```

### `GET /ws` -- WebSocket

```
Browser: GET ws://localhost:3000/ws (upgrade request)
    |
    v
axum router matches: "/ws"  -->  ws_handler
    |
    v
ws_handler(State(state), WebSocketUpgrade)
    |
    v
state.reload_tx.subscribe()  -->  new broadcast receiver
    |
    v
ws.on_upgrade(|socket| handle_ws(socket, rx))
    |
    v
handle_ws: tokio::select! loop
    +-- rx.recv() --> send Message::Text("reload") to socket
    +-- socket.recv() --> handle close/ping/pong
    (runs until disconnect)
```

This entry point is unique because it upgrades from HTTP to a persistent WebSocket connection. The handler runs as a long-lived tokio task, not a request-response cycle.
