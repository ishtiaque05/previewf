//! Markdown parsing and HTML rendering with syntax highlighting and flag support.

use std::sync::LazyLock;

use regex::Regex;
use syntect::highlighting::ThemeSet;
use syntect::html::highlighted_html_for_string;
use syntect::parsing::SyntaxSet;

static CODE_BLOCK_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"<pre><code class="language-(\w+)">([\s\S]*?)</code></pre>"#).unwrap()
});

static DIFF_BLOCK_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"<pre><code class="language-diff">([\s\S]*?)</code></pre>"#).unwrap()
});

static FLAG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<flag:(\d+)>Comment:\s*(.+?)</flag>").unwrap());

static SYNTAX_SET: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_newlines);
static THEME_SET: LazyLock<ThemeSet> = LazyLock::new(ThemeSet::load_defaults);

/// Render markdown content to HTML with syntax highlighting and flag support.
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

fn highlight_code_blocks(html: &str) -> String {
    let theme = &THEME_SET.themes["base16-ocean.dark"];

    CODE_BLOCK_RE
        .replace_all(html, |caps: &regex::Captures| {
            let lang = &caps[1];
            let code = html_escape_decode(&caps[2]);

            if lang == "diff" {
                return caps[0].to_string();
            }

            if let Some(syntax) = SYNTAX_SET.find_syntax_by_token(lang) {
                match highlighted_html_for_string(&code, &SYNTAX_SET, syntax, theme) {
                    Ok(highlighted) => highlighted,
                    Err(_) => caps[0].to_string(),
                }
            } else {
                caps[0].to_string()
            }
        })
        .into_owned()
}

fn render_diff_blocks(html: &str) -> String {
    DIFF_BLOCK_RE
        .replace_all(html, |caps: &regex::Captures| {
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

fn render_flags(html: &str) -> String {
    FLAG_RE
        .replace_all(html, |caps: &regex::Captures| {
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

fn html_escape_decode(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

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
