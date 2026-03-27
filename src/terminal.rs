//! Terminal markdown rendering via termimad.

use std::sync::LazyLock;

use regex::Regex;
use termimad::MadSkin;

static FLAG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<flag:(\d+)>Comment:\s*(.+?)</flag>").unwrap());

/// Render markdown content for terminal display.
pub fn render_terminal(content: &str) -> String {
    let prepared = prepare_flags_for_terminal(content);
    let skin = MadSkin::default();
    skin.text(&prepared, None).to_string()
}

fn prepare_flags_for_terminal(content: &str) -> String {
    FLAG_RE
        .replace_all(content, "**[FLAG #$1:** $2**]**")
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;

    #[gtest]
    fn test_prepare_flags_basic() {
        let input = "<flag:1>Comment: check this</flag>";
        let result = prepare_flags_for_terminal(input);
        expect_that!(result, eq("**[FLAG #1:** check this**]**"));
    }

    #[gtest]
    fn test_prepare_flags_multiple() {
        let input = "Text <flag:1>Comment: first</flag> and <flag:2>Comment: second</flag>.";
        let result = prepare_flags_for_terminal(input);
        expect_that!(result, contains_substring("FLAG #1"));
        expect_that!(result, contains_substring("FLAG #2"));
        expect_that!(result, contains_substring("first"));
        expect_that!(result, contains_substring("second"));
    }

    #[gtest]
    fn test_prepare_flags_no_flags() {
        let input = "Just plain text.";
        let result = prepare_flags_for_terminal(input);
        expect_that!(result, eq(input));
    }
}
