# Error Handling

previewf uses a two-layer error handling strategy: typed errors in the library code and contextual errors in the application code. This chapter covers the error types, propagation patterns, and how errors are presented to users in different contexts.

## The Two Layers

### Layer 1: `PreviewError` (Library)

The library code (`flags.rs`, `server.rs`, `watcher.rs`) uses `PreviewError`, a typed error enum defined in `src/error.rs`:

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

Each variant carries structured data. The `#[error("...")]` attributes define human-readable messages via the `Display` trait. The `#[from]` attributes enable automatic conversion from `std::io::Error` and `notify::Error` using the `?` operator.

**Why typed errors in the library?** Because callers can match on variants. The server can return different HTTP status codes based on the error type:

```rust
match result {
    Err(PreviewError::FileNotFound(_)) => StatusCode::NOT_FOUND,
    Err(PreviewError::FlagParse { .. }) => StatusCode::BAD_REQUEST,
    Err(PreviewError::Server(_)) => StatusCode::INTERNAL_SERVER_ERROR,
    // ...
}
```

### Layer 2: `anyhow::Result` (Application)

The application code (`main.rs`) uses `anyhow::Result`, which wraps any error with additional context:

```rust
let content = std::fs::read_to_string(&path)
    .with_context(|| format!("Cannot read file: {}", path.display()))?;
```

`anyhow` provides:

- **Context chaining.** Each `.with_context()` adds a layer of explanation
- **Ergonomic `?` operator.** Any `Error` type can be propagated with `?`
- **Pretty error display.** When `main` returns `Err`, anyhow prints the full error chain

**Why anyhow in the application?** Because the application's job is to present helpful messages to the user, not to enable programmatic error matching. The CLI user sees:

```
Error: Cannot read file: ./missing.md
Caused by: No such file or directory (os error 2)
```

This is more helpful than a raw `FileNotFound` error because it includes the file path and the OS error.

## Error Variants in Detail

### `FileNotFound(PathBuf)`

Triggered when a requested file does not exist on disk. Used primarily in the server's route handlers when `std::fs::read_to_string` fails.

```rust
// In server.rs route handlers:
let content = match std::fs::read_to_string(&full_path) {
    Ok(c) => c,
    Err(_) => return (StatusCode::NOT_FOUND, "File not found").into_response(),
};
```

Note: In the route handlers, we currently return a direct 404 response rather than constructing a `PreviewError::FileNotFound`. This is a pragmatic choice -- the handler needs an HTTP response, not a Rust error. The `PreviewError::FileNotFound` variant exists for non-HTTP contexts (CLI commands, library usage).

### `NotMarkdown(PathBuf)`

Triggered when a markdown-specific operation is attempted on a non-markdown file. This is a validation error -- the file exists but has the wrong extension.

Currently unused in the implementation but reserved for future validation. For example, the `view` command could validate that the path ends in `.md` before attempting to render.

### `FlagParse { line, detail }`

Triggered by the `inject_flag` function when the requested line is out of range:

```rust
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
```

The structured data (`line` and `detail`) enables callers to include specific context in error messages. In the server, this becomes a 400 Bad Request:

```rust
Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
```

### `Server(std::io::Error)`

Wraps I/O errors from the server layer: binding to a port, reading from a socket, etc. The `#[from]` attribute enables:

```rust
let listener = TcpListener::bind(format!("0.0.0.0:{}", port))
    .await
    .map_err(PreviewError::Server)?;
```

### `Watcher(notify::Error)`

Wraps errors from the file watcher: permission denied on the watched directory, too many file descriptors, etc.

```rust
watcher.watch(&self.path, mode)
    .map_err(PreviewError::Watcher)?;
```

## Error Propagation Patterns

### Pattern 1: Library Function Returns `Result<T, PreviewError>`

```rust
// flags.rs
pub fn inject_flag(content: &str, line: usize, comment: &str)
    -> Result<String, PreviewError>
{
    if line == 0 || line > lines.len() {
        return Err(PreviewError::FlagParse { line, detail: "..." });
    }
    // ... success path
    Ok(output)
}
```

The caller decides how to handle the error:

```rust
// In server route handler:
match inject_flag(&content, line, &payload.comment) {
    Ok(new_content) => {
        // ... write to disk, return 200
    }
    Err(e) => {
        (StatusCode::BAD_REQUEST, e.to_string()).into_response()
    }
}

// In main.rs (CLI):
let result = inject_flag(&content, line, &comment)
    .context("Failed to inject flag")?;
```

### Pattern 2: Route Handler Returns HTTP Response

Route handlers in `server.rs` do not propagate errors with `?`. Instead, they match on success/failure and return appropriate HTTP responses:

```rust
async fn view_handler(
    State(state): State<AppState>,
    AxumPath(filepath): AxumPath<String>,
) -> Response {
    let full_path = resolve_path(&state.config.path, &filepath);

    let content = match std::fs::read_to_string(&full_path) {
        Ok(c) => c,
        Err(_) => return (StatusCode::NOT_FOUND, "File not found").into_response(),
    };

    // ... success path
    Html(page).into_response()
}
```

This pattern is necessary because axum route handlers return `Response`, not `Result`. The handler is responsible for converting any error into an appropriate HTTP status code and body.

### Pattern 3: Main Function Returns `anyhow::Result`

```rust
#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::View { path } => {
            let content = std::fs::read_to_string(&path)
                .with_context(|| format!("Cannot read file: {}", path.display()))?;
            // ...
        }
        // ...
    }
}
```

When `main` returns `Err`, anyhow automatically prints the error chain to stderr and exits with status 1.

## The No-Unwrap Rule

**Rule: No `unwrap()` in library code.** Every function that can fail returns `Result`. The only exceptions are:

1. **Regex compilation.** `Regex::new(r"...")` is called with a compile-time-known pattern. If the pattern is invalid, that is a programmer error (a bug), not a runtime error. `unwrap()` here is acceptable because the program should not continue with a broken regex.

2. **`unwrap_or` / `unwrap_or_else`.** These provide fallbacks and never panic:
   ```rust
   let id: u32 = cap[1].parse().unwrap_or(0);
   ```

3. **Test code.** Tests use `unwrap()` freely because a panic in a test is the expected failure mode.

In all other cases, errors are propagated with `?` or handled with `match`.

## HTTP Error Responses

The server maps errors to appropriate HTTP status codes:

| Situation | Status Code | Body |
|-----------|-------------|------|
| File not found | 404 | "File not found" |
| Asset not found | 404 | "Asset not found" |
| Selected text not found (flag creation) | 400 | "Selected text not found in file" |
| Flag injection error | 400 | Error message from `PreviewError::FlagParse` |
| File write failure (flag creation) | 500 | "Failed to write file" |

Currently, error responses are plain text. Future improvement: render errors using a styled error page template, consistent with the editorial design.

## Error Display for Users

### CLI Errors

Errors from the CLI commands are displayed by anyhow with the full context chain:

```
$ previewf view ./missing.md
Error: Cannot read file: ./missing.md
Caused by: No such file or directory (os error 2)
```

```
$ previewf flags ./binary-file.bin --json
Error: Cannot read file: ./binary-file.bin
Caused by: stream did not contain valid UTF-8
```

### Server Startup Errors

```
$ previewf serve ./docs/ --port 80
Error: Server error
Caused by: Permission denied (os error 13)
```

```
$ previewf serve ./docs/ --port 3000
# (another instance already running on 3000)
Error: Server error
Caused by: Address already in use (os error 48)
```

### Browser Errors

When a route handler returns a non-200 status, the browser displays the error body as plain text. This is functional but not pretty. Future improvement: styled error pages.

## Why thiserror + anyhow (Not Just One)

Using only `thiserror`:
- Library errors are well-typed and matchable
- But the application has to manually construct user-friendly messages
- Context chaining requires manual work

Using only `anyhow`:
- Application errors are easy to construct and chain
- But callers cannot match on error types
- Library API is less precise

Using both:
- Library code returns typed errors (matchable, precise)
- Application code wraps them with context (user-friendly, chainable)
- The boundary is clear: `PreviewError` in `src/`, `anyhow::Result` in `main.rs`

This is the idiomatic Rust pattern. The `thiserror` README itself recommends this approach.
