use previewf::flags::{
    extract_flags, format_flags_text, inject_flag, remove_flag, update_flag_comment, Flag,
    FlagReport,
};

// --- extract_flags ---

#[test]
fn test_extract_flags_from_flagged_file() {
    let content = std::fs::read_to_string("tests/fixtures/flagged.md").unwrap();
    let flags = extract_flags(&content);

    assert_eq!(flags.len(), 4);
    assert_eq!(flags[0].id, 1);
    assert_eq!(flags[0].label, "Comment");
    assert_eq!(flags[0].comment, "need to rethink this approach");
    assert_eq!(flags[0].line, 3);
    assert_eq!(flags[1].id, 2);
    assert_eq!(flags[1].label, "Comment");
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
fn test_extract_flags_empty_string() {
    let flags = extract_flags("");
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
fn test_extract_flags_skips_invalid_id_zero() {
    let content = "<flag:0>Comment: zero id</flag>\n";
    let flags = extract_flags(content);
    assert_eq!(flags.len(), 0, "flag with id=0 should be skipped");
}

#[test]
fn test_extract_flags_skips_overflowed_id() {
    let content = "<flag:99999999999>Comment: overflow</flag>\n";
    let flags = extract_flags(content);
    assert_eq!(
        flags.len(),
        0,
        "flag with overflowed u32 id should be skipped"
    );
}

#[test]
fn test_extract_flags_preserves_leading_whitespace_in_context() {
    let content = "    indented text <flag:1>Comment: check indent</flag>\n";
    let flags = extract_flags(content);
    assert_eq!(flags.len(), 1);
    assert!(
        flags[0].context.starts_with("    "),
        "leading whitespace should be preserved, got: {:?}",
        flags[0].context
    );
}

#[test]
fn test_extract_flags_without_comment_prefix_ignored() {
    let content = "<flag:1>needs work</flag>\n";
    let flags = extract_flags(content);
    assert_eq!(
        flags.len(),
        0,
        "flag without Comment: prefix should not match"
    );
}

#[test]
fn test_extract_flags_parses_label() {
    let content = "Line with <flag:1>Bug: something broken</flag> here.";
    let flags = extract_flags(content);
    assert_eq!(flags.len(), 1);
    assert_eq!(flags[0].id, 1);
    assert_eq!(flags[0].label, "Bug");
    assert_eq!(flags[0].comment, "something broken");
}

#[test]
fn test_extract_flags_custom_label() {
    let content = "Line <flag:1>Perf: slow query</flag> here.";
    let flags = extract_flags(content);
    assert_eq!(flags.len(), 1);
    assert_eq!(flags[0].label, "Perf");
    assert_eq!(flags[0].comment, "slow query");
}

// --- next_flag_id ---

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
fn test_next_flag_id_non_sequential() {
    let content = "A <flag:1>Comment: first</flag>\nB <flag:10>Comment: tenth</flag>\n";
    let next = previewf::flags::next_flag_id(content);
    assert_eq!(next, 11, "should return max+1 even with gaps");
}

// --- inject_flag ---

#[test]
fn test_inject_flag_into_clean_content() {
    let content = "Line one\nLine two\nLine three\n";
    let result = inject_flag(content, 2, "needs work", "Comment").unwrap();
    assert!(result.contains("<flag:1>Comment: needs work</flag>"));
    assert!(result.contains("Line two"));
}

#[test]
fn test_inject_flag_into_flagged_content() {
    let content = "Line one\n<flag:1>Comment: existing</flag> Line two\nLine three\n";
    let result = inject_flag(content, 3, "also this", "Comment").unwrap();
    assert!(result.contains("<flag:2>Comment: also this</flag>"));
    assert!(result.contains("<flag:1>Comment: existing</flag>"));
}

#[test]
fn test_inject_flag_invalid_line_too_high() {
    let content = "Line one\nLine two\n";
    let result = inject_flag(content, 5, "bad line", "Comment");
    assert!(result.is_err());
}

#[test]
fn test_inject_flag_invalid_line_zero() {
    let content = "Line one\nLine two\n";
    let result = inject_flag(content, 0, "bad line", "Comment");
    assert!(result.is_err());
}

#[test]
fn test_inject_flag_preserves_trailing_newline() {
    let content = "Line one\nLine two\n";
    let result = inject_flag(content, 1, "test", "Comment").unwrap();
    assert!(
        result.ends_with('\n'),
        "trailing newline should be preserved"
    );
}

#[test]
fn test_inject_flag_no_trailing_newline_when_absent() {
    let content = "Line one\nLine two";
    let result = inject_flag(content, 1, "test", "Comment").unwrap();
    assert!(
        !result.ends_with('\n'),
        "should not add trailing newline when original lacks one"
    );
}

#[test]
fn test_inject_flag_sanitizes_closing_tag_in_comment() {
    let content = "Line one\nLine two\n";
    let result = inject_flag(content, 1, "break </flag> here", "Comment").unwrap();
    assert!(
        !result.contains("break </flag> here"),
        "raw </flag> in comment should be escaped"
    );
    // The injected flag should still be extractable
    let flags = extract_flags(&result);
    assert_eq!(flags.len(), 1);
}

#[test]
fn test_inject_flag_sanitizes_html_in_comment() {
    let content = "Line one\nLine two\n";
    let result = inject_flag(content, 1, "<script>alert('xss')</script>", "Comment").unwrap();
    assert!(
        !result.contains("<script>"),
        "HTML tags in comment should be escaped"
    );
}

#[test]
fn test_inject_flag_rejects_code_fence_line() {
    let content = "Text before\n```rust\nfn main() {}\n```\nText after\n";
    let result = inject_flag(content, 2, "bad target", "Comment");
    assert!(
        result.is_err(),
        "should reject injection into code fence delimiter"
    );
}

#[test]
fn test_inject_flag_rejects_tilde_code_fence() {
    let content = "Text before\n~~~\ncode\n~~~\nText after\n";
    let result = inject_flag(content, 2, "bad target", "Comment");
    assert!(
        result.is_err(),
        "should reject injection into ~~~ fence delimiter"
    );
}

// --- inject-extract round-trip ---

#[test]
fn test_inject_then_extract_round_trip() {
    let content = "Line one\nLine two\nLine three\n";
    let injected = inject_flag(content, 2, "review this", "Comment").unwrap();
    let flags = extract_flags(&injected);
    assert_eq!(flags.len(), 1);
    assert_eq!(flags[0].id, 1);
    assert_eq!(flags[0].line, 2);
    assert_eq!(flags[0].comment, "review this");
}

// --- flag_report_json ---

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
    assert!(
        json.contains("\"context\""),
        "JSON should use 'context' field name"
    );
}

// --- format_flags_text ---

#[test]
fn test_format_flags_text_with_flags() {
    let report = FlagReport {
        file: "test.md".to_string(),
        flags: vec![
            Flag {
                id: 1,
                line: 3,
                context: "some text".to_string(),
                label: "Comment".to_string(),
                comment: "needs rework".to_string(),
            },
            Flag {
                id: 2,
                line: 7,
                context: "other text".to_string(),
                label: "Comment".to_string(),
                comment: "contradicts intro".to_string(),
            },
        ],
    };
    let output = format_flags_text(&report);
    assert!(output.contains("Flags in test.md:"));
    assert!(output.contains("#1 (line 3): needs rework"));
    assert!(output.contains("Context: some text"));
    assert!(output.contains("#2 (line 7): contradicts intro"));
}

#[test]
fn test_format_flags_text_empty() {
    let report = FlagReport {
        file: "clean.md".to_string(),
        flags: vec![],
    };
    let output = format_flags_text(&report);
    assert!(output.contains("No flags found."));
}

// --- remove_flag ---

#[test]
fn test_remove_flag_removes_single_flag() {
    let content = "This line has <flag:1>Comment: something</flag> a flag.\n";
    let result = remove_flag(content, 1).unwrap();
    assert_eq!(result, "This line has  a flag.\n");
    assert!(extract_flags(&result).is_empty());
}

#[test]
fn test_remove_flag_preserves_other_flags() {
    let content = "Line <flag:1>Comment: first</flag> with <flag:2>Comment: second</flag> two.\n";
    let result = remove_flag(content, 1).unwrap();
    assert!(result.contains("<flag:2>"));
    assert!(!result.contains("<flag:1>"));
}

#[test]
fn test_remove_flag_not_found_returns_error() {
    let content = "No flags here.\n";
    let result = remove_flag(content, 99);
    assert!(result.is_err());
}

#[test]
fn test_remove_flag_preserves_trailing_newline() {
    let content = "Line <flag:1>Comment: test</flag> here.\n";
    let result = remove_flag(content, 1).unwrap();
    assert!(result.ends_with('\n'));
}

#[test]
fn test_remove_flag_no_trailing_newline() {
    let content = "Line <flag:1>Comment: test</flag> here.";
    let result = remove_flag(content, 1).unwrap();
    assert!(!result.ends_with('\n'));
}

// --- update_flag_comment ---

#[test]
fn test_update_flag_comment_changes_comment() {
    let content = "Line <flag:1>Comment: old comment</flag> here.\n";
    let result = update_flag_comment(content, 1, "new comment").unwrap();
    assert!(result.contains("<flag:1>Comment: new comment</flag>"));
    assert!(!result.contains("old comment"));
}

#[test]
fn test_update_flag_comment_sanitizes_input() {
    let content = "Line <flag:1>Comment: safe</flag> here.\n";
    let result = update_flag_comment(content, 1, "<script>alert(1)</script>").unwrap();
    assert!(result.contains("&lt;script&gt;"));
    assert!(!result.contains("<script>"));
}

#[test]
fn test_update_flag_comment_preserves_other_flags() {
    let content = "A <flag:1>Comment: first</flag> B <flag:2>Comment: second</flag>\n";
    let result = update_flag_comment(content, 1, "updated").unwrap();
    assert!(result.contains("<flag:1>Comment: updated</flag>"));
    assert!(result.contains("<flag:2>Comment: second</flag>"));
}

#[test]
fn test_update_flag_comment_not_found_returns_error() {
    let content = "No flags.\n";
    let result = update_flag_comment(content, 99, "anything");
    assert!(result.is_err());
}

#[test]
fn test_update_flag_comment_preserves_trailing_newline() {
    let content = "Line <flag:1>Comment: old</flag> here.\n";
    let result = update_flag_comment(content, 1, "new").unwrap();
    assert!(result.ends_with('\n'));
}

// --- inject_flag label ---

#[test]
fn test_inject_flag_with_label() {
    let content = "Hello world\nSecond line\n";
    let result = inject_flag(content, 1, "something broken", "Bug").unwrap();
    assert!(result.contains("<flag:1>Bug: something broken</flag>"));
}

#[test]
fn test_inject_flag_default_comment_label() {
    let content = "Hello world\n";
    let result = inject_flag(content, 1, "general note", "Comment").unwrap();
    assert!(result.contains("<flag:1>Comment: general note</flag>"));
}
