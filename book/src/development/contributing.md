# Contributing

previewf is a personal developer tool and a Rust learning project. Contributions are welcome, whether they are bug fixes, new features, documentation improvements, or code quality enhancements. This guide covers the development workflow, coding standards, and how to get a change merged.

## Development Setup

### Prerequisites

| Tool | Version | Install |
|------|---------|---------|
| Rust | 1.70+ | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| clippy | (ships with rustup) | `rustup component add clippy` |
| rustfmt | (ships with rustup) | `rustup component add rustfmt` |
| cargo-nextest | latest | `cargo install cargo-nextest` |
| cargo-tarpaulin | latest (Linux only) | `cargo install cargo-tarpaulin` |
| mdbook | latest | `cargo install mdbook` |

### Clone and Build

```bash
git clone https://github.com/ishtiaque05/previewf.git
cd previewf
cargo build
```

### Run Tests

```bash
cargo nextest run
```

### Run Linters

```bash
cargo clippy -- -D warnings
cargo fmt --check
```

### Build Documentation

```bash
cd book
mdbook serve
```

## Project Conventions

### Module Structure

Modules start as single `.rs` files:

```
src/flags.rs       (one file)
```

When a module exceeds ~300 lines, split into a directory:

```
src/flags/
  mod.rs           (re-exports)
  parse.rs         (extraction logic)
  inject.rs        (injection logic)
  export.rs        (formatting and serialization)
```

The `mod.rs` re-exports everything so external callers do not need to change their imports.

### Code Style

The project uses `rustfmt` with a custom configuration (`rustfmt.toml`):

```toml
max_width = 100
tab_spaces = 4
edition = "2021"
```

Key style guidelines:

- **No `unwrap()` in library code.** Use `Result` and `?` for error propagation. `unwrap()` is only acceptable for compile-time-known patterns (like static regex) and test code.

- **Use `Result<T, PreviewError>` for library functions.** The error type should be `PreviewError` for functions in the library modules.

- **Use `anyhow::Result` in `main.rs`.** The application entry point uses anyhow for ergonomic error context chaining.

- **Descriptive variable names.** Prefer `flag_count` over `fc`, `file_content` over `s`.

- **Doc comments on public items.** Every public function, struct, and enum should have a `///` doc comment explaining what it does.

### Error Handling

Follow the two-layer pattern:

| Layer | Tool | Location |
|-------|------|----------|
| Library | `thiserror` + `PreviewError` | `src/*.rs` |
| Application | `anyhow` + `.with_context()` | `src/main.rs` |

When adding a new error case, add a variant to `PreviewError` in `src/error.rs`:

```rust
#[derive(Debug, thiserror::Error)]
pub enum PreviewError {
    // ... existing variants ...

    #[error("Your new error: {0}")]
    NewError(String),
}
```

### Testing

Follow the TDD workflow:

1. Write a failing test first
2. Run the test, verify it fails for the right reason
3. Implement the feature
4. Run the test, verify it passes
5. Run clippy and fmt
6. Commit

Tests go in two places:

- **Integration tests:** `tests/<module>_test.rs` for public API tests
- **Unit tests:** `#[cfg(test)] mod tests` inside source files for internal logic

### Commit Messages

Use conventional commit style:

```
Add flag injection with line validation
Fix terminal rendering of nested bold text
Update CSS for better mobile sidebar behavior
```

Each commit should be a self-contained, compilable change. Do not commit code that does not compile or fails tests.

## Development Workflow

### Adding a New Feature

1. **Write the test first.** Add tests to the appropriate test file or create a new one.

2. **Run the test.** Verify it fails:
   ```bash
   cargo nextest run --test <test_file>
   ```

3. **Implement the feature.** Add code to the relevant module.

4. **Run all tests.** Make sure nothing is broken:
   ```bash
   cargo nextest run
   ```

5. **Run quality checks.**
   ```bash
   cargo clippy -- -D warnings
   cargo fmt --check
   ```

6. **Test manually.** For UI changes, start the server and verify in the browser:
   ```bash
   cargo run -- serve tests/fixtures
   ```

7. **Commit.**

### Modifying the Flag System

The flag system has several interconnected components:

```
flags.rs: extract_flags, inject_flag, next_flag_id, format_flags_text
    |
    +-- Used by: server.rs (route handlers)
    +-- Used by: markdown.rs (render_flag_spans)
    +-- Used by: main.rs (CLI commands)
    +-- Tested by: tests/flags_test.rs
```

If you change the flag format (the regex), you need to update:

1. `extract_flags` regex in `flags.rs`
2. `render_flag_spans` regex in `markdown.rs`
3. `prepare_flags_for_terminal` regex in `terminal.rs`
4. Test fixtures in `tests/fixtures/flagged.md`
5. All affected tests

### Modifying the Frontend

Frontend assets are in the `assets/` directory:

- `style.css`: All CSS. Edit carefully -- the theme system uses CSS custom properties, and changes can affect both light and dark modes.
- `app.js`: Flag UI, theme toggle, WebSocket. Uses safe DOM methods (no innerHTML).
- `document.html`: Template for the document viewer. Uses `{{placeholder}}` syntax.
- `index.html`: Template for the directory listing.

After modifying assets, rebuild with `cargo build` (rust-embed recompiles the assets into the binary).

**Security note:** The JavaScript uses safe DOM methods (`textContent`, `createElement`, `appendChild`) instead of `innerHTML` to prevent XSS attacks. When adding new DOM manipulation, always use safe methods.

### Adding a New Route

1. Define the handler function in `server.rs`
2. Add the route to the `Router` in `create_router`
3. Add integration tests in `tests/server_test.rs`
4. If the route uses a template, add the template to `assets/`

## What Makes a Good Contribution

- **Fixes a real problem.** Bug fixes, performance improvements, or UX enhancements that address actual pain points.

- **Follows existing patterns.** Uses the same error handling, testing, and code organization patterns as the rest of the codebase.

- **Includes tests.** Every behavioral change should have tests. Every bug fix should have a regression test.

- **Does not expand scope unnecessarily.** Check the roadmap before adding features. Non-goal features (HTML flagging, multiple flag categories, etc.) should be discussed before implementation.

- **Is well-documented.** Public API changes should include doc comments. Significant changes should update this book.

## Getting Help

- Read the design spec: `docs/previewf/specs/2026-03-26-previewf-design.md`
- Read the implementation plan: `docs/previewf/plans/2026-03-26-previewf-implementation.md`
- Read the `CLAUDE.md` file for LLM-focused project context
- Browse the test files to understand expected behavior
