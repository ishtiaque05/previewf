# Code Flow

This chapter traces complete code flows through the system, following data from input to output with concrete examples. Each trace shows the exact function calls, data transformations, and module boundaries involved.

## Flow 1: Browser Requests a Markdown File

**Scenario:** The user visits `http://localhost:3000/view/plan.md` in their browser. The server is running with `previewf serve ./docs/`.

### Step-by-Step Trace

**1. HTTP request arrives at axum.**

The browser sends `GET /view/plan.md`. axum's router matches the pattern `/view/{*filepath}` and extracts `filepath = "plan.md"`.

**2. `view_handler` is called.**

```rust
async fn view_handler(
    State(state): State<AppState>,
    AxumPath(filepath): AxumPath<String>,  // "plan.md"
) -> Response
```

axum injects `state` (which contains the config with `path: "./docs/"`) and `filepath` (the path parameter).

**3. Path resolution.**

```rust
let full_path = resolve_path(&state.config.path, &filepath);
// resolve_path("./docs/", "plan.md") -> "./docs/plan.md"
```

The `resolve_path` function joins the base directory with the relative path. If the base is a file (single-file mode), it returns the base path directly.

**4. File read.**

```rust
let content = std::fs::read_to_string(&full_path)?;
```

Reads the entire file into a String. For a typical markdown document (a few KB), this is effectively instant.

Suppose the file contains:

```markdown
# Plan

## Timeline

This project takes <flag:1>Comment: too optimistic</flag> two weeks.

## Code

\```rust
fn main() {
    println!("hello");
}
\```
```

**5. Markdown rendering: `render_html`.**

```rust
let rendered = render_html(&content);
```

This is where most of the transformation happens. Inside `render_html`:

**5a. comrak parsing.**

```rust
let mut options = Options::default();
options.extension.strikethrough = true;
options.extension.table = true;
options.extension.autolink = true;
options.extension.tasklist = true;
options.extension.footnotes = true;
options.render.unsafe_ = true;  // This is key for flags

let raw_html = markdown_to_html(content, &options);
```

comrak processes the markdown and produces HTML. The `unsafe_ = true` option means raw HTML tags (including our `<flag:N>` tags) are passed through:

```html
<h1>Plan</h1>
<h2>Timeline</h2>
<p>This project takes <flag:1>Comment: too optimistic</flag> two weeks.</p>
<h2>Code</h2>
<pre><code class="language-rust">fn main() {
    println!(&quot;hello&quot;);
}
</code></pre>
```

**5b. Syntax highlighting: `highlight_code_blocks`.**

```rust
let highlighted = highlight_code_blocks(&raw_html);
```

The function uses a regex to find `<pre><code class="language-X">` blocks:

```
Pattern: <pre><code class="language-(\w+)">([\s\S]*?)</code></pre>
Match:   language = "rust", code = "fn main() {\n    println!(\"hello\");\n}\n"
```

For the matched code block:

1. HTML entities are decoded: `&quot;` back to `"`
2. syntect looks up the "rust" syntax definition
3. syntect highlights the code using the "base16-ocean.dark" theme
4. The original `<pre><code>` is replaced with syntect's output:

```html
<pre class="highlight" data-lang="rust">
<span style="color:#b48ead;">fn </span>
<span style="color:#8fa1b3;">main</span>
<span style="color:#c0c5ce;">()</span>
...
</pre>
```

**5c. Flag post-processing: `render_flag_spans`.**

```rust
let final_html = render_flag_spans(&highlighted);
```

The function finds `<flag:N>` tags in the HTML and converts them to styled spans:

```
Input:  <flag:1>Comment: too optimistic</flag>
Output: <span class="flag" data-flag-id="1">
            <span class="flag-marker">#1</span>
            <span class="flag-comment">too optimistic</span>
        </span>
```

**6. Template substitution.**

```rust
let template = Assets::get("document.html")
    .map(|f| String::from_utf8_lossy(&f.data).to_string())
    .unwrap_or_else(|| "<html><body>{{content}}</body></html>".to_string());

let page = template
    .replace("{{title}}", &filepath)      // "plan.md"
    .replace("{{filepath}}", &filepath)    // "plan.md"
    .replace("{{content}}", &rendered);    // the full rendered HTML
```

The `document.html` template includes the Google Fonts links, CSS, and JavaScript. The `{{content}}` placeholder is replaced with the rendered markdown.

**7. HTTP response.**

```rust
Html(page).into_response()
```

axum sends the complete HTML page with `Content-Type: text/html` and status 200.

**8. Browser receives and renders.**

The browser parses the HTML, loads the CSS and JavaScript from `/assets/`, and renders the page. The JavaScript:

1. Detects theme preference and applies it
2. Scans for `.flag` spans and populates the sidebar
3. Establishes a WebSocket connection for live reload
4. Sets up text selection handlers for flag creation

### Data Transformation Summary

```
File on disk (markdown with flag tags)
    |  std::fs::read_to_string
    v
Raw markdown string
    |  comrak::markdown_to_html (unsafe_ = true)
    v
Raw HTML (flag tags preserved, code blocks as <pre><code>)
    |  highlight_code_blocks (syntect)
    v
HTML with highlighted code blocks (flag tags still raw)
    |  render_flag_spans (regex replacement)
    v
HTML with styled flag spans
    |  template substitution ({{content}} -> HTML)
    v
Complete HTML page
    |  axum Html response
    v
Browser renders the page
```

## Flow 2: User Creates a Flag

**Scenario:** The user selects the text "two weeks" in the browser and types the comment "too optimistic" in the floating toolbar.

### Step-by-Step Trace

**1. Text selection triggers the toolbar.**

In `app.js`, a `mouseup` event listener on `#document` checks if text is selected:

```javascript
documentEl.addEventListener('mouseup', function() {
    var selection = window.getSelection();
    var text = selection.toString().trim();
    if (text.length > 0) {
        var rect = selection.getRangeAt(0).getBoundingClientRect();
        showToolbar(rect.left + window.scrollX, rect.bottom + window.scrollY + 8);
    }
});
```

The toolbar appears positioned below the selection.

**2. User types comment and submits.**

Clicking "Flag" or pressing Enter calls `submitFlag()`:

```javascript
function submitFlag() {
    var comment = document.getElementById('flag-comment-input').value.trim();
    var selection = window.getSelection();
    var selectedText = selection.getRangeAt(0).toString();
    var filepath = document.querySelector('.file-path').textContent;

    fetch('/flag/' + encodeURIComponent(filepath), {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
            comment: comment,           // "too optimistic"
            selected_text: selectedText  // "two weeks"
        }),
    });
}
```

**3. HTTP POST arrives at the server.**

```rust
async fn flag_post_handler(
    State(state): State<AppState>,
    AxumPath(filepath): AxumPath<String>,          // "plan.md"
    axum::Json(payload): axum::Json<FlagRequest>,   // { comment, selected_text }
) -> Response
```

**4. File read and line search.**

```rust
let content = std::fs::read_to_string(&full_path)?;

let line = content
    .lines()
    .enumerate()
    .find(|(_, l)| l.contains(&payload.selected_text))  // find "two weeks"
    .map(|(i, _)| i + 1);  // convert to 1-indexed
```

The handler reads the file and searches for the first line containing the selected text. In our example, it finds "This project takes ... two weeks." on line 5.

**5. Flag injection.**

```rust
inject_flag(&content, 5, "too optimistic")
```

Inside `inject_flag`:

1. `next_flag_id(&content)` scans existing flags, finds `<flag:1>` is the highest, returns `2`
2. Constructs the tag: `" <flag:2>Comment: too optimistic</flag>"`
3. Appends the tag to line 5

The line changes from:

```
This project takes <flag:1>Comment: too optimistic</flag> two weeks.
```

To:

```
This project takes <flag:1>Comment: too optimistic</flag> two weeks. <flag:2>Comment: too optimistic</flag>
```

(Note: in a real scenario, the selected text "two weeks" would be on a line that does not already have a flag. This example is simplified.)

**6. File write.**

```rust
std::fs::write(&full_path, new_content)?;
```

The modified content is written back to disk atomically (from Rust's perspective -- `std::fs::write` replaces the file contents).

**7. Broadcast reload.**

```rust
let _ = state.reload_tx.send(());
```

This sends a unit value `()` on the broadcast channel. All subscribed WebSocket tasks receive it.

**8. WebSocket notification.**

In `handle_ws`, the `tokio::select!` loop receives the broadcast:

```rust
result = rx.recv() => {
    match result {
        Ok(()) => {
            socket.send(Message::Text("reload".into())).await?;
        }
    }
}
```

**9. Browser reload.**

The JavaScript WebSocket handler receives the message:

```javascript
ws.onmessage = function(event) {
    if (event.data === 'reload') {
        location.reload();
    }
};
```

The page reloads, triggering Flow 1 again. The new flag is now visible in the rendered page and the sidebar.

### Timing

The entire flag creation flow -- from user click to page reload -- typically completes in under 100ms:

| Step | Approximate Time |
|------|-----------------|
| JavaScript -> POST request | ~5ms |
| Server reads file | ~1ms |
| Line search | ~1ms |
| inject_flag | ~1ms |
| File write | ~5ms |
| Broadcast + WebSocket send | ~1ms |
| Browser receives + reloads | ~50ms |

## Flow 3: Terminal View of a Flagged File

**Scenario:** The user runs `previewf view ./plan.md`.

### Step-by-Step Trace

**1. CLI parsing.**

clap matches `Commands::View { path: "./plan.md" }`.

**2. File read.**

```rust
let content = std::fs::read_to_string(&path)?;
```

Content:

```
# Plan

## Timeline

This project takes <flag:1>Comment: too optimistic</flag> two weeks.
```

**3. Terminal rendering: `render_terminal`.**

```rust
let rendered = render_terminal(&content);
```

**3a. Flag preparation.**

```rust
let prepared = prepare_flags_for_terminal(&content);
```

The regex replaces flag tags:

```
Input:  This project takes <flag:1>Comment: too optimistic</flag> two weeks.
Output: This project takes **[FLAG #1:** too optimistic**]** two weeks.
```

**3b. termimad rendering.**

```rust
let skin = MadSkin::default();
skin.term_text(&prepared).to_string()
```

termimad processes the markdown and produces an ANSI-formatted string:

- `# Plan` becomes a bold, colored heading
- `## Timeline` becomes a bold sub-heading
- `**[FLAG #1:** too optimistic**]**` becomes bold text (the `**` markers)
- Regular text is wrapped to terminal width

**4. Output.**

```rust
print!("{}", rendered);
```

The ANSI string is printed to stdout. The terminal interprets the escape codes and displays colored, formatted text.

## Flow 4: Flag Export as JSON

**Scenario:** The user runs `previewf flags ./plan.md --json`.

### Step-by-Step Trace

**1. File read.**

```rust
let content = std::fs::read_to_string(&path)?;
```

**2. Flag extraction.**

```rust
let flags = extract_flags(&content);
```

The regex scans each line:

- Line 5: matches `<flag:1>Comment: too optimistic</flag>`
  - Produces `Flag { id: 1, line: 5, text: "This project takes two weeks.", comment: "too optimistic" }`

**3. Report construction.**

```rust
let report = FlagReport {
    file: "plan.md".to_string(),
    flags,
};
```

**4. JSON serialization.**

```rust
let json = serde_json::to_string_pretty(&report)?;
println!("{}", json);
```

Output:

```json
{
  "file": "plan.md",
  "flags": [
    {
      "id": 1,
      "line": 5,
      "text": "This project takes two weeks.",
      "comment": "too optimistic"
    }
  ]
}
```

### Data Flow Diagram

```
plan.md (on disk)
    |
    v
"# Plan\n\n## Timeline\n\nThis project takes <flag:1>Comment: too optimistic</flag> two weeks.\n"
    |  extract_flags (regex per line)
    v
Vec<Flag> [
    Flag { id: 1, line: 5, text: "This project takes two weeks.", comment: "too optimistic" }
]
    |  FlagReport { file, flags }
    v
FlagReport
    |  serde_json::to_string_pretty
    v
JSON string -> stdout
```

## Flow 5: Live Reload on External File Change

**Scenario:** The user is viewing `plan.md` in the browser. In a separate editor, they modify the file and save it.

### Step-by-Step Trace

**1. File system event.**

The OS detects the file write and notifies the `notify` crate via the platform-native mechanism (FSEvents on macOS, inotify on Linux).

**2. notify callback.**

```rust
let mut watcher = notify::recommended_watcher(move |res: Result<Event, Error>| {
    if let Ok(event) = res {
        if event.kind.is_modify() || event.kind.is_create() {
            for path in event.paths {
                let _ = sender.send(path);
            }
        }
    }
})?;
```

The callback fires on the notify thread. It checks if the event is a modify or create event, then sends the changed path through the broadcast channel.

**3. Broadcast to WebSocket tasks.**

The `broadcast::Sender` delivers the notification to all subscribed receivers. Each active WebSocket connection has its own receiver.

**4. WebSocket message.**

```rust
// In handle_ws:
result = rx.recv() => {
    Ok(()) => {
        socket.send(Message::Text("reload".into())).await?;
    }
}
```

**5. Browser reload.**

```javascript
ws.onmessage = function(event) {
    if (event.data === 'reload') {
        location.reload();
    }
};
```

**6. Fresh render.**

The browser sends a new `GET /view/plan.md` request, which triggers Flow 1 with the updated file content.

### Timing

| Step | Approximate Time |
|------|-----------------|
| File save -> OS notification | ~10-50ms (platform dependent) |
| notify callback fires | ~1ms |
| Broadcast send | ~1ms |
| WebSocket delivery | ~1ms |
| Browser receives message | ~5ms |
| Browser reloads page | ~50ms |
| Server renders updated content | ~10ms |

Total: typically under 150ms from file save to updated page.
