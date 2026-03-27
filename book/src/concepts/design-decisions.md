# Design Decisions

Every project is shaped by a series of decisions, some deliberate and some accidental. previewf is a personal developer tool and a Rust learning project, so the decisions here reflect both practical constraints and educational goals. This chapter documents the significant choices, the alternatives we considered, and the reasoning that led to each decision.

## Why Rust

### The Decision

Build previewf in Rust rather than Go, Python, or TypeScript/Node.

### The Reasoning

**Learning goal.** The primary motivation. previewf is explicitly a project for learning Rust idioms, patterns, and ecosystem tools. Choosing a familiar language would defeat the purpose.

**Single binary distribution.** Rust compiles to a single static binary with no runtime dependencies. No Python virtualenv, no Node.js installation, no Go runtime (though Go also produces single binaries). The user downloads one file, moves it to `/usr/local/bin/`, and they are done.

**Performance.** Markdown parsing, syntax highlighting, and file watching all benefit from native performance. For a personal tool running locally, this is less critical than for a server, but responsiveness matters for the user experience -- especially live reload.

**Type safety.** Rust's type system catches entire classes of errors at compile time. The `Result<T, E>` pattern forces explicit error handling. The borrow checker prevents data races. For a tool that modifies files on disk (flag injection), this safety net is valuable.

### The Trade-offs

**Borrow checker friction.** Rust's ownership model has a learning curve. Simple operations like "read a file, modify it, write it back" require understanding lifetimes and borrowing. This is a feature (it prevents bugs) but also a cost (it slows initial development).

**Async complexity.** The web server (axum) runs on tokio, which means the project has async/await throughout the server code. Async Rust is more complex than sync Rust, with Send/Sync bounds, pinning, and runtime selection. For a personal tool, sync code would have been simpler, but learning async was a goal.

**Ecosystem maturity.** Rust's web ecosystem is mature but smaller than Node.js or Python. Finding the right crate for each job requires research. The payoff is that chosen crates tend to be well-designed and well-tested.

### Alternatives Considered

| Language | Pros | Cons | Why not |
|----------|------|------|---------|
| Go | Single binary, fast compile, simple concurrency | Less type expressiveness, no sum types | Does not serve the learning goal |
| Python | Fast to prototype, huge ecosystem | Requires runtime, slow startup, distribution pain | Distribution and performance |
| TypeScript/Node | Vast ecosystem, easy web dev | Requires Node, large node_modules, startup time | Not a learning target; distribution complexity |

## Why axum over actix-web

### The Decision

Use axum as the web framework instead of actix-web.

### The Reasoning

**Ecosystem integration.** axum is built by the tokio team and integrates natively with the tokio ecosystem. Since we already use tokio for async runtime, file watching (notify), and WebSocket (tokio-tungstenite), axum is the natural fit. No adapter layers, no runtime conflicts.

**Community momentum.** At the time of this decision, axum had approximately 4x more monthly downloads on crates.io than actix-web. This translates to more examples, more Stack Overflow answers, more blog posts, and more maintained middleware.

**Tower middleware.** axum is built on Tower, the standard middleware framework in the Rust ecosystem. This means we get tower-http's static file serving, CORS handling, and other middleware for free, with composable layers.

**API design.** axum uses extractors (function parameters) to inject request data into handlers. This is more idiomatic Rust than actix-web's macro-heavy approach. A handler like `async fn view(State(state): State<AppState>, Path(filepath): Path<String>) -> Response` is self-documenting.

### The Trade-offs

actix-web has longer track record, slightly more mature WebSocket support, and better raw throughput in benchmarks. For a personal tool serving localhost, benchmark differences are irrelevant. The ergonomic and ecosystem advantages of axum dominate.

## Why comrak over pulldown-cmark

### The Decision

Use comrak for markdown parsing instead of pulldown-cmark, despite pulldown-cmark having roughly 20x more downloads.

### The Reasoning

This was the most carefully considered technical decision in the project.

**AST manipulation.** comrak parses markdown into a full Abstract Syntax Tree (AST) that you can traverse, inspect, and modify before rendering to HTML. This is essential for the flag system: we need to find flag tags in the AST, convert them to styled spans, and handle code blocks differently from prose.

**pulldown-cmark is a streaming parser.** It emits a stream of events (`Start(Heading)`, `Text("hello")`, `End(Heading)`) that you process sequentially. This is efficient for simple rendering but makes it difficult to modify nodes in-place or look ahead/behind. Injecting flag UI elements into the stream would require buffering events, tracking state, and carefully re-emitting modified events -- essentially building our own AST on top of a streaming parser.

**CommonMark compliance.** comrak implements the full CommonMark spec plus GitHub Flavored Markdown extensions (tables, strikethrough, autolinks, task lists, footnotes). pulldown-cmark also covers these but comrak's compliance is stricter.

**Unsafe HTML mode.** comrak's `options.render.unsafe_ = true` passes raw HTML through the pipeline. This is how flag tags (`<flag:N>`) survive the markdown-to-HTML conversion. pulldown-cmark also supports raw HTML, but the streaming model makes it harder to post-process specific HTML patterns.

### Why Downloads Are Misleading

pulldown-cmark's 20x download advantage is largely because it is a dependency of other popular crates (mdbook itself uses it). Many of those downloads are transitive, not direct usage decisions. For our specific use case -- AST manipulation with post-processing -- comrak is the right tool.

### The Trade-offs

**Performance.** pulldown-cmark is faster for simple rendering because streaming avoids building a full AST. For a personal tool previewing one file at a time, the difference is imperceptible.

**Memory.** A full AST uses more memory than a streaming parser. Again, for single-file previewing, this is irrelevant.

**API complexity.** comrak's AST API is more complex than pulldown-cmark's event stream. You work with arena-allocated nodes, recursive traversal, and mutable references. This is more to learn but provides more power.

## Why syntect over highlight.js

### The Decision

Use syntect for syntax highlighting (server-side, in Rust) rather than highlight.js (client-side, in JavaScript).

### The Reasoning

**One engine, two outputs.** syntect can produce both HTML spans (for the browser) and ANSI escape codes (for the terminal). With highlight.js, we would need a separate solution for terminal highlighting, meaning two highlighting engines to maintain and keep consistent.

**No JavaScript bundle.** Server-side highlighting means the rendered HTML contains pre-colored `<span>` elements. The browser does not need to download, parse, or execute any highlighting JavaScript. This makes page loads faster, reduces complexity, and works even with JavaScript disabled.

**Language coverage.** syntect uses Sublime Text syntax definitions, which cover virtually every programming language. The default bundle includes Rust, Python, JavaScript, Go, Ruby, SQL, YAML, and many more.

**Consistency.** Highlighting is deterministic and identical on every render because it happens server-side. No flash-of-unstyled-code in the browser, no race between HTML load and JS execution.

### The Trade-offs

**Theme flexibility.** With highlight.js, users can switch themes via CSS. With syntect, the theme is baked into the HTML spans at render time. We use a fixed theme ("base16-ocean.dark") that works with both our light and dark CSS themes. Supporting user-selectable syntax themes would require re-rendering on the server.

**Build size.** syntect's default syntax bundle adds to the binary size. This is acceptable for a personal tool.

## Why a Monolith (Approach A) over a Workspace (Approach B)

### The Decision

Structure the project as a single crate with modules (`src/flags.rs`, `src/server.rs`, etc.) rather than a Cargo workspace with multiple crates.

### The Reasoning

**Fastest path to working tool.** A single crate means one `Cargo.toml`, one compilation unit, no inter-crate dependency management. You can `use crate::flags::extract_flags` from `server.rs` without declaring a workspace dependency.

**Best for learning.** Workspaces introduce coordination overhead: shared dependencies, publish ordering, version alignment. For a learning project, this overhead provides no value until the codebase is large enough that build times or module boundaries demand it.

**The 300-line rule.** We established a simple growth rule: modules start as single `.rs` files. When any module exceeds approximately 300 lines, split it into a directory module (`module/mod.rs` + sub-files). When the total crate exceeds what feels manageable, extract into a workspace. Do not pre-split.

### The Migration Path

```
Phase 1 (current): Single crate
  src/flags.rs      (~100 lines)
  src/markdown.rs   (~100 lines)
  src/server.rs     (~200 lines)
  src/watcher.rs    (~60 lines)
  src/terminal.rs   (~30 lines)

Phase 2 (when modules grow): Directory modules
  src/flags/mod.rs
  src/flags/parse.rs
  src/flags/inject.rs
  src/flags/export.rs

Phase 3 (when warranted): Workspace
  crates/previewf-core/     (flags, markdown)
  crates/previewf-server/   (axum, websocket)
  crates/previewf-cli/      (clap, terminal)
```

We are currently in Phase 1. The migration trigger is developer pain, not arbitrary metrics.

## Why Editorial Design

### The Decision

Design the browser UI with editorial typography (serif fonts, warm colors, constrained line length) rather than a developer-tool aesthetic (monospace, dark, dense).

### The Reasoning

**Reading is the primary activity.** previewf is a reading tool. You use it to read documents carefully, looking for issues to flag. The typography should optimize for sustained reading, not code editing.

**Typography matters for comprehension.** Research on reading shows that serif fonts at appropriate sizes with constrained line lengths (60-75 characters) improve reading speed and comprehension. The 72-character max-width is not arbitrary -- it is the upper end of the optimal range.

**The font choices are deliberate:**

| Font | Role | Why |
|------|------|-----|
| Playfair Display | Headings | High contrast, elegant serifs. Creates clear visual hierarchy. |
| Source Serif 4 | Body text | Designed for screen reading. Open counters, generous x-height. Adobe's answer to Georgia. |
| JetBrains Mono | Code | Designed for code. Ligatures, clear distinction between similar characters (0/O, 1/l/I). |
| DM Sans | Flag comments | Clean sans-serif. Distinguishes annotations from document content. |

**The color system is intentional:**

- **Light theme ("Warm Paper"):** `#FAF8F5` background (warm cream, not harsh white). `#2D2D2D` text (soft black, not `#000000`). This reduces eye strain.
- **Dark theme ("Midnight Ink"):** `#1A1A2E` background (deep navy, not pure black). `#E8E6E3` text (warm white). Navy dark themes are easier on the eyes than pure black.
- **Accent color:** `#C45B28` (burnt orange in light mode), `#E8845A` (coral in dark mode). Warm accent that draws attention without screaming.
- **Flag colors:** Warm yellow/gold. Distinctive but not alarming. Flags are annotations, not errors.

## Why the Builder Pattern

### The Decision

Use the builder pattern for configuration structs (`ServerBuilder`, `FlagBuilder`) rather than constructors with many parameters.

### The Reasoning

**Readable construction.** Compare:

```rust
// Without builder (positional args, unclear at call site)
let config = ServerConfig::new("./docs/", 3000, true);

// With builder (named, self-documenting)
let config = ServerBuilder::new()
    .path("./docs/")
    .port(3000)
    .live_reload(true)
    .build()?;
```

The builder version is longer but every parameter is named at the call site. You do not need to remember parameter order or look up the function signature.

**Optional fields with defaults.** The builder provides sensible defaults (port 3000, live reload enabled) and lets you override only what you need:

```rust
// Use all defaults except path
let config = ServerBuilder::new()
    .path("./docs/")
    .build()?;
```

**Validation at build time.** The `build()` method returns `Result`, so it can validate the configuration and return meaningful errors. A required field like `path` can be checked: if it is `None`, build returns `Err(PreviewError::FileNotFound)`.

**Rust idiom.** The builder pattern is widely used in the Rust ecosystem (reqwest, tonic, clap). Using it here reinforces learning the pattern.

## Why TDD

### The Decision

Use test-driven development throughout, with tests written before implementation.

### The Reasoning

**Behavior needs tests, not just types.** Rust's type system catches many errors (null pointer, data races, type mismatches), but it cannot catch behavioral bugs. A function that parses flags might compile perfectly but return wrong line numbers. Tests catch that.

**The tool stack:**

| Tool | Role |
|------|------|
| cargo-nextest | Parallel test runner, faster than `cargo test` for projects with many test files |
| insta | Snapshot testing -- captures output as `.snap` files, makes it easy to review changes |
| mockall | Trait mocking for testing modules in isolation |

**TDD rhythm:** For each feature, the implementation plan specifies:

1. Write failing tests
2. Run tests, confirm they fail (for the right reason)
3. Implement the feature
4. Run tests, confirm they pass
5. Run clippy and fmt
6. Commit

This rhythm ensures that tests are not an afterthought and that every public function has at least one test.

## Why thiserror + anyhow

### The Decision

Use `thiserror` for the library error type (`PreviewError`) and `anyhow` for the application entry point.

### The Reasoning

**Two layers of error handling.**

The library code (flags.rs, markdown.rs, server.rs) uses `PreviewError`, a typed enum where each variant has structured data:

```rust
#[derive(Debug, thiserror::Error)]
pub enum PreviewError {
    #[error("File not found: {0}")]
    FileNotFound(PathBuf),
    #[error("Invalid flag syntax at line {line}: {detail}")]
    FlagParse { line: usize, detail: String },
    // ...
}
```

The application code (main.rs) uses `anyhow::Result`, which wraps any error with context:

```rust
let content = std::fs::read_to_string(&path)
    .with_context(|| format!("Cannot read file: {}", path.display()))?;
```

**Why both?** `thiserror` gives you typed errors that callers can match on. `anyhow` gives you ergonomic error chaining for the top-level application where you just want to print a helpful message and exit. The library should be precise; the application should be helpful.

## Why This Flag Format Specifically

### The Decision

Use `<flag:N>Comment: description</flag>` rather than any of the alternatives listed in the Flag System chapter.

### The Key Insight

The format must work with comrak's `unsafe_` HTML mode. When comrak sees `<flag:1>Comment: text</flag>` in markdown, it treats it as raw HTML and passes it through to the output. This means:

1. No special parser modification needed
2. The tag appears in the rendered HTML exactly as written
3. A post-processing regex can find and style it
4. The same regex works on both raw markdown (for `extract_flags`) and rendered HTML (for `render_flag_spans`)

The `Comment:` prefix inside the tag serves as a human-readable label and a regex anchor. Without it, the regex would need to be more permissive, potentially matching unrelated HTML-like content.

The numeric ID after `flag:` enables referencing specific flags ("look at flag 3") and ordering them in the sidebar. The closing `</flag>` tag provides an unambiguous boundary for the regex.
