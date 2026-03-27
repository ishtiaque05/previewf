/// Render markdown content to HTML using comrak.
pub fn render_html(content: &str) -> String {
    let mut options = comrak::Options::default();
    options.extension.strikethrough = true;
    options.extension.table = true;
    options.render.unsafe_ = true;
    comrak::markdown_to_html(content, &options)
}
