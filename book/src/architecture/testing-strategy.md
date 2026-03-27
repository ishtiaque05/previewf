# Testing Strategy

previewf follows a test-driven development (TDD) approach where tests are written before implementation. This chapter covers the testing tools, the test structure, fixture management, and the TDD workflow.

## Testing Tools

| Tool | Role | Why |
|------|------|-----|
| cargo-nextest | Test runner | Parallel execution, better output formatting, faster than `cargo test` for multi-file projects |
| insta | Snapshot testing | Captures complex output (HTML, JSON) as `.snap` files for easy review |
| mockall | Trait mocking | Tests modules in isolation by mocking dependencies |
| std fixtures | Test data | Sample `.md` and `.html` files in `tests/fixtures/` |

### Why cargo-nextest

`cargo test` runs tests serially within each test binary. `cargo-nextest` runs each test as a separate process, enabling true parallelism. For a project with many integration test files, this can cut test execution time significantly.

Install:
```bash
cargo install cargo-nextest
```

Run:
```bash
cargo nextest run                          # all tests
cargo nextest run --test flags_test        # specific test file
cargo nextest run -- test_name             # specific test function
```

### Why insta for Snapshots

Some outputs are complex and tedious to assert manually. For example, the HTML output of `render_html` includes syntect-generated `<span>` elements with style attributes. Writing `assert!(html.contains("..."))` for every expected element is brittle and incomplete.

insta captures the full output as a `.snap` file:

```rust
use insta::assert_snapshot;

#[test]
fn test_render_html_snapshot() {
    let html = render_html("# Hello\n\nParagraph with **bold**.");
    assert_snapshot!(html);
}
```

On first run, insta creates `snapshots/test_name.snap` with the output. On subsequent runs, it compares the output against the snapshot. If it changes, the test fails and `cargo insta review` shows a diff for you to accept or reject.

### Why mockall

mockall generates mock implementations of traits. This is useful for testing modules that depend on abstractions (e.g., testing the server without a real file system).

```rust
use mockall::automock;

#[automock]
trait FileReader {
    fn read(&self, path: &str) -> Result<String, std::io::Error>;
}

#[test]
fn test_with_mock_reader() {
    let mut mock = MockFileReader::new();
    mock.expect_read()
        .returning(|_| Ok("# Hello".to_string()));

    // Use mock instead of real file reader
}
```

In practice, previewf's current design uses free functions (`std::fs::read_to_string`) rather than trait objects. mockall becomes more relevant as modules grow and introduce trait abstractions.

## Test Structure

```
tests/
  flags_test.rs          Integration tests for the flag system
  server_test.rs         Integration tests for HTTP routes
  markdown_test.rs       Integration tests for markdown rendering
  terminal_test.rs       Integration tests for terminal rendering
  watcher_test.rs        Integration tests for file watching
  fixtures/
    sample.md            Clean markdown file (no flags)
    flagged.md           Markdown with existing flags
    sample.html          HTML file for raw preview testing
```

Additionally, modules contain unit tests via `#[cfg(test)]` blocks:

```rust
// In src/terminal.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prepare_flags_for_terminal() {
        let input = "Hello <flag:1>Comment: fix this</flag> world.";
        let output = prepare_flags_for_terminal(input);
        assert_eq!(output, "Hello **[FLAG #1:** fix this**]** world.");
    }
}
```

### Unit Tests vs Integration Tests

| Type | Location | What They Test | Access |
|------|----------|---------------|--------|
| Unit tests | `#[cfg(test)] mod tests` inside source files | Internal functions, private helpers | Full access to private functions |
| Integration tests | `tests/*.rs` | Public API as external consumers see it | Only `pub` items via `use previewf::...` |

Unit tests test implementation details. Integration tests test behavior. Both are necessary.

## Test Fixtures

### `tests/fixtures/sample.md`

A clean markdown file with no flags. Used to verify that:
- Rendering works on normal markdown
- Flag extraction returns an empty vector
- `next_flag_id` returns 1

```markdown
# Sample Document

This is a paragraph with **bold** and *italic* text.

## Code Example

\```rust
fn main() {
    println!("Hello, world!");
}
\```

## List

- Item one
- Item two
- Item three
```

### `tests/fixtures/flagged.md`

A markdown file with existing flags. Used to verify:
- Flag extraction finds all flags
- Flag IDs, line numbers, and comments are parsed correctly
- Multiple flags on one line are handled
- `next_flag_id` returns the correct next ID

```markdown
# Plan Review

This section looks <flag:1>Comment: need to rethink this approach</flag> incomplete.

The timeline is <flag:2>Comment: contradicts section 3</flag> unrealistic.

This part is fine.

Multiple flags <flag:3>Comment: first issue</flag> on one <flag:4>Comment: second issue</flag> line.
```

### `tests/fixtures/sample.html`

A simple HTML file for testing the raw preview route:

```html
<!DOCTYPE html>
<html>
<head><title>Sample HTML</title></head>
<body>
    <h1>Sample HTML Page</h1>
    <p>This is a test HTML file.</p>
</body>
</html>
```

## Test Coverage by Module

### `flags_test.rs`

| Test | What It Verifies |
|------|-----------------|
| `test_extract_flags_from_flagged_file` | Correct extraction of flags (id, comment, line) |
| `test_extract_flags_from_clean_file` | Empty vector for unflagged file |
| `test_extract_flags_multiple_on_one_line` | Multiple flags on one line produce separate Flag structs |
| `test_flag_report_json` | JSON serialization of FlagReport |
| `test_next_flag_id_with_existing_flags` | Returns max_id + 1 |
| `test_next_flag_id_no_flags` | Returns 1 for clean file |
| `test_inject_flag_into_clean_content` | Flag insertion at correct line |
| `test_inject_flag_into_flagged_content` | Correct ID assignment with existing flags |
| `test_inject_flag_invalid_line` | Error for out-of-range line |

### `markdown_test.rs`

| Test | What It Verifies |
|------|-----------------|
| `test_render_heading` | `<h1>` tag in output |
| `test_render_code_block_has_syntax_class` | syntect highlighting produces `<pre>` |
| `test_render_inline_code` | `<code>` tag for inline code |
| `test_render_bold_italic` | `<strong>` and `<em>` tags |
| `test_render_flag_tags_preserved` | Flag content survives rendering |
| `test_render_diff_code_block` | Diff classes (`diff-added`, `diff-removed`) |

### `server_test.rs`

| Test | What It Verifies |
|------|-----------------|
| `test_index_route_returns_directory_listing` | Directory listing includes file names |
| `test_view_markdown_file` | Rendered markdown includes content |
| `test_view_html_file` | Raw HTML file served with 200 |
| `test_flags_json_endpoint` | JSON flag extraction via HTTP |
| `test_404_for_missing_file` | 404 for nonexistent file |

Server tests use axum's `oneshot` testing pattern:

```rust
fn create_test_app() -> axum::Router {
    let config = ServerBuilder::new()
        .path("tests/fixtures")
        .port(0)
        .live_reload(false)
        .build()
        .unwrap();

    previewf::server::create_router(config)
}

#[tokio::test]
async fn test_index_route() {
    let app = create_test_app();
    let response = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}
```

The `oneshot` method sends a single request through the router without starting a TCP listener. This makes server tests fast and deterministic -- no port binding, no network I/O.

### `terminal_test.rs`

| Test | What It Verifies |
|------|-----------------|
| `test_terminal_render_basic` | Output contains heading and paragraph text |
| `test_terminal_render_with_flags` | Flag comments appear in output |

### `watcher_test.rs`

| Test | What It Verifies |
|------|-----------------|
| `test_watcher_detects_file_change` | File modification triggers broadcast |

The watcher test uses `tempfile::TempDir` for isolation:

```rust
#[tokio::test]
async fn test_watcher_detects_file_change() {
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("test.md");
    fs::write(&file_path, "# Hello").unwrap();

    let (mut watcher, mut rx) = FileWatcher::new(dir.path().to_path_buf()).unwrap();
    watcher.watch().unwrap();

    tokio::time::sleep(Duration::from_millis(100)).await;
    fs::write(&file_path, "# Hello Updated").unwrap();

    let result = tokio::time::timeout(Duration::from_secs(2), rx.recv()).await;
    assert!(result.is_ok());
}
```

The `sleep` before the file write gives the watcher time to initialize. The `timeout` prevents the test from hanging if the notification never arrives.

## The TDD Workflow

For each feature in the implementation plan, the workflow is:

```
1. Write failing tests
   cargo nextest run --test <test_file>
   Expected: FAIL (function/type not found)

2. Implement the feature
   Add the function/type to the module

3. Run tests to verify they pass
   cargo nextest run --test <test_file>
   Expected: PASS

4. Run quality checks
   cargo clippy -- -D warnings
   cargo fmt --check
   Expected: Clean

5. Commit
   git add <files>
   git commit -m "descriptive message"
```

This rhythm ensures:
- Tests exist for every feature
- Tests actually test the feature (they fail without it)
- Code is clean (clippy, fmt) before committing
- Commits are atomic (one feature per commit)

## Code Coverage

On Linux, `cargo-tarpaulin` measures code coverage:

```bash
cargo tarpaulin --out xml
```

This produces an XML report (Cobertura format) suitable for CI integration. Coverage is part of the CI pipeline on Linux runners.

Coverage targets are not enforced numerically (no "must be >80%" gate). Instead, the TDD workflow naturally produces high coverage because tests are written first.

## Testing Philosophy

**Test behavior, not implementation.** Tests assert on observable outputs, not internal state. For example:

```rust
// Good: tests observable behavior
let flags = extract_flags(&content);
assert_eq!(flags.len(), 2);
assert_eq!(flags[0].comment, "needs work");

// Bad: tests implementation detail
let re = flags::FLAG_REGEX;
assert!(re.is_match("<flag:1>..."));
```

**Tests are documentation.** A well-named test function (`test_inject_flag_into_flagged_content`) describes what the feature does. Reading the test file gives you a specification of the module's behavior.

**Fixtures are stable.** Test fixture files (`sample.md`, `flagged.md`) are committed to the repository and do not change unless the feature they test changes. Tests that need mutable files use `tempfile::TempDir`.

**Prefer integration tests for public API.** The public API (`extract_flags`, `inject_flag`, `render_html`, etc.) is tested via integration tests in `tests/`. Internal helpers are tested via unit tests in `#[cfg(test)]` blocks only when their behavior is complex enough to warrant it.
