//! Markdown parsing and HTML rendering with syntax highlighting and flag support.

use std::sync::LazyLock;

use regex::Regex;

use crate::flags::FLAG_RE;
use crate::html;
use syntect::highlighting::ThemeSet;
use syntect::html::highlighted_html_for_string;
use syntect::parsing::SyntaxSet;

static CODE_BLOCK_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"<pre><code class="language-(\w+)">([\s\S]*?)</code></pre>"#).unwrap()
});

static DIFF_BLOCK_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"<pre><code class="language-diff">([\s\S]*?)</code></pre>"#).unwrap()
});

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
    let theme = THEME_SET
        .themes
        .get("base16-ocean.dark")
        .expect("default syntect theme set must include base16-ocean.dark");

    CODE_BLOCK_RE
        .replace_all(html, |caps: &regex::Captures| {
            let lang = &caps[1];
            let code = html::unescape(&caps[2]);

            if lang == "diff" {
                return caps[0].to_string();
            }

            if let Some(syntax) = SYNTAX_SET.find_syntax_by_token(lang) {
                match highlighted_html_for_string(&code, &SYNTAX_SET, syntax, theme) {
                    Ok(highlighted) => highlighted,
                    Err(e) => {
                        eprintln!("Warning: syntax highlighting failed for '{lang}': {e}");
                        caps[0].to_string()
                    }
                }
            } else {
                eprintln!("Warning: no syntax definition for '{lang}'");
                caps[0].to_string()
            }
        })
        .into_owned()
}

fn render_diff_blocks(html: &str) -> String {
    DIFF_BLOCK_RE
        .replace_all(html, |caps: &regex::Captures| {
            let raw = html::unescape(&caps[1]);
            let mut lines_html = String::new();

            for line in raw.lines() {
                let (class, escaped) = if line.starts_with('+') {
                    ("diff-added", html::escape(line))
                } else if line.starts_with('-') {
                    ("diff-removed", html::escape(line))
                } else if line.starts_with("@@") {
                    ("diff-hunk", html::escape(line))
                } else {
                    ("diff-context", html::escape(line))
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
            let comment = html::escape(caps[2].trim());
            format!(
                "<span class=\"flag\" data-flag-id=\"{id}\">\
                 <span class=\"flag-marker\">#{id}</span>\
                 <span class=\"flag-comment\">{comment}</span>\
                 </span>"
            )
        })
        .into_owned()
}


#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;

    #[gtest]
    fn test_html_escape_roundtrip() {
        let original = "<p>Hello & \"world\"</p>";
        let encoded = html::escape(original);
        let decoded = html::unescape(&encoded);
        expect_that!(decoded, eq(original));
    }

    #[gtest]
    fn test_render_flags_basic() {
        let input = "<flag:1>Comment: something</flag>";
        let result = render_flags(input);
        expect_that!(result, contains_substring("class=\"flag\""));
        expect_that!(result, contains_substring("data-flag-id=\"1\""));
        expect_that!(result, contains_substring("class=\"flag-marker\""));
        expect_that!(result, contains_substring("class=\"flag-comment\""));
        expect_that!(result, contains_substring("something"));
    }

    #[gtest]
    fn test_render_flags_escapes_html_in_comment() {
        let input = "<flag:1>Comment: <script>alert(1)</script></flag>";
        let result = render_flags(input);
        expect_that!(result, not(contains_substring("<script>")));
        expect_that!(result, contains_substring("&lt;script&gt;"));
    }

    #[gtest]
    fn test_html_escape_decode_no_double_decode() {
        // Source code containing literal "&lt;" should survive encode→decode roundtrip
        let input = "&amp;lt;script&amp;gt;";
        let decoded = html::unescape(input);
        expect_that!(decoded, eq("&lt;script&gt;"));
    }
}
