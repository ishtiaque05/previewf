# Collapsible Sidebar & Flag Labels Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add collapsible sidebar and label-based flag categorization (Bug, Todo, Question, Note, Style, Comment, Custom) with colored badges.

**Architecture:** The label replaces the hardcoded `Comment:` prefix in the flag syntax. `FLAG_RE` gains a capture group for the label. The `Flag` struct gets a `label` field. Frontend adds a label picker to the toolbar and edit mode, plus sidebar collapse toggle with localStorage persistence.

**Tech Stack:** Rust (flags/server), vanilla JS, CSS

---

## File Map

| File | Action | Responsibility |
|------|--------|---------------|
| `src/flags.rs` | Modify | Regex, Flag struct, extract/inject/update/format functions |
| `src/markdown.rs` | Modify | `render_flags` — add label to HTML span output |
| `src/terminal.rs` | Modify | `prepare_flags_for_terminal` — include label in terminal output |
| `src/server.rs` | Modify | `FlagRequest`, `UpdateFlagRequest`, `flag_handler`, `update_flag_handler` |
| `assets/style.css` | Modify | Label badge colors, collapsed sidebar styles |
| `assets/app.js` | Modify | Label picker, sidebar collapse toggle, label display |
| `assets/document.html` | Modify | Add collapse toggle button to sidebar header |
| `tests/flags_test.rs` | Modify | Update all existing tests + add label-specific tests |
| `tests/server_test.rs` | Modify | Update POST/PUT tests for label field |
| `tests/terminal_test.rs` | Modify | Update terminal rendering tests for label |
| `tests/markdown_test.rs` | Modify | Update flag rendering test for label |

---

### Task 1: Update FLAG_RE and Flag struct

**Files:**
- Modify: `src/flags.rs:8-17`
- Test: `tests/flags_test.rs`

- [ ] **Step 1: Update existing test imports and add label to Flag assertions**

In `tests/flags_test.rs`, update the first test to expect the new `label` field. Change `test_extract_flags_from_flagged_file`:

```rust
#[test]
fn test_extract_flags_from_flagged_file() {
    let content = std::fs::read_to_string("tests/fixtures/flagged.md").unwrap();
    let flags = extract_flags(&content);
    assert!(!flags.is_empty());
    // Existing flags use "Comment:" prefix, so label should be "Comment"
    assert_eq!(flags[0].label, "Comment");
}
```

Also add a new test for a non-Comment label:

```rust
#[test]
fn test_extract_flags_parses_label() {
    let content = "Line with <flag:1>Bug: something broken</flag> here.";
    let flags = extract_flags(content);
    assert_eq!(flags.len(), 1);
    assert_eq!(flags[0].id, 1);
    assert_eq!(flags[0].label, "Bug");
    assert_eq!(flags[0].comment, "something broken");
}

#[test]
fn test_extract_flags_custom_label() {
    let content = "Line <flag:1>Perf: slow query</flag> here.";
    let flags = extract_flags(content);
    assert_eq!(flags.len(), 1);
    assert_eq!(flags[0].label, "Perf");
    assert_eq!(flags[0].comment, "slow query");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd /Users/syed/Thinkific/workspace/previewf-flag-crud && cargo test test_extract_flags_parses_label test_extract_flags_custom_label -- --nocapture 2>&1`

Expected: compilation error — `Flag` has no field `label`, and regex doesn't capture labels.

- [ ] **Step 3: Update FLAG_RE, Flag struct, and extract_flags**

In `src/flags.rs`, update:

```rust
pub static FLAG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<flag:(\d+)>(\w+):\s*(.+?)</flag>").unwrap());
```

Add `label` field to `Flag`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Flag {
    pub id: u32,
    pub line: usize,
    pub context: String,
    pub label: String,
    pub comment: String,
}
```

Update `extract_flags` to parse the label (group 2) and comment (now group 3):

```rust
pub fn extract_flags(content: &str) -> Vec<Flag> {
    let mut flags = Vec::new();

    for (line_num, line) in content.lines().enumerate() {
        for cap in FLAG_RE.captures_iter(line) {
            let id: u32 = match cap[1].parse() {
                Ok(id) if id > 0 => id,
                _ => continue,
            };
            let label = cap[2].to_string();
            let comment = cap[3].to_string();
            let context = FLAG_RE.replace_all(line, "").to_string();

            flags.push(Flag {
                id,
                line: line_num + 1,
                context,
                label,
                comment,
            });
        }
    }

    flags
}
```

- [ ] **Step 4: Fix all existing tests that construct Flag literals**

Any test that constructs `Flag { id, line, context, comment }` needs the `label` field added. Search `tests/flags_test.rs` for `Flag {` and add `label: "Comment".to_string(),` to each. Key tests to update:

- `test_extract_flags_from_flagged_file` — assert `flags[0].label == "Comment"`
- `test_extract_flags_from_clean_file` — no change (returns empty vec)
- `test_flag_report_json` — add `label: "Comment".to_string()` to the Flag literal
- `test_format_flags_text_with_flags` — add `label: "Comment".to_string()` to the Flag literal
- `test_format_flags_text_empty` — no Flag literal, no change

- [ ] **Step 5: Run tests to verify they pass**

Run: `cd /Users/syed/Thinkific/workspace/previewf-flag-crud && cargo test 2>&1`

Expected: some tests may still fail due to `inject_flag`, `remove_flag`, `update_flag_comment` still using `Comment:` hardcoded in their regexes. That's expected — we fix those in subsequent tasks.

- [ ] **Step 6: Commit**

```bash
cd /Users/syed/Thinkific/workspace/previewf-flag-crud
git add src/flags.rs tests/flags_test.rs
git commit -m "Add label field to Flag struct and update regex to capture labels"
```

---

### Task 2: Update inject_flag to accept a label parameter

**Files:**
- Modify: `src/flags.rs:74-110`
- Test: `tests/flags_test.rs`

- [ ] **Step 1: Add tests for labeled injection**

```rust
#[test]
fn test_inject_flag_with_label() {
    let content = "Hello world\nSecond line\n";
    let result = inject_flag(content, 1, "something broken", "Bug").unwrap();
    assert!(result.contains("<flag:1>Bug: something broken</flag>"));
}

#[test]
fn test_inject_flag_default_comment_label() {
    let content = "Hello world\n";
    let result = inject_flag(content, 1, "general note", "Comment").unwrap();
    assert!(result.contains("<flag:1>Comment: general note</flag>"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd /Users/syed/Thinkific/workspace/previewf-flag-crud && cargo test test_inject_flag_with_label test_inject_flag_default_comment_label 2>&1`

Expected: compilation error — `inject_flag` doesn't accept a `label` parameter.

- [ ] **Step 3: Update inject_flag signature and implementation**

In `src/flags.rs`, change `inject_flag`:

```rust
pub fn inject_flag(content: &str, line: usize, comment: &str, label: &str) -> Result<String, PreviewError> {
    let lines: Vec<&str> = content.lines().collect();

    if line == 0 || line > lines.len() {
        return Err(PreviewError::FlagParse {
            line,
            detail: format!(
                "Line {} is out of range (file has {} lines)",
                line,
                lines.len()
            ),
        });
    }

    let target_line = lines[line - 1];
    if is_code_fence(target_line) {
        return Err(PreviewError::FlagParse {
            line,
            detail: "Cannot inject flag into a code fence delimiter".to_string(),
        });
    }

    let sanitized = sanitize_comment(comment);
    let next_id = next_flag_id(content);
    let flag_tag = format!(" <flag:{next_id}>{label}: {sanitized}</flag>");

    let mut result: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
    result[line - 1].push_str(&flag_tag);

    let mut output = result.join("\n");
    if content.ends_with('\n') {
        output.push('\n');
    }

    Ok(output)
}
```

- [ ] **Step 4: Fix all existing callers of inject_flag**

Update every call to `inject_flag` to pass `"Comment"` as the label:

- `src/server.rs` in `flag_handler`: `inject_flag(&content, line, &body.comment)` becomes `inject_flag(&content, line, &body.comment, "Comment")` (will be updated to use body.label in Task 5)
- All tests in `tests/flags_test.rs` that call `inject_flag(content, line, comment)` — add `"Comment"` as the 4th argument.

- [ ] **Step 5: Run all tests**

Run: `cd /Users/syed/Thinkific/workspace/previewf-flag-crud && cargo test 2>&1`

Expected: PASS (all tests should pass now)

- [ ] **Step 6: Commit**

```bash
cd /Users/syed/Thinkific/workspace/previewf-flag-crud
git add src/flags.rs src/server.rs tests/flags_test.rs
git commit -m "Add label parameter to inject_flag"
```

---

### Task 3: Update remove_flag and update_flag_comment for label-aware regex

**Files:**
- Modify: `src/flags.rs:112-192`
- Test: `tests/flags_test.rs`

- [ ] **Step 1: Add tests**

```rust
#[test]
fn test_remove_flag_with_non_comment_label() {
    let content = "Line <flag:1>Bug: broken thing</flag> here.\n";
    let result = remove_flag(content, 1).unwrap();
    assert!(!result.contains("<flag:1>"));
    assert!(result.contains("Line"));
}

#[test]
fn test_update_flag_comment_with_label() {
    let content = "Line <flag:1>Bug: old text</flag> here.\n";
    let result = update_flag_comment(content, 1, "new text", None).unwrap();
    assert!(result.contains("<flag:1>Bug: new text</flag>"));
}

#[test]
fn test_update_flag_changes_label() {
    let content = "Line <flag:1>Comment: some note</flag> here.\n";
    let result = update_flag_comment(content, 1, "some note", Some("Bug")).unwrap();
    assert!(result.contains("<flag:1>Bug: some note</flag>"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd /Users/syed/Thinkific/workspace/previewf-flag-crud && cargo test test_remove_flag_with_non_comment_label test_update_flag_comment_with_label test_update_flag_changes_label 2>&1`

Expected: `remove_flag` fails because its regex still uses `Comment:` hardcoded. `update_flag_comment` signature mismatch.

- [ ] **Step 3: Update remove_flag to use label-aware regex**

In `src/flags.rs`, change `remove_flag`:

```rust
pub fn remove_flag(content: &str, id: u32) -> Result<String, PreviewError> {
    let target = Regex::new(&format!(r"<flag:{id}>\w+:\s*.+?</flag>")).map_err(|e| {
        PreviewError::FlagParse {
            line: 0,
            detail: format!("Invalid flag regex for ID {id}: {e}"),
        }
    })?;
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

- [ ] **Step 4: Update update_flag_comment to accept optional label**

```rust
pub fn update_flag_comment(
    content: &str,
    id: u32,
    new_comment: &str,
    new_label: Option<&str>,
) -> Result<String, PreviewError> {
    let target = Regex::new(&format!(r"<flag:{id}>(\w+):\s*.+?</flag>")).map_err(|e| {
        PreviewError::FlagParse {
            line: 0,
            detail: format!("Invalid flag regex for ID {id}: {e}"),
        }
    })?;
    let sanitized = sanitize_comment(new_comment);
    let mut found = false;

    let result: Vec<String> = content
        .lines()
        .map(|line| {
            if target.is_match(line) {
                found = true;
                target
                    .replace_all(line, |caps: &regex::Captures| {
                        let label = new_label.unwrap_or(&caps[1]);
                        format!("<flag:{id}>{label}: {sanitized}</flag>")
                    })
                    .to_string()
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

- [ ] **Step 5: Fix all callers of update_flag_comment**

Update every call to `update_flag_comment` to pass `None` for the label:

- `src/server.rs` in `update_flag_handler`: `update_flag_comment(&content, id, &body.comment, None)` (will be updated to use body.label in Task 5)
- All tests in `tests/flags_test.rs` that call `update_flag_comment(content, id, comment)` — add `None` as the 4th argument.

- [ ] **Step 6: Run all tests**

Run: `cd /Users/syed/Thinkific/workspace/previewf-flag-crud && cargo test 2>&1`

Expected: PASS

- [ ] **Step 7: Commit**

```bash
cd /Users/syed/Thinkific/workspace/previewf-flag-crud
git add src/flags.rs src/server.rs tests/flags_test.rs
git commit -m "Make remove_flag and update_flag_comment label-aware"
```

---

### Task 4: Update render_flags, terminal output, and format_flags_text

**Files:**
- Modify: `src/markdown.rs:99-112`
- Modify: `src/terminal.rs:13-17`
- Modify: `src/flags.rs:194-211` (format_flags_text)
- Test: `tests/markdown_test.rs`, `tests/terminal_test.rs`

- [ ] **Step 1: Update markdown render_flags test**

In `tests/markdown_test.rs`, add a label-specific test:

```rust
#[test]
fn test_render_flag_includes_label() {
    let html = render_html("<flag:1>Bug: broken thing</flag>");
    assert!(html.contains("data-flag-id=\"1\""));
    assert!(html.contains("data-flag-label=\"Bug\""));
    assert!(html.contains("broken thing"));
}
```

- [ ] **Step 2: Update render_flags in markdown.rs**

In `src/markdown.rs`, update `render_flags` — now group 2 is the label, group 3 is the comment:

```rust
fn render_flags(html: &str) -> String {
    FLAG_RE
        .replace_all(html, |caps: &regex::Captures| {
            let id = &caps[1];
            let label = &caps[2];
            let comment = html::escape(caps[3].trim());
            let label_lower = label.to_lowercase();
            format!(
                "<span class=\"flag\" data-flag-id=\"{id}\" data-flag-label=\"{label}\">\
                 <span class=\"flag-marker\">#{id}</span>\
                 <span class=\"flag-label\" data-label=\"{label_lower}\">{label}</span>\
                 <span class=\"flag-comment\">{comment}</span>\
                 </span>"
            )
        })
        .into_owned()
}
```

- [ ] **Step 3: Update prepare_flags_for_terminal**

In `src/terminal.rs`, update the replacement — group 2 is now label, group 3 is comment:

```rust
fn prepare_flags_for_terminal(content: &str) -> String {
    FLAG_RE
        .replace_all(content, "**[FLAG #$1 $2:** $3**]**")
        .into_owned()
}
```

- [ ] **Step 4: Update format_flags_text**

In `src/flags.rs`, update `format_flags_text`:

```rust
pub fn format_flags_text(report: &FlagReport) -> String {
    let mut output = format!("Flags in {}:\n\n", report.file);

    if report.flags.is_empty() {
        output.push_str("  No flags found.\n");
        return output;
    }

    for flag in &report.flags {
        output.push_str(&format!(
            "  #{} [{}] (line {}): {}\n    Context: {}\n\n",
            flag.id, flag.label, flag.line, flag.comment, flag.context
        ));
    }

    output
}
```

- [ ] **Step 5: Fix existing terminal and markdown tests**

Update `tests/terminal_test.rs`:
- `test_prepare_flags_basic`: input `"<flag:1>Comment: check this</flag>"` expected output becomes `"**[FLAG #1 Comment:** check this**]**"`
- `test_prepare_flags_multiple`: update expected outputs similarly.

Update `tests/markdown_test.rs`:
- `test_render_flag_tags_preserved`: update assertion for new HTML structure with `data-flag-label`.
- `test_render_flags_escapes_html_in_comment`: input uses `Comment:` prefix, update expected to check `data-flag-label`.

Update `tests/flags_test.rs`:
- `test_format_flags_text_with_flags`: expected output now includes `[Comment]` in format.

- [ ] **Step 6: Run all tests**

Run: `cd /Users/syed/Thinkific/workspace/previewf-flag-crud && cargo test 2>&1`

Expected: PASS

- [ ] **Step 7: Commit**

```bash
cd /Users/syed/Thinkific/workspace/previewf-flag-crud
git add src/markdown.rs src/terminal.rs src/flags.rs tests/markdown_test.rs tests/terminal_test.rs tests/flags_test.rs
git commit -m "Add label to flag rendering, terminal output, and text format"
```

---

### Task 5: Update server handlers for label field

**Files:**
- Modify: `src/server.rs:698-768` (FlagRequest, UpdateFlagRequest, flag_handler, update_flag_handler)
- Test: `tests/server_test.rs`

- [ ] **Step 1: Add tests for label in POST and PUT**

In `tests/server_test.rs`:

```rust
#[tokio::test]
async fn test_flag_post_with_label() {
    let dir = tempfile::TempDir::new().unwrap();
    let file_path = dir.path().join("test.md");
    std::fs::write(&file_path, "Hello world.\n").unwrap();

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
                        "comment": "broken thing",
                        "selected_text": "Hello world.",
                        "label": "Bug"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let content = std::fs::read_to_string(&file_path).unwrap();
    assert!(content.contains("<flag:1>Bug: broken thing</flag>"));
}

#[tokio::test]
async fn test_flag_post_without_label_defaults_to_comment() {
    let dir = tempfile::TempDir::new().unwrap();
    let file_path = dir.path().join("test.md");
    std::fs::write(&file_path, "Hello world.\n").unwrap();

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
                        "comment": "general note",
                        "selected_text": "Hello world."
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let content = std::fs::read_to_string(&file_path).unwrap();
    assert!(content.contains("<flag:1>Comment: general note</flag>"));
}

#[tokio::test]
async fn test_flag_put_changes_label() {
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
                    serde_json::json!({ "comment": "old", "label": "Bug" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let content = std::fs::read_to_string(&file_path).unwrap();
    assert!(content.contains("<flag:1>Bug: old</flag>"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd /Users/syed/Thinkific/workspace/previewf-flag-crud && cargo test test_flag_post_with_label test_flag_post_without_label_defaults_to_comment test_flag_put_changes_label 2>&1`

Expected: FAIL — `FlagRequest` doesn't have a `label` field.

- [ ] **Step 3: Update request structs and handlers**

In `src/server.rs`:

```rust
#[derive(Deserialize)]
struct FlagRequest {
    comment: String,
    selected_text: String,
    #[serde(default = "default_label")]
    label: String,
}

fn default_label() -> String {
    "Comment".to_string()
}

#[derive(Deserialize)]
struct UpdateFlagRequest {
    comment: String,
    label: Option<String>,
}
```

In `flag_handler`, update the `inject_flag` call:

```rust
match inject_flag(&content, line, &body.comment, &body.label) {
```

In `update_flag_handler`, update the `update_flag_comment` call:

```rust
match update_flag_comment(&content, id, &body.comment, body.label.as_deref()) {
```

- [ ] **Step 4: Run all tests**

Run: `cd /Users/syed/Thinkific/workspace/previewf-flag-crud && cargo test 2>&1`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
cd /Users/syed/Thinkific/workspace/previewf-flag-crud
git add src/server.rs tests/server_test.rs
git commit -m "Add label field to flag POST and PUT endpoints"
```

---

### Task 6: Add label badge CSS and collapsible sidebar styles

**Files:**
- Modify: `assets/style.css`
- Modify: `assets/document.html:41-44`

- [ ] **Step 1: Add label color CSS variables and badge styles**

In `assets/style.css`, add after the existing CSS variable definitions in `:root` (around line 2):

```css
:root {
    /* existing variables... */
    --label-comment: #8b949e;
    --label-bug: #ef4444;
    --label-todo: #3b82f6;
    --label-question: #ffa500;
    --label-note: #10b981;
    --label-style: #a855f7;
    --label-custom: #ec4899;
}
```

And in `[data-theme="dark"]` (around line 30):

```css
[data-theme="dark"] {
    /* existing variables... */
    --label-comment: #8b949e;
    --label-bug: #f87171;
    --label-todo: #60a5fa;
    --label-question: #fbbf24;
    --label-note: #34d399;
    --label-style: #c084fc;
    --label-custom: #f472b6;
}
```

Add label badge styles (after the `.flag-item-id` block around line 534):

```css
.flag-label {
    font-size: 0.55rem;
    font-weight: 600;
    padding: 0.1em 0.4em;
    border-radius: 3px;
    text-transform: uppercase;
    letter-spacing: 0.03em;
}

.flag-label[data-label="comment"] {
    background: color-mix(in srgb, var(--label-comment) 20%, transparent);
    color: var(--label-comment);
}
.flag-label[data-label="bug"] {
    background: color-mix(in srgb, var(--label-bug) 20%, transparent);
    color: var(--label-bug);
}
.flag-label[data-label="todo"] {
    background: color-mix(in srgb, var(--label-todo) 20%, transparent);
    color: var(--label-todo);
}
.flag-label[data-label="question"] {
    background: color-mix(in srgb, var(--label-question) 20%, transparent);
    color: var(--label-question);
}
.flag-label[data-label="note"] {
    background: color-mix(in srgb, var(--label-note) 20%, transparent);
    color: var(--label-note);
}
.flag-label[data-label="style"] {
    background: color-mix(in srgb, var(--label-style) 20%, transparent);
    color: var(--label-style);
}
/* Fallback for custom/unknown labels */
.flag-label:not([data-label="comment"]):not([data-label="bug"]):not([data-label="todo"]):not([data-label="question"]):not([data-label="note"]):not([data-label="style"]) {
    background: color-mix(in srgb, var(--label-custom) 20%, transparent);
    color: var(--label-custom);
}
```

- [ ] **Step 2: Add collapsible sidebar CSS**

Add after the existing `.sidebar` block (around line 444):

```css
.sidebar-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 1em;
    padding-bottom: 0.6em;
    border-bottom: 1px solid var(--border-color);
}

.sidebar-header .sidebar-title {
    margin: 0;
    padding: 0;
    border: none;
}

.sidebar-toggle {
    background: none;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    font-size: 0.9rem;
    padding: 0.1em 0.3em;
    border-radius: 3px;
    transition: color 200ms ease, background-color 200ms ease;
}

.sidebar-toggle:hover {
    color: var(--accent);
    background-color: var(--bg-surface);
}

/* Collapsed state */
.sidebar.collapsed {
    width: 36px;
    padding: 0.8em 0;
    display: flex;
    flex-direction: column;
    align-items: center;
}

.sidebar.collapsed .sidebar-header {
    flex-direction: column;
    border: none;
    margin: 0;
    padding: 0;
    gap: 0.5em;
}

.sidebar.collapsed .sidebar-title {
    display: none;
}

.sidebar.collapsed .flag-list {
    display: none;
}

.sidebar-badge {
    background: var(--accent);
    color: var(--bg);
    font-size: 0.55rem;
    font-weight: 700;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
}

.sidebar:not(.collapsed) .sidebar-badge {
    display: none;
}
```

- [ ] **Step 3: Update document.html sidebar structure**

In `assets/document.html`, replace the sidebar `<aside>` block:

```html
<aside class="sidebar" id="sidebar">
    <div class="sidebar-header">
        <h3 class="sidebar-title">Flags</h3>
        <span class="sidebar-badge" id="sidebar-badge">0</span>
        <button class="sidebar-toggle" id="sidebar-toggle" type="button" title="Toggle sidebar">&#8249;</button>
    </div>
    <div class="flag-list" id="flag-list"></div>
</aside>
```

Note: `&#8249;` is the left single angle quotation mark (chevron). The JS toggle will swap between `\u2039` (left/collapse) and `\u203A` (right/expand) using `textContent`.

- [ ] **Step 4: Commit**

```bash
cd /Users/syed/Thinkific/workspace/previewf-flag-crud
git add assets/style.css assets/document.html
git commit -m "Add label badge colors and collapsible sidebar CSS"
```

---

### Task 7: Add sidebar collapse toggle and label display in JS

**Files:**
- Modify: `assets/app.js`

- [ ] **Step 1: Add sidebar collapse toggle logic**

At the top of the IIFE (after the `suppressReload` declarations), add:

```javascript
var SIDEBAR_COLLAPSED_KEY = 'previewf-sidebar-collapsed';

function initSidebarToggle() {
    var sidebar = document.getElementById('sidebar');
    var toggle = document.getElementById('sidebar-toggle');
    if (!sidebar || !toggle) return;

    // Restore persisted state
    if (localStorage.getItem(SIDEBAR_COLLAPSED_KEY) === 'true') {
        sidebar.classList.add('collapsed');
        toggle.textContent = '\u203A';
    }

    toggle.addEventListener('click', function (e) {
        e.stopPropagation();
        var isCollapsed = sidebar.classList.toggle('collapsed');
        toggle.textContent = isCollapsed ? '\u203A' : '\u2039';
        localStorage.setItem(SIDEBAR_COLLAPSED_KEY, isCollapsed ? 'true' : 'false');
    });

    // Clicking anywhere on the collapsed strip expands
    sidebar.addEventListener('click', function (e) {
        if (sidebar.classList.contains('collapsed') && e.target !== toggle) {
            sidebar.classList.remove('collapsed');
            toggle.textContent = '\u2039';
            localStorage.setItem(SIDEBAR_COLLAPSED_KEY, 'false');
        }
    });
}
```

Call `initSidebarToggle()` in the `DOMContentLoaded` handler alongside the other init functions.

- [ ] **Step 2: Update refreshFlagSidebar to show label badges**

In the `refreshFlagSidebar` function, update the badge to also update the collapsed sidebar badge:

```javascript
// Update badge (both header and collapsed strip)
if (flagCountEl) {
    flagCountEl.textContent = String(flags.length);
}
var sidebarBadge = document.getElementById('sidebar-badge');
if (sidebarBadge) {
    sidebarBadge.textContent = String(flags.length);
}
```

- [ ] **Step 3: Update createFlagItemFromData to show label badge**

In `createFlagItemFromData`, after the header `idLabel` creation, add the label badge:

```javascript
// Label badge
var labelBadge = document.createElement('span');
labelBadge.className = 'flag-label';
labelBadge.setAttribute('data-label', flag.label.toLowerCase());
labelBadge.textContent = flag.label;
header.appendChild(labelBadge);
```

- [ ] **Step 4: Run server and verify**

Kill any existing server, then:

Run: `cd /Users/syed/Thinkific/workspace/previewf-flag-crud && pkill -f "previewf serve" 2>/dev/null; cargo run -- serve tests/fixtures --port 3030 &`

Open http://localhost:3030, navigate to `flagged.md`. Verify:
- Label badges appear next to flag IDs
- Collapse toggle button works
- Collapsed state shows badge count
- Collapse state persists on reload

- [ ] **Step 5: Commit**

```bash
cd /Users/syed/Thinkific/workspace/previewf-flag-crud
git add assets/app.js
git commit -m "Add sidebar collapse toggle and label badge display"
```

---

### Task 8: Add label picker to flag creation toolbar and edit mode

**Files:**
- Modify: `assets/app.js`
- Modify: `assets/style.css`

- [ ] **Step 1: Add label picker CSS**

In `assets/style.css`, add:

```css
.flag-label-picker {
    display: flex;
    flex-wrap: wrap;
    gap: 0.3em;
    margin-bottom: 0.5em;
}

.flag-label-pill {
    font-family: 'DM Sans', system-ui, sans-serif;
    font-size: 0.6rem;
    font-weight: 600;
    padding: 0.2em 0.5em;
    border-radius: 3px;
    border: 1px solid var(--border-color);
    background: var(--bg);
    color: var(--text-muted);
    cursor: pointer;
    transition: border-color 200ms ease, background-color 200ms ease, color 200ms ease;
}

.flag-label-pill:hover {
    border-color: var(--accent);
    color: var(--accent);
}

.flag-label-pill.selected {
    border-color: var(--accent);
    background: color-mix(in srgb, var(--accent) 15%, transparent);
    color: var(--accent);
}

.flag-label-pill-custom {
    border-style: dashed;
}

.flag-label-custom-input {
    flex: 1;
    min-width: 100px;
    padding: 0.2em 0.5em;
    font-family: 'DM Sans', system-ui, sans-serif;
    font-size: 0.6rem;
    border: 1px solid var(--accent);
    border-radius: 3px;
    background: var(--bg);
    color: var(--text);
    outline: none;
}
```

- [ ] **Step 2: Add label picker helper function in JS**

Add a reusable function that creates the label pill row:

```javascript
var PREDEFINED_LABELS = ['Comment', 'Bug', 'Todo', 'Question', 'Note', 'Style'];

function createLabelPicker(selectedLabel, onSelect) {
    var container = document.createElement('div');
    container.className = 'flag-label-picker';

    function renderPills() {
        while (container.firstChild) {
            container.removeChild(container.firstChild);
        }

        for (var i = 0; i < PREDEFINED_LABELS.length; i++) {
            (function (label) {
                var pill = document.createElement('button');
                pill.type = 'button';
                pill.className = 'flag-label-pill';
                pill.setAttribute('data-label', label.toLowerCase());
                pill.textContent = label;
                if (label === selectedLabel) {
                    pill.classList.add('selected');
                }
                pill.addEventListener('click', function (e) {
                    e.stopPropagation();
                    selectedLabel = label;
                    onSelect(label);
                    renderPills();
                });
                container.appendChild(pill);
            })(PREDEFINED_LABELS[i]);
        }

        // Custom... button
        var customBtn = document.createElement('button');
        customBtn.type = 'button';
        customBtn.className = 'flag-label-pill flag-label-pill-custom';
        customBtn.textContent = 'Custom\u2026';
        if (PREDEFINED_LABELS.indexOf(selectedLabel) === -1 && selectedLabel !== '') {
            customBtn.classList.add('selected');
            customBtn.textContent = selectedLabel;
        }
        customBtn.addEventListener('click', function (e) {
            e.stopPropagation();
            showCustomInput();
        });
        container.appendChild(customBtn);
    }

    function showCustomInput() {
        while (container.firstChild) {
            container.removeChild(container.firstChild);
        }
        var input = document.createElement('input');
        input.type = 'text';
        input.className = 'flag-label-custom-input';
        input.placeholder = 'Label name...';
        input.value = PREDEFINED_LABELS.indexOf(selectedLabel) === -1 ? selectedLabel : '';
        container.appendChild(input);
        input.focus();

        input.addEventListener('keydown', function (ev) {
            if (ev.key === 'Enter') {
                ev.preventDefault();
                var val = input.value.trim();
                if (val) {
                    selectedLabel = val;
                    onSelect(val);
                }
                renderPills();
            }
            if (ev.key === 'Escape') {
                renderPills();
            }
        });
        input.addEventListener('blur', function () {
            renderPills();
        });
    }

    renderPills();
    return container;
}
```

- [ ] **Step 3: Wire label picker into the creation toolbar**

In `initFlagToolbar`, after creating the toolbar element but before the comment input, add:

```javascript
var selectedLabel = 'Comment';
var labelPicker = createLabelPicker(selectedLabel, function (label) {
    selectedLabel = label;
});
toolbar.appendChild(labelPicker);
```

Update `submitFlag` to accept and send the label. Change the signature:

```javascript
function submitFlag(comment, selectedText, label) {
    if (!comment || !selectedText || !currentFilepath) {
        hideToolbar();
        return;
    }

    var url = '/flag/' + currentFilepath.split('/').map(encodeURIComponent).join('/');
    var body = JSON.stringify({
        comment: comment,
        selected_text: selectedText,
        label: label || 'Comment'
    });
    // ... rest of function unchanged
```

Update the submit button click handler and Enter key handler to pass `selectedLabel`:

```javascript
submitFlag(input.value.trim(), currentSelection, selectedLabel);
```

- [ ] **Step 4: Wire label picker into edit mode**

In `enterEditMode`, add a label picker above the edit input:

```javascript
function enterEditMode(item, flag, commentEl, actionsEl) {
    commentEl.style.display = 'none';
    actionsEl.style.display = 'none';

    var editContainer = document.createElement('div');
    editContainer.className = 'flag-edit-container';

    // Label picker for edit
    var editLabel = flag.label;
    var editLabelPicker = createLabelPicker(editLabel, function (label) {
        editLabel = label;
    });
    editContainer.appendChild(editLabelPicker);

    var input = document.createElement('input');
    // ... rest of input creation unchanged

    function saveEdit() {
        var newComment = input.value.trim();
        if (!newComment) return;
        var url = '/flag/' + flag.id + '/' + currentFilepath.split('/').map(encodeURIComponent).join('/');
        fetch(url, {
            method: 'PUT',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ comment: newComment, label: editLabel })
        })
        // ... rest unchanged
    }
    // ... rest of enterEditMode unchanged
}
```

- [ ] **Step 5: Test in browser**

Kill and restart server:

Run: `cd /Users/syed/Thinkific/workspace/previewf-flag-crud && pkill -f "previewf serve" 2>/dev/null; cargo run -- serve tests/fixtures --port 3030 &`

Verify:
- Label picker shows in creation toolbar when selecting text
- Clicking a label pill selects it
- "Custom..." opens text input, Enter confirms, Escape cancels
- New flag uses selected label
- Edit mode shows label picker with current label pre-selected
- Changing label in edit mode saves correctly

- [ ] **Step 6: Commit**

```bash
cd /Users/syed/Thinkific/workspace/previewf-flag-crud
git add assets/app.js assets/style.css
git commit -m "Add label picker to flag creation toolbar and edit mode"
```

---

### Task 9: Final verification and push

**Files:**
- Modify: `tests/fixtures/flagged.md` (if needed — existing `Comment:` flags should work)

- [ ] **Step 1: Verify existing fixtures still work**

Run: `cd /Users/syed/Thinkific/workspace/previewf-flag-crud && cargo run -- flags tests/fixtures/flagged.md 2>&1`

Expected: Output shows flags with `[Comment]` label (since existing flags use `Comment:` prefix).

- [ ] **Step 2: Run full test suite**

Run: `cd /Users/syed/Thinkific/workspace/previewf-flag-crud && cargo test 2>&1`

Expected: ALL tests pass.

- [ ] **Step 3: Run clippy**

Run: `cd /Users/syed/Thinkific/workspace/previewf-flag-crud && cargo clippy -- -D warnings 2>&1`

Expected: No warnings.

- [ ] **Step 4: Run fmt check**

Run: `cd /Users/syed/Thinkific/workspace/previewf-flag-crud && cargo fmt --check 2>&1`

Expected: No formatting issues.

- [ ] **Step 5: Push all commits**

```bash
cd /Users/syed/Thinkific/workspace/previewf-flag-crud
git push origin feature/flag-crud-sidebar
```
