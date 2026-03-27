# previewf — Design Specification

**Date:** 2026-03-26
**Status:** Approved
**Author:** Syed + Claude

## Overview

`previewf` is a personal developer tool for previewing and annotating markdown and HTML files. It serves files on localhost with a beautifully typeset reading experience, supports inline flagging of markdown content for LLM-driven plan review, and provides a terminal-based markdown viewer.

### Goals

- Preview `.md` files in a browser with rich typography, syntax highlighting, dark/light mode
- Preview `.html` files as-is in the browser (read-only, no flagging)
- Flag/annotate lines in markdown files via the browser UI, persisted as `<flag:N>` tags in the source file
- Export flags as structured JSON for piping to LLMs
- View markdown in the terminal with color rendering
- Live reload on file changes
- Single binary, cross-platform (macOS + Linux)
- Serve as a Rust learning project

### Non-Goals (Roadmap / Future)

- CLI editing mode (edit flags from terminal)
- HTML file flagging
- Multiple flag categories (flag, todo, question)
- Flag resolution tracking (resolved/unresolved)
- Homebrew formula
- Custom CSS themes / user stylesheets
- PDF export

---

## Architecture

### Approach

Single binary (Approach A) with a path to workspace extraction (Approach B) when modules grow. Subcommand-driven via `clap`.

### CLI Interface

```bash
previewf serve ./docs/              # serve directory on localhost:3000
previewf serve ./file.md            # serve single file
previewf serve ./docs/ --port 8080  # custom port
previewf view ./file.md             # render markdown in terminal
previewf flags ./file.md            # extract all flags (human-readable)
previewf flags ./file.md --json     # extract all flags as JSON
previewf book                       # build and serve the mdBook
```

### Project Structure

```
previewf/
├── Cargo.toml
├── rustfmt.toml
├── .github/workflows/
│   ├── ci.yml
│   └── release.yml
├── src/
│   ├── main.rs              # entry + clap CLI definition
│   ├── lib.rs               # public API, re-exports
│   ├── server.rs            # axum server, routes, websocket
│   ├── markdown.rs          # parsing + rendering (comrak, syntect)
│   ├── flags.rs             # flag model, inject, extract
│   └── watcher.rs           # file watching
├── assets/
│   ├── index.html
│   ├── style.css
│   └── app.js
├── tests/
│   ├── server_test.rs
│   ├── flags_test.rs
│   └── fixtures/
│       ├── sample.md
│       ├── flagged.md
│       └── sample.html
├── book/
│   ├── book.toml
│   └── src/
│       ├── SUMMARY.md
│       ├── getting-started/
│       │   ├── installation.md
│       │   └── quickstart.md
│       ├── usage/
│       │   ├── serve.md
│       │   ├── view.md
│       │   ├── flags.md
│       │   └── themes.md
│       ├── development/
│       │   ├── architecture.md
│       │   ├── contributing.md
│       │   └── ci-cd.md
│       └── roadmap.md
├── CLAUDE.md
├── README.md
└── LICENSE
```

**Module growth rule:** Modules start as single `.rs` files. When a module exceeds ~300 lines, split into a directory (`module/mod.rs` + sub-files). Do not pre-split.

### Builder Pattern

Used for configuration structs with optional fields:

```rust
let server = ServerBuilder::new()
    .path("./docs/")
    .port(3000)
    .live_reload(true)
    .build()?;

server.run().await?;
```

```rust
let flag = FlagBuilder::new()
    .source_file("./plan.md")
    .comment("need to revisit this section")
    .line_range(10..15)
    .build()?;
```

---

## Data Flow

### Web Request Flow

```
Browser request -> axum router
  ├── GET /              -> file listing (directory mode) or single file view
  ├── GET /view/:path    -> parse .md with comrak -> render HTML with flag UI
  ├── GET /raw/:path     -> serve .html files as-is (preview only)
  ├── POST /flag/:path   -> receive flag data -> inject <flag:N> into source .md
  ├── GET /flags/:path   -> return all flags as JSON
  └── WS /ws             -> websocket for live reload notifications

File watcher (notify) -> detects .md/.html change -> broadcasts via WS -> browser reloads
```

### Flag Format

Flags are inline annotations in the markdown source file:

```markdown
Some normal text here.

This line has a <flag:1>Comment: need to rethink this approach</flag> problem.

Another <flag:2>Comment: contradicts section 3</flag> section.
```

### Flag Export

`previewf flags ./plan.md --json` produces:

```json
{
  "file": "plan.md",
  "flags": [
    {
      "id": 1,
      "line": 3,
      "text": "This line has a ... problem.",
      "comment": "need to rethink this approach"
    },
    {
      "id": 2,
      "line": 5,
      "text": "Another ... section.",
      "comment": "contradicts section 3"
    }
  ]
}
```

### Terminal View

- `termimad` renders markdown with colors, bold, code blocks
- Flags highlighted with distinct color, comment shown inline
- Read-only (editing is future roadmap)

---

## Frontend Design: "The Annotated Page"

### Aesthetic Direction

Editorial/literary — a well-typeset technical book meets a refined code review tool. Clean, warm, highly readable.

### Typography

| Role | Font | Fallback |
|------|------|----------|
| Headings | Playfair Display | Georgia, serif |
| Body text | Source Serif 4 | Charter, serif |
| Code / monospace | JetBrains Mono | Menlo, monospace |
| Flag comments | DM Sans | system-ui, sans-serif |

All loaded from Google Fonts.

### Color System

```
                    Light ("Warm Paper")         Dark ("Midnight Ink")
────────────────────────────────────────────────────────────────────
--bg                #FAF8F5 (warm cream)         #1A1A2E (deep navy)
--bg-surface        #FFFFFF                      #16213E
--text              #2D2D2D (soft black)         #E8E6E3 (warm white)
--text-muted        #6B6B6B                      #8B8FA3
--accent            #C45B28 (burnt orange)       #E8845A (coral)
--flag-bg           #FFF3CD (warm yellow)        #3D2E1F (dark amber)
--flag-border       #E6A817 (gold)               #D4952B
--code-bg           #F5F2EF                      #0F0F23
--link              #1A6B4F (forest green)       #4ECDC4 (teal)
--sidebar-bg        #F0EDE8                      #12122A
```

### Diff Coloring (Git-style)

```
                    Light mode                   Dark mode
────────────────────────────────────────────────────────────────────
+ added bg          #DAFBE1 (soft green)         #1B3829 (deep green)
+ added text        #1A7F37                      #3FB950
- removed bg        #FFE2DD (soft red)           #3D1F1F (deep red)
- removed text      #CF222E                      #F85149
@@ hunk header bg   #EDE8FD (soft purple)        #2D1F4E (deep purple)
@@ hunk header      #6639BA                      #BC8CFF
```

Detection: Language tag `diff`, or content with `@@` hunk headers + `+`/`-` prefixes.

### Syntax Highlighting

Server-side via `syntect`. Supports all major languages (ruby, js, rust, python, go, sql, yaml, etc.). Same engine for both web and terminal output (HTML spans for web, ANSI codes for terminal).

### Layout

```
┌──────────────────────────────────────────────────────────┐
│  previewf    [file path]                  [sun/moon] [3] │  top bar
├────────────────────────────────┬─────────────────────────┤
│                                │                         │
│   # Document Title             │  FLAGS                  │
│                                │                         │
│   Body text in Source Serif,   │  #1 line 14             │
│   max-width 72ch for optimal   │  "need to rethink..."   │
│   reading line length.         │                         │
│                                │  #2 line 28             │
│   Flagged text highlighted     │  "contradicts S3"       │
│   with warm underline +        │                         │
│   subtle left-border glow.     │  + Add flag             │
│                                │                         │
│   ```rust                      │                         │
│   fn main() {                  │                         │
│       println!("hello");       │                         │
│   }                            │                         │
│   ```                          │                         │
│                                │                         │
├────────────────────────────────┴─────────────────────────┤
│  previewf v0.1.0 · watching · connected                  │  status bar
└──────────────────────────────────────────────────────────┘
```

- Reading column: max `72ch`, centered
- Flag sidebar: slides in from right, collapsible
- Flag creation: select text -> floating toolbar -> comment input inline
- Theme toggle: `prefers-color-scheme` default, manual override in `localStorage`
- Live reload indicator: green dot pulses on reload, red on disconnect

### Directory Listing View

```
┌──────────────────────────────────────────┐
│  previewf   ~/docs/            [sun/moon]│
├──────────────────────────────────────────┤
│                                          │
│   * architecture.md        3 flags       │
│   * plan.md                1 flag        │
│   * readme.md              --            │
│   o preview.html           (html)        │
│   o report.html            (html)        │
│                                          │
│   4 markdown · 2 html                    │
└──────────────────────────────────────────┘
```

### Animations

- Page load: content fade-in, 200ms stagger per section
- Flag highlight: 300ms ease-in background transition on hover
- Theme switch: 300ms transition on all color properties
- WebSocket reconnect: status dot throb red -> green
- File list items: slide-in with 50ms stagger

---

## Error Handling

### Error Type

```rust
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

- CLI: human-readable errors via `anyhow` for context chaining
- Web routes: proper HTTP status codes (404, 400, 500) with styled error page
- No `unwrap()` in library code — `Result` everywhere

---

## Testing Strategy

| Layer | What | Tool |
|-------|------|------|
| Unit tests | Flag parsing, injection, extraction, markdown rendering | `cargo test` / `nextest`, `mockall` |
| Snapshot tests | Rendered HTML output, flag JSON export | `insta` |
| Integration tests | Server routes, WebSocket connection | `axum::test` helpers, `wiremock` |
| Fixture files | Sample `.md` and `.html` files | `tests/fixtures/` |

---

## CI/CD

### CI Pipeline (`.github/workflows/ci.yml`)

Triggers: push to main, all PRs.
Matrix: `ubuntu-latest`, `macos-latest`.

```
- cargo fmt --check
- cargo clippy -- -D warnings
- cargo nextest run
- cargo tarpaulin --out xml    (Linux only)
```

### Release Pipeline (`.github/workflows/release.yml`)

Triggers: push tag `v*`.

Build targets:
- `x86_64-unknown-linux-gnu` (Linux x86)
- `aarch64-unknown-linux-gnu` (Linux ARM)
- `x86_64-apple-darwin` (macOS Intel)
- `aarch64-apple-darwin` (macOS Apple Silicon)

Each target: `cargo build --release` -> tar.gz -> upload to GitHub Release with SHA256 checksums.

### Installation

```bash
# macOS Apple Silicon
curl -L https://github.com/user/previewf/releases/latest/download/previewf-aarch64-apple-darwin.tar.gz | tar xz
sudo mv previewf /usr/local/bin/

# macOS Intel
curl -L https://github.com/user/previewf/releases/latest/download/previewf-x86_64-apple-darwin.tar.gz | tar xz
sudo mv previewf /usr/local/bin/

# Linux x86
curl -L https://github.com/user/previewf/releases/latest/download/previewf-x86_64-unknown-linux-gnu.tar.gz | tar xz
sudo mv previewf /usr/local/bin/

# From source
cargo install --git https://github.com/user/previewf
```

---

## Crate Dependencies

### Runtime

| Crate | Version | Role |
|-------|---------|------|
| `tokio` | latest | Async runtime |
| `axum` | latest | Web framework |
| `clap` | latest | CLI parsing (derive API) |
| `comrak` | latest | Markdown parsing + AST manipulation |
| `syntect` | latest | Syntax highlighting (web + terminal) |
| `termimad` | latest | Terminal markdown rendering |
| `notify` | latest | File watching |
| `rust-embed` | latest | Embed static assets in binary |
| `tower-http` | latest | Static file serving, CORS |
| `tokio-tungstenite` | latest | WebSocket support |
| `serde` / `serde_json` | latest | Serialization |
| `thiserror` | latest | Library error types |
| `anyhow` | latest | Application error context |

### Dev / Test

| Crate | Role |
|-------|------|
| `mockall` | Trait mocking |
| `insta` | Snapshot testing |
| `wiremock` | HTTP server mocking |

### Tooling

| Tool | Role |
|------|------|
| `clippy` | Linter (official) |
| `rustfmt` | Formatter (official) |
| `cargo-nextest` | Fast parallel test runner |
| `cargo-tarpaulin` | Code coverage |
| `mdbook` | Documentation book |

---

## CLAUDE.md

The project includes a `CLAUDE.md` file that teaches LLMs how to work with previewf and its flag system:

- Flag syntax: `<flag:N>Comment: description</flag>`
- How to resolve flags when asked
- Build/test/lint commands
- Architecture overview and conventions
- Module growth rules and patterns

---

## Roadmap (Future Phases)

| Feature | Phase |
|---------|-------|
| CLI editing mode | Future |
| HTML file flagging | Future |
| Multiple flag categories | Future |
| Flag resolution tracking | Future |
| Homebrew formula | Future |
| Custom CSS themes | Future |
| PDF export | Future |
