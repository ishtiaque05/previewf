# Flag CRUD & Sidebar Fix — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the broken flag sidebar, add delete/edit flag functionality, and replace full page reloads with partial sidebar refresh.

**Architecture:** Two new Rust functions (`remove_flag`, `update_flag_comment`) in `flags.rs`, two new HTTP handlers in `server.rs` (`DELETE /flag/{id}/{*filepath}`, `PUT /flag/{id}/{*filepath}`), and a reworked JavaScript sidebar that fetches flags via API and supports inline edit/delete actions.

**Tech Stack:** Rust/Axum (backend), vanilla JavaScript (frontend), CSS custom properties (theming)

---

### Task 1: Add `remove_flag()` to `flags.rs` with tests

**Files:**
- Modify: `src/flags.rs`
- Modify: `tests/flags_test.rs`

- [ ] **Step 1: Write the failing tests**

Add to `tests/flags_test.rs`:

```rust
use previewf::flags::{extract_flags, format_flags_text, inject_flag, remove_flag, Flag, FlagReport};

#[test]
fn test_remove_flag_removes_single_flag() {
    let content = "This line has <flag:1>Comment: something</flag> a flag.\n";
    let result = remove_flag(content, 1).unwrap();
    assert_eq!(result, "This line has  a flag.\n");
    assert!(extract_flags(&result).is_empty());
}

#[test]
fn test_remove_flag_preserves_other_flags() {
    let content = "Line <flag:1>Comment: first</flag> with <flag:2>Comment: second</flag> two.\n";
    let result = remove_flag(content, 1).unwrap();
    assert!(result.contains("<flag:2>"));
    assert!(!result.contains("<flag:1>"));
}

#[test]
fn test_remove_flag_not_found_returns_error() {
    let content = "No flags here.\n";
    let result = remove_flag(content, 99);
    assert!(result.is_err());
}

#[test]
fn test_remove_flag_preserves_trailing_newline() {
    let content = "Line <flag:1>Comment: test</flag> here.\n";
    let result = remove_flag(content, 1).unwrap();
    assert!(result.ends_with('\n'));
}

#[test]
fn test_remove_flag_no_trailing_newline() {
    let content = "Line <flag:1>Comment: test</flag> here.";
    let result = remove_flag(content, 1).unwrap();
    assert!(!result.ends_with('\n'));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test test_remove_flag -v 2>&1 | head -30`
Expected: compilation error — `remove_flag` not found.

- [ ] **Step 3: Implement `remove_flag()`**

Add to `src/flags.rs`, after the `inject_flag` function:

```rust
/// Remove a flag by ID from the content.
/// Returns the content with the flag tag stripped, preserving surrounding text.
pub fn remove_flag(content: &str, id: u32) -> Result<String, PreviewError> {
    let target = Regex::new(&format!(r"<flag:{id}>Comment:\s*.+?</flag>")).unwrap();
    let mut found = false;

    let result: Vec<String> = content
        .lines()
        .map(|line| {
            if target.is_match(line) {
                found = true;
                target.replace_all(line, "").to_string()
            } else {
                line.to_string()
            }
        })
        .collect();

    if !found {
        return Err(PreviewError::FlagParse {
            line: 0,
            detail: format!("Flag with ID {} not found", id),
        });
    }

    let mut output = result.join("\n");
    if content.ends_with('\n') {
        output.push('\n');
    }
    Ok(output)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test test_remove_flag -v`
Expected: all 5 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add src/flags.rs tests/flags_test.rs
git commit -m "Add remove_flag() for deleting flags by ID"
```

---

### Task 2: Add `update_flag_comment()` to `flags.rs` with tests

**Files:**
- Modify: `src/flags.rs`
- Modify: `tests/flags_test.rs`

- [ ] **Step 1: Write the failing tests**

Add to `tests/flags_test.rs`:

```rust
use previewf::flags::{extract_flags, format_flags_text, inject_flag, remove_flag, update_flag_comment, Flag, FlagReport};

#[test]
fn test_update_flag_comment_changes_comment() {
    let content = "Line <flag:1>Comment: old comment</flag> here.\n";
    let result = update_flag_comment(content, 1, "new comment").unwrap();
    assert!(result.contains("<flag:1>Comment: new comment</flag>"));
    assert!(!result.contains("old comment"));
}

#[test]
fn test_update_flag_comment_sanitizes_input() {
    let content = "Line <flag:1>Comment: safe</flag> here.\n";
    let result = update_flag_comment(content, 1, "<script>alert(1)</script>").unwrap();
    assert!(result.contains("&lt;script&gt;"));
    assert!(!result.contains("<script>"));
}

#[test]
fn test_update_flag_comment_preserves_other_flags() {
    let content = "A <flag:1>Comment: first</flag> B <flag:2>Comment: second</flag>\n";
    let result = update_flag_comment(content, 1, "updated").unwrap();
    assert!(result.contains("<flag:1>Comment: updated</flag>"));
    assert!(result.contains("<flag:2>Comment: second</flag>"));
}

#[test]
fn test_update_flag_comment_not_found_returns_error() {
    let content = "No flags.\n";
    let result = update_flag_comment(content, 99, "anything");
    assert!(result.is_err());
}

#[test]
fn test_update_flag_comment_preserves_trailing_newline() {
    let content = "Line <flag:1>Comment: old</flag> here.\n";
    let result = update_flag_comment(content, 1, "new").unwrap();
    assert!(result.ends_with('\n'));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test test_update_flag_comment -v 2>&1 | head -30`
Expected: compilation error — `update_flag_comment` not found.

- [ ] **Step 3: Implement `update_flag_comment()`**

Add to `src/flags.rs`, after `remove_flag`:

```rust
/// Update the comment of an existing flag by ID.
/// The new comment is sanitized before insertion.
pub fn update_flag_comment(
    content: &str,
    id: u32,
    new_comment: &str,
) -> Result<String, PreviewError> {
    let target = Regex::new(&format!(r"<flag:{id}>Comment:\s*.+?</flag>")).unwrap();
    let sanitized = sanitize_comment(new_comment);
    let replacement = format!("<flag:{id}>Comment: {sanitized}</flag>");
    let mut found = false;

    let result: Vec<String> = content
        .lines()
        .map(|line| {
            if target.is_match(line) {
                found = true;
                target.replace_all(line, replacement.as_str()).to_string()
            } else {
                line.to_string()
            }
        })
        .collect();

    if !found {
        return Err(PreviewError::FlagParse {
            line: 0,
            detail: format!("Flag with ID {} not found", id),
        });
    }

    let mut output = result.join("\n");
    if content.ends_with('\n') {
        output.push('\n');
    }
    Ok(output)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test test_update_flag_comment -v`
Expected: all 5 tests PASS.

- [ ] **Step 5: Run all tests**

Run: `cargo test`
Expected: all tests pass (existing + new).

- [ ] **Step 6: Commit**

```bash
git add src/flags.rs tests/flags_test.rs
git commit -m "Add update_flag_comment() for editing flag comments"
```

---

### Task 3: Add DELETE and PUT route handlers in `server.rs`

**Files:**
- Modify: `src/server.rs`

- [ ] **Step 1: Add imports**

In `src/server.rs`, update the routing import and flags import:

```rust
use axum::routing::{delete, get, post, put};
```

```rust
use crate::flags::{extract_flags, inject_flag, remove_flag, update_flag_comment, FlagReport};
```

- [ ] **Step 2: Add the `UpdateFlagRequest` struct**

Add near the existing `FlagRequest` struct (around line 694):

```rust
/// Request body for flag comment update.
#[derive(Deserialize)]
struct UpdateFlagRequest {
    comment: String,
}
```

- [ ] **Step 3: Add the `delete_flag_handler` function**

Add after `flag_handler`:

```rust
/// `DELETE /flag/{id}/{*filepath}` — remove a flag by ID.
async fn delete_flag_handler(
    State(state): State<AppState>,
    AxumPath((id, filepath)): AxumPath<(u32, String)>,
) -> Response {
    if !is_markdown(&filepath) {
        return (
            StatusCode::BAD_REQUEST,
            "Flags can only be removed from markdown files",
        )
            .into_response();
    }

    let full_path = match resolve_path(state.config.path(), &filepath) {
        Some(p) => p,
        None => return not_found_response(&filepath),
    };

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

    match remove_flag(&content, id) {
        Ok(new_content) => match std::fs::write(&full_path, &new_content) {
            Ok(_) => (StatusCode::OK, "Flag removed").into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        },
        Err(e) => (StatusCode::NOT_FOUND, e.to_string()).into_response(),
    }
}
```

- [ ] **Step 4: Add the `update_flag_handler` function**

Add after `delete_flag_handler`:

```rust
/// `PUT /flag/{id}/{*filepath}` — update a flag's comment.
async fn update_flag_handler(
    State(state): State<AppState>,
    AxumPath((id, filepath)): AxumPath<(u32, String)>,
    axum::Json(body): axum::Json<UpdateFlagRequest>,
) -> Response {
    if !is_markdown(&filepath) {
        return (
            StatusCode::BAD_REQUEST,
            "Flags can only be edited in markdown files",
        )
            .into_response();
    }

    if body.comment.trim().is_empty() {
        return (StatusCode::BAD_REQUEST, "Comment cannot be empty").into_response();
    }

    let full_path = match resolve_path(state.config.path(), &filepath) {
        Some(p) => p,
        None => return not_found_response(&filepath),
    };

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

    match update_flag_comment(&content, id, &body.comment) {
        Ok(new_content) => match std::fs::write(&full_path, &new_content) {
            Ok(_) => (StatusCode::OK, "Flag updated").into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        },
        Err(e) => (StatusCode::NOT_FOUND, e.to_string()).into_response(),
    }
}
```

- [ ] **Step 5: Register routes in `create_router_with_reload`**

Add two new routes after the existing `.route("/flag/{*filepath}", post(flag_handler))` line:

```rust
        .route("/flag/{*filepath}", post(flag_handler))
        .route("/flag/{id}/{*filepath}", delete(delete_flag_handler))
        .route("/flag/{id}/{*filepath}", put(update_flag_handler))
```

- [ ] **Step 6: Build to verify compilation**

Run: `cargo build`
Expected: compiles without errors.

- [ ] **Step 7: Commit**

```bash
git add src/server.rs
git commit -m "Add DELETE and PUT handlers for flag remove/edit"
```

---

### Task 4: Add integration tests for DELETE and PUT endpoints

**Files:**
- Modify: `tests/server_test.rs`

- [ ] **Step 1: Write DELETE endpoint tests**

Add to `tests/server_test.rs`:

```rust
// --- Flag DELETE endpoint ---

#[tokio::test]
async fn test_flag_delete_removes_flag() {
    let dir = tempfile::TempDir::new().unwrap();
    let file_path = dir.path().join("test.md");
    std::fs::write(&file_path, "Hello <flag:1>Comment: remove me</flag> world.\n").unwrap();

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
                .method("DELETE")
                .uri("/flag/1/test.md")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let content = std::fs::read_to_string(&file_path).unwrap();
    assert!(!content.contains("<flag:1>"), "Flag should be removed from file");
    assert!(content.contains("Hello"), "Surrounding text should be preserved");
}

#[tokio::test]
async fn test_flag_delete_not_found_returns_404() {
    let dir = tempfile::TempDir::new().unwrap();
    let file_path = dir.path().join("test.md");
    std::fs::write(&file_path, "No flags here.\n").unwrap();

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
                .method("DELETE")
                .uri("/flag/99/test.md")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_flag_delete_non_markdown_returns_400() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::write(dir.path().join("test.html"), "<html></html>").unwrap();

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
                .method("DELETE")
                .uri("/flag/1/test.html")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
```

- [ ] **Step 2: Write PUT endpoint tests**

Add to `tests/server_test.rs`:

```rust
// --- Flag PUT endpoint ---

#[tokio::test]
async fn test_flag_put_updates_comment() {
    let dir = tempfile::TempDir::new().unwrap();
    let file_path = dir.path().join("test.md");
    std::fs::write(&file_path, "Line <flag:1>Comment: old</flag> here.\n").unwrap();

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
                .method("PUT")
                .uri("/flag/1/test.md")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({ "comment": "updated comment" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let content = std::fs::read_to_string(&file_path).unwrap();
    assert!(content.contains("updated comment"), "Comment should be updated");
    assert!(!content.contains("old"), "Old comment should be replaced");
}

#[tokio::test]
async fn test_flag_put_not_found_returns_404() {
    let dir = tempfile::TempDir::new().unwrap();
    let file_path = dir.path().join("test.md");
    std::fs::write(&file_path, "No flags.\n").unwrap();

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
                .method("PUT")
                .uri("/flag/99/test.md")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({ "comment": "anything" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_flag_put_empty_comment_returns_400() {
    let dir = tempfile::TempDir::new().unwrap();
    let file_path = dir.path().join("test.md");
    std::fs::write(&file_path, "Line <flag:1>Comment: old</flag> here.\n").unwrap();

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
                .method("PUT")
                .uri("/flag/1/test.md")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({ "comment": "  " }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
```

- [ ] **Step 3: Run all tests**

Run: `cargo test`
Expected: all tests pass (existing + new DELETE/PUT tests).

- [ ] **Step 4: Commit**

```bash
git add tests/server_test.rs
git commit -m "Add integration tests for flag DELETE and PUT endpoints"
```

---

### Task 5: Diagnose and fix the sidebar population bug

**Files:**
- Modify: `src/markdown.rs` (likely)

- [ ] **Step 1: Reproduce the bug**

Run previewf against the test fixtures and check the HTML output:

```bash
cargo run -- serve tests/fixtures --port 4567
```

Open `http://localhost:4567/view/flagged.md` in a browser. Check:
1. Does the rendered HTML contain `<span class="flag" data-flag-id="1">`?
2. Open the browser console and run: `document.querySelectorAll('.flag[data-flag-id]').length`
3. Check if `initFlagSidebar()` is running.

- [ ] **Step 2: Check the rendering pipeline**

The issue is likely that comrak's markdown rendering with `unsafe_ = true` processes the `<flag:N>` tags before `render_flags()` gets to them. Comrak may be escaping or stripping the custom tags.

Verify by checking if the flag tags survive comrak processing. The current flow is:
1. `comrak::markdown_to_html(content)` — may mangle `<flag:N>` tags
2. `render_flags()` runs on the post-comrak HTML — but if tags were mangled, regex won't match

- [ ] **Step 3: Implement the fix**

The fix is to convert flag tags to HTML spans **before** passing content to comrak. Since `unsafe_ = true` is set, comrak passes raw HTML through untouched.

In `src/markdown.rs`, change `render_html` to convert flags before comrak:

```rust
pub fn render_html(content: &str) -> String {
    // Convert flag tags to HTML spans BEFORE comrak processes the markdown.
    // With unsafe_=true, comrak passes raw HTML through untouched.
    let content = render_flags(content);

    let mut options = comrak::Options::default();
    options.extension.strikethrough = true;
    options.extension.table = true;
    options.extension.autolink = true;
    options.extension.tasklist = true;
    options.extension.footnotes = true;
    options.render.unsafe_ = true;

    let html = comrak::markdown_to_html(&content, &options);

    let html = highlight_code_blocks(&html);
    render_diff_blocks(&html)
}
```

This moves `render_flags()` to run on the raw markdown (before comrak), converting `<flag:N>Comment: text</flag>` into `<span class="flag" ...>` HTML. Since `unsafe_ = true`, comrak will pass these HTML spans through to the output.

- [ ] **Step 4: Run tests**

Run: `cargo test`
Expected: all tests pass. The existing `test_render_flags_basic` and `test_render_flags_escapes_html_in_comment` tests in `markdown.rs` test `render_flags()` directly, so they still work.

- [ ] **Step 5: Manual verification**

Run: `cargo run -- serve tests/fixtures --port 4567`
Open `http://localhost:4567/view/flagged.md` and verify:
1. Flags appear as highlighted inline markers in the document.
2. The sidebar (now API-driven from Task 6) will show flags.

- [ ] **Step 6: Commit**

```bash
git add src/markdown.rs
git commit -m "Fix flag rendering: convert tags before comrak processing"
```

---

### Task 6: Rework JavaScript sidebar with `refreshFlagSidebar()` and edit/delete actions

**Files:**
- Modify: `assets/app.js`

- [ ] **Step 1: Add `refreshFlagSidebar()` function**

Add after the `initNavSidebar()` section (around line 258) and before the `initFlagSidebar()` section:

```javascript
    function refreshFlagSidebar() {
        if (!currentFilepath) return;

        var flagList = document.getElementById('flag-list');
        var flagCountEl = document.getElementById('flag-count');
        if (!flagList) return;

        var url = '/flags/' + currentFilepath.split('/').map(encodeURIComponent).join('/');

        fetch(url)
            .then(function (r) {
                if (!r.ok) throw new Error('Flags API returned ' + r.status);
                return r.json();
            })
            .then(function (report) {
                // Clear existing items using safe DOM methods
                while (flagList.firstChild) {
                    flagList.removeChild(flagList.firstChild);
                }

                var flags = report.flags || [];

                // Update badge
                if (flagCountEl) {
                    flagCountEl.textContent = String(flags.length);
                }

                if (flags.length === 0) {
                    var emptyMsg = document.createElement('p');
                    emptyMsg.className = 'flag-list-empty';
                    emptyMsg.textContent = 'No flags in this document.';
                    emptyMsg.style.fontSize = '0.82rem';
                    emptyMsg.style.color = 'var(--text-muted)';
                    emptyMsg.style.fontFamily = "'DM Sans', system-ui, sans-serif";
                    flagList.appendChild(emptyMsg);
                    return;
                }

                for (var i = 0; i < flags.length; i++) {
                    var item = createFlagItemFromData(flags[i]);
                    flagList.appendChild(item);
                }
            })
            .catch(function (err) {
                console.warn('Failed to refresh flag sidebar:', err);
            });
    }
```

- [ ] **Step 2: Add `createFlagItemFromData()` with edit/delete buttons**

Add after `refreshFlagSidebar`:

```javascript
    function createFlagItemFromData(flag) {
        var item = document.createElement('div');
        item.className = 'flag-item';
        item.setAttribute('data-flag-id', flag.id);

        // Header
        var header = document.createElement('div');
        header.className = 'flag-item-header';
        var idLabel = document.createElement('span');
        idLabel.className = 'flag-item-id';
        idLabel.textContent = 'Flag #' + flag.id;
        header.appendChild(idLabel);
        item.appendChild(header);

        // Comment
        var commentEl = document.createElement('div');
        commentEl.className = 'flag-item-comment';
        commentEl.textContent = flag.comment;
        item.appendChild(commentEl);

        // Actions row
        var actions = document.createElement('div');
        actions.className = 'flag-item-actions';

        var editBtn = document.createElement('button');
        editBtn.className = 'flag-action-btn flag-action-btn-edit';
        editBtn.type = 'button';
        editBtn.textContent = 'Edit';
        actions.appendChild(editBtn);

        var deleteBtn = document.createElement('button');
        deleteBtn.className = 'flag-action-btn flag-action-btn-delete';
        deleteBtn.type = 'button';
        deleteBtn.textContent = 'Delete';
        actions.appendChild(deleteBtn);

        item.appendChild(actions);

        // Click item -> scroll to flag in document
        item.addEventListener('click', function (e) {
            if (e.target === editBtn || e.target === deleteBtn) return;
            var flagEl = document.querySelector('.flag[data-flag-id="' + flag.id + '"]');
            if (flagEl) {
                flagEl.scrollIntoView({ behavior: 'smooth', block: 'center' });
                flagEl.classList.add('flag-highlight');
                setTimeout(function () {
                    flagEl.classList.remove('flag-highlight');
                }, 2000);
            }
        });

        // Delete handler
        deleteBtn.addEventListener('click', function () {
            var url = '/flag/' + flag.id + '/' + currentFilepath.split('/').map(encodeURIComponent).join('/');
            fetch(url, { method: 'DELETE' })
                .then(function (r) {
                    if (!r.ok) throw new Error('Delete failed: ' + r.status);
                    refreshFlagSidebar();
                })
                .catch(function (err) {
                    console.warn('Failed to delete flag:', err);
                });
        });

        // Edit handler
        editBtn.addEventListener('click', function () {
            enterEditMode(item, flag, commentEl, actions);
        });

        return item;
    }
```

- [ ] **Step 3: Add `enterEditMode()` function**

```javascript
    function enterEditMode(item, flag, commentEl, actionsEl) {
        // Hide comment and actions
        commentEl.style.display = 'none';
        actionsEl.style.display = 'none';

        // Create edit input
        var editContainer = document.createElement('div');
        editContainer.className = 'flag-edit-container';

        var input = document.createElement('input');
        input.className = 'flag-edit-input';
        input.type = 'text';
        input.value = flag.comment;
        editContainer.appendChild(input);

        var editActions = document.createElement('div');
        editActions.className = 'flag-edit-actions';

        var saveBtn = document.createElement('button');
        saveBtn.className = 'flag-action-btn flag-action-btn-save';
        saveBtn.type = 'button';
        saveBtn.textContent = 'Save';
        editActions.appendChild(saveBtn);

        var cancelBtn = document.createElement('button');
        cancelBtn.className = 'flag-action-btn flag-action-btn-cancel';
        cancelBtn.type = 'button';
        cancelBtn.textContent = 'Cancel';
        editActions.appendChild(cancelBtn);

        editContainer.appendChild(editActions);
        item.appendChild(editContainer);

        input.focus();
        input.select();

        function exitEditMode() {
            commentEl.style.display = '';
            actionsEl.style.display = '';
            if (editContainer.parentNode) {
                editContainer.parentNode.removeChild(editContainer);
            }
        }

        function saveEdit() {
            var newComment = input.value.trim();
            if (!newComment) {
                exitEditMode();
                return;
            }
            var url = '/flag/' + flag.id + '/' + currentFilepath.split('/').map(encodeURIComponent).join('/');
            fetch(url, {
                method: 'PUT',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ comment: newComment })
            })
            .then(function (r) {
                if (!r.ok) throw new Error('Update failed: ' + r.status);
                refreshFlagSidebar();
            })
            .catch(function (err) {
                console.warn('Failed to update flag:', err);
                exitEditMode();
            });
        }

        saveBtn.addEventListener('click', saveEdit);
        cancelBtn.addEventListener('click', exitEditMode);
        input.addEventListener('keydown', function (e) {
            if (e.key === 'Enter') {
                e.preventDefault();
                saveEdit();
            }
            if (e.key === 'Escape') {
                exitEditMode();
            }
        });
    }
```

- [ ] **Step 4: Rework `initFlagSidebar()` to use API-driven refresh**

Replace the entire `initFlagSidebar()` function with:

```javascript
    function initFlagSidebar() {
        refreshFlagSidebar();
    }
```

The `currentFilepath` is already set by `initFlagToolbar()` which extracts it from breadcrumbs. Since `initFlagSidebar()` is called before `initFlagToolbar()` in the DOMContentLoaded handler, move the filepath extraction into `initFlagSidebar()`:

```javascript
    function initFlagSidebar() {
        // Extract filepath from breadcrumb for API calls
        var breadcrumbCurrent = document.querySelector('.breadcrumb-current');
        var breadcrumbLinks = document.querySelectorAll('.breadcrumb-link');
        if (breadcrumbCurrent) {
            var parts = [];
            for (var i = 1; i < breadcrumbLinks.length; i++) {
                parts.push(breadcrumbLinks[i].textContent.trim());
            }
            parts.push(breadcrumbCurrent.textContent.trim());
            currentFilepath = parts.join('/');
        }

        refreshFlagSidebar();
    }
```

And remove the duplicate filepath extraction from `initFlagToolbar()`.

- [ ] **Step 5: Update `submitFlag()` — replace `window.location.reload()` with `refreshFlagSidebar()`**

In the `submitFlag` function, replace:

```javascript
                window.location.reload();
```

with:

```javascript
                refreshFlagSidebar();
```

- [ ] **Step 6: Remove old `createFlagItem()` function**

Delete the old `createFlagItem(flagId, comment, flagElement)` function and the old bidirectional navigation setup code that was in the previous `initFlagSidebar()`. These are fully replaced by `createFlagItemFromData()` and `refreshFlagSidebar()`.

- [ ] **Step 7: Build and verify**

Run: `cargo build`
Expected: compiles (assets are embedded at build time).

- [ ] **Step 8: Commit**

```bash
git add assets/app.js
git commit -m "Rework flag sidebar with API-driven refresh, edit, and delete"
```

---

### Task 7: Add CSS styles for edit/delete buttons and inline edit mode

**Files:**
- Modify: `assets/style.css`

- [ ] **Step 1: Add action button and edit mode styles**

Add in the Flag Sidebar section of `style.css`:

```css
/* --------------------------------------------------------------------------
   Flag Item Actions
   -------------------------------------------------------------------------- */
.flag-item-actions {
    display: flex;
    gap: 0.5em;
    margin-top: 0.4em;
}

.flag-action-btn {
    font-family: 'DM Sans', system-ui, sans-serif;
    font-size: 0.72rem;
    font-weight: 600;
    padding: 0.2em 0.5em;
    border: 1px solid var(--border-color);
    border-radius: 4px;
    background: var(--bg);
    color: var(--text-muted);
    cursor: pointer;
    transition: background-color 200ms ease, border-color 200ms ease, color 200ms ease;
}

.flag-action-btn:hover {
    background-color: var(--bg-surface);
    border-color: var(--accent);
    color: var(--accent);
}

.flag-action-btn-delete:hover {
    border-color: #EF4444;
    color: #EF4444;
}

[data-theme="dark"] .flag-action-btn-delete:hover {
    border-color: #F87171;
    color: #F87171;
}

.flag-action-btn-save:hover {
    border-color: var(--accent);
    color: var(--accent);
}

.flag-edit-container {
    margin-top: 0.4em;
}

.flag-edit-input {
    width: 100%;
    padding: 0.35em 0.5em;
    font-family: 'DM Sans', system-ui, sans-serif;
    font-size: 0.82rem;
    border: 1px solid var(--border-color);
    border-radius: 4px;
    background: var(--bg);
    color: var(--text);
    box-sizing: border-box;
    transition: border-color 200ms ease;
}

.flag-edit-input:focus {
    outline: none;
    border-color: var(--accent);
}

.flag-edit-actions {
    display: flex;
    gap: 0.5em;
    margin-top: 0.3em;
}
```

- [ ] **Step 2: Build and verify**

Run: `cargo build`
Expected: compiles.

- [ ] **Step 3: Commit**

```bash
git add assets/style.css
git commit -m "Add CSS for flag edit/delete buttons and inline edit mode"
```

---

### Task 8: Manual end-to-end testing and cleanup

- [ ] **Step 1: Run full test suite**

Run: `cargo test`
Expected: all tests pass.

- [ ] **Step 2: Run clippy**

Run: `cargo clippy`
Expected: no warnings.

- [ ] **Step 3: Start the server and test**

Run: `cargo run -- serve tests/fixtures --port 4567`

Open `http://localhost:4567/view/flagged.md` and verify:
1. Flags appear as inline markers in the document.
2. Sidebar shows flag items with "Edit" and "Delete" buttons.
3. Badge shows correct count.
4. Clicking a sidebar item scrolls to the flag.
5. Clicking "Delete" removes the flag from sidebar immediately, document updates via live reload.
6. Clicking "Edit" shows inline input, Save/Cancel work, Enter/Escape work.
7. Adding a flag via text highlight updates sidebar without page reload.

- [ ] **Step 4: Final commit if any fixes needed**

```bash
git add -A
git commit -m "Fix issues found during manual testing"
```
