use previewf::flags::{extract_flags, inject_flag, Flag, FlagReport};

#[test]
fn test_extract_flags_from_flagged_file() {
    let content = std::fs::read_to_string("tests/fixtures/flagged.md").unwrap();
    let flags = extract_flags(&content);

    assert_eq!(flags.len(), 4);
    assert_eq!(flags[0].id, 1);
    assert_eq!(flags[0].comment, "need to rethink this approach");
    assert_eq!(flags[0].line, 3);
    assert_eq!(flags[1].id, 2);
    assert_eq!(flags[1].comment, "contradicts section 3");
    assert_eq!(flags[1].line, 5);
}

#[test]
fn test_extract_flags_from_clean_file() {
    let content = std::fs::read_to_string("tests/fixtures/sample.md").unwrap();
    let flags = extract_flags(&content);
    assert_eq!(flags.len(), 0);
}

#[test]
fn test_extract_flags_multiple_on_one_line() {
    let content = std::fs::read_to_string("tests/fixtures/flagged.md").unwrap();
    let flags = extract_flags(&content);
    let line_9_flags: Vec<&Flag> = flags.iter().filter(|f| f.line == 9).collect();
    assert_eq!(line_9_flags.len(), 2);
    assert_eq!(line_9_flags[0].id, 3);
    assert_eq!(line_9_flags[1].id, 4);
}

#[test]
fn test_flag_report_json() {
    let content = std::fs::read_to_string("tests/fixtures/flagged.md").unwrap();
    let flags = extract_flags(&content);
    let report = FlagReport {
        file: "flagged.md".to_string(),
        flags,
    };
    let json = serde_json::to_string_pretty(&report).unwrap();
    assert!(json.contains("\"id\": 1"));
    assert!(json.contains("need to rethink this approach"));
}

#[test]
fn test_next_flag_id_with_existing_flags() {
    let content = std::fs::read_to_string("tests/fixtures/flagged.md").unwrap();
    let next = previewf::flags::next_flag_id(&content);
    assert_eq!(next, 5);
}

#[test]
fn test_next_flag_id_no_flags() {
    let content = std::fs::read_to_string("tests/fixtures/sample.md").unwrap();
    let next = previewf::flags::next_flag_id(&content);
    assert_eq!(next, 1);
}

#[test]
fn test_inject_flag_into_clean_content() {
    let content = "Line one\nLine two\nLine three\n";
    let result = inject_flag(content, 2, "needs work").unwrap();
    assert!(result.contains("<flag:1>Comment: needs work</flag>"));
    assert!(result.contains("Line two"));
}

#[test]
fn test_inject_flag_into_flagged_content() {
    let content = "Line one\n<flag:1>Comment: existing</flag> Line two\nLine three\n";
    let result = inject_flag(content, 3, "also this").unwrap();
    assert!(result.contains("<flag:2>Comment: also this</flag>"));
    assert!(result.contains("<flag:1>Comment: existing</flag>"));
}

#[test]
fn test_inject_flag_invalid_line() {
    let content = "Line one\nLine two\n";
    let result = inject_flag(content, 5, "bad line");
    assert!(result.is_err());
}
