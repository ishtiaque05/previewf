# Installation

previewf is distributed as a single binary with no runtime dependencies. There are three ways to install it: downloading a pre-built release binary, building from source with Cargo, or (eventually) installing via Homebrew.

## Pre-built Binaries

Each release publishes binaries for four targets. Download the one matching your platform, extract it, and move it to a directory on your `PATH`.

### macOS (Apple Silicon)

```bash
curl -L https://github.com/ishtiaque05/previewf/releases/latest/download/previewf-aarch64-apple-darwin.tar.gz | tar xz
sudo mv previewf /usr/local/bin/
```

### macOS (Intel)

```bash
curl -L https://github.com/ishtiaque05/previewf/releases/latest/download/previewf-x86_64-apple-darwin.tar.gz | tar xz
sudo mv previewf /usr/local/bin/
```

### Linux (x86_64)

```bash
curl -L https://github.com/ishtiaque05/previewf/releases/latest/download/previewf-x86_64-unknown-linux-gnu.tar.gz | tar xz
sudo mv previewf /usr/local/bin/
```

### Linux (ARM64)

```bash
curl -L https://github.com/ishtiaque05/previewf/releases/latest/download/previewf-aarch64-unknown-linux-gnu.tar.gz | tar xz
sudo mv previewf /usr/local/bin/
```

Each release archive includes a SHA256 checksum file. To verify your download:

```bash
# Download the checksum file
curl -L https://github.com/ishtiaque05/previewf/releases/latest/download/checksums.txt -o checksums.txt

# Verify (macOS)
shasum -a 256 -c checksums.txt --ignore-missing

# Verify (Linux)
sha256sum -c checksums.txt --ignore-missing
```

## Building from Source

If you have a Rust toolchain installed (1.70+), you can install directly from the repository:

```bash
cargo install --git https://github.com/ishtiaque05/previewf
```

This compiles the project in release mode and installs the `previewf` binary to `~/.cargo/bin/`, which should be on your `PATH` if you installed Rust via rustup.

### Development Build

To clone and build locally for development:

```bash
git clone https://github.com/ishtiaque05/previewf.git
cd previewf
cargo build
```

The debug binary will be at `target/debug/previewf`. For a release-optimized build:

```bash
cargo build --release
```

The release binary will be at `target/release/previewf`.

## Prerequisites for Development

If you plan to work on previewf itself, you will need:

| Tool | Purpose | Install |
|------|---------|---------|
| Rust 1.70+ | Compiler and standard library | [rustup.rs](https://rustup.rs) |
| cargo-nextest | Fast parallel test runner | `cargo install cargo-nextest` |
| cargo-tarpaulin | Code coverage (Linux only) | `cargo install cargo-tarpaulin` |
| mdbook | Building this documentation book | `cargo install mdbook` |
| clippy | Linter (ships with rustup) | `rustup component add clippy` |
| rustfmt | Formatter (ships with rustup) | `rustup component add rustfmt` |

## Verifying Installation

After installation, verify that previewf is available:

```bash
previewf --version
```

Expected output:

```
previewf 0.1.0
```

Run the help command to see all available subcommands:

```bash
previewf --help
```

Expected output:

```
Preview and annotate markdown files

Usage: previewf <COMMAND>

Commands:
  serve  Serve files on localhost for browser preview
  view   View a markdown file in the terminal
  flags  Extract flags from a markdown file
  help   Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

## Building the Documentation

This book is built with mdBook. From the project root:

```bash
cd book
mdbook serve
```

This starts a local server at `http://localhost:3000` (by default) with live reload. You can also build a static version:

```bash
mdbook build
```

The output goes to `book/book/` and can be served by any static file server or deployed to GitHub Pages.
