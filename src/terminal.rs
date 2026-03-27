//! Terminal markdown rendering via termimad.

use regex::Regex;
use termimad::MadSkin;

/// Render markdown content for terminal display using termimad.
///
/// Converts flag tags to bold-formatted annotations before rendering,
/// then uses termimad's `MadSkin` to produce terminal-formatted output.
pub fn render_terminal(content: &str) -> String {
    let prepared = prepare_flags_for_terminal(content);
    let skin = MadSkin::default();
    skin.text(&prepared, None).to_string()
}

/// Convert `<flag:N>Comment: text</flag>` to bold markdown annotations
/// suitable for terminal rendering.
///
/// Transforms: `<flag:1>Comment: check this</flag>`
/// Into: `**[FLAG #1:** check this**]**`
fn prepare_flags_for_terminal(content: &str) -> String {
    let re = Regex::new(r"<flag:(\d+)>Comment:\s*(.+?)</flag>").unwrap();
    re.replace_all(content, "**[FLAG #$1:** $2**]**")
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prepare_flags_basic() {
        let input = "<flag:1>Comment: check this</flag>";
        let result = prepare_flags_for_terminal(input);
        assert_eq!(result, "**[FLAG #1:** check this**]**");
    }

    #[test]
    fn test_prepare_flags_multiple() {
        let input = "Text <flag:1>Comment: first</flag> and <flag:2>Comment: second</flag>.";
        let result = prepare_flags_for_terminal(input);
        assert!(result.contains("FLAG #1"));
        assert!(result.contains("FLAG #2"));
        assert!(result.contains("first"));
        assert!(result.contains("second"));
    }

    #[test]
    fn test_prepare_flags_no_flags() {
        let input = "Just plain text.";
        let result = prepare_flags_for_terminal(input);
        assert_eq!(result, input);
    }
}
