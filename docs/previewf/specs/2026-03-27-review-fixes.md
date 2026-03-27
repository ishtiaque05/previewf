# Review Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Address all blocking and high-priority findings from the 9-agent code review + staff engineer review of the server stack.

**Architecture:** Extract shared utilities (html_escape, FLAG_RE) to eliminate duplication. Add security headers via axum middleware. Fix data contract between Rust rendering and JS sidebar. Add per-file locking for flag writes. Add security regression tests. Bind to localhost by default.

**Tech Stack:** Rust (axum, tokio, comrak), JavaScript (vanilla), tower-http

---

## File Structure

| File | Action | Responsibility |
|------|--------|---------------|
| `src/html.rs` | Create | Shared `html_escape` and `html_unescape` utilities |
| `src/lib.rs` | Modify | Add `pub mod html` |
| `src/flags.rs` | Modify | Export `FLAG_RE`, use shared `html::escape`, add `is_markdown` guard |
| `src/markdown.rs` | Modify | Import `FLAG_RE` from flags, import `html::escape`/`html::unescape` |
| `src/terminal.rs` | Modify | Import `FLAG_RE` from flags |
| `src/server.rs` | Modify | Add security headers middleware, per-file mutex, bind 127.0.0.1, use shared html::escape, fix resolve_path single-file mode, add is_markdown guard to flag_handler, remove duplicate reload send |
| `src/watcher.rs` | Modify | Merge new()+watch() into single constructor, log watcher errors |
| `assets/app.js` | Modify | Fix sidebar to read .flag-comment child, fix encodeURIComponent for paths, fix OS theme listener dead code |
| `tests/server_test.rs` | Modify | Add path traversal tests, XSS tests, flag POST integration tests, non-markdown view test |
| `tests/watcher_test.rs` | Modify | Update for new single-step FileWatcher constructor |
| `Cargo.toml` | Modify | Remove unused tower-http dependency |

---

### Task 1: Extract shared html utility module

**Files:**
- Create: `src/html.rs`
- Modify: `src/lib.rs`
- Modify: `src/server.rs`
- Modify: `src/markdown.rs`
- Modify: `src/flags.rs`

- [ ] **Step 1: Create `src/html.rs` with escape/unescape functions**

```rust
/// Shared HTML escaping utilities.

/// Escape HTML special characters to prevent XSS.
pub fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Decode HTML entities back to their original characters.
pub fn unescape(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
}
```

- [ ] **Step 2: Add `pub mod html` to `src/lib.rs`**

Add `pub mod html;` after `pub mod flags;`.

- [ ] **Step 3: Replace `html_escape` in `src/server.rs` with `crate::html::escape`**

Remove the `html_escape` function at the bottom of `server.rs` (lines 478-484). Add `use crate::html;` to imports. Replace all calls:
- `html_escape(...)` -> `html::escape(...)`

- [ ] **Step 4: Replace `html_escape_encode` and `html_escape_decode` in `src/markdown.rs` with shared versions**

Remove `html_escape_encode` (lines 119-125) and `html_escape_decode` (lines 111-117). Add `use crate::html;`. Replace calls:
- `html_escape_encode(...)` -> `html::escape(...)`
- `html_escape_decode(...)` -> `html::unescape(...)`

- [ ] **Step 5: Update `sanitize_comment` in `src/flags.rs` to use shared `html::escape`**

Replace the body of `sanitize_comment`:
```rust
fn sanitize_comment(comment: &str) -> String {
    // First escape flag-specific patterns, then general HTML
    let s = comment
        .replace("</flag>", "[/flag]")
        .replace("<flag:", "[flag:");
    crate::html::escape(&s)
}
```

- [ ] **Step 6: Run tests to verify**

Run: `cargo test`
Expected: All existing tests pass.

- [ ] **Step 7: Commit**

```bash
git add src/html.rs src/lib.rs src/server.rs src/markdown.rs src/flags.rs
git commit -m "refactor: extract shared html escape utilities to src/html.rs"
```

---

### Task 2: Deduplicate FLAG_RE regex into flags.rs

**Files:**
- Modify: `src/flags.rs`
- Modify: `src/markdown.rs`
- Modify: `src/terminal.rs`

- [ ] **Step 1: Make `FLAG_RE` public in `src/flags.rs`**

Change line 8 from:
```rust
static FLAG_RE: LazyLock<Regex> =
```
to:
```rust
pub static FLAG_RE: LazyLock<Regex> =
```

- [ ] **Step 2: Remove `FLAG_RE` from `src/markdown.rs` and import from flags**

Remove lines 18-19 (`static FLAG_RE: ...`). Remove the `use regex::Regex;` import (keep it only if `CODE_BLOCK_RE` and `DIFF_BLOCK_RE` still need it — they do, so keep `Regex` but remove the unused `LazyLock` import only if no other LazyLock remains — actually `CODE_BLOCK_RE`, `DIFF_BLOCK_RE`, `SYNTAX_SET`, `THEME_SET` all use `LazyLock`, so keep the import). Add to the imports section:

```rust
use crate::flags::FLAG_RE;
```

- [ ] **Step 3: Remove `FLAG_RE` from `src/terminal.rs` and import from flags**

Remove lines 1-2 (`use std::sync::LazyLock;`) and lines 8-9 (`static FLAG_RE: ...`). Remove `use regex::Regex;`. Add:

```rust
use crate::flags::FLAG_RE;
```

- [ ] **Step 4: Run tests**

Run: `cargo test`
Expected: All tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/flags.rs src/markdown.rs src/terminal.rs
git commit -m "refactor: deduplicate FLAG_RE regex into flags.rs"
```

---

### Task 3: Fix sidebar flag comments (JS/Rust data contract)

**Files:**
- Modify: `assets/app.js`

- [ ] **Step 1: Fix `initFlagSidebar` to read comment from child element**

In `assets/app.js`, change line 87 from:
```javascript
var flagComment = flagEl.getAttribute('data-flag-comment') || '';
```
to:
```javascript
var flagCommentEl = flagEl.querySelector('.flag-comment');
var flagComment = flagCommentEl ? flagCommentEl.textContent : '';
```

- [ ] **Step 2: Commit**

```bash
git add assets/app.js
git commit -m "fix: read flag comments from child element instead of missing data attribute"
```

---

### Task 4: Fix OS theme change listener dead code

**Files:**
- Modify: `assets/app.js`

- [ ] **Step 1: Fix `applyTheme` to distinguish manual vs auto theme**

Change `applyTheme` and `initTheme` to only persist on manual toggle:

Replace `applyTheme` function (lines 27-30):
```javascript
function applyTheme(theme) {
    document.documentElement.setAttribute('data-theme', theme);
}

function persistTheme(theme) {
    applyTheme(theme);
    localStorage.setItem(THEME_KEY, theme);
}
```

Change the toggle click handler (line 41) to use `persistTheme`:
```javascript
toggle.addEventListener('click', function () {
    var current = document.documentElement.getAttribute('data-theme');
    var next = current === 'dark' ? 'light' : 'dark';
    persistTheme(next);
});
```

Keep `initTheme` calling `applyTheme(theme)` (not `persistTheme`) on line 34 — this way if the theme came from OS preference, it won't be stored to localStorage, and the OS listener will work correctly.

- [ ] **Step 2: Commit**

```bash
git add assets/app.js
git commit -m "fix: allow OS theme changes when user hasn't manually toggled theme"
```

---

### Task 5: Fix encodeURIComponent double-encoding filepath in flag POST

**Files:**
- Modify: `assets/app.js`

- [ ] **Step 1: Fix URL construction in `submitFlag`**

Change line 285 from:
```javascript
var url = '/flag/' + encodeURIComponent(currentFilepath);
```
to:
```javascript
var url = '/flag/' + currentFilepath.split('/').map(encodeURIComponent).join('/');
```

- [ ] **Step 2: Commit**

```bash
git add assets/app.js
git commit -m "fix: encode filepath segments individually to preserve path separators"
```

---

### Task 6: Merge FileWatcher new() and watch() into single constructor

**Files:**
- Modify: `src/watcher.rs`
- Modify: `src/server.rs`
- Modify: `tests/watcher_test.rs`

- [ ] **Step 1: Rewrite `FileWatcher` to single-step constructor with error logging**

Replace `src/watcher.rs` contents:

```rust
use std::path::PathBuf;

use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::broadcast;

use crate::PreviewError;

pub struct FileWatcher {
    path: PathBuf,
    _watcher: RecommendedWatcher,
    sender: broadcast::Sender<PathBuf>,
}

impl FileWatcher {
    /// Create and start a file watcher for the given path.
    /// Returns the watcher and a receiver for change notifications.
    pub fn new(path: PathBuf) -> Result<(Self, broadcast::Receiver<PathBuf>), PreviewError> {
        let (sender, receiver) = broadcast::channel(100);
        let tx = sender.clone();

        let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            match res {
                Ok(event) => {
                    if event.kind.is_modify() || event.kind.is_create() {
                        for path in event.paths {
                            let _ = tx.send(path);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Warning: file watcher error: {e}");
                }
            }
        })
        .map_err(PreviewError::Watcher)?;

        let mode = if path.is_dir() {
            RecursiveMode::Recursive
        } else {
            RecursiveMode::NonRecursive
        };

        watcher
            .watch(&path, mode)
            .map_err(PreviewError::Watcher)?;

        Ok((
            Self {
                path,
                _watcher: watcher,
                sender,
            },
            receiver,
        ))
    }

    /// Get a new receiver for change notifications.
    pub fn subscribe(&self) -> broadcast::Receiver<PathBuf> {
        self.sender.subscribe()
    }
}
```

- [ ] **Step 2: Update `src/server.rs` run() to use single-step constructor**

Replace the watcher spawn block (lines 135-156) with:

```rust
        let watcher_path = config.path.clone();
        let tx = reload_tx.clone();
        tokio::spawn(async move {
            match crate::watcher::FileWatcher::new(watcher_path) {
                Ok((_fw, mut rx)) => {
                    loop {
                        match rx.recv().await {
                            Ok(_) => {
                                let _ = tx.send(());
                            }
                            Err(broadcast::error::RecvError::Lagged(_)) => continue,
                            Err(_) => break,
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Warning: file watcher failed to start: {e}");
                }
            }
        });
```

- [ ] **Step 3: Update `tests/watcher_test.rs`**

Change the test to use the new single-step API:

```rust
use previewf::watcher::FileWatcher;
use std::fs;
use std::time::Duration;
use tempfile::TempDir;

#[tokio::test]
async fn test_watcher_detects_file_change() {
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("test.md");
    fs::write(&file_path, "# Hello").unwrap();

    let (_watcher, mut rx) = FileWatcher::new(dir.path().to_path_buf()).unwrap();

    // Modify the file
    tokio::time::sleep(Duration::from_millis(100)).await;
    fs::write(&file_path, "# Hello Updated").unwrap();

    // Should receive a notification within 2 seconds
    let result = tokio::time::timeout(Duration::from_secs(2), rx.recv()).await;
    assert!(result.is_ok(), "Should receive file change notification");
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test`
Expected: All tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/watcher.rs src/server.rs tests/watcher_test.rs
git commit -m "refactor: merge FileWatcher new/watch into single constructor, log watcher errors"
```

---

### Task 7: Add security headers middleware and bind to localhost

**Files:**
- Modify: `src/server.rs`

- [ ] **Step 1: Add security headers middleware to the router**

Add these imports to the top of `server.rs`:
```rust
use axum::middleware::{self, Next};
use axum::http::Request as HttpRequest;
```

Add a middleware function before the router construction:

```rust
async fn security_headers(request: HttpRequest<Body>, next: Next) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert("X-Content-Type-Options", "nosniff".parse().unwrap());
    headers.insert("X-Frame-Options", "DENY".parse().unwrap());
    headers.insert(
        "Content-Security-Policy",
        "default-src 'self'; script-src 'self'; style-src 'self' https://fonts.googleapis.com 'unsafe-inline'; font-src 'self' https://fonts.gstatic.com; img-src 'self' data:; connect-src 'self' ws: wss:"
            .parse()
            .unwrap(),
    );
    response
}
```

Add the middleware layer to the router in `create_router_with_reload`:
```rust
    Router::new()
        .route("/", get(index_handler))
        // ... all routes ...
        .with_state(state)
        .layer(middleware::from_fn(security_headers))
```

- [ ] **Step 2: Change bind address from `0.0.0.0` to `127.0.0.1`**

Change line 161 from:
```rust
let addr = format!("0.0.0.0:{}", config.port);
```
to:
```rust
let addr = format!("127.0.0.1:{}", config.port);
```

- [ ] **Step 3: Run tests**

Run: `cargo test`
Expected: All tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/server.rs
git commit -m "feat: add security headers middleware and bind to localhost by default"
```

---

### Task 8: Add is_markdown guard to flag_handler and per-file mutex

**Files:**
- Modify: `src/server.rs`

- [ ] **Step 1: Add per-file mutex to AppState**

Add imports:
```rust
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
```

Change `AppState`:
```rust
#[derive(Clone)]
struct AppState {
    config: ServerConfig,
    reload_tx: broadcast::Sender<()>,
    file_locks: Arc<Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>>,
}
```

Update `create_router_with_reload` to initialize the locks:
```rust
let state = AppState {
    config,
    reload_tx,
    file_locks: Arc::new(Mutex::new(HashMap::new())),
};
```

- [ ] **Step 2: Add is_markdown guard and per-file locking to flag_handler**

Replace `flag_handler` with:

```rust
async fn flag_handler(
    State(state): State<AppState>,
    AxumPath(filepath): AxumPath<String>,
    axum::Json(body): axum::Json<FlagRequest>,
) -> Response {
    if !is_markdown(&filepath) {
        return (StatusCode::BAD_REQUEST, "Flags can only be added to markdown files").into_response();
    }

    let full_path = match resolve_path(&state.config.path, &filepath) {
        Some(p) => p,
        None => return not_found_response(&filepath),
    };

    // Acquire per-file lock to prevent concurrent read-modify-write races
    let file_lock = {
        let mut locks = state.file_locks.lock().await;
        locks
            .entry(full_path.clone())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    };
    let _guard = file_lock.lock().await;

    let content = match std::fs::read_to_string(&full_path) {
        Ok(c) => c,
        Err(_) => return not_found_response(&filepath),
    };

    let line = content
        .lines()
        .enumerate()
        .find(|(_, l)| l.contains(&body.selected_text))
        .map(|(i, _)| i + 1);

    let line = match line {
        Some(l) => l,
        None => {
            return (StatusCode::BAD_REQUEST, "Selected text not found in file").into_response()
        }
    };

    match inject_flag(&content, line, &body.comment) {
        Ok(new_content) => match std::fs::write(&full_path, &new_content) {
            Ok(_) => {
                // Don't send explicit reload — the file watcher will detect the write
                (StatusCode::OK, "Flag injected").into_response()
            }
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        },
        Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    }
}
```

Note: This also removes the explicit `reload_tx.send(())` (B8 — double reload fix) and adds the `is_markdown` guard (S2).

- [ ] **Step 3: Run tests**

Run: `cargo test`
Expected: All tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/server.rs
git commit -m "fix: add is_markdown guard and per-file mutex to flag_handler, remove double reload"
```

---

### Task 9: Make ServerConfig fields private with getters

**Files:**
- Modify: `src/server.rs`

- [ ] **Step 1: Make fields private and add getters**

Change `ServerConfig`:
```rust
#[derive(Clone, Debug)]
pub struct ServerConfig {
    path: PathBuf,
    port: u16,
    live_reload: bool,
}

impl ServerConfig {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn live_reload(&self) -> bool {
        self.live_reload
    }
}
```

- [ ] **Step 2: Update all field accesses in `server.rs` to use getters**

Replace throughout:
- `config.path` -> `config.path()` (when used as reference)
- `config.port` -> `config.port()`
- `config.live_reload` -> `config.live_reload()`
- `state.config.path` -> `state.config.path()`

Keep `config.path.clone()` as `config.path().to_path_buf()` where ownership is needed.

In `ServerBuilder::build()`, the struct literal construction still works because it's in the same module.

- [ ] **Step 3: Run tests**

Run: `cargo test`
Expected: All tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/server.rs
git commit -m "refactor: make ServerConfig fields private with getters"
```

---

### Task 10: Add security regression tests

**Files:**
- Modify: `tests/server_test.rs`

- [ ] **Step 1: Add path traversal tests**

Add to `tests/server_test.rs`:

```rust
// --- Path traversal prevention ---

#[tokio::test]
async fn test_view_path_traversal_returns_404() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/view/../../etc/passwd")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_raw_path_traversal_returns_404() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/raw/../../etc/passwd")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_flags_path_traversal_returns_404() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/flags/../../etc/passwd")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
```

- [ ] **Step 2: Add XSS prevention test**

```rust
// --- XSS prevention ---

#[tokio::test]
async fn test_view_xss_in_path_is_escaped() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/view/%3Cscript%3Ealert(1)%3C%2Fscript%3E.md")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let html = String::from_utf8(body.to_vec()).unwrap();

    assert!(
        !html.contains("<script>alert(1)</script>"),
        "Response body must not contain unescaped script tags"
    );
}
```

- [ ] **Step 3: Add view non-markdown rejection test**

```rust
// --- View handler validation ---

#[tokio::test]
async fn test_view_non_markdown_returns_400() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/view/sample.html")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
```

- [ ] **Step 4: Add flag POST integration tests**

```rust
// --- Flag POST endpoint ---

#[tokio::test]
async fn test_flag_post_injects_flag_into_markdown() {
    let dir = tempfile::TempDir::new().unwrap();
    let file_path = dir.path().join("test.md");
    std::fs::write(&file_path, "# Hello\n\nThis is a test line.\n").unwrap();

    let config = ServerBuilder::new()
        .path(dir.path())
        .port(0)
        .live_reload(false)
        .build()
        .unwrap();
    let app = create_router(config);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/flag/test.md")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "comment": "needs review",
                        "selected_text": "test line"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let content = std::fs::read_to_string(&file_path).unwrap();
    assert!(
        content.contains("<flag:1>"),
        "File should contain injected flag"
    );
}

#[tokio::test]
async fn test_flag_post_selected_text_not_found_returns_400() {
    let dir = tempfile::TempDir::new().unwrap();
    let file_path = dir.path().join("test.md");
    std::fs::write(&file_path, "# Hello\n").unwrap();

    let config = ServerBuilder::new()
        .path(dir.path())
        .port(0)
        .live_reload(false)
        .build()
        .unwrap();
    let app = create_router(config);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/flag/test.md")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "comment": "test",
                        "selected_text": "nonexistent text"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_flag_post_nonexistent_file_returns_404() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/flag/nonexistent.md")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "comment": "test",
                        "selected_text": "text"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_flag_post_non_markdown_returns_400() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/flag/sample.html")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "comment": "test",
                        "selected_text": "text"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
```

- [ ] **Step 5: Add security headers test**

```rust
// --- Security headers ---

#[tokio::test]
async fn test_responses_include_security_headers() {
    let app = create_test_app();

    let response = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(
        response.headers().get("X-Content-Type-Options").unwrap(),
        "nosniff"
    );
    assert_eq!(
        response.headers().get("X-Frame-Options").unwrap(),
        "DENY"
    );
    assert!(response
        .headers()
        .get("Content-Security-Policy")
        .is_some());
}
```

- [ ] **Step 6: Run all tests**

Run: `cargo test`
Expected: All tests pass.

- [ ] **Step 7: Commit**

```bash
git add tests/server_test.rs
git commit -m "test: add security regression tests for path traversal, XSS, flag POST, and headers"
```

---

### Task 11: Remove unused tower-http dependency

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Remove tower-http from Cargo.toml**

Remove line 23:
```toml
tower-http = { version = "0.6", features = ["cors", "fs"] }
```

- [ ] **Step 2: Run tests to confirm nothing depends on it**

Run: `cargo test`
Expected: All tests pass.

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "chore: remove unused tower-http dependency"
```

---

### Task 12: Final verification

- [ ] **Step 1: Run full test suite**

Run: `cargo test`
Expected: All tests pass.

- [ ] **Step 2: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: No warnings.

- [ ] **Step 3: Run fmt check**

Run: `cargo fmt --check`
Expected: No formatting issues.
