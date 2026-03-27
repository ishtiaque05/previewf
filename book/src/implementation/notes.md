# Implementation Notes

This chapter documents what was actually built, deviations from the original plan, and lessons learned during implementation. It serves as a living log — updated after each PR.

## PR #1: Flag System (Tasks 2-3)

**Branch:** `feat/flag-system`

### What was built

- `Flag` struct with `id`, `line`, `text`, `comment` fields — derives Debug, Clone, Serialize, Deserialize, PartialEq
- `FlagReport` struct wrapping a filename and `Vec<Flag>`
- `extract_flags()` — regex-based parser for `<flag:N>Comment: description</flag>` patterns
- `next_flag_id()` — finds max existing ID and returns +1
- `format_flags_text()` — human-readable output formatter
- `inject_flag()` — appends flag tag at end of specified line with auto-ID assignment

### Design decisions made during implementation

**Regex over AST parsing for flags:** We use a simple regex `<flag:(\d+)>Comment:\s*(.+?)</flag>` rather than parsing the markdown AST. This is deliberate — flags are a layer on top of markdown, not part of it. The regex approach is simpler, faster, and works regardless of where in the document the flag appears.

**Text field for multi-flag lines:** When a line has multiple flags, each extracted `Flag`'s `text` field contains the full line with ALL flag tags stripped (not just the one flag). This was noted during spec review but accepted — no tests assert on the `text` field for multi-flag lines, and the behavior is consistent.

### Test coverage

9 tests covering:
- Extraction from flagged and clean files
- Multiple flags on a single line
- JSON serialization roundtrip
- Next-ID with and without existing flags
- Injection into clean and flagged content
- Invalid line number error handling

---

## PR #2: CI/CD Pipeline (Task 11)

**Branch:** `feat/ci-pipeline`

### What was built

**CI workflow (`.github/workflows/ci.yml`):**
- Format check (`cargo fmt --check`) — runs on ubuntu only
- Clippy lint — runs on ubuntu + macos matrix
- Tests via nextest — runs on ubuntu + macos matrix
- Coverage via tarpaulin — runs on ubuntu only
- All jobs run in parallel for fast feedback

**Release workflow (`.github/workflows/release.yml`):**
- Triggers on `v*` tags
- Builds for 4 targets: x86_64-linux, aarch64-linux, x86_64-darwin, aarch64-darwin
- Packages as tar.gz with SHA256 checksums
- Creates GitHub Release automatically

### Dependencies added

- `googletest = "0.14"` — Google's expressive test matchers for Rust
- `tempfile = "3"` — temporary directories for watcher tests
- `tower = { version = "0.5", features = ["util"] }` — for axum router testing with `oneshot()`

---

## PR #3: Markdown + Terminal Rendering (Tasks 4-5)

**Branch:** `feat/markdown-rendering`

### What was built

**Markdown rendering (`src/markdown.rs`):**
- `render_html()` — comrak parsing with GFM extensions + syntect highlighting
- Code block highlighting via syntect with `base16-ocean.dark` theme
- Diff block detection and rendering with CSS classes (`diff-added`, `diff-removed`, `diff-hunk`, `diff-context`)
- Flag tag conversion to styled spans: `<span class="flag" data-flag-id="N">` with `.flag-marker` and `.flag-comment` sub-spans
- HTML escape/unescape helpers for syntect processing

**Terminal rendering (`src/terminal.rs`):**
- `render_terminal()` — termimad-based markdown rendering
- `prepare_flags_for_terminal()` — converts `<flag:N>` to `**[FLAG #N:** text**]**` for bold terminal display

### Spec review findings

The spec review caught a bug before the PR was created:

**Issue:** `render_flags()` initially produced `<span class="flag-marker" data-flag-id="N">` (wrong outer class) with no inner sub-spans. The spec requires `<span class="flag">` as the outer wrapper with `.flag-marker` and `.flag-comment` sub-spans inside.

**Fix:** Restructured to match spec — `<span class="flag" data-flag-id="N"><span class="flag-marker">#N</span><span class="flag-comment">text</span></span>`

**Accepted gap:** Diff auto-detection only works for explicitly tagged ````diff` blocks, not untagged code blocks that happen to contain `@@` hunk headers. This is the primary use case and the edge case was deferred.

---

## PR #4: Server Stack (Tasks 6-8)

**Branch:** `feat/server-stack`

### What was built

**File watcher (`src/watcher.rs`):**
- `FileWatcher` struct with `new()` and `watch()` methods
- Uses `notify::recommended_watcher` — platform-native (FSEvents on macOS, inotify on Linux)
- `broadcast::channel<PathBuf>` for fan-out to multiple WebSocket connections
- Supports both file and directory watching (recursive for directories)
- `subscribe()` method for additional receivers

**Frontend assets (`assets/`):**
- `document.html` — document viewer with top bar, sidebar, status bar, template placeholders
- `index.html` — directory listing with file entries and summary
- `style.css` (~957 lines) — full "Annotated Page" editorial design with:
  - Light/dark theme CSS custom properties
  - Typography: Playfair Display, Source Serif 4, JetBrains Mono, DM Sans
  - 72ch reading column, 280px sidebar
  - Diff coloring, flag highlighting, code block styling
  - Animations (fadeIn, slideIn, pulse, throb)
  - Print styles, responsive breakpoints
- `app.js` (~442 lines) — NO innerHTML anywhere (XSS prevention):
  - Theme toggle with localStorage persistence
  - Flag sidebar population via safe DOM methods
  - Flag creation toolbar (text selection → floating input → POST)
  - WebSocket live reload with auto-reconnect

**Web server (`src/server.rs`):**
- `ServerBuilder` — builder pattern with `path()`, `port()`, `live_reload()`, `build()`
- `ServerConfig` — immutable config struct
- `create_router()` — public for testing, creates full axum Router
- `run()` — starts server with background file watcher task
- 7 routes: index, view, raw, flags, flag-post, websocket, assets
- Embedded assets via `rust-embed`
- WebSocket handler with `tokio::select!` for concurrent recv/send
- Template rendering with `{{placeholder}}` replacement

### Architecture: How a request flows

```
Browser GET /view/plan.md
    → axum router matches /view/{*filepath}
    → view_handler() extracts filepath from URL
    → resolves against base path (directory or file mode)
    → reads file from disk (std::fs::read_to_string)
    → calls render_html() from markdown module
    → loads document.html template from embedded assets
    → replaces {{title}}, {{filepath}}, {{content}}
    → returns Html response
```

```
Browser POST /flag/plan.md {comment, selected_text}
    → flag_post_handler() reads current file content
    → finds line containing selected_text
    → calls inject_flag() to add flag tag
    → writes modified content back to disk
    → sends () on reload_tx broadcast channel
    → WebSocket task receives, sends "reload" to all browsers
    → JavaScript client calls location.reload()
```

### Test coverage

12 server integration tests using axum's `oneshot()` testing pattern:
- Directory listing (markdown and HTML files appear)
- Markdown rendering (contains `<h1>` and content)
- Raw HTML serving
- Flags JSON endpoint (with and without flags)
- 404 for missing files
- Static asset serving (CSS, JS)
- Builder validation (missing path error)

---

## PR #6: CLI Integration (Task 9)

**Branch:** `feat/cli-integration`

### What was built

Wired the three stub CLI subcommands in `src/main.rs` to their actual implementations:

- **`serve`** — builds `ServerConfig` via `ServerBuilder`, calls `server::run()` which starts the axum server with file watcher integration
- **`view`** — reads the markdown file, passes it through `render_terminal()` for termimad output
- **`flags`** — reads the markdown file, calls `extract_flags()`, outputs either human-readable text via `format_flags_text()` or JSON via `serde_json`

### Notes

- Task 10 (file watcher integration with server) was already completed as part of the server stack (PR #4) — the `run()` function in `server.rs` spawns a background watcher task when `live_reload` is enabled
- No new dependencies added — `anyhow::Context` was already available
- All 53 existing tests continue to pass
