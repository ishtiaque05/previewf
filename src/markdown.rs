//! Markdown parsing and HTML rendering with syntax highlighting and flag support.

use regex::Regex;
use syntect::highlighting::ThemeSet;
use syntect::html::highlighted_html_for_string;
use syntect::parsing::SyntaxSet;

/// Render markdown content to HTML with syntax highlighting and flag support.
///
/// This function:
/// 1. Converts markdown to HTML using comrak (with GFM extensions)
/// 2. Post-processes code blocks with syntect for syntax highlighting
/// 3. Detects `diff` language blocks and renders with git-style CSS classes
/// 4. Converts `<flag:N>Comment: text</flag>` to styled `<span>` elements
pub fn render_html(content: &str) -> String {
    let mut options = comrak::Options::default();
    options.extension.strikethrough = true;
    options.extension.table = true;
    options.extension.autolink = true;
    options.extension.tasklist = true;
    options.extension.footnotes = true;
    options.render.unsafe_ = true;

    let html = comrak::markdown_to_html(content, &options);

    let html = highlight_code_blocks(&html);
    let html = render_diff_blocks(&html);
    render_flags(&html)
}

/// Apply syntect syntax highlighting to fenced code blocks (except diff blocks).
fn highlight_code_blocks(html: &str) -> String {
    let re = Regex::new(r#"<pre><code class="language-(\w+)">([\s\S]*?)</code></pre>"#).unwrap();

    let ss = SyntaxSet::load_defaults_newlines();
    let ts = ThemeSet::load_defaults();
    let theme = &ts.themes["base16-ocean.dark"];

    re.replace_all(html, |caps: &regex::Captures| {
        let lang = &caps[1];
        let code = html_escape_decode(&caps[2]);

        // diff blocks are handled separately
        if lang == "diff" {
            return caps[0].to_string();
        }

        if let Some(syntax) = ss.find_syntax_by_token(lang) {
            match highlighted_html_for_string(&code, &ss, syntax, theme) {
                Ok(highlighted) => highlighted,
                Err(_) => caps[0].to_string(),
            }
        } else {
            caps[0].to_string()
        }
    })
    .into_owned()
}

/// Render diff code blocks with git-style CSS classes.
fn render_diff_blocks(html: &str) -> String {
    let re = Regex::new(r#"<pre><code class="language-diff">([\s\S]*?)</code></pre>"#).unwrap();

    re.replace_all(html, |caps: &regex::Captures| {
        let raw = html_escape_decode(&caps[1]);
        let mut lines_html = String::new();

        for line in raw.lines() {
            let (class, escaped) = if line.starts_with('+') {
                ("diff-added", html_escape_encode(line))
            } else if line.starts_with('-') {
                ("diff-removed", html_escape_encode(line))
            } else if line.starts_with("@@") {
                ("diff-hunk", html_escape_encode(line))
            } else {
                ("diff-context", html_escape_encode(line))
            };
            lines_html.push_str(&format!("<span class=\"{class}\">{escaped}</span>\n"));
        }

        format!("<pre class=\"diff-block\"><code>{lines_html}</code></pre>")
    })
    .into_owned()
}

/// Convert `<flag:N>Comment: text</flag>` to styled `<span>` elements.
fn render_flags(html: &str) -> String {
    let re = Regex::new(r"<flag:(\d+)>Comment:\s*(.+?)</flag>").unwrap();

    re.replace_all(html, |caps: &regex::Captures| {
        let id = &caps[1];
        let comment = caps[2].trim();
        format!(
            "<span class=\"flag\" data-flag-id=\"{id}\">\
             <span class=\"flag-marker\">#{id}</span>\
             <span class=\"flag-comment\">{comment}</span>\
             </span>"
        )
    })
    .into_owned()
}

/// Decode HTML entities back to their character equivalents for syntect input.
fn html_escape_decode(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

/// Encode special characters as HTML entities.
fn html_escape_encode(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_html_escape_roundtrip() {
        let original = "<p>Hello & \"world\"</p>";
        let encoded = html_escape_encode(original);
        let decoded = html_escape_decode(&encoded);
        assert_eq!(decoded, original);
    }

    #[test]
    fn test_render_flags_basic() {
        let input = "<flag:1>Comment: something</flag>";
        let result = render_flags(input);
        assert!(result.contains("class=\"flag\""));
        assert!(result.contains("data-flag-id=\"1\""));
        assert!(result.contains("class=\"flag-marker\""));
        assert!(result.contains("class=\"flag-comment\""));
        assert!(result.contains("something"));
    }
}
