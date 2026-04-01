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

use crate::docker::{self, validate_container_name};
use crate::flags::{extract_flags, inject_flag, remove_flag, update_flag_comment, FlagReport};
use crate::html;
use crate::markdown::render_html;
use crate::source::docker::DockerSource;
use crate::source::local::LocalSource;
use crate::source::{EntryType, FileSource};
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
    host: String,
    live_reload: bool,
    docker: bool,
}

impl ServerConfig {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn host(&self) -> &str {
        &self.host
    }

    pub fn live_reload(&self) -> bool {
        self.live_reload
    }

    pub fn docker(&self) -> bool {
        self.docker
    }
}

/// Builder for [`ServerConfig`].
pub struct ServerBuilder {
    path: PathBuf,
    port: u16,
    host: String,
    live_reload: bool,
    docker: bool,
}

impl ServerBuilder {
    pub fn new() -> Self {
        Self {
            path: PathBuf::from("."),
            port: 4567,
            host: "127.0.0.1".to_string(),
            live_reload: true,
            docker: false,
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

    pub fn host<S: Into<String>>(mut self, host: S) -> Self {
        self.host = host.into();
        self
    }

    pub fn live_reload(mut self, enabled: bool) -> Self {
        self.live_reload = enabled;
        self
    }

    pub fn docker(mut self, enabled: bool) -> Self {
        self.docker = enabled;
        self
    }

    /// Build the configuration, validating that the path exists.
    pub fn build(self) -> Result<ServerConfig, PreviewError> {
        let path = std::fs::canonicalize(&self.path)
            .map_err(|_| PreviewError::FileNotFound(self.path.clone()))?;
        Ok(ServerConfig {
            path,
            port: self.port,
            host: self.host,
            live_reload: self.live_reload,
            docker: self.docker,
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
    source: Arc<dyn FileSource>,
    reload_tx: broadcast::Sender<()>,
    file_locks: Arc<Mutex<HashMap<String, Arc<Mutex<()>>>>>,
    docker_available: bool,
    docker_sources: Arc<Mutex<HashMap<String, Arc<DockerSource>>>>,
    docker_reload_txs: Arc<Mutex<HashMap<String, broadcast::Sender<()>>>>,
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

/// Check whether Docker is available on the system (non-blocking).
async fn check_docker_available_async() -> bool {
    tokio::task::spawn_blocking(|| {
        std::process::Command::new("docker")
            .args(["version", "--format", "{{.Client.Version}}"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    })
    .await
    .unwrap_or(false)
}

/// Create the axum [`Router`] for the preview server.
///
/// Exposed publicly so integration tests can drive requests without binding
/// a TCP listener.
pub fn create_router(config: ServerConfig) -> Router {
    create_router_with_reload(config, broadcast::channel::<()>(16).0, false)
}

fn create_router_with_reload(
    config: ServerConfig,
    reload_tx: broadcast::Sender<()>,
    docker_available: bool,
) -> Router {
    let source: Arc<dyn FileSource> = Arc::new(
        LocalSource::new(config.path())
            .expect("LocalSource creation should not fail after ServerBuilder validated path"),
    );

    let state = AppState {
        config,
        source,
        reload_tx,
        file_locks: Arc::new(Mutex::new(HashMap::new())),
        docker_available,
        docker_sources: Arc::new(Mutex::new(HashMap::new())),
        docker_reload_txs: Arc::new(Mutex::new(HashMap::new())),
    };

    let mut router = Router::new()
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
        .route("/ws", get(ws_handler));

    // Docker routes are only registered when explicitly enabled via --docker
    if docker_available {
        router = router
            .route("/docker", get(docker_dashboard_handler))
            .route("/api/docker/containers", get(docker_containers_handler))
            .route("/docker/{container}", get(docker_index_handler))
            .route(
                "/docker/{container}/browse/{*dirpath}",
                get(docker_browse_handler),
            )
            .route(
                "/docker/{container}/view/{*filepath}",
                get(docker_view_handler),
            )
            .route(
                "/docker/{container}/flags/{*filepath}",
                get(docker_flags_handler),
            )
            .route(
                "/docker/{container}/flag/{*filepath}",
                post(docker_flag_handler)
                    .delete(docker_delete_flag_handler)
                    .put(docker_update_flag_handler),
            )
            .route("/docker/{container}/api/tree", get(docker_tree_handler))
            .route("/docker/{container}/ws", get(docker_ws_handler));
    }

    router
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

    // Only probe Docker when --docker flag was passed
    let docker_available = if config.docker() {
        let available = check_docker_available_async().await;
        if available {
            eprintln!("Docker detected — container browsing enabled");
        } else {
            eprintln!("Warning: --docker flag passed but Docker is not available");
        }
        available
    } else {
        false
    };

    let app = create_router_with_reload(config.clone(), reload_tx, docker_available);

    let addr = format!("{}:{}", config.host(), config.port());
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    eprintln!(
        "previewf serving {} on http://{}:{}",
        config.path().display(),
        config.host(),
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
    listing_response(&state, "").await
}

/// `GET /browse/{*dirpath}` — browse into a subdirectory.
async fn browse_handler(
    State(state): State<AppState>,
    AxumPath(dirpath): AxumPath<String>,
) -> Response {
    listing_response(&state, &dirpath).await
}

/// Shared listing logic for both `/` and `/browse/{dirpath}`.
async fn listing_response(state: &AppState, subpath: &str) -> Response {
    let base = state.config.path();
    let source = &state.source;

    // If the configured path is a single file, redirect to the viewer.
    if base.is_file() && subpath.is_empty() {
        let name = base.file_name().unwrap_or_default().to_string_lossy();
        return if is_markdown(&name) {
            Redirect::temporary(&format!("/view/{}", name)).into_response()
        } else {
            Redirect::temporary(&format!("/raw/{}", name)).into_response()
        };
    }

    // Validate that the subdirectory exists, preventing path traversal.
    if !subpath.is_empty() && !source.is_dir(subpath).await {
        return not_found_response(subpath);
    }

    // Build breadcrumbs
    let breadcrumb_html = build_breadcrumbs(subpath);

    // Collect directories, markdown, json, and html files
    let mut dirs: Vec<String> = Vec::new();
    let mut md_files: Vec<(String, usize)> = Vec::new();
    let mut json_files: Vec<String> = Vec::new();
    let mut html_files: Vec<String> = Vec::new();

    if let Ok(entries) = source.list_dir(subpath).await {
        for entry in entries {
            let name = entry.name;

            match entry.entry_type {
                EntryType::Directory => {
                    dirs.push(name);
                }
                EntryType::File => {
                    if is_markdown(&name) {
                        let file_path = if subpath.is_empty() {
                            name.clone()
                        } else {
                            format!("{}/{}", subpath.trim_end_matches('/'), name)
                        };
                        let content = source.read_file(&file_path).await.unwrap_or_default();
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
        source.display_root()
    } else {
        format!("{}/{}", source.display_root(), subpath)
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
    let content = match state.source.read_file(&filepath).await {
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
    match state.source.read_file(&filepath).await {
        Ok(content) => Html(content).into_response(),
        Err(_) => not_found_response(&filepath),
    }
}

/// `GET /flags/{*filepath}` — return flags as JSON.
async fn flags_handler(
    State(state): State<AppState>,
    AxumPath(filepath): AxumPath<String>,
) -> Response {
    let content = match state.source.read_file(&filepath).await {
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
    #[serde(default = "default_label")]
    label: String,
}

fn default_label() -> String {
    "Comment".to_string()
}

/// Request body for flag comment update.
#[derive(Deserialize)]
struct UpdateFlagRequest {
    comment: String,
    label: Option<String>,
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

    // Acquire per-file lock to prevent concurrent read-modify-write races
    let file_lock = {
        let mut locks = state.file_locks.lock().await;
        locks
            .entry(filepath.clone())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    };
    let _guard = file_lock.lock().await;

    let content = match state.source.read_file(&filepath).await {
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

    match inject_flag(&content, line, &body.comment, &body.label) {
        Ok(new_content) => match state.source.write_file(&filepath, &new_content).await {
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

    let file_lock = {
        let mut locks = state.file_locks.lock().await;
        locks
            .entry(filepath.clone())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    };
    let _guard = file_lock.lock().await;

    let content = match state.source.read_file(&filepath).await {
        Ok(c) => c,
        Err(_) => return not_found_response(&filepath),
    };

    match remove_flag(&content, id) {
        Ok(new_content) => match state.source.write_file(&filepath, &new_content).await {
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

    let file_lock = {
        let mut locks = state.file_locks.lock().await;
        locks
            .entry(filepath.clone())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    };
    let _guard = file_lock.lock().await;

    let content = match state.source.read_file(&filepath).await {
        Ok(c) => c,
        Err(_) => return not_found_response(&filepath),
    };

    match update_flag_comment(&content, id, &body.comment, body.label.as_deref()) {
        Ok(new_content) => match state.source.write_file(&filepath, &new_content).await {
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
    let tree = build_tree_async(&*state.source, "", 0).await;

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

/// Recursively build a tree of directories and viewable files via FileSource.
fn build_tree_async<'a>(
    source: &'a dyn FileSource,
    path: &'a str,
    depth: usize,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Vec<TreeNode>> + Send + 'a>> {
    Box::pin(async move {
        if depth >= MAX_TREE_DEPTH {
            return Vec::new();
        }

        let Ok(entries) = source.list_dir(path).await else {
            return Vec::new();
        };

        let mut dir_nodes: Vec<TreeNode> = Vec::new();
        let mut file_nodes: Vec<TreeNode> = Vec::new();

        for entry in entries {
            let name = entry.name;

            // Hidden files are already filtered by LocalSource::list_dir

            let rel_path = if path.is_empty() {
                name.clone()
            } else {
                format!("{}/{}", path, name)
            };

            match entry.entry_type {
                EntryType::Directory => {
                    let children = build_tree_async(source, &rel_path, depth + 1).await;
                    dir_nodes.push(TreeNode {
                        name,
                        node_type: "dir",
                        path: Some(rel_path),
                        children: Some(children),
                    });
                }
                EntryType::File => {
                    if is_markdown(&name) {
                        file_nodes.push(TreeNode {
                            name,
                            node_type: "md",
                            path: Some(rel_path),
                            children: None,
                        });
                    } else if is_json(&name) {
                        file_nodes.push(TreeNode {
                            name,
                            node_type: "json",
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
            }
        }

        dir_nodes.sort_by_key(|a| a.name.to_lowercase());
        file_nodes.sort_by_key(|a| a.name.to_lowercase());
        dir_nodes.extend(file_nodes);
        dir_nodes
    })
}

// ---------------------------------------------------------------------------
// Docker routes
// ---------------------------------------------------------------------------

/// Look up (or lazily create) a [`DockerSource`] for the given container.
async fn get_docker_source(
    state: &AppState,
    container: &str,
) -> Result<Arc<DockerSource>, Response> {
    if !state.docker_available {
        return Err((StatusCode::SERVICE_UNAVAILABLE, "Docker not available").into_response());
    }
    if let Err(e) = validate_container_name(container) {
        return Err((StatusCode::BAD_REQUEST, e.to_string()).into_response());
    }
    let mut sources = state.docker_sources.lock().await;
    if let Some(source) = sources.get(container) {
        return Ok(Arc::clone(source));
    }
    // Use the container's configured WorkingDir as the base path
    let workdir = docker::get_container_workdir(container).await;
    let source = Arc::new(
        DockerSource::new(container.to_string(), workdir)
            .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()).into_response())?,
    );
    sources.insert(container.to_string(), Arc::clone(&source));
    Ok(source)
}

/// `GET /docker` — dedicated Docker dashboard page.
async fn docker_dashboard_handler() -> Response {
    let template = Assets::get("docker.html")
        .map(|f| String::from_utf8_lossy(&f.data).into_owned())
        .unwrap_or_else(|| "<html><body>Template missing</body></html>".to_string());

    Html(template).into_response()
}

/// `GET /api/docker/containers` — list running Docker containers as JSON.
async fn docker_containers_handler(State(state): State<AppState>) -> Response {
    if !state.docker_available {
        return (StatusCode::SERVICE_UNAVAILABLE, "Docker not available").into_response();
    }
    match docker::list_containers().await {
        Ok(containers) => match serde_json::to_string(&containers) {
            Ok(json) => (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "application/json")],
                json,
            )
                .into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        },
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

/// `GET /docker/{container}` — root listing for a container.
async fn docker_index_handler(
    State(state): State<AppState>,
    AxumPath(container): AxumPath<String>,
) -> Response {
    docker_listing_response(&state, &container, "").await
}

/// `GET /docker/{container}/browse/{*dirpath}` — browse into a container subdirectory.
async fn docker_browse_handler(
    State(state): State<AppState>,
    AxumPath((container, dirpath)): AxumPath<(String, String)>,
) -> Response {
    docker_listing_response(&state, &container, &dirpath).await
}

/// System directories to hide when browsing a container root.
const SYSTEM_DIRS: &[&str] = &[
    "bin", "boot", "dev", "etc", "lib", "lib32", "lib64", "libx32", "media", "mnt", "opt", "proc",
    "root", "run", "sbin", "srv", "sys", "tmp", "usr", "var",
];

/// Shared listing logic for Docker container browsing.
async fn docker_listing_response(state: &AppState, container: &str, subpath: &str) -> Response {
    let source = match get_docker_source(state, container).await {
        Ok(s) => s,
        Err(e) => return e,
    };

    // Validate that the subdirectory exists
    if !subpath.is_empty() && !source.is_dir(subpath).await {
        return not_found_response(subpath);
    }

    let breadcrumb_html = build_docker_breadcrumbs(container, subpath);

    // When browsing the root of a container whose workdir is "/", hide system dirs
    let is_root = subpath.is_empty();
    let hide_system = is_root && source.display_root().ends_with(":/");

    let mut dirs: Vec<String> = Vec::new();
    let mut md_files: Vec<(String, usize)> = Vec::new();
    let mut json_files: Vec<String> = Vec::new();
    let mut html_files: Vec<String> = Vec::new();

    if let Ok(entries) = source.list_dir(subpath).await {
        for entry in entries {
            let name = entry.name;
            match entry.entry_type {
                EntryType::Directory => {
                    if hide_system && SYSTEM_DIRS.contains(&name.as_str()) {
                        continue;
                    }
                    dirs.push(name);
                }
                EntryType::File => {
                    if is_markdown(&name) {
                        let file_path = if subpath.is_empty() {
                            name.clone()
                        } else {
                            format!("{}/{}", subpath.trim_end_matches('/'), name)
                        };
                        let content = source.read_file(&file_path).await.unwrap_or_default();
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
    }

    dirs.sort_by_key(|a| a.to_lowercase());
    md_files.sort_by_key(|a| a.0.to_lowercase());
    json_files.sort_by_key(|a| a.to_lowercase());
    html_files.sort_unstable();

    let safe_container = html::escape(container);

    let prefix = if subpath.is_empty() {
        String::new()
    } else {
        format!("{}/", subpath.trim_end_matches('/'))
    };

    let mut file_entries_parts: Vec<String> = Vec::new();

    for name in &dirs {
        let safe_name = html::escape(name);
        let browse_path = html::escape(&format!("{}{}", prefix, name));
        file_entries_parts.push(format!(
            r#"<a class="file-entry file-entry-dir" href="/docker/{safe_container}/browse/{browse_path}"><span class="file-entry-name-group"><span class="file-entry-icon dir-icon">&#128193;</span><span class="file-entry-name">{safe_name}/</span></span><span class="file-entry-badge dir-badge">folder</span></a>"#,
        ));
    }

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
            r#"<a class="file-entry" href="/docker/{safe_container}/view/{view_path}"><span class="file-entry-name-group"><span class="file-entry-icon md-icon">&#9670;</span><span class="file-entry-name">{safe_name}</span></span>{badge}</a>"#,
        ));
    }

    for name in &json_files {
        let safe_name = html::escape(name);
        let view_path = html::escape(&format!("{}{}", prefix, name));
        file_entries_parts.push(format!(
            r#"<a class="file-entry" href="/docker/{safe_container}/view/{view_path}"><span class="file-entry-name-group"><span class="file-entry-icon json-icon">&#123;&#125;</span><span class="file-entry-name">{safe_name}</span></span><span class="file-entry-badge json-badge">json</span></a>"#,
        ));
    }

    for name in &html_files {
        let safe_name = html::escape(name);
        let view_path = html::escape(&format!("{}{}", prefix, name));
        file_entries_parts.push(format!(
            r#"<a class="file-entry" href="/docker/{safe_container}/view/{view_path}"><span class="file-entry-name-group"><span class="file-entry-icon html-icon">&#9671;</span><span class="file-entry-name">{safe_name}</span></span><span class="file-entry-badge html-badge">html</span></a>"#,
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
        source.display_root()
    } else {
        format!("{}/{}", source.display_root(), subpath)
    };

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

/// Build breadcrumb HTML for a Docker container path.
fn build_docker_breadcrumbs(container: &str, path: &str) -> String {
    let safe_container = html::escape(container);
    let mut parts = Vec::new();
    parts.push(r#"<a class="breadcrumb-link" href="/">root</a>"#.to_string());
    parts.push(format!(
        r#"<a class="breadcrumb-link" href="/docker/{safe_container}">&#128051; {safe_container}</a>"#,
    ));

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
                r#"<a class="breadcrumb-link" href="/docker/{safe_container}/browse/{safe_path}">{safe_seg}</a>"#
            ));
        }
    }

    format!(
        r#"<nav class="breadcrumbs">{}</nav>"#,
        parts.join(r#"<span class="breadcrumb-sep">/</span>"#)
    )
}

/// `GET /docker/{container}/view/{*filepath}` — render a file from a Docker container.
async fn docker_view_handler(
    State(state): State<AppState>,
    AxumPath((container, filepath)): AxumPath<(String, String)>,
) -> Response {
    let source = match get_docker_source(&state, &container).await {
        Ok(s) => s,
        Err(e) => return e,
    };

    let content = match source.read_file(&filepath).await {
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

    let breadcrumb_html = build_docker_breadcrumbs(&container, &filepath);

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

/// `GET /docker/{container}/flags/{*filepath}` — return flags as JSON for a Docker file.
async fn docker_flags_handler(
    State(state): State<AppState>,
    AxumPath((container, filepath)): AxumPath<(String, String)>,
) -> Response {
    let source = match get_docker_source(&state, &container).await {
        Ok(s) => s,
        Err(e) => return e,
    };

    let content = match source.read_file(&filepath).await {
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

/// `POST /docker/{container}/flag/{*filepath}` — inject a flag into a Docker container file.
async fn docker_flag_handler(
    State(state): State<AppState>,
    AxumPath((container, filepath)): AxumPath<(String, String)>,
    axum::Json(body): axum::Json<FlagRequest>,
) -> Response {
    let source = match get_docker_source(&state, &container).await {
        Ok(s) => s,
        Err(e) => return e,
    };

    if !is_markdown(&filepath) {
        return (
            StatusCode::BAD_REQUEST,
            "Flags can only be added to markdown files",
        )
            .into_response();
    }

    // Use docker-namespaced lock key to avoid collisions with local file locks
    let lock_key = format!("docker:{}:{}", container, filepath);
    let file_lock = {
        let mut locks = state.file_locks.lock().await;
        locks
            .entry(lock_key)
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    };
    let _guard = file_lock.lock().await;

    let content = match source.read_file(&filepath).await {
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

    match inject_flag(&content, line, &body.comment, &body.label) {
        Ok(new_content) => match source.write_file(&filepath, &new_content).await {
            Ok(_) => (StatusCode::OK, "Flag injected").into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        },
        Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    }
}

/// `DELETE /docker/{container}/flag/{id}/{filepath…}` — remove a flag from a Docker container file.
async fn docker_delete_flag_handler(
    State(state): State<AppState>,
    AxumPath((container, raw_path)): AxumPath<(String, String)>,
) -> Response {
    let source = match get_docker_source(&state, &container).await {
        Ok(s) => s,
        Err(e) => return e,
    };

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

    let lock_key = format!("docker:{}:{}", container, filepath);
    let file_lock = {
        let mut locks = state.file_locks.lock().await;
        locks
            .entry(lock_key)
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    };
    let _guard = file_lock.lock().await;

    let content = match source.read_file(&filepath).await {
        Ok(c) => c,
        Err(_) => return not_found_response(&filepath),
    };

    match remove_flag(&content, id) {
        Ok(new_content) => match source.write_file(&filepath, &new_content).await {
            Ok(_) => (StatusCode::OK, "Flag removed").into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        },
        Err(e) => (StatusCode::NOT_FOUND, e.to_string()).into_response(),
    }
}

/// `PUT /docker/{container}/flag/{id}/{filepath…}` — update a flag in a Docker container file.
async fn docker_update_flag_handler(
    State(state): State<AppState>,
    AxumPath((container, raw_path)): AxumPath<(String, String)>,
    axum::Json(body): axum::Json<UpdateFlagRequest>,
) -> Response {
    let source = match get_docker_source(&state, &container).await {
        Ok(s) => s,
        Err(e) => return e,
    };

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

    let lock_key = format!("docker:{}:{}", container, filepath);
    let file_lock = {
        let mut locks = state.file_locks.lock().await;
        locks
            .entry(lock_key)
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    };
    let _guard = file_lock.lock().await;

    let content = match source.read_file(&filepath).await {
        Ok(c) => c,
        Err(_) => return not_found_response(&filepath),
    };

    match update_flag_comment(&content, id, &body.comment, body.label.as_deref()) {
        Ok(new_content) => match source.write_file(&filepath, &new_content).await {
            Ok(_) => (StatusCode::OK, "Flag updated").into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        },
        Err(e) => (StatusCode::NOT_FOUND, e.to_string()).into_response(),
    }
}

/// `GET /docker/{container}/api/tree` — return the directory tree for a Docker container.
async fn docker_tree_handler(
    State(state): State<AppState>,
    AxumPath(container): AxumPath<String>,
) -> Response {
    let source = match get_docker_source(&state, &container).await {
        Ok(s) => s,
        Err(e) => return e,
    };

    let tree = build_tree_async(&*source as &dyn FileSource, "", 0).await;

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

/// `GET /docker/{container}/ws` — per-container WebSocket reload channel.
async fn docker_ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    AxumPath(container): AxumPath<String>,
) -> Response {
    let reload_tx = {
        let mut txs = state.docker_reload_txs.lock().await;
        txs.entry(container.clone())
            .or_insert_with(|| broadcast::channel::<()>(16).0)
            .clone()
    };
    let rx = reload_tx.subscribe();
    ws.on_upgrade(move |socket| handle_docker_ws(socket, rx))
}

async fn handle_docker_ws(mut socket: WebSocket, mut rx: broadcast::Receiver<()>) {
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
                    _ => {}
                }
            }
        }
    }
}

/// Create a router for `previewf docker serve` that uses a [`DockerSource`] as
/// the primary source.
pub fn create_docker_router(
    config: ServerConfig,
    source: Arc<DockerSource>,
    reload_tx: broadcast::Sender<()>,
) -> Router {
    let state = AppState {
        config,
        source: source as Arc<dyn FileSource>,
        reload_tx,
        file_locks: Arc::new(Mutex::new(HashMap::new())),
        docker_available: true,
        docker_sources: Arc::new(Mutex::new(HashMap::new())),
        docker_reload_txs: Arc::new(Mutex::new(HashMap::new())),
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
