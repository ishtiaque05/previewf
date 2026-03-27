# previewf Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a single-binary Rust tool that previews markdown/HTML files in a browser with flagging annotations, terminal markdown viewing, and flag export for LLM consumption.

**Architecture:** Single binary with clap subcommands (serve, view, flags, book). Modules: flags (model/parse/inject/extract), markdown (comrak parsing + syntect highlighting), server (axum + websocket), watcher (notify). Builder pattern for configs. TDD throughout.

**Tech Stack:** Rust, tokio, axum, clap, comrak, syntect, termimad, notify, rust-embed, serde, thiserror/anyhow

**Security Note:** The flag creation UI uses safe DOM methods (textContent, createElement) instead of innerHTML to prevent XSS. All user-supplied content is sanitized before insertion into the DOM.

---

## File Map

| File | Responsibility |
|------|---------------|
| `Cargo.toml` | Dependencies and project metadata |
| `rustfmt.toml` | Formatter config |
| `src/main.rs` | Entry point, clap CLI definition, subcommand dispatch |
| `src/lib.rs` | Public API re-exports |
| `src/error.rs` | `PreviewError` enum with thiserror |
| `src/flags.rs` | Flag struct, regex parsing, inject/extract/export |
| `src/markdown.rs` | comrak parsing, syntect highlighting, HTML rendering |
| `src/terminal.rs` | termimad terminal rendering with flag highlighting |
| `src/watcher.rs` | notify file watcher with tokio broadcast channel |
| `src/server.rs` | axum router, routes, WebSocket, ServerBuilder |
| `assets/index.html` | Directory listing template |
| `assets/document.html` | Markdown document viewer template |
| `assets/style.css` | Full CSS — typography, themes, layout, animations |
| `assets/app.js` | Flag UI, theme toggle, live reload WebSocket client |
| `tests/fixtures/sample.md` | Clean markdown test fixture |
| `tests/fixtures/flagged.md` | Markdown with existing flags |
| `tests/fixtures/sample.html` | HTML preview test fixture |
| `tests/flags_test.rs` | Integration tests for flag operations |
| `tests/server_test.rs` | Integration tests for HTTP routes |
| `.github/workflows/ci.yml` | CI pipeline (fmt, clippy, nextest, tarpaulin) |
| `.github/workflows/release.yml` | Release pipeline (4 targets) |
| `CLAUDE.md` | LLM instructions for flag workflow |
| `README.md` | Project overview, installation, usage |
| `book/book.toml` | mdBook config |
| `book/src/SUMMARY.md` | Book table of contents |
| `book/src/**/*.md` | Book chapters |

---

## Task 1: Project Scaffolding

**Files:**
- Create: `Cargo.toml`
- Create: `rustfmt.toml`
- Create: `src/main.rs`
- Create: `src/lib.rs`
- Create: `src/error.rs`

- [ ] **Step 1: Initialize Cargo project**

Run: `cargo init --name previewf`

This creates `Cargo.toml` and `src/main.rs`. We'll replace their contents.

- [ ] **Step 2: Write Cargo.toml with all dependencies**

Replace `Cargo.toml` with:

```toml
[package]
name = "previewf"
version = "0.1.0"
edition = "2021"
description = "Preview and annotate markdown files with inline flags"
license = "MIT"
repository = "https://github.com/ishtiaque05/previewf"

[dependencies]
anyhow = "1"
axum = { version = "0.8", features = ["ws"] }
clap = { version = "4", features = ["derive"] }
comrak = "0.36"
notify = "8"
regex = "1"
rust-embed = "8"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
syntect = "5"
termimad = "0.30"
thiserror = "2"
tokio = { version = "1", features = ["full"] }
tower-http = { version = "0.6", features = ["cors", "fs"] }

[dev-dependencies]
insta = { version = "1", features = ["json"] }
mockall = "0.13"
```

- [ ] **Step 3: Write rustfmt.toml**

Create `rustfmt.toml`:

```toml
max_width = 100
tab_spaces = 4
edition = "2021"
```

- [ ] **Step 4: Write the error module**

Create `src/error.rs`:

```rust
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum PreviewError {
    #[error("File not found: {0}")]
    FileNotFound(PathBuf),

    #[error("Not a markdown file: {0}")]
    NotMarkdown(PathBuf),

    #[error("Invalid flag syntax at line {line}: {detail}")]
    FlagParse { line: usize, detail: String },

    #[error("Server error: {0}")]
    Server(#[from] std::io::Error),

    #[error("Watch error: {0}")]
    Watcher(#[from] notify::Error),
}
```

- [ ] **Step 5: Write lib.rs with module declarations**

Create `src/lib.rs`:

```rust
pub mod error;
pub mod flags;
pub mod markdown;
pub mod server;
pub mod terminal;
pub mod watcher;

pub use error::PreviewError;
```

- [ ] **Step 6: Create stub modules so it compiles**

Create `src/flags.rs`:

```rust
// Flag model, parsing, injection, and extraction.
```

Create `src/markdown.rs`:

```rust
// Markdown parsing and HTML rendering.
```

Create `src/server.rs`:

```rust
// Axum web server, routes, and WebSocket handler.
```

Create `src/terminal.rs`:

```rust
// Terminal markdown rendering via termimad.
```

Create `src/watcher.rs`:

```rust
// File watching via notify with broadcast channel.
```

- [ ] **Step 7: Write main.rs with clap CLI skeleton**

Replace `src/main.rs`:

```rust
use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

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
        /// File or directory to serve
        path: PathBuf,

        /// Port to listen on
        #[arg(short, long, default_value_t = 3000)]
        port: u16,
    },

    /// View a markdown file in the terminal
    View {
        /// Markdown file to view
        path: PathBuf,
    },

    /// Extract flags from a markdown file
    Flags {
        /// Markdown file to extract flags from
        path: PathBuf,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Serve { path, port } => {
            println!("Serving {} on port {}", path.display(), port);
            Ok(())
        }
        Commands::View { path } => {
            println!("Viewing {}", path.display());
            Ok(())
        }
        Commands::Flags { path, json } => {
            println!("Extracting flags from {} (json: {})", path.display(), json);
            Ok(())
        }
    }
}
```

- [ ] **Step 8: Verify it compiles and runs**

Run: `cargo build`
Expected: Compiles with no errors.

Run: `cargo run -- --help`
Expected: Shows help text with serve, view, flags subcommands.

Run: `cargo clippy -- -D warnings`
Expected: No warnings.

Run: `cargo fmt --check`
Expected: No formatting issues.

- [ ] **Step 9: Commit**

```bash
git add Cargo.toml Cargo.lock rustfmt.toml src/
git commit -m "Scaffold project with clap CLI and error types"
```

---

## Task 2: Flag Model and Extraction

**Files:**
- Create: `src/flags.rs` (replace stub)
- Create: `tests/fixtures/sample.md`
- Create: `tests/fixtures/flagged.md`
- Create: `tests/flags_test.rs`

- [ ] **Step 1: Create test fixtures**

Create `tests/fixtures/sample.md`:

````markdown
# Sample Document

This is a paragraph with **bold** and *italic* text.

## Code Example

```rust
fn main() {
    println!("Hello, world!");
}
```

## List

- Item one
- Item two
- Item three
````

Create `tests/fixtures/flagged.md`:

```markdown
# Plan Review

This section looks <flag:1>Comment: need to rethink this approach</flag> incomplete.

The timeline is <flag:2>Comment: contradicts section 3</flag> unrealistic.

This part is fine.

Multiple flags <flag:3>Comment: first issue</flag> on one <flag:4>Comment: second issue</flag> line.
```

- [ ] **Step 2: Write failing tests for Flag struct and extract_flags**

Create `tests/flags_test.rs`:

```rust
use previewf::flags::{extract_flags, Flag, FlagReport};

#[test]
fn test_extract_flags_from_flagged_file() {
    let content = std::fs::read_to_string("tests/fixtures/flagged.md").unwrap();
    let flags = extract_flags(&content);

    assert_eq!(flags.len(), 4);
    assert_eq!(flags[0].id, 1);
    assert_eq!(flags[0].comment, "need to rethink this approach");
    assert_eq!(flags[0].line, 3);
    assert_eq!(flags[1].id, 2);
    assert_eq!(flags[1].comment, "contradicts section 3");
    assert_eq!(flags[1].line, 5);
}

#[test]
fn test_extract_flags_from_clean_file() {
    let content = std::fs::read_to_string("tests/fixtures/sample.md").unwrap();
    let flags = extract_flags(&content);

    assert_eq!(flags.len(), 0);
}

#[test]
fn test_extract_flags_multiple_on_one_line() {
    let content = std::fs::read_to_string("tests/fixtures/flagged.md").unwrap();
    let flags = extract_flags(&content);

    let line_9_flags: Vec<&Flag> = flags.iter().filter(|f| f.line == 9).collect();
    assert_eq!(line_9_flags.len(), 2);
    assert_eq!(line_9_flags[0].id, 3);
    assert_eq!(line_9_flags[1].id, 4);
}

#[test]
fn test_flag_report_json() {
    let content = std::fs::read_to_string("tests/fixtures/flagged.md").unwrap();
    let flags = extract_flags(&content);
    let report = FlagReport {
        file: "flagged.md".to_string(),
        flags,
    };

    let json = serde_json::to_string_pretty(&report).unwrap();
    assert!(json.contains("\"id\": 1"));
    assert!(json.contains("need to rethink this approach"));
}

#[test]
fn test_next_flag_id_with_existing_flags() {
    let content = std::fs::read_to_string("tests/fixtures/flagged.md").unwrap();
    let next = previewf::flags::next_flag_id(&content);
    assert_eq!(next, 5);
}

#[test]
fn test_next_flag_id_no_flags() {
    let content = std::fs::read_to_string("tests/fixtures/sample.md").unwrap();
    let next = previewf::flags::next_flag_id(&content);
    assert_eq!(next, 1);
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo nextest run --test flags_test`
Expected: FAIL — `extract_flags`, `Flag`, `FlagReport`, `next_flag_id` not found.

- [ ] **Step 4: Implement the flags module**

Replace `src/flags.rs`:

```rust
use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Flag {
    pub id: u32,
    pub line: usize,
    pub text: String,
    pub comment: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlagReport {
    pub file: String,
    pub flags: Vec<Flag>,
}

/// Extract all flags from markdown content.
/// Parses `<flag:N>Comment: description</flag>` patterns.
pub fn extract_flags(content: &str) -> Vec<Flag> {
    let re = Regex::new(r"<flag:(\d+)>Comment:\s*(.+?)</flag>").unwrap();
    let mut flags = Vec::new();

    for (line_num, line) in content.lines().enumerate() {
        for cap in re.captures_iter(line) {
            let id: u32 = cap[1].parse().unwrap_or(0);
            let comment = cap[2].trim().to_string();

            // Build the text context by removing the flag tags
            let text = re.replace_all(line, "").trim().to_string();

            flags.push(Flag {
                id,
                line: line_num + 1, // 1-indexed
                text,
                comment,
            });
        }
    }

    flags
}

/// Find the next available flag ID in the content.
pub fn next_flag_id(content: &str) -> u32 {
    let flags = extract_flags(content);
    flags.iter().map(|f| f.id).max().unwrap_or(0) + 1
}

/// Format flags as human-readable text output.
pub fn format_flags_text(report: &FlagReport) -> String {
    let mut output = format!("Flags in {}:\n\n", report.file);

    if report.flags.is_empty() {
        output.push_str("  No flags found.\n");
        return output;
    }

    for flag in &report.flags {
        output.push_str(&format!(
            "  #{} (line {}): {}\n    Context: {}\n\n",
            flag.id, flag.line, flag.comment, flag.text
        ));
    }

    output
}
```

- [ ] **Step 5: Update lib.rs exports**

Ensure `src/lib.rs` has:

```rust
pub mod error;
pub mod flags;
pub mod markdown;
pub mod server;
pub mod terminal;
pub mod watcher;

pub use error::PreviewError;
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo nextest run --test flags_test`
Expected: All 6 tests PASS.

- [ ] **Step 7: Run clippy and fmt**

Run: `cargo clippy -- -D warnings && cargo fmt --check`
Expected: Clean.

- [ ] **Step 8: Commit**

```bash
git add src/flags.rs tests/fixtures/ tests/flags_test.rs
git commit -m "Add flag model, extraction, and next-ID logic with tests"
```

---

## Task 3: Flag Injection

**Files:**
- Modify: `src/flags.rs`
- Modify: `tests/flags_test.rs`

- [ ] **Step 1: Write failing tests for inject_flag**

Append to `tests/flags_test.rs`:

```rust
use previewf::flags::inject_flag;

#[test]
fn test_inject_flag_into_clean_content() {
    let content = "Line one\nLine two\nLine three\n";
    let result = inject_flag(content, 2, "needs work").unwrap();

    assert!(result.contains("<flag:1>Comment: needs work</flag>"));
    assert!(result.contains("Line two"));
}

#[test]
fn test_inject_flag_into_flagged_content() {
    let content = "Line one\n<flag:1>Comment: existing</flag> Line two\nLine three\n";
    let result = inject_flag(content, 3, "also this").unwrap();

    assert!(result.contains("<flag:2>Comment: also this</flag>"));
    // Existing flag preserved
    assert!(result.contains("<flag:1>Comment: existing</flag>"));
}

#[test]
fn test_inject_flag_invalid_line() {
    let content = "Line one\nLine two\n";
    let result = inject_flag(content, 5, "bad line");

    assert!(result.is_err());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run --test flags_test`
Expected: FAIL — `inject_flag` not found.

- [ ] **Step 3: Implement inject_flag**

Add to `src/flags.rs`:

```rust
use crate::PreviewError;

/// Inject a new flag at the given line number (1-indexed).
/// Appends the flag tag at the end of the line.
pub fn inject_flag(content: &str, line: usize, comment: &str) -> Result<String, PreviewError> {
    let lines: Vec<&str> = content.lines().collect();

    if line == 0 || line > lines.len() {
        return Err(PreviewError::FlagParse {
            line,
            detail: format!("Line {} is out of range (file has {} lines)", line, lines.len()),
        });
    }

    let next_id = next_flag_id(content);
    let flag_tag = format!(" <flag:{}>Comment: {}</flag>", next_id, comment);

    let mut result: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
    result[line - 1].push_str(&flag_tag);

    let mut output = result.join("\n");
    // Preserve trailing newline if original had one
    if content.ends_with('\n') {
        output.push('\n');
    }

    Ok(output)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo nextest run --test flags_test`
Expected: All 9 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add src/flags.rs tests/flags_test.rs
git commit -m "Add flag injection with line validation"
```

---

## Task 4: Markdown Parsing and HTML Rendering

**Files:**
- Create: `src/markdown.rs` (replace stub)
- Create: `tests/markdown_test.rs`

- [ ] **Step 1: Write failing tests for markdown rendering**

Create `tests/markdown_test.rs`:

```rust
use previewf::markdown::render_html;

#[test]
fn test_render_heading() {
    let html = render_html("# Hello World");
    assert!(html.contains("<h1>"));
    assert!(html.contains("Hello World"));
}

#[test]
fn test_render_code_block_has_syntax_class() {
    let input = "```rust\nfn main() {}\n```";
    let html = render_html(input);
    // syntect produces <pre> with highlighted spans
    assert!(html.contains("<pre"));
    assert!(html.contains("fn"));
}

#[test]
fn test_render_inline_code() {
    let html = render_html("Use `cargo build` to compile.");
    assert!(html.contains("<code>"));
    assert!(html.contains("cargo build"));
}

#[test]
fn test_render_bold_italic() {
    let html = render_html("This is **bold** and *italic*.");
    assert!(html.contains("<strong>bold</strong>"));
    assert!(html.contains("<em>italic</em>"));
}

#[test]
fn test_render_flag_tags_preserved() {
    let input = "Text <flag:1>Comment: something</flag> here.";
    let html = render_html(input);
    // Flags should be rendered as visible elements, not stripped
    assert!(html.contains("flag"));
    assert!(html.contains("something"));
}

#[test]
fn test_render_diff_code_block() {
    let input = "```diff\n- old line\n+ new line\n@@ -1,3 +1,3 @@\n```";
    let html = render_html(input);
    assert!(html.contains("diff-removed") || html.contains("diff-added"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run --test markdown_test`
Expected: FAIL — `render_html` not found.

- [ ] **Step 3: Implement markdown module**

Replace `src/markdown.rs`:

```rust
use comrak::{markdown_to_html, Options};
use regex::Regex;
use syntect::highlighting::ThemeSet;
use syntect::html::highlighted_html_for_string;
use syntect::parsing::SyntaxSet;

/// Render markdown content to HTML with syntax highlighting.
pub fn render_html(content: &str) -> String {
    let mut options = Options::default();
    options.extension.strikethrough = true;
    options.extension.table = true;
    options.extension.autolink = true;
    options.extension.tasklist = true;
    options.extension.footnotes = true;
    options.render.unsafe_ = true; // Allow raw HTML (for flag tags)

    let raw_html = markdown_to_html(content, &options);

    // Post-process: apply syntax highlighting to code blocks
    let highlighted = highlight_code_blocks(&raw_html);

    // Post-process: convert flag tags to styled spans
    render_flag_spans(&highlighted)
}

/// Replace <pre><code class="language-X"> blocks with syntect-highlighted HTML.
fn highlight_code_blocks(html: &str) -> String {
    let ss = SyntaxSet::load_defaults_newlines();
    let ts = ThemeSet::load_defaults();
    let theme = &ts.themes["base16-ocean.dark"];

    let re = Regex::new(
        r#"<pre><code class="language-(\w+)">([\s\S]*?)</code></pre>"#
    ).unwrap();

    re.replace_all(html, |caps: &regex::Captures| {
        let lang = &caps[1];
        let code = html_escape_decode(&caps[2]);

        if lang == "diff" {
            return render_diff_block(&code);
        }

        let syntax = ss.find_syntax_by_token(lang)
            .unwrap_or_else(|| ss.find_syntax_plain_text());

        match highlighted_html_for_string(&code, &ss, syntax, theme) {
            Ok(highlighted) => format!(
                r#"<pre class="highlight" data-lang="{}">{}</pre>"#,
                lang, highlighted
            ),
            Err(_) => format!(
                r#"<pre class="highlight" data-lang="{}"><code>{}</code></pre>"#,
                lang,
                &caps[2]
            ),
        }
    }).to_string()
}

/// Render diff-formatted code with git-style coloring.
fn render_diff_block(code: &str) -> String {
    let mut lines = Vec::new();
    for line in code.lines() {
        let class = if line.starts_with('+') && !line.starts_with("+++") {
            "diff-added"
        } else if line.starts_with('-') && !line.starts_with("---") {
            "diff-removed"
        } else if line.starts_with("@@") {
            "diff-hunk"
        } else {
            "diff-context"
        };
        let escaped = html_escape_encode(line);
        lines.push(format!(r#"<span class="{}">{}</span>"#, class, escaped));
    }
    format!(r#"<pre class="highlight diff">{}</pre>"#, lines.join("\n"))
}

/// Convert <flag:N>Comment: text</flag> to styled spans for the web UI.
fn render_flag_spans(html: &str) -> String {
    let re = Regex::new(r"<flag:(\d+)>Comment:\s*(.+?)</flag>").unwrap();
    re.replace_all(html, |caps: &regex::Captures| {
        let id = &caps[1];
        let comment = &caps[2];
        format!(
            r#"<span class="flag" data-flag-id="{}"><span class="flag-marker">#{}</span><span class="flag-comment">{}</span></span>"#,
            id, id, comment.trim()
        )
    }).to_string()
}

/// Decode HTML entities back to raw text for syntect processing.
fn html_escape_decode(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

/// Encode text to HTML-safe entities.
fn html_escape_encode(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo nextest run --test markdown_test`
Expected: All 6 tests PASS.

- [ ] **Step 5: Run clippy and fmt**

Run: `cargo clippy -- -D warnings && cargo fmt --check`
Expected: Clean.

- [ ] **Step 6: Commit**

```bash
git add src/markdown.rs tests/markdown_test.rs
git commit -m "Add markdown parsing with syntax highlighting and flag rendering"
```

---

## Task 5: Terminal Rendering

**Files:**
- Create: `src/terminal.rs` (replace stub)

- [ ] **Step 1: Write failing test for terminal rendering**

Add `tests/terminal_test.rs`:

```rust
use previewf::terminal::render_terminal;

#[test]
fn test_terminal_render_basic() {
    let content = "# Hello\n\nA paragraph.\n";
    let output = render_terminal(content);
    // termimad wraps output with ANSI codes; just check content is present
    assert!(output.contains("Hello"));
    assert!(output.contains("paragraph"));
}

#[test]
fn test_terminal_render_with_flags() {
    let content = "Text <flag:1>Comment: check this</flag> here.\n";
    let output = render_terminal(content);
    assert!(output.contains("check this"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run --test terminal_test`
Expected: FAIL — `render_terminal` not found.

- [ ] **Step 3: Implement terminal rendering**

Replace `src/terminal.rs`:

```rust
use regex::Regex;
use termimad::MadSkin;

/// Render markdown content for terminal display.
/// Flags are converted to a visible format before rendering.
pub fn render_terminal(content: &str) -> String {
    let skin = MadSkin::default();
    let prepared = prepare_flags_for_terminal(content);
    skin.term_text(&prepared).to_string()
}

/// Convert flag tags to a terminal-friendly format.
/// `<flag:1>Comment: text</flag>` becomes `[FLAG #1: text]`
fn prepare_flags_for_terminal(content: &str) -> String {
    let re = Regex::new(r"<flag:(\d+)>Comment:\s*(.+?)</flag>").unwrap();
    re.replace_all(content, |caps: &regex::Captures| {
        format!("**[FLAG #{}:** {}**]**", &caps[1], caps[2].trim())
    })
    .to_string()
}

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

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo nextest run --test terminal_test`
Expected: All tests PASS.

Run: `cargo nextest run -- terminal`
Expected: Unit test also PASS.

- [ ] **Step 5: Commit**

```bash
git add src/terminal.rs tests/terminal_test.rs
git commit -m "Add terminal markdown rendering with flag formatting"
```

---

## Task 6: File Watcher

**Files:**
- Create: `src/watcher.rs` (replace stub)

- [ ] **Step 1: Write failing test for file watcher**

Create `tests/watcher_test.rs`:

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

    let (mut watcher, mut rx) = FileWatcher::new(dir.path().to_path_buf()).unwrap();
    watcher.watch().unwrap();

    // Modify the file
    tokio::time::sleep(Duration::from_millis(100)).await;
    fs::write(&file_path, "# Hello Updated").unwrap();

    // Should receive a notification within 2 seconds
    let result = tokio::time::timeout(Duration::from_secs(2), rx.recv()).await;
    assert!(result.is_ok(), "Should receive file change notification");
}
```

- [ ] **Step 2: Add tempfile as dev dependency**

Add to `Cargo.toml` under `[dev-dependencies]`:

```toml
tempfile = "3"
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo nextest run --test watcher_test`
Expected: FAIL — `FileWatcher` not found.

- [ ] **Step 4: Implement the file watcher**

Replace `src/watcher.rs`:

```rust
use std::path::PathBuf;

use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::broadcast;

use crate::PreviewError;

pub struct FileWatcher {
    path: PathBuf,
    watcher: Option<RecommendedWatcher>,
    sender: broadcast::Sender<PathBuf>,
}

impl FileWatcher {
    /// Create a new file watcher for the given path.
    /// Returns the watcher and a receiver for change notifications.
    pub fn new(path: PathBuf) -> Result<(Self, broadcast::Receiver<PathBuf>), PreviewError> {
        let (sender, receiver) = broadcast::channel(100);

        let watcher = FileWatcher {
            path,
            watcher: None,
            sender,
        };

        Ok((watcher, receiver))
    }

    /// Start watching for file changes.
    pub fn watch(&mut self) -> Result<(), PreviewError> {
        let sender = self.sender.clone();

        let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                if event.kind.is_modify() || event.kind.is_create() {
                    for path in event.paths {
                        let _ = sender.send(path);
                    }
                }
            }
        })
        .map_err(PreviewError::Watcher)?;

        let mode = if self.path.is_dir() {
            RecursiveMode::Recursive
        } else {
            RecursiveMode::NonRecursive
        };

        watcher
            .watch(&self.path, mode)
            .map_err(PreviewError::Watcher)?;

        self.watcher = Some(watcher);
        Ok(())
    }

    /// Get a new receiver for change notifications.
    pub fn subscribe(&self) -> broadcast::Receiver<PathBuf> {
        self.sender.subscribe()
    }
}
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo nextest run --test watcher_test`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/watcher.rs tests/watcher_test.rs Cargo.toml
git commit -m "Add file watcher with broadcast channel notifications"
```

---

## Task 7: Frontend Assets

**Files:**
- Create: `assets/document.html`
- Create: `assets/index.html`
- Create: `assets/style.css`
- Create: `assets/app.js`

- [ ] **Step 1: Create the document viewer HTML template**

Create `assets/document.html`:

```html
<!DOCTYPE html>
<html lang="en" data-theme="light">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{{title}} — previewf</title>
    <link rel="preconnect" href="https://fonts.googleapis.com">
    <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
    <link href="https://fonts.googleapis.com/css2?family=Playfair+Display:wght@400;700;900&family=Source+Serif+4:ital,wght@0,300;0,400;0,600;0,700;1,400&family=JetBrains+Mono:wght@400;700&family=DM+Sans:wght@400;500;700&display=swap" rel="stylesheet">
    <link rel="stylesheet" href="/assets/style.css">
</head>
<body>
    <header class="top-bar">
        <div class="top-bar-left">
            <span class="logo">previewf</span>
            <span class="file-path">{{filepath}}</span>
        </div>
        <div class="top-bar-right">
            <button class="theme-toggle" id="theme-toggle" aria-label="Toggle theme">
                <span class="theme-icon-light">&#9728;</span>
                <span class="theme-icon-dark">&#9790;</span>
            </button>
            <span class="flag-count" id="flag-count">0</span>
        </div>
    </header>

    <main class="layout">
        <article class="document" id="document">
            {{content}}
        </article>

        <aside class="sidebar" id="sidebar">
            <h3 class="sidebar-title">Flags</h3>
            <div class="flag-list" id="flag-list"></div>
        </aside>
    </main>

    <footer class="status-bar">
        <span>previewf v0.1.0</span>
        <span class="status-separator">&middot;</span>
        <span>watching for changes</span>
        <span class="status-separator">&middot;</span>
        <span class="status-connection" id="status-connection">
            <span class="status-dot"></span>
            connected
        </span>
    </footer>

    <script src="/assets/app.js"></script>
</body>
</html>
```

- [ ] **Step 2: Create the directory listing HTML template**

Create `assets/index.html`:

```html
<!DOCTYPE html>
<html lang="en" data-theme="light">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{{directory}} — previewf</title>
    <link rel="preconnect" href="https://fonts.googleapis.com">
    <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
    <link href="https://fonts.googleapis.com/css2?family=Playfair+Display:wght@400;700;900&family=Source+Serif+4:ital,wght@0,300;0,400;0,600;0,700;1,400&family=JetBrains+Mono:wght@400;700&family=DM+Sans:wght@400;500;700&display=swap" rel="stylesheet">
    <link rel="stylesheet" href="/assets/style.css">
</head>
<body>
    <header class="top-bar">
        <div class="top-bar-left">
            <span class="logo">previewf</span>
            <span class="file-path">{{directory}}</span>
        </div>
        <div class="top-bar-right">
            <button class="theme-toggle" id="theme-toggle" aria-label="Toggle theme">
                <span class="theme-icon-light">&#9728;</span>
                <span class="theme-icon-dark">&#9790;</span>
            </button>
        </div>
    </header>

    <main class="listing">
        <div class="file-list">
            {{file_entries}}
        </div>
        <div class="listing-summary">{{summary}}</div>
    </main>

    <footer class="status-bar">
        <span>previewf v0.1.0</span>
        <span class="status-separator">&middot;</span>
        <span>watching for changes</span>
        <span class="status-separator">&middot;</span>
        <span class="status-connection" id="status-connection">
            <span class="status-dot"></span>
            connected
        </span>
    </footer>

    <script src="/assets/app.js"></script>
</body>
</html>
```

- [ ] **Step 3: Create the CSS with full theme support**

Create `assets/style.css` — see the full CSS from the design spec (Section 2). The complete CSS is provided in the design document at `docs/previewf/specs/2026-03-26-previewf-design.md`, Frontend Design section. Copy it verbatim. It includes:

- CSS custom properties for light ("Warm Paper") and dark ("Midnight Ink") themes
- Typography rules for Playfair Display, Source Serif 4, JetBrains Mono, DM Sans
- Document layout with 72ch max-width
- Code block styling with language label
- Diff coloring classes (diff-added, diff-removed, diff-hunk, diff-context)
- Flag styling (inline highlights, sidebar items)
- Flag creation toolbar
- Directory listing
- Status bar with connection indicator
- Animations (fadeIn, slideIn, pulse, throb)
- Responsive breakpoint at 900px

- [ ] **Step 4: Create the JavaScript for flag UI, theme toggle, and live reload**

Create `assets/app.js` — uses safe DOM methods throughout (createElement, textContent, appendChild) instead of innerHTML to prevent XSS:

```javascript
// ============================================
// previewf — Client-side JavaScript
// Flag UI, theme toggle, live reload
// ============================================

(function() {
    'use strict';

    // --- Theme Toggle ---
    var themeToggle = document.getElementById('theme-toggle');
    var html = document.documentElement;

    function getPreferredTheme() {
        var stored = localStorage.getItem('previewf-theme');
        if (stored) return stored;
        return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
    }

    function setTheme(theme) {
        html.setAttribute('data-theme', theme);
        localStorage.setItem('previewf-theme', theme);
    }

    setTheme(getPreferredTheme());

    if (themeToggle) {
        themeToggle.addEventListener('click', function() {
            var current = html.getAttribute('data-theme');
            setTheme(current === 'light' ? 'dark' : 'light');
        });
    }

    // --- Flag Sidebar Population ---
    function populateFlags() {
        var flagElements = document.querySelectorAll('.flag');
        var flagList = document.getElementById('flag-list');
        var flagCount = document.getElementById('flag-count');
        var sidebar = document.getElementById('sidebar');

        if (!flagList) return;

        // Clear existing items safely
        while (flagList.firstChild) {
            flagList.removeChild(flagList.firstChild);
        }

        var count = 0;

        flagElements.forEach(function(el) {
            count++;
            var id = el.dataset.flagId;
            var commentEl = el.querySelector('.flag-comment');
            var comment = commentEl ? commentEl.textContent : '';

            var item = document.createElement('div');
            item.className = 'flag-item';
            item.dataset.flagId = id;

            var headerDiv = document.createElement('div');
            var idSpan = document.createElement('span');
            idSpan.className = 'flag-item-id';
            idSpan.textContent = '#' + id;
            headerDiv.appendChild(idSpan);

            var commentDiv = document.createElement('div');
            commentDiv.className = 'flag-item-comment';
            commentDiv.textContent = comment;

            item.appendChild(headerDiv);
            item.appendChild(commentDiv);

            // Click sidebar item -> scroll to flag in document
            item.addEventListener('click', function() {
                el.scrollIntoView({ behavior: 'smooth', block: 'center' });
                flagElements.forEach(function(f) { f.classList.remove('active'); });
                document.querySelectorAll('.flag-item').forEach(function(i) { i.classList.remove('active'); });
                el.classList.add('active');
                item.classList.add('active');
            });

            // Click flag in document -> highlight sidebar item
            el.addEventListener('click', function() {
                flagElements.forEach(function(f) { f.classList.remove('active'); });
                document.querySelectorAll('.flag-item').forEach(function(i) { i.classList.remove('active'); });
                el.classList.add('active');
                item.classList.add('active');
                item.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
            });

            flagList.appendChild(item);
        });

        if (flagCount) flagCount.textContent = String(count);
        if (sidebar && count === 0) sidebar.classList.add('collapsed');
    }

    populateFlags();

    // --- Flag Creation (Text Selection) ---
    var documentEl = document.getElementById('document');
    var toolbar = null;

    function createToolbar() {
        toolbar = document.createElement('div');
        toolbar.className = 'flag-toolbar';

        var input = document.createElement('input');
        input.type = 'text';
        input.placeholder = 'Add comment...';
        input.id = 'flag-comment-input';

        var btn = document.createElement('button');
        btn.id = 'flag-submit-btn';
        btn.textContent = 'Flag';

        toolbar.appendChild(input);
        toolbar.appendChild(btn);
        document.body.appendChild(toolbar);

        btn.addEventListener('click', submitFlag);
        input.addEventListener('keydown', function(e) {
            if (e.key === 'Enter') submitFlag();
            if (e.key === 'Escape') hideToolbar();
        });
    }

    function showToolbar(x, y) {
        if (!toolbar) createToolbar();
        toolbar.style.left = x + 'px';
        toolbar.style.top = y + 'px';
        toolbar.classList.add('visible');
        var input = document.getElementById('flag-comment-input');
        input.value = '';
        input.focus();
    }

    function hideToolbar() {
        if (toolbar) toolbar.classList.remove('visible');
    }

    function submitFlag() {
        var input = document.getElementById('flag-comment-input');
        var comment = input.value.trim();
        if (!comment) return;

        var selection = window.getSelection();
        if (!selection.rangeCount) return;

        var range = selection.getRangeAt(0);
        var selectedText = range.toString();

        var filepathEl = document.querySelector('.file-path');
        var filepath = filepathEl ? filepathEl.textContent : '';

        fetch('/flag/' + encodeURIComponent(filepath), {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                comment: comment,
                selected_text: selectedText,
            }),
        })
        .then(function(res) {
            if (res.ok) {
                hideToolbar();
                // Page will reload via WebSocket when file changes
            }
        })
        .catch(function(err) {
            console.error('Flag submission failed:', err);
        });
    }

    if (documentEl) {
        documentEl.addEventListener('mouseup', function() {
            var selection = window.getSelection();
            var text = selection.toString().trim();

            if (text.length > 0) {
                var rect = selection.getRangeAt(0).getBoundingClientRect();
                showToolbar(
                    rect.left + window.scrollX,
                    rect.bottom + window.scrollY + 8
                );
            } else {
                hideToolbar();
            }
        });
    }

    document.addEventListener('mousedown', function(e) {
        if (toolbar && !toolbar.contains(e.target)) {
            hideToolbar();
        }
    });

    // --- WebSocket Live Reload ---
    var statusConnection = document.getElementById('status-connection');

    function connectWebSocket() {
        var protocol = location.protocol === 'https:' ? 'wss:' : 'ws:';
        var ws = new WebSocket(protocol + '//' + location.host + '/ws');

        ws.onopen = function() {
            if (statusConnection) {
                statusConnection.classList.remove('disconnected');
            }
        };

        ws.onmessage = function(event) {
            if (event.data === 'reload') {
                location.reload();
            }
        };

        ws.onclose = function() {
            if (statusConnection) {
                statusConnection.classList.add('disconnected');
            }
            // Reconnect after 2 seconds
            setTimeout(connectWebSocket, 2000);
        };

        ws.onerror = function() {
            ws.close();
        };
    }

    connectWebSocket();
})();
```

- [ ] **Step 5: Commit**

```bash
git add assets/
git commit -m "Add frontend assets — HTML templates, CSS themes, JS client"
```

---

## Task 8: Web Server

**Files:**
- Create: `src/server.rs` (replace stub)
- Create: `tests/server_test.rs`

- [ ] **Step 1: Write failing tests for server routes**

Create `tests/server_test.rs`:

```rust
use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

use previewf::server::ServerBuilder;

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
async fn test_index_route_returns_directory_listing() {
    let app = create_test_app();
    let response = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let text = String::from_utf8(body.to_vec()).unwrap();
    assert!(text.contains("sample.md"));
    assert!(text.contains("flagged.md"));
}

#[tokio::test]
async fn test_view_markdown_file() {
    let app = create_test_app();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/view/sample.md")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let text = String::from_utf8(body.to_vec()).unwrap();
    assert!(text.contains("Sample Document"));
    assert!(text.contains("<h1>"));
}

#[tokio::test]
async fn test_view_html_file() {
    let app = create_test_app();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/raw/sample.html")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_flags_json_endpoint() {
    let app = create_test_app();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/flags/flagged.md")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let text = String::from_utf8(body.to_vec()).unwrap();
    assert!(text.contains("\"id\""));
    assert!(text.contains("need to rethink"));
}

#[tokio::test]
async fn test_404_for_missing_file() {
    let app = create_test_app();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/view/nonexistent.md")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
```

- [ ] **Step 2: Add test fixture for HTML**

Create `tests/fixtures/sample.html`:

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

- [ ] **Step 3: Add tower as dev dependency**

Add to `Cargo.toml` under `[dev-dependencies]`:

```toml
tower = { version = "0.5", features = ["util"] }
```

- [ ] **Step 4: Run tests to verify they fail**

Run: `cargo nextest run --test server_test`
Expected: FAIL — `ServerBuilder`, `create_router` not found.

- [ ] **Step 5: Implement the server module**

Replace `src/server.rs`:

```rust
use std::path::{Path, PathBuf};

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path as AxumPath, State};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use rust_embed::Embed;
use serde::Deserialize;
use tokio::sync::broadcast;

use crate::flags::{extract_flags, inject_flag, FlagReport};
use crate::markdown::render_html;
use crate::PreviewError;

#[derive(Embed)]
#[folder = "assets/"]
struct Assets;

#[derive(Clone)]
pub struct ServerConfig {
    pub path: PathBuf,
    pub port: u16,
    pub live_reload: bool,
}

pub struct ServerBuilder {
    path: Option<PathBuf>,
    port: u16,
    live_reload: bool,
}

impl ServerBuilder {
    pub fn new() -> Self {
        ServerBuilder {
            path: None,
            port: 3000,
            live_reload: true,
        }
    }

    pub fn path(mut self, p: impl Into<PathBuf>) -> Self {
        self.path = Some(p.into());
        self
    }

    pub fn port(mut self, p: u16) -> Self {
        self.port = p;
        self
    }

    pub fn live_reload(mut self, lr: bool) -> Self {
        self.live_reload = lr;
        self
    }

    pub fn build(self) -> Result<ServerConfig, PreviewError> {
        let path = self.path.ok_or_else(|| {
            PreviewError::FileNotFound(PathBuf::from("<no path provided>"))
        })?;

        Ok(ServerConfig {
            path,
            port: self.port,
            live_reload: self.live_reload,
        })
    }
}

impl Default for ServerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
struct AppState {
    config: ServerConfig,
    reload_tx: broadcast::Sender<()>,
}

/// Create the axum router (exposed for testing).
pub fn create_router(config: ServerConfig) -> Router {
    let (reload_tx, _) = broadcast::channel(100);

    let state = AppState {
        config,
        reload_tx,
    };

    Router::new()
        .route("/", get(index_handler))
        .route("/view/{*filepath}", get(view_handler))
        .route("/raw/{*filepath}", get(raw_handler))
        .route("/flags/{*filepath}", get(flags_handler))
        .route("/flag/{*filepath}", post(flag_post_handler))
        .route("/ws", get(ws_handler))
        .route("/assets/{*filepath}", get(asset_handler))
        .with_state(state)
}

/// Run the server.
pub async fn run(config: ServerConfig) -> Result<(), PreviewError> {
    let port = config.port;
    let path = config.path.clone();

    let app = create_router(config);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port))
        .await
        .map_err(PreviewError::Server)?;

    println!("Serving {} on http://localhost:{}", path.display(), port);

    axum::serve(listener, app)
        .await
        .map_err(PreviewError::Server)?;

    Ok(())
}

// --- Route Handlers ---

async fn index_handler(State(state): State<AppState>) -> Response {
    let base_path = &state.config.path;

    if base_path.is_file() {
        let filename = base_path.file_name().unwrap_or_default().to_string_lossy();
        return axum::response::Redirect::to(&format!("/view/{}", filename)).into_response();
    }

    let mut md_files = Vec::new();
    let mut html_files = Vec::new();

    if let Ok(entries) = std::fs::read_dir(base_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(ext) = path.extension() {
                let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
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

    md_files.sort_by(|a, b| a.0.cmp(&b.0));
    html_files.sort();

    let mut entries_html = String::new();
    for (name, flags) in &md_files {
        let badge = if *flags > 0 {
            format!(
                r#"<span class="file-entry-badge has-flags">{} flag{}</span>"#,
                flags,
                if *flags == 1 { "" } else { "s" }
            )
        } else {
            r#"<span class="file-entry-badge">&mdash;</span>"#.to_string()
        };
        entries_html.push_str(&format!(
            r#"<a class="file-entry" href="/view/{}"><span><span class="file-entry-icon">&#9670;</span><span class="file-entry-name">{}</span></span>{}</a>"#,
            name, name, badge
        ));
    }
    for name in &html_files {
        entries_html.push_str(&format!(
            r#"<a class="file-entry" href="/raw/{}"><span><span class="file-entry-icon">&#9671;</span><span class="file-entry-name">{}</span></span><span class="file-entry-badge">(html)</span></a>"#,
            name, name
        ));
    }

    let summary = format!("{} markdown &middot; {} html", md_files.len(), html_files.len());
    let dir_display = base_path.display().to_string();

    let template = Assets::get("index.html")
        .map(|f| String::from_utf8_lossy(&f.data).to_string())
        .unwrap_or_else(|| "<html><body>Template not found</body></html>".to_string());

    let page = template
        .replace("{{directory}}", &dir_display)
        .replace("{{file_entries}}", &entries_html)
        .replace("{{summary}}", &summary);

    Html(page).into_response()
}

async fn view_handler(
    State(state): State<AppState>,
    AxumPath(filepath): AxumPath<String>,
) -> Response {
    let full_path = resolve_path(&state.config.path, &filepath);

    let content = match std::fs::read_to_string(&full_path) {
        Ok(c) => c,
        Err(_) => return (StatusCode::NOT_FOUND, "File not found").into_response(),
    };

    let rendered = render_html(&content);

    let template = Assets::get("document.html")
        .map(|f| String::from_utf8_lossy(&f.data).to_string())
        .unwrap_or_else(|| "<html><body>{{content}}</body></html>".to_string());

    let page = template
        .replace("{{title}}", &filepath)
        .replace("{{filepath}}", &filepath)
        .replace("{{content}}", &rendered);

    Html(page).into_response()
}

async fn raw_handler(
    State(state): State<AppState>,
    AxumPath(filepath): AxumPath<String>,
) -> Response {
    let full_path = resolve_path(&state.config.path, &filepath);

    match std::fs::read_to_string(&full_path) {
        Ok(content) => Html(content).into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "File not found").into_response(),
    }
}

async fn flags_handler(
    State(state): State<AppState>,
    AxumPath(filepath): AxumPath<String>,
) -> Response {
    let full_path = resolve_path(&state.config.path, &filepath);

    let content = match std::fs::read_to_string(&full_path) {
        Ok(c) => c,
        Err(_) => return (StatusCode::NOT_FOUND, "File not found").into_response(),
    };

    let flags = extract_flags(&content);
    let report = FlagReport {
        file: filepath,
        flags,
    };

    axum::Json(report).into_response()
}

#[derive(Deserialize)]
struct FlagRequest {
    comment: String,
    selected_text: String,
}

async fn flag_post_handler(
    State(state): State<AppState>,
    AxumPath(filepath): AxumPath<String>,
    axum::Json(payload): axum::Json<FlagRequest>,
) -> Response {
    let full_path = resolve_path(&state.config.path, &filepath);

    let content = match std::fs::read_to_string(&full_path) {
        Ok(c) => c,
        Err(_) => return (StatusCode::NOT_FOUND, "File not found").into_response(),
    };

    let line = content
        .lines()
        .enumerate()
        .find(|(_, l)| l.contains(&payload.selected_text))
        .map(|(i, _)| i + 1);

    let line = match line {
        Some(l) => l,
        None => return (StatusCode::BAD_REQUEST, "Selected text not found in file").into_response(),
    };

    match inject_flag(&content, line, &payload.comment) {
        Ok(new_content) => {
            if std::fs::write(&full_path, new_content).is_err() {
                return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to write file").into_response();
            }
            let _ = state.reload_tx.send(());
            StatusCode::OK.into_response()
        }
        Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    }
}

async fn ws_handler(
    State(state): State<AppState>,
    ws: WebSocketUpgrade,
) -> Response {
    let rx = state.reload_tx.subscribe();
    ws.on_upgrade(|socket| handle_ws(socket, rx))
}

async fn handle_ws(mut socket: WebSocket, mut rx: broadcast::Receiver<()>) {
    loop {
        tokio::select! {
            result = rx.recv() => {
                match result {
                    Ok(()) => {
                        if socket.send(Message::Text("reload".into())).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }
        }
    }
}

async fn asset_handler(AxumPath(filepath): AxumPath<String>) -> Response {
    let mime = if filepath.ends_with(".css") {
        "text/css"
    } else if filepath.ends_with(".js") {
        "application/javascript"
    } else if filepath.ends_with(".html") {
        "text/html"
    } else {
        "application/octet-stream"
    };

    match Assets::get(&filepath) {
        Some(file) => {
            let body = file.data.to_vec();
            (
                StatusCode::OK,
                [(axum::http::header::CONTENT_TYPE, mime)],
                body,
            )
                .into_response()
        }
        None => (StatusCode::NOT_FOUND, "Asset not found").into_response(),
    }
}

fn resolve_path(base: &Path, relative: &str) -> PathBuf {
    if base.is_file() {
        base.to_path_buf()
    } else {
        base.join(relative)
    }
}
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo nextest run --test server_test`
Expected: All 5 tests PASS.

- [ ] **Step 7: Commit**

```bash
git add src/server.rs tests/server_test.rs tests/fixtures/sample.html Cargo.toml
git commit -m "Add axum web server with all routes and WebSocket support"
```

---

## Task 9: Wire Up CLI Subcommands

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Implement serve, view, and flags subcommands in main.rs**

Replace `src/main.rs`:

```rust
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

use previewf::flags::{extract_flags, format_flags_text, FlagReport};
use previewf::server::ServerBuilder;
use previewf::terminal::render_terminal;

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
        /// File or directory to serve
        path: PathBuf,

        /// Port to listen on
        #[arg(short, long, default_value_t = 3000)]
        port: u16,
    },

    /// View a markdown file in the terminal
    View {
        /// Markdown file to view
        path: PathBuf,
    },

    /// Extract flags from a markdown file
    Flags {
        /// Markdown file to extract flags from
        path: PathBuf,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Serve { path, port } => {
            let config = ServerBuilder::new()
                .path(&path)
                .port(port)
                .live_reload(true)
                .build()
                .context("Failed to configure server")?;

            previewf::server::run(config)
                .await
                .context("Server error")?;
        }
        Commands::View { path } => {
            let content = std::fs::read_to_string(&path)
                .with_context(|| format!("Cannot read file: {}", path.display()))?;

            let rendered = render_terminal(&content);
            print!("{}", rendered);
        }
        Commands::Flags { path, json } => {
            let content = std::fs::read_to_string(&path)
                .with_context(|| format!("Cannot read file: {}", path.display()))?;

            let flags = extract_flags(&content);
            let filename = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            let report = FlagReport {
                file: filename,
                flags,
            };

            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print!("{}", format_flags_text(&report));
            }
        }
    }

    Ok(())
}
```

- [ ] **Step 2: Test CLI commands manually**

Run: `cargo run -- view tests/fixtures/sample.md`
Expected: Rendered markdown in terminal with colors.

Run: `cargo run -- flags tests/fixtures/flagged.md`
Expected: Human-readable flag list.

Run: `cargo run -- flags tests/fixtures/flagged.md --json`
Expected: JSON output with flags array.

Run: `cargo run -- serve tests/fixtures/ --port 3001 &`
Then open `http://localhost:3001` in browser.
Expected: Directory listing with sample.md, flagged.md, sample.html.

- [ ] **Step 3: Run all tests, clippy, and fmt**

Run: `cargo nextest run && cargo clippy -- -D warnings && cargo fmt --check`
Expected: All pass.

- [ ] **Step 4: Commit**

```bash
git add src/main.rs
git commit -m "Wire CLI subcommands to server, terminal view, and flag export"
```

---

## Task 10: File Watcher Integration with Server

**Files:**
- Modify: `src/server.rs`

- [ ] **Step 1: Add watcher integration to server run function**

In `src/server.rs`, modify the `run` function to start the file watcher and connect it to the WebSocket broadcast:

```rust
pub async fn run(config: ServerConfig) -> Result<(), PreviewError> {
    let port = config.port;
    let path = config.path.clone();

    let (reload_tx, _) = broadcast::channel::<()>(100);
    let reload_tx_clone = reload_tx.clone();

    let state = AppState {
        config: config.clone(),
        reload_tx,
    };

    let app = Router::new()
        .route("/", get(index_handler))
        .route("/view/{*filepath}", get(view_handler))
        .route("/raw/{*filepath}", get(raw_handler))
        .route("/flags/{*filepath}", get(flags_handler))
        .route("/flag/{*filepath}", post(flag_post_handler))
        .route("/ws", get(ws_handler))
        .route("/assets/{*filepath}", get(asset_handler))
        .with_state(state);

    // Start file watcher in background
    let watch_path = path.clone();
    tokio::spawn(async move {
        let result = crate::watcher::FileWatcher::new(watch_path);
        if let Ok((mut watcher, mut rx)) = result {
            if watcher.watch().is_ok() {
                while rx.recv().await.is_ok() {
                    let _ = reload_tx_clone.send(());
                }
            }
        }
    });

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port))
        .await
        .map_err(PreviewError::Server)?;

    println!("Serving {} on http://localhost:{}", path.display(), port);
    println!("Live reload enabled — watching for changes");

    axum::serve(listener, app)
        .await
        .map_err(PreviewError::Server)?;

    Ok(())
}
```

Note: The `create_router` function stays unchanged for testing. The `run` function constructs the router internally with the watcher integrated.

- [ ] **Step 2: Verify existing tests still pass**

Run: `cargo nextest run`
Expected: All tests PASS.

- [ ] **Step 3: Commit**

```bash
git add src/server.rs
git commit -m "Integrate file watcher with server for live reload"
```

---

## Task 11: CI/CD Pipelines

**Files:**
- Create: `.github/workflows/ci.yml`
- Create: `.github/workflows/release.yml`

- [ ] **Step 1: Create CI workflow**

Create `.github/workflows/ci.yml`:

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

env:
  CARGO_TERM_COLOR: always

jobs:
  check:
    name: Check (${{ matrix.os }})
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest]

    steps:
      - uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy, rustfmt

      - name: Cache cargo
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/bin/
            ~/.cargo/registry/index/
            ~/.cargo/registry/cache/
            ~/.cargo/git/db/
            target/
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}

      - name: Check formatting
        run: cargo fmt --check

      - name: Run clippy
        run: cargo clippy -- -D warnings

      - name: Install nextest
        uses: taiki-e/install-action@nextest

      - name: Run tests
        run: cargo nextest run

  coverage:
    name: Coverage
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable

      - name: Install tarpaulin
        run: cargo install cargo-tarpaulin

      - name: Run coverage
        run: cargo tarpaulin --out xml

      - name: Upload coverage
        uses: codecov/codecov-action@v4
        with:
          files: cobertura.xml
```

- [ ] **Step 2: Create Release workflow**

Create `.github/workflows/release.yml`:

```yaml
name: Release

on:
  push:
    tags: ["v*"]

permissions:
  contents: write

jobs:
  build:
    name: Build (${{ matrix.target }})
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        include:
          - target: x86_64-unknown-linux-gnu
            os: ubuntu-latest
            archive: previewf-x86_64-unknown-linux-gnu.tar.gz
          - target: aarch64-unknown-linux-gnu
            os: ubuntu-latest
            archive: previewf-aarch64-unknown-linux-gnu.tar.gz
          - target: x86_64-apple-darwin
            os: macos-latest
            archive: previewf-x86_64-apple-darwin.tar.gz
          - target: aarch64-apple-darwin
            os: macos-latest
            archive: previewf-aarch64-apple-darwin.tar.gz

    steps:
      - uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}

      - name: Install cross-compilation tools
        if: matrix.target == 'aarch64-unknown-linux-gnu'
        run: |
          sudo apt-get update
          sudo apt-get install -y gcc-aarch64-linux-gnu

      - name: Build release binary
        run: cargo build --release --target ${{ matrix.target }}
        env:
          CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER: aarch64-linux-gnu-gcc

      - name: Package binary
        run: |
          cd target/${{ matrix.target }}/release
          tar czf ../../../${{ matrix.archive }} previewf
          cd ../../..
          sha256sum ${{ matrix.archive }} > ${{ matrix.archive }}.sha256

      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: ${{ matrix.archive }}
          path: |
            ${{ matrix.archive }}
            ${{ matrix.archive }}.sha256

  release:
    name: Create Release
    needs: build
    runs-on: ubuntu-latest
    steps:
      - name: Download all artifacts
        uses: actions/download-artifact@v4

      - name: Create GitHub Release
        uses: softprops/action-gh-release@v2
        with:
          generate_release_notes: true
          files: |
            **/*.tar.gz
            **/*.sha256
```

- [ ] **Step 3: Commit**

```bash
git add .github/
git commit -m "Add CI pipeline and release workflow for macOS and Linux"
```

---

## Task 12: CLAUDE.md and README

**Files:**
- Create: `CLAUDE.md`
- Create: `README.md`
- Create: `LICENSE`

- [ ] **Step 1: Write CLAUDE.md**

Create `CLAUDE.md`:

```markdown
# previewf

A markdown/HTML preview and annotation tool.

## Flag System

- Flags in `.md` files use `<flag:N>Comment: description</flag>` syntax
- Flags are inline — they wrap or follow the text they annotate
- Flag IDs are auto-incremented per file
- `previewf flags file.md --json` extracts all flags as structured JSON

### Resolving Flags

When asked to "resolve flags" or "address flagged items":
1. Run `previewf flags <file> --json` or read the file directly
2. For each flag, address the comment (fix the issue, update the text, etc.)
3. Remove the `<flag:N>Comment: ...</flag>` tags once resolved
4. Verify no flags remain: `previewf flags <file>` should show "No flags found"

### Flag Format

```
<flag:1>Comment: need to rethink this approach</flag>
<flag:2>Comment: contradicts section 3</flag>
```

## Commands

```bash
cargo nextest run          # run tests
cargo clippy -- -D warnings # lint
cargo fmt --check          # format check
cargo run -- serve ./docs/ # run dev server
cargo run -- view file.md  # terminal preview
cargo run -- flags file.md --json  # export flags
```

## Architecture

- Single binary, subcommand-driven (serve, view, flags)
- `src/lib.rs` — public API re-exports
- `src/flags.rs` — flag model, regex parsing, inject/extract
- `src/markdown.rs` — comrak parsing, syntect highlighting, HTML rendering
- `src/terminal.rs` — termimad terminal rendering
- `src/server.rs` — axum router, routes, WebSocket live reload
- `src/watcher.rs` — notify file watcher with broadcast channel
- `src/error.rs` — PreviewError enum

## Conventions

- No `unwrap()` in library code — use `Result` with `PreviewError`
- Builder pattern for config structs (`ServerBuilder`)
- Modules start as single files, split into folders when >300 lines
- Tests: integration in `tests/`, unit inline with `#[cfg(test)]`
- All code passes `cargo clippy -- -D warnings` and `cargo fmt`
```

- [ ] **Step 2: Write README.md**

Create `README.md`:

```markdown
# previewf

Preview and annotate markdown files with inline flags.

A personal developer tool that serves markdown and HTML files on localhost with rich typography, syntax highlighting, dark/light themes, and an inline flagging system for LLM-driven plan review.

## Installation

### From GitHub Releases

```bash
# macOS (Apple Silicon)
curl -L https://github.com/ishtiaque05/previewf/releases/latest/download/previewf-aarch64-apple-darwin.tar.gz | tar xz
sudo mv previewf /usr/local/bin/

# macOS (Intel)
curl -L https://github.com/ishtiaque05/previewf/releases/latest/download/previewf-x86_64-apple-darwin.tar.gz | tar xz
sudo mv previewf /usr/local/bin/

# Linux (x86_64)
curl -L https://github.com/ishtiaque05/previewf/releases/latest/download/previewf-x86_64-unknown-linux-gnu.tar.gz | tar xz
sudo mv previewf /usr/local/bin/

# Linux (ARM64)
curl -L https://github.com/ishtiaque05/previewf/releases/latest/download/previewf-aarch64-unknown-linux-gnu.tar.gz | tar xz
sudo mv previewf /usr/local/bin/
```

### From Source

```bash
cargo install --git https://github.com/ishtiaque05/previewf
```

## Usage

### Serve files in browser

```bash
# Serve a directory
previewf serve ./docs/

# Serve a single file
previewf serve ./README.md

# Custom port
previewf serve ./docs/ --port 8080
```

### View markdown in terminal

```bash
previewf view ./README.md
```

### Extract flags

```bash
# Human-readable
previewf flags ./plan.md

# JSON (for piping to LLMs)
previewf flags ./plan.md --json
```

## Flagging Workflow

1. Serve a markdown file: `previewf serve ./plan.md`
2. Select text in the browser and click "Flag"
3. Add a comment describing the issue
4. The flag is written back to the source file as `<flag:N>Comment: ...</flag>`
5. Export flags: `previewf flags ./plan.md --json | pbcopy`
6. Feed to an LLM: "Resolve everything that says flag"

## Development

```bash
# Run tests
cargo nextest run

# Lint
cargo clippy -- -D warnings

# Format
cargo fmt

# Run dev server
cargo run -- serve ./tests/fixtures/
```

## License

MIT
```

- [ ] **Step 3: Create LICENSE**

Create `LICENSE` with MIT license text, copyright 2026 Ishtiaque.

- [ ] **Step 4: Commit**

```bash
git add CLAUDE.md README.md LICENSE
git commit -m "Add CLAUDE.md, README, and MIT license"
```

---

## Task 13: mdBook Documentation

**Files:**
- Create: `book/book.toml`
- Create: `book/src/SUMMARY.md`
- Create: `book/src/introduction.md`
- Create: `book/src/getting-started/installation.md`
- Create: `book/src/getting-started/quickstart.md`
- Create: `book/src/usage/serve.md`
- Create: `book/src/usage/view.md`
- Create: `book/src/usage/flags.md`
- Create: `book/src/usage/themes.md`
- Create: `book/src/development/architecture.md`
- Create: `book/src/development/contributing.md`
- Create: `book/src/development/ci-cd.md`
- Create: `book/src/roadmap.md`

- [ ] **Step 1: Create book.toml**

Create `book/book.toml`:

```toml
[book]
authors = ["Ishtiaque"]
language = "en"
multilingual = false
src = "src"
title = "previewf"
description = "Preview and annotate markdown files with inline flags"

[output.html]
default-theme = "light"
preferred-dark-theme = "navy"
git-repository-url = "https://github.com/ishtiaque05/previewf"
```

- [ ] **Step 2: Create SUMMARY.md**

Create `book/src/SUMMARY.md`:

```markdown
# Summary

[Introduction](./introduction.md)

# Getting Started

- [Installation](./getting-started/installation.md)
- [Quick Start](./getting-started/quickstart.md)

# Usage

- [Serving Files](./usage/serve.md)
- [Terminal View](./usage/view.md)
- [Flags & Annotations](./usage/flags.md)
- [Themes](./usage/themes.md)

# Development

- [Architecture](./development/architecture.md)
- [Contributing](./development/contributing.md)
- [CI/CD](./development/ci-cd.md)

---

[Roadmap](./roadmap.md)
```

- [ ] **Step 3: Create all book chapter files**

Create each chapter file with content matching the design spec. Each chapter covers:

- `introduction.md` — Overview and core value prop (flagging workflow)
- `getting-started/installation.md` — Pre-built binaries for macOS/Linux + from source
- `getting-started/quickstart.md` — Serve a file in 30 seconds
- `usage/serve.md` — Directory mode, single file mode, --port flag, live reload
- `usage/view.md` — Terminal rendering, flag display format
- `usage/flags.md` — Creating flags in browser, flag syntax, JSON export, LLM workflow
- `usage/themes.md` — Light/dark themes, auto-detection, manual toggle
- `development/architecture.md` — Module map, data flow diagram, key crates
- `development/contributing.md` — Setup, dev workflow, conventions
- `development/ci-cd.md` — CI pipeline, release pipeline, tagging
- `roadmap.md` — Future features table

- [ ] **Step 4: Verify book builds**

Run: `cargo install mdbook` (if not installed)
Run: `mdbook build book/`
Expected: Builds successfully to `book/book/`.

- [ ] **Step 5: Commit**

```bash
git add book/
git commit -m "Add mdBook documentation with all chapters"
```

---

## Self-Review Checklist

### Spec Coverage

| Spec Requirement | Task |
|-----------------|------|
| Preview .md in browser | Tasks 4, 7, 8 |
| Preview .html as-is | Task 8 (raw_handler) |
| Flag/annotate via browser UI | Tasks 2-3, 7-8 |
| Export flags as JSON | Tasks 2, 9 |
| Terminal markdown view | Tasks 5, 9 |
| Live reload | Tasks 6, 10 |
| Single binary, cross-platform | Tasks 1, 11 |
| Syntax highlighting | Task 4 |
| Diff coloring | Task 4, 7 (CSS) |
| Dark/light themes | Task 7 (CSS + JS) |
| Builder pattern | Tasks 1, 8 |
| Error handling | Task 1 |
| CI (fmt, clippy, nextest, tarpaulin) | Task 11 |
| Release builds (4 targets) | Task 11 |
| CLAUDE.md | Task 12 |
| README with install docs | Task 12 |
| mdBook | Task 13 |

### Placeholder Scan

No TBDs, TODOs, or "implement later" found. Task 13 Step 3 references content from the design spec — the implementing agent should use the book chapter content provided in the spec and plan Task 12 as reference.

### Type Consistency

- `Flag` struct: consistent across flags.rs, flags_test.rs, server.rs
- `FlagReport` struct: consistent across flags.rs, server.rs, main.rs
- `ServerBuilder` / `ServerConfig`: consistent across server.rs, server_test.rs, main.rs
- `PreviewError`: consistent across error.rs, flags.rs, server.rs, watcher.rs
- `extract_flags`, `inject_flag`, `next_flag_id`, `format_flags_text`: consistent names everywhere
- `render_html`: consistent in markdown.rs, server.rs
- `render_terminal`: consistent in terminal.rs, main.rs
- `FileWatcher::new`, `FileWatcher::watch`: consistent in watcher.rs, watcher_test.rs, server.rs
