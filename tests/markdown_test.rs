use previewf::markdown::render_html;

#[test]
fn test_render_heading() {
    let html = render_html("# Hello World");
    assert!(html.contains("<h1>"));
    assert!(html.contains("Hello World"));
}

#[test]
fn test_render_code_block_has_syntax_class() {
    let input = "```rust\nfn main() {}\n```";
    let html = render_html(input);
    assert!(html.contains("<pre"));
    assert!(html.contains("fn"));
}

#[test]
fn test_render_inline_code() {
    let html = render_html("Use `cargo build` to compile.");
    assert!(html.contains("<code>"));
    assert!(html.contains("cargo build"));
}

#[test]
fn test_render_bold_italic() {
    let html = render_html("This is **bold** and *italic*.");
    assert!(html.contains("<strong>bold</strong>"));
    assert!(html.contains("<em>italic</em>"));
}

#[test]
fn test_render_flag_tags_preserved() {
    let input = "Text <flag:1>Comment: something</flag> here.";
    let html = render_html(input);
    assert!(html.contains("flag"));
    assert!(html.contains("something"));
}

#[test]
fn test_render_diff_code_block() {
    let input = "```diff\n- old line\n+ new line\n@@ -1,3 +1,3 @@\n```";
    let html = render_html(input);
    assert!(html.contains("diff-removed") || html.contains("diff-added"));
}
