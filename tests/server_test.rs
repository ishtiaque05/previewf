use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

use previewf::server::{create_router, ServerBuilder};

fn create_test_app() -> axum::Router {
    let config = ServerBuilder::new()
        .path("tests/fixtures")
        .port(0)
        .live_reload(false)
        .build()
        .unwrap();
    create_router(config)
}

// --- Index / directory listing ---

#[tokio::test]
async fn test_index_returns_directory_listing() {
    let app = create_test_app();

    let response = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let html = String::from_utf8(body.to_vec()).unwrap();

    assert!(
        html.contains("sample.md"),
        "listing should include sample.md"
    );
    assert!(
        html.contains("sample.html"),
        "listing should include sample.html"
    );
    assert!(
        html.contains("flagged.md"),
        "listing should include flagged.md"
    );
}

// --- View markdown ---

#[tokio::test]
async fn test_view_renders_markdown_as_html() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/view/sample.md")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let html = String::from_utf8(body.to_vec()).unwrap();

    assert!(
        html.contains("<h1>"),
        "rendered HTML should contain an h1 tag"
    );
    assert!(
        html.contains("Sample Document"),
        "rendered HTML should contain the title"
    );
}

#[tokio::test]
async fn test_view_nonexistent_file_returns_404() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/view/nonexistent.md")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// --- Raw HTML ---

#[tokio::test]
async fn test_raw_serves_html_file() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/raw/sample.html")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let html = String::from_utf8(body.to_vec()).unwrap();

    assert!(
        html.contains("Sample HTML Page"),
        "raw should serve the HTML file as-is"
    );
}

#[tokio::test]
async fn test_raw_nonexistent_file_returns_404() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/raw/nonexistent.html")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// --- Flags JSON ---

#[tokio::test]
async fn test_flags_returns_json_for_flagged_file() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/flags/flagged.md")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let content_type = response
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(
        content_type.contains("application/json"),
        "flags endpoint should return JSON"
    );

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["file"], "flagged.md");
    assert!(json["flags"].is_array());
    assert!(
        json["flags"].as_array().unwrap().len() >= 2,
        "flagged.md should have multiple flags"
    );
}

#[tokio::test]
async fn test_flags_returns_empty_for_clean_file() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/flags/sample.md")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(
        json["flags"].as_array().unwrap().len(),
        0,
        "sample.md should have no flags"
    );
}

#[tokio::test]
async fn test_flags_nonexistent_file_returns_404() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/flags/no_such_file.md")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// --- Static assets ---

#[tokio::test]
async fn test_assets_serves_css() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/assets/style.css")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let content_type = response
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(
        content_type.contains("css"),
        "CSS asset should have a css content-type"
    );
}

#[tokio::test]
async fn test_assets_nonexistent_returns_404() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/assets/no_such_file.xyz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// --- ServerBuilder ---

#[test]
fn test_builder_rejects_nonexistent_path() {
    let result = ServerBuilder::new()
        .path("/tmp/does_not_exist_previewf_test")
        .build();
    assert!(result.is_err(), "build should fail for nonexistent path");
}

#[test]
fn test_builder_accepts_valid_path() {
    let result = ServerBuilder::new().path("tests/fixtures").build();
    assert!(result.is_ok(), "build should succeed for valid path");
}

// --- Path traversal prevention ---

#[tokio::test]
async fn test_view_path_traversal_returns_404() {
    let app = create_test_app();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/view/../../etc/passwd")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_raw_path_traversal_returns_404() {
    let app = create_test_app();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/raw/../../etc/passwd")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_flags_path_traversal_returns_404() {
    let app = create_test_app();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/flags/../../etc/passwd")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// --- XSS prevention ---

#[tokio::test]
async fn test_view_xss_in_path_is_escaped() {
    let app = create_test_app();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/view/%3Cscript%3Ealert(1)%3C%2Fscript%3E.md")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let html = String::from_utf8(body.to_vec()).unwrap();
    assert!(
        !html.contains("<script>alert(1)</script>"),
        "Response body must not contain unescaped script tags"
    );
}

// --- View handler validation ---

#[tokio::test]
async fn test_view_non_markdown_returns_400() {
    let app = create_test_app();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/view/sample.html")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

// --- Flag POST endpoint ---

#[tokio::test]
async fn test_flag_post_injects_flag_into_markdown() {
    let dir = tempfile::TempDir::new().unwrap();
    let file_path = dir.path().join("test.md");
    std::fs::write(&file_path, "# Hello\n\nThis is a test line.\n").unwrap();

    let config = ServerBuilder::new()
        .path(dir.path())
        .port(0)
        .live_reload(false)
        .build()
        .unwrap();
    let app = create_router(config);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/flag/test.md")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "comment": "needs review",
                        "selected_text": "test line"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let content = std::fs::read_to_string(&file_path).unwrap();
    assert!(
        content.contains("<flag:1>"),
        "File should contain injected flag"
    );
}

#[tokio::test]
async fn test_flag_post_selected_text_not_found_returns_400() {
    let dir = tempfile::TempDir::new().unwrap();
    let file_path = dir.path().join("test.md");
    std::fs::write(&file_path, "# Hello\n").unwrap();

    let config = ServerBuilder::new()
        .path(dir.path())
        .port(0)
        .live_reload(false)
        .build()
        .unwrap();
    let app = create_router(config);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/flag/test.md")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "comment": "test",
                        "selected_text": "nonexistent text"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_flag_post_nonexistent_file_returns_404() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/flag/nonexistent.md")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "comment": "test",
                        "selected_text": "text"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_flag_post_non_markdown_returns_400() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/flag/sample.html")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "comment": "test",
                        "selected_text": "text"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

// --- Flag DELETE endpoint ---

#[tokio::test]
async fn test_flag_delete_removes_flag() {
    let dir = tempfile::TempDir::new().unwrap();
    let file_path = dir.path().join("test.md");
    std::fs::write(
        &file_path,
        "Hello <flag:1>Comment: remove me</flag> world.\n",
    )
    .unwrap();

    let config = ServerBuilder::new()
        .path(dir.path())
        .port(0)
        .live_reload(false)
        .build()
        .unwrap();
    let app = create_router(config);

    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/flag/1/test.md")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let content = std::fs::read_to_string(&file_path).unwrap();
    assert!(
        !content.contains("<flag:1>"),
        "Flag should be removed from file"
    );
    assert!(
        content.contains("Hello"),
        "Surrounding text should be preserved"
    );
}

#[tokio::test]
async fn test_flag_delete_not_found_returns_404() {
    let dir = tempfile::TempDir::new().unwrap();
    let file_path = dir.path().join("test.md");
    std::fs::write(&file_path, "No flags here.\n").unwrap();

    let config = ServerBuilder::new()
        .path(dir.path())
        .port(0)
        .live_reload(false)
        .build()
        .unwrap();
    let app = create_router(config);

    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/flag/99/test.md")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_flag_delete_non_markdown_returns_400() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::write(dir.path().join("test.html"), "<html></html>").unwrap();

    let config = ServerBuilder::new()
        .path(dir.path())
        .port(0)
        .live_reload(false)
        .build()
        .unwrap();
    let app = create_router(config);

    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/flag/1/test.html")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

// --- Flag PUT endpoint ---

#[tokio::test]
async fn test_flag_put_updates_comment() {
    let dir = tempfile::TempDir::new().unwrap();
    let file_path = dir.path().join("test.md");
    std::fs::write(&file_path, "Line <flag:1>Comment: old</flag> here.\n").unwrap();

    let config = ServerBuilder::new()
        .path(dir.path())
        .port(0)
        .live_reload(false)
        .build()
        .unwrap();
    let app = create_router(config);

    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/flag/1/test.md")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({ "comment": "updated comment" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let content = std::fs::read_to_string(&file_path).unwrap();
    assert!(
        content.contains("updated comment"),
        "Comment should be updated"
    );
    assert!(!content.contains("old"), "Old comment should be replaced");
}

#[tokio::test]
async fn test_flag_put_not_found_returns_404() {
    let dir = tempfile::TempDir::new().unwrap();
    let file_path = dir.path().join("test.md");
    std::fs::write(&file_path, "No flags.\n").unwrap();

    let config = ServerBuilder::new()
        .path(dir.path())
        .port(0)
        .live_reload(false)
        .build()
        .unwrap();
    let app = create_router(config);

    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/flag/99/test.md")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({ "comment": "anything" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_flag_put_empty_comment_returns_400() {
    let dir = tempfile::TempDir::new().unwrap();
    let file_path = dir.path().join("test.md");
    std::fs::write(&file_path, "Line <flag:1>Comment: old</flag> here.\n").unwrap();

    let config = ServerBuilder::new()
        .path(dir.path())
        .port(0)
        .live_reload(false)
        .build()
        .unwrap();
    let app = create_router(config);

    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/flag/1/test.md")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({ "comment": "  " }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

// --- Flag DELETE/PUT path traversal ---

#[tokio::test]
async fn test_flag_delete_path_traversal_returns_404() {
    let app = create_test_app();
    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/flag/1/../../etc/shadow.md")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_flag_put_path_traversal_returns_404() {
    let app = create_test_app();
    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/flag/1/../../etc/shadow.md")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({ "comment": "traversal" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// --- Security headers ---

#[tokio::test]
async fn test_responses_include_security_headers() {
    let app = create_test_app();
    let response = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(
        response.headers().get("X-Content-Type-Options").unwrap(),
        "nosniff"
    );
    assert_eq!(response.headers().get("X-Frame-Options").unwrap(), "DENY");
    assert!(response.headers().get("Content-Security-Policy").is_some());
}
