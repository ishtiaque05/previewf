use previewf::terminal::render_terminal;

#[test]
fn test_terminal_render_basic() {
    let content = "# Hello\n\nA paragraph.\n";
    let output = render_terminal(content);
    assert!(output.contains("Hello"));
    assert!(output.contains("paragraph"));
}

#[test]
fn test_terminal_render_with_flags() {
    let content = "Text <flag:1>Comment: check this</flag> here.\n";
    let output = render_terminal(content);
    assert!(output.contains("check this"));
}
