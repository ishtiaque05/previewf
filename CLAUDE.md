# previewf

A markdown/HTML preview and annotation tool.

## Flag System

- Flags in `.md` files use `<flag:N>Label: description</flag>` syntax
- The label is a category: Comment (default), Bug, Todo, Question, Note, Style, or custom
- Flags are inline — they wrap or follow the text they annotate
- Flag IDs are auto-incremented per file
- `previewf flags file.md --json` extracts all flags as structured JSON

### Resolving Flags

When asked to "resolve flags" or "address flagged items":
1. Run `previewf flags <file> --json` or read the file directly
2. For each flag, address the comment (fix the issue, update the text, etc.)
3. Remove the `<flag:N>Label: ...</flag>` tags once resolved
4. Verify no flags remain: `previewf flags <file>` should show "No flags found"

### Flag Format

```
<flag:1>Bug: off-by-one error in loop</flag>
<flag:2>Todo: add input validation</flag>
<flag:3>Comment: need to rethink this approach</flag>
```

### Flag Labels

Predefined: Comment, Bug, Todo, Question, Note, Style. Custom single-word labels also supported.
The label is the prefix before the colon — `\w+` regex (no spaces).

## Commands

```bash
cargo test                     # run tests
cargo clippy -- -D warnings    # lint
cargo fmt --check              # format check
cargo run -- serve ./docs/     # run dev server
cargo run -- view file.md      # terminal preview
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
