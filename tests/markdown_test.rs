use googletest::prelude::*;
use previewf::markdown::render_html;

#[gtest]
fn test_render_heading() {
    let html = render_html("# Hello World");
    expect_that!(html, contains_substring("<h1>"));
    expect_that!(html, contains_substring("Hello World"));
}

#[gtest]
fn test_render_code_block_has_syntax_class() {
    let input = "```rust\nfn main() {}\n```";
    let html = render_html(input);
    expect_that!(html, contains_substring("<pre"));
    expect_that!(html, contains_substring("fn"));
}

#[gtest]
fn test_render_inline_code() {
    let html = render_html("Use `cargo build` to compile.");
    expect_that!(html, contains_substring("<code>"));
    expect_that!(html, contains_substring("cargo build"));
}

#[gtest]
fn test_render_bold_italic() {
    let html = render_html("This is **bold** and *italic*.");
    expect_that!(html, contains_substring("<strong>bold</strong>"));
    expect_that!(html, contains_substring("<em>italic</em>"));
}

#[gtest]
fn test_render_flag_tags_preserved() {
    let input = "Text <flag:1>Comment: something</flag> here.";
    let html = render_html(input);
    expect_that!(html, contains_substring("flag"));
    expect_that!(html, contains_substring("something"));
}

#[gtest]
fn test_render_diff_code_block() {
    let input = "```diff\n- old line\n+ new line\n@@ -1,3 +1,3 @@\n```";
    let html = render_html(input);
    expect_that!(
        html,
        any!(
            contains_substring("diff-removed"),
            contains_substring("diff-added")
        )
    );
}
