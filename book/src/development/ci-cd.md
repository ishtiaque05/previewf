# CI/CD

previewf uses GitHub Actions for continuous integration and release automation. This chapter covers the CI pipeline, the release pipeline, and the build targets.

## CI Pipeline

**File:** `.github/workflows/ci.yml`

**Triggers:** Push to `main`, all pull requests.

**Matrix:** `ubuntu-latest`, `macos-latest`.

### Pipeline Steps

```
1. cargo fmt --check       Check code formatting
2. cargo clippy -- -D warnings    Lint with all warnings as errors
3. cargo nextest run       Run all tests in parallel
4. cargo tarpaulin --out xml    Code coverage (Linux only)
```

### Why This Order

1. **Formatting first.** `cargo fmt --check` is the fastest check (< 1 second). If the code is not formatted, fail fast before running anything expensive.

2. **Linting second.** `cargo clippy` catches common mistakes, anti-patterns, and potential bugs. It runs in seconds and catches issues that tests might miss (unused variables, redundant clones, etc.).

3. **Tests third.** `cargo nextest run` runs all integration and unit tests. This is the most expensive step but also the most important. Tests validate behavior.

4. **Coverage last.** `cargo tarpaulin` instruments the code and measures line coverage. This is expensive and only runs on Linux (tarpaulin uses Linux-specific features). Coverage reports are generated as XML (Cobertura format) for potential integration with coverage services.

### The Matrix

Running on both `ubuntu-latest` and `macos-latest` ensures:

- **Linux compatibility.** The primary deployment target. Catches platform-specific issues with file paths, inotify limits, and signal handling.

- **macOS compatibility.** The primary development platform. Catches issues with FSEvents, case-insensitive file systems, and macOS-specific behavior.

The pipeline does not currently run on Windows. This could be added if Windows support becomes a goal.

### Example Workflow File

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:

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

      - name: Cache cargo registry and build
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}

      - name: Check formatting
        run: cargo fmt --check

      - name: Lint
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
          file: cobertura.xml
          fail_ci_if_error: false
```

### Caching

The workflow caches:

- `~/.cargo/registry`: Downloaded crate sources
- `~/.cargo/git`: Git-based dependencies
- `target`: Compilation artifacts

The cache key includes the `Cargo.lock` hash, so the cache invalidates when dependencies change. This typically reduces CI time from ~3 minutes to ~1 minute for incremental builds.

## Release Pipeline

**File:** `.github/workflows/release.yml`

**Triggers:** Push of a tag matching `v*` (e.g., `v0.1.0`, `v1.0.0`).

### Build Targets

| Target Triple | Platform | Architecture |
|--------------|----------|-------------|
| `x86_64-unknown-linux-gnu` | Linux | x86_64 (Intel/AMD) |
| `aarch64-unknown-linux-gnu` | Linux | ARM64 (Raspberry Pi, Graviton) |
| `x86_64-apple-darwin` | macOS | Intel |
| `aarch64-apple-darwin` | macOS | Apple Silicon |

### Pipeline Steps

For each target:

```
1. Cross-compile with cargo build --release --target <target>
2. Strip the binary (reduce size)
3. Create tar.gz archive
4. Compute SHA256 checksum
5. Upload to GitHub Release
```

### Example Workflow File

```yaml
name: Release

on:
  push:
    tags:
      - 'v*'

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
          - target: aarch64-unknown-linux-gnu
            os: ubuntu-latest
          - target: x86_64-apple-darwin
            os: macos-latest
          - target: aarch64-apple-darwin
            os: macos-latest

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

      - name: Build
        run: cargo build --release --target ${{ matrix.target }}

      - name: Package
        run: |
          cd target/${{ matrix.target }}/release
          tar czf previewf-${{ matrix.target }}.tar.gz previewf
          shasum -a 256 previewf-${{ matrix.target }}.tar.gz > previewf-${{ matrix.target }}.sha256

      - name: Upload to Release
        uses: softprops/action-gh-release@v2
        with:
          files: |
            target/${{ matrix.target }}/release/previewf-${{ matrix.target }}.tar.gz
            target/${{ matrix.target }}/release/previewf-${{ matrix.target }}.sha256
```

### Creating a Release

To create a new release:

```bash
# Update version in Cargo.toml
# Commit the version bump
git add Cargo.toml
git commit -m "Bump version to 0.2.0"

# Create and push the tag
git tag v0.2.0
git push origin main
git push origin v0.2.0
```

The tag push triggers the release workflow, which builds all four targets and creates a GitHub Release with the archives and checksums.

### Binary Size

Approximate release binary sizes (with `--release` optimization and stripping):

| Target | Approximate Size |
|--------|-----------------|
| Linux x86_64 | ~5-8 MB |
| Linux ARM64 | ~5-8 MB |
| macOS Intel | ~5-8 MB |
| macOS Apple Silicon | ~5-8 MB |

The binary includes:

- The Rust standard library (statically linked)
- All crate dependencies compiled in
- Embedded assets (HTML, CSS, JS via rust-embed)
- syntect's default syntax and theme bundles

## Local CI Simulation

Before pushing, you can run the same checks locally:

```bash
# Run all CI checks in order
cargo fmt --check && \
cargo clippy -- -D warnings && \
cargo nextest run
```

Or as a single script:

```bash
#!/bin/bash
set -e
echo "=== Format check ==="
cargo fmt --check
echo "=== Clippy ==="
cargo clippy -- -D warnings
echo "=== Tests ==="
cargo nextest run
echo "=== All checks passed ==="
```

## Dependency Updates

Dependencies should be updated periodically:

```bash
# Check for outdated dependencies
cargo outdated

# Update all dependencies within semver ranges
cargo update

# Run tests to verify
cargo nextest run
```

For major version updates (breaking changes), update `Cargo.toml` manually and fix any compilation errors.

## Future CI Improvements

Potential additions to the CI pipeline:

| Improvement | Benefit |
|-------------|---------|
| `cargo deny` | License compliance and advisory database checking |
| `cargo audit` | Security vulnerability scanning |
| MSRV check | Verify minimum supported Rust version |
| mdBook build | Ensure documentation compiles |
| Binary size tracking | Detect unexpected size increases |
| Windows target | Expand platform support |
