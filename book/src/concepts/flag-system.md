# The Flag System

The flag system is the central feature that distinguishes previewf from a plain markdown previewer. Flags are inline annotations embedded directly in markdown source files, designed to be both human-readable and machine-parseable. This chapter covers the flag format, the parsing and injection mechanisms, the lifecycle of a flag from creation to export, and the design rationale behind every choice.

## Flag Format

A flag is an inline HTML-like tag embedded in markdown:

```
<flag:N>Comment: description text</flag>
```

Where:

- `N` is a positive integer ID, unique within the file, auto-assigned by the tool
- `Comment:` is a literal prefix (always present)
- `description text` is free-form text describing the annotation

### Example in Context

```markdown
# Implementation Plan

## Phase 1: Foundation

Set up the basic project structure with error handling.
This phase should take approximately two weeks.

## Phase 2: Core Features

Implement the markdown rendering pipeline and flag system.
The timeline assumes a single developer. <flag:1>Comment: Is this still accurate?</flag>

## Phase 3: Polish

Consider whether the scope is <flag:2>Comment: contradicts section 1</flag> realistic.

Multiple flags <flag:3>Comment: first issue</flag> on one <flag:4>Comment: second issue</flag> line.
```

### Why This Format

The flag format was chosen to satisfy several constraints simultaneously:

**Machine-readable.** The format is regular enough to parse with a single regex: `<flag:(\d+)>Comment:\s*(.+?)</flag>`. No ambiguous boundaries, no nested structures, no context-dependent parsing.

**Greppable.** You can find all flags in a codebase with `grep -r "<flag:" .` or use ripgrep for speed. The format is distinctive enough that it will not collide with normal prose or HTML.

**Markdown-safe.** This is the subtlest constraint. When comrak parses a markdown file with `options.render.unsafe_ = true`, it passes raw HTML through to the output. The `<flag:N>` tags are treated as raw HTML and survive the markdown-to-HTML pipeline. This means:

- In the markdown source, flags are visible as inline tags
- In the rendered HTML, they appear as raw HTML elements that the post-processor converts to styled spans
- They do not interfere with markdown syntax (bold, italic, code, links)

**Human-readable.** When you open the markdown file in any text editor, the flag is immediately understandable: `<flag:1>Comment: needs work</flag>`. No binary encoding, no reference to an external database.

**VCS-friendly.** Flags are part of the file content, so they appear in diffs, are tracked by git, and survive merges (with the same caveats as any text merge).

### Alternatives Considered

We evaluated several alternative formats before settling on inline HTML-like tags:

| Format | Pros | Cons |
|--------|------|------|
| `<!-- flag:1 comment -->` | HTML comment, invisible in renders | Invisible means easy to forget; harder regex |
| `[^flag1]: comment` | Uses markdown footnote syntax | Collides with real footnotes; footnote must be at end of file |
| `{#flag1 .flag comment="..."}` | Pandoc attribute syntax | Only works in Pandoc; not standard markdown |
| `%%flag:1 comment%%` | Custom delimiter | Not valid HTML; might be stripped by parsers |
| Sidecar JSON file | Clean source files | Separate file to manage; line numbers drift on edit |
| `<flag:N>Comment: ...</flag>` | All the pros listed above | Slightly verbose; requires unsafe HTML mode |

The inline HTML approach won because it satisfies all constraints without requiring any markdown parser modification. The `unsafe_` mode in comrak is well-tested and deliberately designed for this kind of use case.

## Parsing: `extract_flags`

The `extract_flags` function in `src/flags.rs` scans markdown content and returns all flags as structured data.

### The Implementation

```rust
use regex::Regex;

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
```

### How It Works

1. **Regex compilation.** The regex `<flag:(\d+)>Comment:\s*(.+?)</flag>` captures two groups: the numeric ID and the comment text. The `\s*` after `Comment:` handles optional whitespace. The `.+?` is non-greedy to handle multiple flags on one line correctly.

2. **Line-by-line scan.** The function iterates over lines (not the whole content) because flags are line-associated. The line number (1-indexed) becomes part of the flag data.

3. **Multiple flags per line.** The `captures_iter` call handles lines with multiple flags. For the line `"A <flag:3>Comment: first</flag> B <flag:4>Comment: second</flag> C"`, it will produce two Flag structs, both with the same line number.

4. **Context extraction.** The `text` field contains the line content with all flag tags stripped, giving the surrounding prose context for the flag.

### The Flag Struct

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Flag {
    pub id: u32,
    pub line: usize,
    pub text: String,
    pub comment: String,
}
```

- `id`: Unique within the file, auto-incremented from the highest existing ID
- `line`: 1-indexed line number in the source file
- `text`: The line content with flag tags removed (the "context")
- `comment`: The annotation text

### The FlagReport Struct

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlagReport {
    pub file: String,
    pub flags: Vec<Flag>,
}
```

The report wraps a vector of flags with the filename, designed for JSON serialization and human-readable formatting.

## Injection: `inject_flag`

The `inject_flag` function adds a new flag to a specific line in the content.

```rust
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

    let next_id = next_flag_id(content);
    let flag_tag = format!(" <flag:{}>Comment: {}</flag>", next_id, comment);

    let mut result: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
    result[line - 1].push_str(&flag_tag);

    let mut output = result.join("\n");
    if content.ends_with('\n') {
        output.push('\n');
    }

    Ok(output)
}
```

### How It Works

1. **Line validation.** The function rejects line 0 (lines are 1-indexed) and lines beyond the file length. This returns a `PreviewError::FlagParse` with a descriptive message.

2. **ID assignment.** `next_flag_id` scans all existing flags and returns `max_id + 1`, or `1` if there are no flags. This guarantees unique IDs within the file.

3. **Tag appending.** The flag tag is appended to the end of the target line with a leading space. This is the simplest injection strategy: it does not try to insert the tag at a specific character position within the line.

4. **Trailing newline preservation.** If the original content ended with `\n`, the output preserves it. This prevents unnecessary diffs when the file is saved.

### The Flow from Browser to Disk

When a user selects text and creates a flag in the browser UI:

```
Browser: user selects text, types comment, clicks "Flag"
    |
    v
JavaScript: POST /flag/:filepath { comment, selected_text }
    |
    v
axum handler: reads file, finds line containing selected_text
    |
    v
inject_flag(content, line, comment) -> new content with <flag:N> tag
    |
    v
std::fs::write(path, new_content) -> file updated on disk
    |
    v
broadcast::Sender::send(()) -> notify all WebSocket subscribers
    |
    v
WebSocket -> browser receives "reload" message -> page reloads
    |
    v
Browser: re-renders page with new flag visible and highlighted
```

## ID Management: `next_flag_id`

```rust
pub fn next_flag_id(content: &str) -> u32 {
    let flags = extract_flags(content);
    flags.iter().map(|f| f.id).max().unwrap_or(0) + 1
}
```

This function finds the highest existing flag ID and returns the next one. If no flags exist, it returns 1.

**Why not UUID or timestamp-based IDs?** Simplicity. Sequential integers are easy to read, easy to reference in conversation ("look at flag 3"), and the file is the single source of truth. There is no distributed system here -- one file, one tool, one user.

**What about ID gaps?** If you delete flag 2 from a file that has flags 1, 2, 3, the next flag will be 4 (not 2). This is intentional -- reusing IDs could cause confusion in git history where an old flag 2 and a new flag 2 have different meanings.

## Formatting: `format_flags_text`

```rust
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
```

This produces human-readable output for the `previewf flags` command (without `--json`):

```
Flags in plan.md:

  #1 (line 11): Is this still accurate?
    Context: The timeline assumes a single developer working part-time.

  #2 (line 15): contradicts section 1
    Context: Consider whether the scope is realistic.
```

## Flag Rendering in HTML

When a markdown file is rendered for the browser, flags go through a two-stage transformation:

### Stage 1: comrak Parsing

comrak processes the markdown with `options.render.unsafe_ = true`. This means the `<flag:N>` tags are treated as raw HTML and passed through to the output verbatim:

```
Input markdown: "Text <flag:1>Comment: check this</flag> here."
After comrak:   "<p>Text <flag:1>Comment: check this</flag> here.</p>"
```

### Stage 2: Post-processing

The `render_flag_spans` function in `src/markdown.rs` converts flag tags to styled HTML spans:

```rust
fn render_flag_spans(html: &str) -> String {
    let re = Regex::new(r"<flag:(\d+)>Comment:\s*(.+?)</flag>").unwrap();
    re.replace_all(html, |caps: &regex::Captures| {
        let id = &caps[1];
        let comment = &caps[2];
        format!(
            r#"<span class="flag" data-flag-id="{}">
                <span class="flag-marker">#{}</span>
                <span class="flag-comment">{}</span>
            </span>"#,
            id, id, comment.trim()
        )
    }).to_string()
}
```

The output is a `<span class="flag">` with a data attribute for the ID, a visible marker (`#1`), and the comment text. CSS styles this with a warm yellow background and gold left border. JavaScript in `app.js` populates the sidebar from these spans and wires up click-to-scroll navigation.

## Flag Rendering in the Terminal

For terminal output, flags are converted to a bold bracketed format before being passed to termimad:

```rust
fn prepare_flags_for_terminal(content: &str) -> String {
    let re = Regex::new(r"<flag:(\d+)>Comment:\s*(.+?)</flag>").unwrap();
    re.replace_all(content, |caps: &regex::Captures| {
        format!("**[FLAG #{}:** {}**]**", &caps[1], caps[2].trim())
    }).to_string()
}
```

The double asterisks make the flag bold in termimad's output, and the bracketed format makes it visually distinct from surrounding text.

## The Complete Flag Lifecycle

```
1. CREATION
   User selects text in browser -> types comment -> clicks Flag
   OR: user manually types <flag:N>Comment: text</flag> in editor

2. STORAGE
   Flag tag is injected into the markdown source file at the target line
   File is saved to disk

3. DETECTION
   On next render (web or terminal), extract_flags scans the content
   Regex matches all <flag:N> patterns and produces Flag structs

4. WEB RENDERING
   comrak passes tags through as raw HTML (unsafe_ mode)
   render_flag_spans converts to styled <span> elements
   JavaScript populates sidebar, wires up navigation

5. TERMINAL RENDERING
   prepare_flags_for_terminal converts to **[FLAG #N: text]** format
   termimad renders with ANSI bold/color

6. EXPORT
   previewf flags --json serializes FlagReport to JSON
   previewf flags serializes to human-readable text
   JSON output is designed for piping to LLMs

7. RESOLUTION
   User manually removes the <flag:N>...</flag> tag from the source
   (Future: UI button to resolve/remove flags)
```

## Edge Cases

### Multiple Flags on One Line

Handled correctly by `captures_iter`. Each match produces a separate Flag struct with the same line number.

### Nested or Malformed Tags

The regex is non-greedy (`.+?`), so `<flag:1>Comment: a</flag> text <flag:2>Comment: b</flag>` parses as two separate flags, not one flag containing the other.

Malformed tags like `<flag:1>Missing comment prefix</flag>` or `<flag:abc>Comment: non-numeric ID</flag>` are silently ignored by the regex. They remain in the file as inert text.

### Empty Files

`extract_flags("")` returns an empty vector. `next_flag_id("")` returns 1.

### Flags in Code Blocks

If a flag tag appears inside a fenced code block, comrak will escape it as HTML entities (`&lt;flag:1&gt;`), so the regex will not match it in the rendered output. However, `extract_flags` operates on the raw source content (not rendered HTML), so it will still find the flag. This is an edge case to be aware of: a flag inside a code block will appear in `previewf flags` output but will not be highlighted in the browser view.
