# Scenario: Serving a Directory

This walkthrough traces what happens, step by step, when you run `previewf serve ./docs/` on a directory containing markdown and HTML files.

## Setup

Suppose you have this directory:

```
docs/
  architecture.md     (has 3 flags)
  plan.md             (has 1 flag)
  readme.md           (no flags)
  preview.html
  report.html
```

## Step 1: Command Execution

```bash
previewf serve ./docs/
```

### What clap does

clap parses the arguments into:

```rust
Commands::Serve {
    path: PathBuf::from("./docs/"),
    port: 3000,  // default
}
```

### What main.rs does

```rust
let config = ServerBuilder::new()
    .path(&path)           // "./docs/"
    .port(port)            // 3000
    .live_reload(true)     // hardcoded
    .build()?;

previewf::server::run(config).await?;
```

## Step 2: Server Startup

Inside `server::run`:

1. **Router creation.** `create_router(config)` builds the axum router with all routes and shared state. The `AppState` holds the config (including the directory path) and a `broadcast::Sender` for live reload.

2. **TCP binding.** The server binds to `0.0.0.0:3000`. If the port is in use, the program exits with an error.

3. **Console output.** The user sees:

   ```
   Serving ./docs/ on http://localhost:3000
   ```

4. **Server loop.** `axum::serve(listener, app).await` starts the event loop. The server is now ready to handle requests.

## Step 3: Browser Opens the Root URL

The user navigates to `http://localhost:3000/`.

### Request routing

axum matches `GET /` to `index_handler`.

### Directory detection

```rust
if base_path.is_file() {
    // This is false -- base_path is "./docs/", a directory
}
```

Since the path is a directory, the handler proceeds to build the file listing.

### File scanning

```rust
let mut md_files = Vec::new();
let mut html_files = Vec::new();

if let Ok(entries) = std::fs::read_dir(base_path) {
    for entry in entries.flatten() {
        let path = entry.path();
        if let Some(ext) = path.extension() {
            match ext.to_string_lossy().as_ref() {
                "md" | "markdown" => {
                    let content = std::fs::read_to_string(&path).unwrap_or_default();
                    let flag_count = extract_flags(&content).len();
                    md_files.push((name, flag_count));
                }
                "html" | "htm" => {
                    html_files.push(name);
                }
                _ => {}
            }
        }
    }
}
```

For each entry in `./docs/`:

| File | Extension | Action |
|------|-----------|--------|
| `architecture.md` | md | Read content, count flags (3) |
| `plan.md` | md | Read content, count flags (1) |
| `readme.md` | md | Read content, count flags (0) |
| `preview.html` | html | Record name |
| `report.html` | html | Record name |

### Sorting

```rust
md_files.sort_by(|a, b| a.0.cmp(&b.0));
html_files.sort();
```

After sorting: `architecture.md`, `plan.md`, `readme.md`, then `preview.html`, `report.html`.

### HTML generation

Each markdown file becomes a file entry link:

```html
<a class="file-entry" href="/view/architecture.md">
    <span>
        <span class="file-entry-icon">&#9670;</span>
        <span class="file-entry-name">architecture.md</span>
    </span>
    <span class="file-entry-badge has-flags">3 flags</span>
</a>
```

Each HTML file becomes:

```html
<a class="file-entry" href="/raw/preview.html">
    <span>
        <span class="file-entry-icon">&#9671;</span>
        <span class="file-entry-name">preview.html</span>
    </span>
    <span class="file-entry-badge">(html)</span>
</a>
```

The summary line: `"3 markdown . 2 html"`.

### Template rendering

The `index.html` template is loaded from embedded assets and populated:

```rust
let page = template
    .replace("{{directory}}", &dir_display)    // "./docs/"
    .replace("{{file_entries}}", &entries_html) // all the <a> elements
    .replace("{{summary}}", &summary);         // "3 markdown . 2 html"
```

### Response

The browser receives a complete HTML page with the directory listing styled according to the active theme.

## Step 4: User Clicks a Markdown File

The user clicks `architecture.md`. The browser navigates to `/view/architecture.md`.

### Request routing

axum matches `GET /view/architecture.md` to `view_handler` with `filepath = "architecture.md"`.

### Path resolution

```rust
resolve_path(&state.config.path, &filepath)
// resolve_path("./docs/", "architecture.md") -> "./docs/architecture.md"
```

### Rendering pipeline

1. **File read:** `std::fs::read_to_string("./docs/architecture.md")`
2. **comrak parsing:** Markdown to HTML with `unsafe_ = true`
3. **Code highlighting:** syntect processes `<pre><code class="language-X">` blocks
4. **Flag rendering:** `<flag:N>` tags become styled `<span class="flag">` elements
5. **Template filling:** The `document.html` template wraps the content

### Browser display

The user sees:

- The document rendered with Playfair Display headings and Source Serif 4 body text
- Code blocks with syntax highlighting
- Three flags highlighted with warm yellow background
- The sidebar showing all three flags with their IDs and comments
- A status bar indicating the WebSocket connection is active

## Step 5: User Clicks an HTML File

Going back to the listing and clicking `preview.html` navigates to `/raw/preview.html`.

### Request routing

axum matches `GET /raw/preview.html` to `raw_handler` with `filepath = "preview.html"`.

### Direct serving

```rust
match std::fs::read_to_string(&full_path) {
    Ok(content) => Html(content).into_response(),
    Err(_) => (StatusCode::NOT_FOUND, "File not found").into_response(),
}
```

The HTML file is served exactly as written. No markdown processing, no template wrapping, no flag system. The browser renders whatever HTML is in the file.

## Step 6: WebSocket Connection

While the user is viewing any page, the JavaScript establishes a WebSocket connection.

### Connection setup

```javascript
var protocol = location.protocol === 'https:' ? 'wss:' : 'ws:';
var ws = new WebSocket(protocol + '//' + location.host + '/ws');
```

### Server-side handling

axum matches `GET /ws` to `ws_handler`, which upgrades the connection:

```rust
async fn ws_handler(
    State(state): State<AppState>,
    ws: WebSocketUpgrade,
) -> Response {
    let rx = state.reload_tx.subscribe();
    ws.on_upgrade(|socket| handle_ws(socket, rx))
}
```

The handler creates a new broadcast receiver and spawns a long-lived task that:
- Waits for broadcast messages (file changes)
- Sends "reload" to the WebSocket client
- Handles disconnection gracefully

### Status indicator

The JavaScript updates the connection status indicator:

```javascript
ws.onopen = function() {
    statusConnection.classList.remove('disconnected');
    // Green dot visible
};

ws.onclose = function() {
    statusConnection.classList.add('disconnected');
    // Red dot visible, reconnect in 2s
    setTimeout(connectWebSocket, 2000);
};
```

## Step 7: File Changes Externally

While the user is viewing `architecture.md`, someone edits the file in another editor and saves.

### File system notification

The `notify` watcher detects the modify event and broadcasts the changed path through the channel.

### WebSocket relay

The `handle_ws` task receives the broadcast and sends `"reload"` to the browser.

### Page reload

The browser reloads the current page. The `view_handler` re-reads the file, re-renders it, and the user sees the updated content.

## Complete Request Flow Diagram

```
User: previewf serve ./docs/
    |
    v
Server starts on :3000
    |
Browser: GET /
    |
    v
index_handler: scan ./docs/, count flags per .md file
    |
    v
Browser: directory listing page
    |
User clicks: architecture.md
    |
    v
Browser: GET /view/architecture.md
    |
    v
view_handler: read file -> comrak -> syntect -> flag spans -> template
    |
    v
Browser: rendered document with flags and sidebar
    |
Browser: GET /ws (WebSocket upgrade)
    |
    v
ws_handler: subscribe to broadcast, send "reload" on changes
    |
External edit: architecture.md saved
    |
    v
notify watcher: detect modify event -> broadcast
    |
    v
ws_handler: send "reload"
    |
    v
Browser: location.reload()
    |
    v
Browser: GET /view/architecture.md (fresh render)
```
