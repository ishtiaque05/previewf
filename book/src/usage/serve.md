# Serve Command

The `serve` command is the primary way to use previewf. It starts a local web server that renders markdown files with rich typography, syntax highlighting, and the flag annotation UI.

## Basic Usage

```bash
# Serve a directory
previewf serve ./docs/

# Serve a single file
previewf serve ./plan.md

# Custom port
previewf serve ./docs/ --port 8080
```

## Command Syntax

```
previewf serve <PATH> [OPTIONS]

Arguments:
  <PATH>    File or directory to serve

Options:
  -p, --port <PORT>    Port to listen on [default: 3000]
  -h, --help           Print help
```

## Directory Mode

When `PATH` is a directory, previewf scans for `.md`, `.markdown`, `.html`, and `.htm` files and presents a file listing at the root URL.

```bash
previewf serve ./docs/
```

Visiting `http://localhost:3000` shows:

```
+------------------------------------------+
|  previewf   ~/docs/            [theme]   |
+------------------------------------------+
|                                          |
|   * architecture.md        3 flags       |
|   * plan.md                1 flag        |
|   * readme.md              --            |
|   o preview.html           (html)        |
|   o report.html            (html)        |
|                                          |
|   3 markdown . 2 html                    |
+------------------------------------------+
```

- Markdown files are shown with a filled diamond (*) and link to `/view/<filename>`
- HTML files are shown with an open diamond (o) and link to `/raw/<filename>`
- Flag counts are displayed next to each markdown file (extracted at listing time)
- Files are sorted alphabetically within their type group

### How the Listing Is Built

1. The `index_handler` reads the directory with `std::fs::read_dir`
2. For each `.md` file, it reads the content and runs `extract_flags` to count flags
3. For each `.html` file, it just records the name
4. The `assets/index.html` template is loaded via `rust-embed` and filled with the entries
5. The response is returned as `Html`

## Single File Mode

When `PATH` is a file, the root URL redirects to the file viewer.

```bash
previewf serve ./plan.md
```

Visiting `http://localhost:3000` redirects to `http://localhost:3000/view/plan.md`, which shows the document with the full reading experience: typography, syntax highlighting, flag UI, and sidebar.

### How the Redirect Works

The `index_handler` checks `base_path.is_file()`. If true, it extracts the filename and returns an axum `Redirect::to("/view/<filename>")`.

## Routes

The server exposes these routes:

| Route | Method | Purpose |
|-------|--------|---------|
| `/` | GET | Directory listing or redirect (single file mode) |
| `/view/:path` | GET | Render a markdown file with the document viewer |
| `/raw/:path` | GET | Serve an HTML file as-is (preview only) |
| `/flags/:path` | GET | Return all flags in a file as JSON |
| `/flag/:path` | POST | Create a new flag in a file |
| `/ws` | GET | WebSocket endpoint for live reload |
| `/assets/:path` | GET | Serve embedded static assets (CSS, JS) |

### `/view/:path` -- Markdown Viewer

This is the primary route. It:

1. Resolves the file path relative to the served directory
2. Reads the file content from disk
3. Passes the content through `render_html` (comrak + syntect + flag post-processing)
4. Loads the `document.html` template from embedded assets
5. Replaces `{{title}}`, `{{filepath}}`, and `{{content}}` placeholders
6. Returns the complete HTML page

The rendered page includes:

- The document content in the main column (max-width 72ch)
- A flag sidebar populated by JavaScript from the `.flag` spans in the content
- A status bar showing version, watch status, and WebSocket connection state
- Theme toggle button
- Text selection handler for creating new flags

### `/raw/:path` -- HTML Preview

For HTML files, the content is served directly without any processing. This is a simple file-to-response pipeline: read the file, return it as `Html`.

HTML files cannot be flagged (this is a non-goal for v0.1). They are included in the file listing for convenience -- if you have a mix of markdown and HTML in a directory, you can preview both from the same interface.

### `/flags/:path` -- Flag Export API

Returns all flags in a file as JSON:

```bash
curl http://localhost:3000/flags/plan.md
```

```json
{
  "file": "plan.md",
  "flags": [
    {
      "id": 1,
      "line": 11,
      "text": "The timeline assumes a single developer.",
      "comment": "Is this still accurate?"
    }
  ]
}
```

This endpoint exists for two reasons:

1. The JavaScript sidebar uses it to populate flag data on page load (redundant with the inline spans, but provides structured data for future UI features)
2. External tools can query it programmatically while the server is running

### `/flag/:path` -- Flag Creation

Accepts a POST request with JSON body:

```json
{
  "comment": "needs work",
  "selected_text": "The timeline assumes a single developer."
}
```

The handler:

1. Reads the file from disk
2. Finds the line containing `selected_text` (first match)
3. Calls `inject_flag(content, line, comment)` to produce new content with the flag tag
4. Writes the new content back to disk
5. Sends a broadcast notification to trigger live reload
6. Returns 200 OK

Error responses:

- 404: File not found
- 400: Selected text not found in file, or flag injection error

### `/ws` -- WebSocket

The WebSocket endpoint provides live reload. When the server modifies a file (flag creation) or the file watcher detects an external change, it broadcasts a `"reload"` message to all connected WebSocket clients. The JavaScript client receives the message and calls `location.reload()`.

### `/assets/:path` -- Static Assets

CSS, JavaScript, and HTML templates are embedded in the binary via `rust-embed`. The `#[derive(Embed)]` macro on the `Assets` struct includes everything in the `assets/` directory at compile time.

The `asset_handler` serves these files with appropriate MIME types:

- `.css` -> `text/css`
- `.js` -> `application/javascript`
- `.html` -> `text/html`

## Server Configuration

The server is configured via the builder pattern:

```rust
let config = ServerBuilder::new()
    .path("./docs/")
    .port(3000)
    .live_reload(true)
    .build()?;

previewf::server::run(config).await?;
```

### ServerConfig Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `path` | `PathBuf` | (required) | File or directory to serve |
| `port` | `u16` | `3000` | Port to listen on |
| `live_reload` | `bool` | `true` | Enable WebSocket live reload |

## Port Selection

The default port is 3000. If port 3000 is in use, specify a different port:

```bash
previewf serve ./docs/ --port 8080
```

The server binds to `0.0.0.0:<port>`, which means it is accessible from other devices on the same network. For a personal tool, this is convenient (you can preview on your phone) but be aware of the security implications if you are on an untrusted network.

## Live Reload Behavior

When live reload is enabled (the default), the server:

1. Starts a file watcher on the served path (file or directory)
2. On any file change (modify or create), broadcasts to all WebSocket clients
3. The browser receives the broadcast and reloads the page

The file watcher uses the `notify` crate with platform-native backends (FSEvents on macOS, inotify on Linux). Changes are typically detected within 100ms.

For more details on the live reload mechanism, see the [Live Reload scenario](../scenarios/live-reload.md).
