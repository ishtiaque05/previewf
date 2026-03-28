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
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, Mutex};

use crate::flags::{extract_flags, inject_flag, remove_flag, update_flag_comment, FlagReport};
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
            port: 4567,
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
        .route("/browse/{*dirpath}", get(browse_handler))
        .route("/view/{*filepath}", get(view_handler))
        .route("/raw/{*filepath}", get(raw_handler))
        .route("/flags/{*filepath}", get(flags_handler))
        .route(
            "/flag/{*filepath}",
            post(flag_handler)
                .delete(delete_flag_handler)
                .put(update_flag_handler),
        )
        .route("/api/tree", get(tree_handler))
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
    listing_response(&state, "")
}

/// `GET /browse/{*dirpath}` — browse into a subdirectory.
async fn browse_handler(
    State(state): State<AppState>,
    AxumPath(dirpath): AxumPath<String>,
) -> Response {
    listing_response(&state, &dirpath)
}

/// Shared listing logic for both `/` and `/browse/{dirpath}`.
fn listing_response(state: &AppState, subpath: &str) -> Response {
    let base = state.config.path();

    // If the configured path is a single file, redirect to the viewer.
    if base.is_file() && subpath.is_empty() {
        let name = base.file_name().unwrap_or_default().to_string_lossy();
        return if is_markdown(&name) {
            Redirect::temporary(&format!("/view/{}", name)).into_response()
        } else {
            Redirect::temporary(&format!("/raw/{}", name)).into_response()
        };
    }

    // Resolve the subdirectory, preventing path traversal.
    let dir = if subpath.is_empty() {
        base.to_path_buf()
    } else {
        match resolve_path(base, subpath) {
            Some(p) if p.is_dir() => p,
            _ => return not_found_response(subpath),
        }
    };

    // Build breadcrumbs
    let breadcrumb_html = build_breadcrumbs(subpath);

    // Collect directories, markdown, json, and html files
    let mut dirs: Vec<String> = Vec::new();
    let mut md_files: Vec<(String, usize)> = Vec::new();
    let mut json_files: Vec<String> = Vec::new();
    let mut html_files: Vec<String> = Vec::new();

    if let Ok(read_dir) = std::fs::read_dir(&dir) {
        for entry in read_dir.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let file_type = entry.file_type();

            if let Ok(ft) = file_type {
                if ft.is_dir() {
                    dirs.push(name);
                } else if is_markdown(&name) {
                    let content = std::fs::read_to_string(entry.path()).unwrap_or_default();
                    let flag_count = extract_flags(&content).len();
                    md_files.push((name, flag_count));
                } else if is_json(&name) {
                    json_files.push(name);
                } else if name.ends_with(".html") || name.ends_with(".htm") {
                    html_files.push(name);
                }
            }
        }
    }

    dirs.sort_by_key(|a| a.to_lowercase());
    md_files.sort_by_key(|a| a.0.to_lowercase());
    json_files.sort_by_key(|a| a.to_lowercase());
    html_files.sort_unstable();

    // Build path prefix for links
    let prefix = if subpath.is_empty() {
        String::new()
    } else {
        format!("{}/", subpath.trim_end_matches('/'))
    };

    let mut file_entries_parts: Vec<String> = Vec::new();

    // Directory entries
    for name in &dirs {
        let safe_name = html::escape(name);
        let browse_path = html::escape(&format!("{}{}", prefix, name));
        file_entries_parts.push(format!(
            r#"<a class="file-entry file-entry-dir" href="/browse/{browse_path}"><span class="file-entry-name-group"><span class="file-entry-icon dir-icon">&#128193;</span><span class="file-entry-name">{safe_name}/</span></span><span class="file-entry-badge dir-badge">folder</span></a>"#,
        ));
    }

    // Markdown entries
    for (name, flag_count) in &md_files {
        let safe_name = html::escape(name);
        let view_path = html::escape(&format!("{}{}", prefix, name));
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
            r#"<a class="file-entry" href="/view/{view_path}"><span class="file-entry-name-group"><span class="file-entry-icon md-icon">&#9670;</span><span class="file-entry-name">{safe_name}</span></span>{badge}</a>"#,
        ));
    }

    // JSON entries
    for name in &json_files {
        let safe_name = html::escape(name);
        let view_path = html::escape(&format!("{}{}", prefix, name));
        file_entries_parts.push(format!(
            r#"<a class="file-entry" href="/view/{view_path}"><span class="file-entry-name-group"><span class="file-entry-icon json-icon">&#123;&#125;</span><span class="file-entry-name">{safe_name}</span></span><span class="file-entry-badge json-badge">json</span></a>"#,
        ));
    }

    // HTML entries
    for name in &html_files {
        let safe_name = html::escape(name);
        let view_path = html::escape(&format!("{}{}", prefix, name));
        file_entries_parts.push(format!(
            r#"<a class="file-entry" href="/raw/{view_path}"><span class="file-entry-name-group"><span class="file-entry-icon html-icon">&#9671;</span><span class="file-entry-name">{safe_name}</span></span><span class="file-entry-badge html-badge">html</span></a>"#,
        ));
    }

    let file_entries = file_entries_parts.join("\n");
    let summary = format!(
        "{} folder{} &middot; {} markdown &middot; {} json &middot; {} html",
        dirs.len(),
        if dirs.len() == 1 { "" } else { "s" },
        md_files.len(),
        json_files.len(),
        html_files.len(),
    );

    let dir_display = if subpath.is_empty() {
        base.display().to_string()
    } else {
        format!("{}/{}", base.display(), subpath)
    };

    // Load the index template from embedded assets
    let template = Assets::get("index.html")
        .map(|f| String::from_utf8_lossy(&f.data).into_owned())
        .unwrap_or_else(|| "<html><body>Template missing</body></html>".to_string());

    let page_html = template
        .replace("{{directory}}", &html::escape(&dir_display))
        .replace("{{breadcrumbs}}", &breadcrumb_html)
        .replace("{{file_entries}}", &file_entries)
        .replace("{{summary}}", &summary);

    Html(page_html).into_response()
}

/// Build breadcrumb HTML for any path (directory or file).
/// The last segment is rendered as non-linked "current" text;
/// intermediate segments link to `/browse/`.
fn build_breadcrumbs(path: &str) -> String {
    let mut parts = Vec::new();
    parts.push(r#"<a class="breadcrumb-link" href="/">root</a>"#.to_string());

    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    let mut accumulated = String::new();
    for (i, seg) in segments.iter().enumerate() {
        if !accumulated.is_empty() {
            accumulated.push('/');
        }
        accumulated.push_str(seg);
        let safe_seg = html::escape(seg);
        if i == segments.len() - 1 {
            parts.push(format!(
                r#"<span class="breadcrumb-current">{safe_seg}</span>"#
            ));
        } else {
            let safe_path = html::escape(&accumulated);
            parts.push(format!(
                r#"<a class="breadcrumb-link" href="/browse/{safe_path}">{safe_seg}</a>"#
            ));
        }
    }

    format!(
        r#"<nav class="breadcrumbs">{}</nav>"#,
        parts.join(r#"<span class="breadcrumb-sep">/</span>"#)
    )
}

/// `GET /view/{*filepath}` — render a markdown or JSON file as HTML.
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

    let rendered = if is_markdown(&filepath) {
        render_html(&content)
    } else if is_json(&filepath) {
        render_json_html(&content)
    } else {
        return (StatusCode::BAD_REQUEST, "Unsupported file type").into_response();
    };

    // Build breadcrumb for the file path
    let breadcrumb_html = build_breadcrumbs(&filepath);

    // Load the document template
    let template = Assets::get("document.html")
        .map(|f| String::from_utf8_lossy(&f.data).into_owned())
        .unwrap_or_else(|| "<html><body>{{content}}</body></html>".to_string());

    let title = filepath.rsplit('/').next().unwrap_or(&filepath).to_string();

    let page_html = template
        .replace("{{title}}", &html::escape(&title))
        .replace("{{breadcrumbs}}", &breadcrumb_html)
        .replace("{{content}}", &rendered);

    Html(page_html).into_response()
}

/// Render a JSON string as pretty-printed HTML with syntax highlighting.
fn render_json_html(content: &str) -> String {
    // Try to pretty-print the JSON; fall back to raw content.
    let pretty = match serde_json::from_str::<serde_json::Value>(content) {
        Ok(val) => serde_json::to_string_pretty(&val).unwrap_or_else(|_| content.to_string()),
        Err(_) => content.to_string(),
    };

    // Colorize JSON tokens
    let mut out = String::from(r#"<pre class="json-viewer"><code>"#);
    for ch in JsonTokenizer::new(&pretty) {
        match ch {
            JsonToken::Key(s) => {
                out.push_str(&format!(
                    r#"<span class="json-key">{}</span>"#,
                    html::escape(&s)
                ));
            }
            JsonToken::StringVal(s) => {
                out.push_str(&format!(
                    r#"<span class="json-string">{}</span>"#,
                    html::escape(&s)
                ));
            }
            JsonToken::Number(s) => {
                out.push_str(&format!(
                    r#"<span class="json-number">{}</span>"#,
                    html::escape(&s)
                ));
            }
            JsonToken::Bool(s) | JsonToken::Null(s) => {
                out.push_str(&format!(
                    r#"<span class="json-keyword">{}</span>"#,
                    html::escape(&s)
                ));
            }
            JsonToken::Punct(s) => {
                out.push_str(&html::escape(&s));
            }
        }
    }
    out.push_str("</code></pre>");
    out
}

/// Simple JSON tokenizer for syntax highlighting.
enum JsonToken {
    Key(String),
    StringVal(String),
    Number(String),
    Bool(String),
    Null(String),
    Punct(String),
}

struct JsonTokenizer<'a> {
    chars: std::iter::Peekable<std::str::Chars<'a>>,
    expect_key: bool,
    /// Stack tracking nesting context: `true` = object, `false` = array.
    context_stack: Vec<bool>,
}

impl<'a> JsonTokenizer<'a> {
    fn new(s: &'a str) -> Self {
        Self {
            chars: s.chars().peekable(),
            expect_key: false,
            context_stack: Vec::new(),
        }
    }
}

impl Iterator for JsonTokenizer<'_> {
    type Item = JsonToken;

    fn next(&mut self) -> Option<JsonToken> {
        // Skip whitespace but emit it as punctuation for formatting
        let mut ws = String::new();
        while let Some(&c) = self.chars.peek() {
            if c.is_whitespace() {
                ws.push(c);
                self.chars.next();
            } else {
                break;
            }
        }
        if !ws.is_empty() {
            return Some(JsonToken::Punct(ws));
        }

        let &c = self.chars.peek()?;

        match c {
            '{' => {
                self.chars.next();
                self.context_stack.push(true); // entering object
                self.expect_key = true;
                Some(JsonToken::Punct("{".into()))
            }
            '}' => {
                self.chars.next();
                self.context_stack.pop();
                self.expect_key = false;
                Some(JsonToken::Punct("}".into()))
            }
            '[' => {
                self.chars.next();
                self.context_stack.push(false); // entering array
                self.expect_key = false;
                Some(JsonToken::Punct("[".into()))
            }
            ']' => {
                self.chars.next();
                self.context_stack.pop();
                Some(JsonToken::Punct("]".into()))
            }
            ':' => {
                self.chars.next();
                self.expect_key = false;
                Some(JsonToken::Punct(":".into()))
            }
            ',' => {
                self.chars.next();
                let in_object = self.context_stack.last().copied().unwrap_or(false);
                self.expect_key = in_object;
                Some(JsonToken::Punct(",".into()))
            }
            '"' => {
                let s = self.read_string();
                if self.expect_key {
                    self.expect_key = false;
                    Some(JsonToken::Key(s))
                } else {
                    Some(JsonToken::StringVal(s))
                }
            }
            't' | 'f' => {
                let word = self.read_word();
                Some(JsonToken::Bool(word))
            }
            'n' => {
                let word = self.read_word();
                Some(JsonToken::Null(word))
            }
            _ if c == '-' || c.is_ascii_digit() => {
                let num = self.read_number();
                Some(JsonToken::Number(num))
            }
            _ => {
                self.chars.next();
                Some(JsonToken::Punct(c.to_string()))
            }
        }
    }
}

impl JsonTokenizer<'_> {
    fn read_string(&mut self) -> String {
        let mut s = String::new();
        s.push(self.chars.next().unwrap()); // opening "
        let mut escaped = false;
        for c in self.chars.by_ref() {
            s.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                break;
            }
        }
        s
    }

    fn read_word(&mut self) -> String {
        let mut s = String::new();
        while let Some(&c) = self.chars.peek() {
            if c.is_ascii_alphabetic() {
                s.push(c);
                self.chars.next();
            } else {
                break;
            }
        }
        s
    }

    fn read_number(&mut self) -> String {
        let mut s = String::new();
        while let Some(&c) = self.chars.peek() {
            if c.is_ascii_digit() || c == '.' || c == '-' || c == '+' || c == 'e' || c == 'E' {
                s.push(c);
                self.chars.next();
            } else {
                break;
            }
        }
        s
    }
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

/// Request body for flag comment update.
#[derive(Deserialize)]
struct UpdateFlagRequest {
    comment: String,
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

/// `DELETE /flag/{id}/{filepath…}` — remove a flag by ID.
///
/// The route shares `/flag/{*filepath}` with the POST handler; the first
/// path segment is the numeric flag ID and the remainder is the file path.
async fn delete_flag_handler(
    State(state): State<AppState>,
    AxumPath(raw_path): AxumPath<String>,
) -> Response {
    let (id, filepath) = match parse_id_and_filepath(&raw_path) {
        Some(pair) => pair,
        None => return (StatusCode::BAD_REQUEST, "Invalid flag path").into_response(),
    };

    if !is_markdown(&filepath) {
        return (
            StatusCode::BAD_REQUEST,
            "Flags can only be removed from markdown files",
        )
            .into_response();
    }

    let full_path = match resolve_path(state.config.path(), &filepath) {
        Some(p) => p,
        None => return not_found_response(&filepath),
    };

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

    match remove_flag(&content, id) {
        Ok(new_content) => match std::fs::write(&full_path, &new_content) {
            Ok(_) => (StatusCode::OK, "Flag removed").into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        },
        Err(e) => (StatusCode::NOT_FOUND, e.to_string()).into_response(),
    }
}

/// `PUT /flag/{id}/{filepath…}` — update a flag's comment.
///
/// Same path-parsing strategy as [`delete_flag_handler`].
async fn update_flag_handler(
    State(state): State<AppState>,
    AxumPath(raw_path): AxumPath<String>,
    axum::Json(body): axum::Json<UpdateFlagRequest>,
) -> Response {
    let (id, filepath) = match parse_id_and_filepath(&raw_path) {
        Some(pair) => pair,
        None => return (StatusCode::BAD_REQUEST, "Invalid flag path").into_response(),
    };

    if !is_markdown(&filepath) {
        return (
            StatusCode::BAD_REQUEST,
            "Flags can only be edited in markdown files",
        )
            .into_response();
    }

    if body.comment.trim().is_empty() {
        return (StatusCode::BAD_REQUEST, "Comment cannot be empty").into_response();
    }

    let full_path = match resolve_path(state.config.path(), &filepath) {
        Some(p) => p,
        None => return not_found_response(&filepath),
    };

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

    match update_flag_comment(&content, id, &body.comment) {
        Ok(new_content) => match std::fs::write(&full_path, &new_content) {
            Ok(_) => (StatusCode::OK, "Flag updated").into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        },
        Err(e) => (StatusCode::NOT_FOUND, e.to_string()).into_response(),
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
// Tree API
// ---------------------------------------------------------------------------

/// A node in the directory tree.
#[derive(Serialize)]
struct TreeNode {
    name: String,
    #[serde(rename = "type")]
    node_type: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    children: Option<Vec<TreeNode>>,
}

/// `GET /api/tree` — return the full directory tree as JSON.
async fn tree_handler(State(state): State<AppState>) -> Response {
    let base = state.config.path();
    let tree = build_tree(base, base);

    match serde_json::to_string(&tree) {
        Ok(json) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/json")],
            json,
        )
            .into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

const MAX_TREE_DEPTH: usize = 10;

/// Recursively build a tree of directories and viewable files.
fn build_tree(dir: &Path, base: &Path) -> Vec<TreeNode> {
    build_tree_inner(dir, base, 0)
}

fn build_tree_inner(dir: &Path, base: &Path, depth: usize) -> Vec<TreeNode> {
    if depth >= MAX_TREE_DEPTH {
        return Vec::new();
    }

    let Ok(read_dir) = std::fs::read_dir(dir) else {
        return Vec::new();
    };

    let mut dir_nodes: Vec<TreeNode> = Vec::new();
    let mut file_nodes: Vec<TreeNode> = Vec::new();

    for entry in read_dir.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();

        // Skip hidden files/directories (e.g. .git, .cache)
        if name.starts_with('.') {
            continue;
        }

        let Ok(ft) = entry.file_type() else {
            continue;
        };

        // Compute relative path from base; skip if strip_prefix fails
        let rel_path = match entry.path().strip_prefix(base) {
            Ok(p) => p.to_string_lossy().to_string(),
            Err(_) => continue,
        };

        if ft.is_dir() {
            let children = build_tree_inner(&entry.path(), base, depth + 1);
            dir_nodes.push(TreeNode {
                name,
                node_type: "dir",
                path: Some(rel_path),
                children: Some(children),
            });
        } else if is_markdown(&name) || is_json(&name) {
            let node_type = if is_markdown(&name) { "md" } else { "json" };
            file_nodes.push(TreeNode {
                name,
                node_type,
                path: Some(rel_path),
                children: None,
            });
        } else if name.ends_with(".html") || name.ends_with(".htm") {
            file_nodes.push(TreeNode {
                name,
                node_type: "html",
                path: Some(rel_path),
                children: None,
            });
        }
    }

    dir_nodes.sort_by_key(|a| a.name.to_lowercase());
    file_nodes.sort_by_key(|a| a.name.to_lowercase());
    dir_nodes.extend(file_nodes);
    dir_nodes
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Parse a raw catch-all path of the form `"{id}/{filepath}"` into its
/// numeric flag ID and the remaining file path.  Returns `None` when the
/// first segment is not a valid `u32` or there is no file path after it.
fn parse_id_and_filepath(raw: &str) -> Option<(u32, String)> {
    let (id_str, rest) = raw.split_once('/')?;
    let id: u32 = id_str.parse().ok()?;
    if rest.is_empty() {
        return None;
    }
    Some((id, rest.to_string()))
}

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

pub fn is_json(name: &str) -> bool {
    name.to_lowercase().ends_with(".json")
}

fn not_found_response(filepath: &str) -> Response {
    (
        StatusCode::NOT_FOUND,
        format!("File not found: {}", html::escape(filepath)),
    )
        .into_response()
}
