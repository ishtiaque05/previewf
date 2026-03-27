use googletest::prelude::*;
use previewf::terminal::render_terminal;

#[gtest]
fn test_terminal_render_basic() {
    let content = "# Hello\n\nA paragraph.\n";
    let output = render_terminal(content);
    expect_that!(output, contains_substring("Hello"));
    expect_that!(output, contains_substring("paragraph"));
}

#[gtest]
fn test_terminal_render_with_flags() {
    let content = "Text <flag:1>Comment: check this</flag> here.\n";
    let output = render_terminal(content);
    expect_that!(output, contains_substring("check this"));
}
