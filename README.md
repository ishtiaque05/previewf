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
cargo test

# Lint
cargo clippy -- -D warnings

# Format
cargo fmt

# Run dev server
cargo run -- serve ./tests/fixtures/
```

## License

MIT
