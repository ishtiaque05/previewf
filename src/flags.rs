use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Flag {
    pub id: u32,
    pub line: usize,
    pub text: String,
    pub comment: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlagReport {
    pub file: String,
    pub flags: Vec<Flag>,
}

/// Extract all flags from markdown content.
pub fn extract_flags(content: &str) -> Vec<Flag> {
    let re = Regex::new(r"<flag:(\d+)>Comment:\s*(.+?)</flag>").unwrap();
    let mut flags = Vec::new();

    for (line_num, line) in content.lines().enumerate() {
        for cap in re.captures_iter(line) {
            let id: u32 = cap[1].parse().unwrap_or(0);
            let comment = cap[2].trim().to_string();
            let text = re.replace_all(line, "").trim().to_string();

            flags.push(Flag {
                id,
                line: line_num + 1,
                text,
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
            flag.id, flag.line, flag.comment, flag.text
        ));
    }

    output
}
