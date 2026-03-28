use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::PreviewError;

pub static FLAG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<flag:(\d+)>Comment:\s*(.+?)</flag>").unwrap());

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Flag {
    pub id: u32,
    pub line: usize,
    pub context: String,
    pub comment: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FlagReport {
    pub file: String,
    pub flags: Vec<Flag>,
}

/// Sanitize a comment string to prevent flag tag corruption.
/// Escapes `</flag>`, `<flag:`, and bare angle brackets to prevent tag nesting.
/// General HTML escaping (`&`, `"`, `'`) is NOT done here; `render_flags` handles that later.
fn sanitize_comment(comment: &str) -> String {
    comment
        .replace("</flag>", "[/flag]")
        .replace("<flag:", "[flag:")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Extract all flags from markdown content.
/// Skips flags with unparseable IDs (e.g., u32 overflow).
pub fn extract_flags(content: &str) -> Vec<Flag> {
    let mut flags = Vec::new();

    for (line_num, line) in content.lines().enumerate() {
        for cap in FLAG_RE.captures_iter(line) {
            let id: u32 = match cap[1].parse() {
                Ok(id) if id > 0 => id,
                _ => continue,
            };
            let comment = cap[2].to_string();
            let context = FLAG_RE.replace_all(line, "").to_string();

            flags.push(Flag {
                id,
                line: line_num + 1,
                context,
                comment,
            });
        }
    }

    flags
}

/// Find the next available flag ID in the content.
pub fn next_flag_id(content: &str) -> u32 {
    let flags = extract_flags(content);
    flags.iter().map(|f| f.id).max().unwrap_or(0) + 1
}

/// Returns true if the line is a code fence delimiter (``` or ~~~).
fn is_code_fence(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("```") || trimmed.starts_with("~~~")
}

/// Inject a new flag at the given line number (1-indexed).
pub fn inject_flag(content: &str, line: usize, comment: &str) -> Result<String, PreviewError> {
    let lines: Vec<&str> = content.lines().collect();

    if line == 0 || line > lines.len() {
        return Err(PreviewError::FlagParse {
            line,
            detail: format!(
                "Line {} is out of range (file has {} lines)",
                line,
                lines.len()
            ),
        });
    }

    let target_line = lines[line - 1];
    if is_code_fence(target_line) {
        return Err(PreviewError::FlagParse {
            line,
            detail: "Cannot inject flag into a code fence delimiter".to_string(),
        });
    }

    let sanitized = sanitize_comment(comment);
    let next_id = next_flag_id(content);
    let flag_tag = format!(" <flag:{}>Comment: {}</flag>", next_id, sanitized);

    let mut result: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
    result[line - 1].push_str(&flag_tag);

    let mut output = result.join("\n");
    if content.ends_with('\n') {
        output.push('\n');
    }

    Ok(output)
}

/// Remove a flag by ID from the content.
/// Returns the content with the flag tag stripped, preserving surrounding text.
pub fn remove_flag(content: &str, id: u32) -> Result<String, PreviewError> {
    let target = Regex::new(&format!(r"<flag:{id}>Comment:\s*.+?</flag>")).unwrap();
    let mut found = false;

    let result: Vec<String> = content
        .lines()
        .map(|line| {
            if target.is_match(line) {
                found = true;
                target.replace_all(line, "").to_string()
            } else {
                line.to_string()
            }
        })
        .collect();

    if !found {
        return Err(PreviewError::FlagParse {
            line: 0,
            detail: format!("Flag with ID {} not found", id),
        });
    }

    let mut output = result.join("\n");
    if content.ends_with('\n') {
        output.push('\n');
    }
    Ok(output)
}

/// Format flags as human-readable text output.
pub fn format_flags_text(report: &FlagReport) -> String {
    let mut output = format!("Flags in {}:\n\n", report.file);

    if report.flags.is_empty() {
        output.push_str("  No flags found.\n");
        return output;
    }

    for flag in &report.flags {
        output.push_str(&format!(
            "  #{} (line {}): {}\n    Context: {}\n\n",
            flag.id, flag.line, flag.comment, flag.context
        ));
    }

    output
}
