# View Command

The `view` command renders a markdown file directly in the terminal with ANSI colors, bold text, and inline flag highlighting. It is designed for situations where you cannot or do not want to open a browser: SSH sessions, terminal-only environments, or quick inspections.

## Basic Usage

```bash
previewf view ./plan.md
```

This reads the file, converts flag tags to a terminal-friendly format, and renders the markdown using termimad's default skin.

## Command Syntax

```
previewf view <PATH>

Arguments:
  <PATH>    Markdown file to view

Options:
  -h, --help    Print help
```

The `view` command only accepts a single file, not a directory. It reads the file synchronously (no async runtime needed for this path, though tokio is still initialized because `main` is `#[tokio::main]`).

## What the Output Looks Like

Given this markdown file:

```markdown
# Project Plan

## Phase 1: Foundation

Set up the basic project structure.
The timeline is <flag:1>Comment: Is this realistic?</flag> two weeks.

## Code Example

\```rust
fn main() {
    println!("hello");
}
\```
```

The terminal output will show:

- `# Project Plan` rendered as a bold, possibly colored heading
- `## Phase 1: Foundation` as a sub-heading
- Body text wrapped to the terminal width
- The flag rendered as `**[FLAG #1:** Is this realistic?**]**` in bold
- The code block with syntax-appropriate formatting (termimad handles basic code block rendering)

The exact appearance depends on your terminal's color support, font, and termimad's default skin configuration.

## How It Works Internally

The `view` command follows this path:

```
main.rs: Commands::View { path }
    |
    v
std::fs::read_to_string(&path)  -- read the file
    |
    v
previewf::terminal::render_terminal(&content)
    |
    +-- prepare_flags_for_terminal(&content)
    |       Converts <flag:N>Comment: text</flag>
    |       to **[FLAG #N:** text**]**
    |
    +-- MadSkin::default().term_text(&prepared)
    |       termimad renders to ANSI string
    |
    v
print!("{}", rendered)  -- output to stdout
```

### Flag Preparation

Before passing the content to termimad, flag tags are converted to a format that termimad can style:

```rust
fn prepare_flags_for_terminal(content: &str) -> String {
    let re = Regex::new(r"<flag:(\d+)>Comment:\s*(.+?)</flag>").unwrap();
    re.replace_all(content, |caps: &regex::Captures| {
        format!("**[FLAG #{}:** {}**]**", &caps[1], caps[2].trim())
    }).to_string()
}
```

The double asterisks (`**`) trigger bold rendering in termimad. The bracket format (`[FLAG #1: ...]`) makes flags visually distinct from surrounding text.

### Why Not Use syntect for Terminal Highlighting?

syntect can produce ANSI escape codes for terminal output, and we use syntect for browser-side code highlighting. However, for the `view` command, we use termimad's built-in code rendering instead. The reasons:

1. **Simplicity.** termimad handles the full markdown-to-terminal pipeline. Adding syntect on top would mean parsing markdown twice (once for termimad's layout, once for syntect's highlighting) and merging the outputs.

2. **Consistency.** termimad produces a coherent terminal rendering where headings, lists, tables, and code blocks all use compatible formatting. Injecting syntect-highlighted code into termimad's output would require careful coordination.

3. **Future improvement.** If terminal code highlighting becomes a priority, syntect integration can be added to the terminal module without changing the `view` command's interface.

## Error Handling

The `view` command can fail in two ways:

1. **File not found.** `std::fs::read_to_string` returns an error, which is wrapped with `anyhow::Context`:

   ```rust
   let content = std::fs::read_to_string(&path)
       .with_context(|| format!("Cannot read file: {}", path.display()))?;
   ```

   The user sees: `Error: Cannot read file: ./missing.md`

2. **Not a markdown file.** Currently, the `view` command does not validate the file extension. It will attempt to render any text file as markdown. This is by design -- you might want to view a `.txt` file or a file without an extension. If the content is not markdown, the output will be the raw text with minimal formatting.

## Limitations

- **Read-only.** The `view` command cannot create or modify flags. Flagging is currently only available through the browser UI. Terminal-based flag editing is on the roadmap.

- **No live reload.** The `view` command is a one-shot render. It reads the file, renders it, prints it, and exits. There is no file watching or re-rendering. If you want continuous updates, use `watch` or a similar tool:

  ```bash
  watch -n 1 previewf view ./plan.md
  ```

- **No paging.** The output is printed to stdout in one shot. For long files, pipe to a pager:

  ```bash
  previewf view ./plan.md | less -R
  ```

  The `-R` flag tells less to interpret ANSI color codes.

- **Terminal width dependency.** termimad wraps text to the terminal width. If your terminal is very narrow, the output may be hard to read. A width of 80+ columns is recommended.

## Comparison with Other Commands

| Feature | `previewf view` | `previewf serve` |
|---------|-----------------|------------------|
| Output | Terminal (ANSI) | Browser (HTML/CSS) |
| Flags | Read-only display | Full create/view/navigate |
| Live reload | No | Yes |
| Typography | Terminal defaults | Editorial (serif fonts) |
| Syntax highlighting | Basic (termimad) | Full (syntect) |
| Interactivity | None | Flag creation, sidebar navigation |
