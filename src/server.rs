use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::body::Body;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path as AxumPath, State};
use axum::http::{header, Request as HttpRequest, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::Router;
use rust_embed::Embed;
use serde::Deserialize;
use tokio::sync::{broadcast, Mutex};

use crate::flags::{extract_flags, inject_flag, FlagReport};
use crate::html;
use crate::markdown::render_html;
use crate::PreviewError;

// ---------------------------------------------------------------------------
// Embedded static assets
// ---------------------------------------------------------------------------

#[derive(Embed)]
#[folder = "assets/"]
struct Assets;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Immutable server configuration produced by [`ServerBuilder`].
#[derive(Clone, Debug)]
pub struct ServerConfig {
    path: PathBuf,
    port: u16,
    live_reload: bool,
}

impl ServerConfig {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn live_reload(&self) -> bool {
        self.live_reload
    }
}

/// Builder for [`ServerConfig`].
pub struct ServerBuilder {
    path: PathBuf,
    port: u16,
    live_reload: bool,
}

impl ServerBuilder {
    pub fn new() -> Self {
        Self {
            path: PathBuf::from("."),
            port: 3000,
            live_reload: true,
        }
    }

    pub fn path<P: Into<PathBuf>>(mut self, path: P) -> Self {
        self.path = path.into();
        self
    }

    pub fn port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    pub fn live_reload(mut self, enabled: bool) -> Self {
        self.live_reload = enabled;
        self
    }

    /// Build the configuration, validating that the path exists.
    pub fn build(self) -> Result<ServerConfig, PreviewError> {
        let path = std::fs::canonicalize(&self.path)
            .map_err(|_| PreviewError::FileNotFound(self.path.clone()))?;
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

// ---------------------------------------------------------------------------
// Shared application state
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct AppState {
    config: ServerConfig,
    reload_tx: broadcast::Sender<()>,
    file_locks: Arc<Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>>,
}

// ---------------------------------------------------------------------------
// Router construction (public for testing)
// ---------------------------------------------------------------------------

async fn security_headers(request: HttpRequest<Body>, next: Next) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert("X-Content-Type-Options", "nosniff".parse().unwrap());
    headers.insert("X-Frame-Options", "DENY".parse().unwrap());
    headers.insert(
        "Content-Security-Policy",
        "default-src 'self'; script-src 'self'; style-src 'self' https://fonts.googleapis.com 'unsafe-inline'; font-src 'self' https://fonts.gstatic.com; img-src 'self' data:; connect-src 'self' ws: wss:"
            .parse()
            .unwrap(),
    );
    response
}

/// Create the axum [`Router`] for the preview server.
///
/// Exposed publicly so integration tests can drive requests without binding
/// a TCP listener.
pub fn create_router(config: ServerConfig) -> Router {
    create_router_with_reload(config, broadcast::channel::<()>(16).0)
}

fn create_router_with_reload(config: ServerConfig, reload_tx: broadcast::Sender<()>) -> Router {
    let state = AppState {
        config,
        reload_tx,
        file_locks: Arc::new(Mutex::new(HashMap::new())),
    };

    Router::new()
        .route("/", get(index_handler))
        .route("/view/{*filepath}", get(view_handler))
        .route("/raw/{*filepath}", get(raw_handler))
        .route("/flags/{*filepath}", get(flags_handler))
        .route("/flag/{*filepath}", post(flag_handler))
        .route("/ws", get(ws_handler))
        .route("/assets/{*filepath}", get(asset_handler))
        .with_state(state)
        .layer(middleware::from_fn(security_headers))
}

// ---------------------------------------------------------------------------
// `run` — start the server with optional file watcher
// ---------------------------------------------------------------------------

/// Start the preview server, optionally spawning a file watcher that sends
/// reload notifications over WebSocket.
pub async fn run(config: ServerConfig) -> Result<(), PreviewError> {
    let (reload_tx, _) = broadcast::channel::<()>(16);

    // Optionally start the file watcher
    if config.live_reload() {
        let watcher_path = config.path().to_path_buf();
        let tx = reload_tx.clone();
        tokio::spawn(async move {
            match crate::watcher::FileWatcher::new(watcher_path) {
                Ok((_fw, mut rx)) => loop {
                    match rx.recv().await {
                        Ok(_) => {
                            let _ = tx.send(());
                        }
                        Err(broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(_) => break,
                    }
                },
                Err(e) => {
                    eprintln!("Warning: file watcher failed to start: {e}");
                }
            }
        });
    }

    let app = create_router_with_reload(config.clone(), reload_tx);

    let addr = format!("127.0.0.1:{}", config.port());
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    eprintln!(
        "previewf serving {} on http://localhost:{}",
        config.path().display(),
        config.port()
    );

    axum::serve(listener, app).await?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Route handlers
// ---------------------------------------------------------------------------

/// `GET /` — directory listing, or redirect to single file.
async fn index_handler(State(state): State<AppState>) -> Response {
    let base = state.config.path();

    // If the path is a single file, redirect to the appropriate viewer.
    if base.is_file() {
        let name = base.file_name().unwrap_or_default().to_string_lossy();
        return if is_markdown(&name) {
            Redirect::temporary(&format!("/view/{}", name)).into_response()
        } else {
            Redirect::temporary(&format!("/raw/{}", name)).into_response()
        };
    }

    // Collect eligible files (.md and .html) with flag counts for markdown
    let mut md_files: Vec<(String, usize)> = Vec::new();
    let mut html_files: Vec<String> = Vec::new();
    if let Ok(read_dir) = std::fs::read_dir(base) {
        for entry in read_dir.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if is_markdown(&name) {
                let content = std::fs::read_to_string(entry.path()).unwrap_or_default();
                let flag_count = extract_flags(&content).len();
                md_files.push((name, flag_count));
            } else if name.ends_with(".html") || name.ends_with(".htm") {
                html_files.push(name);
            }
        }
    }
    md_files.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
    html_files.sort_unstable();

    // Build HTML entries
    let mut file_entries_parts: Vec<String> = Vec::new();

    for (name, flag_count) in &md_files {
        let safe_name = html::escape(name);
        let badge = if *flag_count > 0 {
            format!(
                r#"<span class="file-entry-badge has-flags">{} flag{}</span>"#,
                flag_count,
                if *flag_count == 1 { "" } else { "s" }
            )
        } else {
            r#"<span class="file-entry-badge">&mdash;</span>"#.to_string()
        };
        file_entries_parts.push(format!(
            r#"<a class="file-entry" href="/view/{safe_name}"><span><span class="file-entry-icon">&#9670;</span><span class="file-entry-name">{safe_name}</span></span>{badge}</a>"#,
        ));
    }

    for name in &html_files {
        let safe_name = html::escape(name);
        file_entries_parts.push(format!(
            r#"<a class="file-entry" href="/raw/{safe_name}"><span><span class="file-entry-icon">&#9671;</span><span class="file-entry-name">{safe_name}</span></span><span class="file-entry-badge">(html)</span></a>"#,
        ));
    }

    let file_entries = file_entries_parts.join("\n");
    let summary = format!(
        "{} markdown &middot; {} html",
        md_files.len(),
        html_files.len()
    );
    let dir_display = base.display().to_string();

    // Load the index template from embedded assets
    let template = Assets::get("index.html")
        .map(|f| String::from_utf8_lossy(&f.data).into_owned())
        .unwrap_or_else(|| "<html><body>Template missing</body></html>".to_string());

    let html = template
        .replace("{{directory}}", &html::escape(&dir_display))
        .replace("{{file_entries}}", &file_entries)
        .replace("{{summary}}", &summary);

    Html(html).into_response()
}

/// `GET /view/{*filepath}` — render a markdown file as HTML.
async fn view_handler(
    State(state): State<AppState>,
    AxumPath(filepath): AxumPath<String>,
) -> Response {
    let full_path = match resolve_path(state.config.path(), &filepath) {
        Some(p) => p,
        None => return not_found_response(&filepath),
    };

    let content = match std::fs::read_to_string(&full_path) {
        Ok(c) => c,
        Err(_) => return not_found_response(&filepath),
    };

    if !is_markdown(&filepath) {
        return (StatusCode::BAD_REQUEST, "Not a markdown file").into_response();
    }

    let rendered = render_html(&content);

    // Load the document template
    let template = Assets::get("document.html")
        .map(|f| String::from_utf8_lossy(&f.data).into_owned())
        .unwrap_or_else(|| "<html><body>{{content}}</body></html>".to_string());

    let title = filepath.rsplit('/').next().unwrap_or(&filepath).to_string();

    let html = template
        .replace("{{title}}", &html::escape(&title))
        .replace("{{filepath}}", &html::escape(&filepath))
        .replace("{{content}}", &rendered);

    Html(html).into_response()
}

/// `GET /raw/{*filepath}` — serve an HTML file as-is.
async fn raw_handler(
    State(state): State<AppState>,
    AxumPath(filepath): AxumPath<String>,
) -> Response {
    let full_path = match resolve_path(state.config.path(), &filepath) {
        Some(p) => p,
        None => return not_found_response(&filepath),
    };

    match std::fs::read_to_string(&full_path) {
        Ok(content) => Html(content).into_response(),
        Err(_) => not_found_response(&filepath),
    }
}

/// `GET /flags/{*filepath}` — return flags as JSON.
async fn flags_handler(
    State(state): State<AppState>,
    AxumPath(filepath): AxumPath<String>,
) -> Response {
    let full_path = match resolve_path(state.config.path(), &filepath) {
        Some(p) => p,
        None => return not_found_response(&filepath),
    };

    let content = match std::fs::read_to_string(&full_path) {
        Ok(c) => c,
        Err(_) => return not_found_response(&filepath),
    };

    let flags = extract_flags(&content);
    let report = FlagReport {
        file: filepath,
        flags,
    };

    match serde_json::to_string(&report) {
        Ok(json) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/json")],
            json,
        )
            .into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

/// Request body for flag injection.
#[derive(Deserialize)]
struct FlagRequest {
    comment: String,
    selected_text: String,
}

/// `POST /flag/{*filepath}` — inject a flag into a markdown file.
async fn flag_handler(
    State(state): State<AppState>,
    AxumPath(filepath): AxumPath<String>,
    axum::Json(body): axum::Json<FlagRequest>,
) -> Response {
    if !is_markdown(&filepath) {
        return (
            StatusCode::BAD_REQUEST,
            "Flags can only be added to markdown files",
        )
            .into_response();
    }

    let full_path = match resolve_path(state.config.path(), &filepath) {
        Some(p) => p,
        None => return not_found_response(&filepath),
    };

    // Acquire per-file lock to prevent concurrent read-modify-write races
    let file_lock = {
        let mut locks = state.file_locks.lock().await;
        locks
            .entry(full_path.clone())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    };
    let _guard = file_lock.lock().await;

    let content = match std::fs::read_to_string(&full_path) {
        Ok(c) => c,
        Err(_) => return not_found_response(&filepath),
    };

    let line = content
        .lines()
        .enumerate()
        .find(|(_, l)| l.contains(&body.selected_text))
        .map(|(i, _)| i + 1);

    let line = match line {
        Some(l) => l,
        None => {
            return (StatusCode::BAD_REQUEST, "Selected text not found in file").into_response()
        }
    };

    match inject_flag(&content, line, &body.comment) {
        Ok(new_content) => match std::fs::write(&full_path, &new_content) {
            Ok(_) => {
                // Don't send explicit reload — the file watcher will detect the write
                (StatusCode::OK, "Flag injected").into_response()
            }
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        },
        Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    }
}

/// `GET /ws` — WebSocket endpoint for live reload notifications.
async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    ws.on_upgrade(move |socket| handle_ws(socket, state))
}

async fn handle_ws(mut socket: WebSocket, state: AppState) {
    let mut rx = state.reload_tx.subscribe();

    loop {
        tokio::select! {
            result = rx.recv() => {
                match result {
                    Ok(()) => {
                        if socket.send(Message::Text("reload".into())).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(_) => break,
                }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Err(_)) => break,
                    _ => {} // ignore other messages
                }
            }
        }
    }
}

/// `GET /assets/{*filepath}` — serve embedded static assets.
async fn asset_handler(AxumPath(filepath): AxumPath<String>) -> Response {
    match Assets::get(&filepath) {
        Some(file) => {
            let mime = mime_guess::from_path(&filepath)
                .first_or_octet_stream()
                .to_string();
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, mime)],
                file.data.to_vec(),
            )
                .into_response()
        }
        None => (StatusCode::NOT_FOUND, "Asset not found").into_response(),
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Resolve a relative filepath against the configured base path.
///
/// Returns `None` if the resolved path escapes the base directory
/// (path traversal prevention).
fn resolve_path(base: &Path, filepath: &str) -> Option<PathBuf> {
    if base.is_file() {
        return Some(base.to_path_buf());
    }

    let joined = base.join(filepath);
    let canonical = std::fs::canonicalize(&joined).ok()?;
    let base_canonical = std::fs::canonicalize(base).ok()?;

    if canonical.starts_with(&base_canonical) {
        Some(canonical)
    } else {
        None
    }
}

pub fn is_markdown(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.ends_with(".md") || lower.ends_with(".markdown") || lower.ends_with(".mkd")
}

fn not_found_response(filepath: &str) -> Response {
    (
        StatusCode::NOT_FOUND,
        format!("File not found: {}", html::escape(filepath)),
    )
        .into_response()
}
