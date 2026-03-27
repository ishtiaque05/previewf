# Quick Start

This guide gets you from zero to previewing and flagging markdown in under five minutes.

## Step 1: Create a Sample Document

Create a file called `plan.md` with some content:

```markdown
# Project Plan

## Phase 1: Foundation

Set up the basic project structure with error handling and CLI scaffolding.
This phase should take approximately two weeks.

## Phase 2: Core Features

Implement the markdown rendering pipeline and flag system.
The timeline assumes a single developer working part-time.

## Phase 3: Polish

Add live reload, theme support, and terminal rendering.
Consider whether the scope is realistic given the timeline.

## Dependencies

- tokio for async runtime
- axum for web serving
- comrak for markdown parsing
```

## Step 2: Serve It in the Browser

```bash
previewf serve ./plan.md
```

Output:

```
Serving ./plan.md on http://localhost:3000
```

Open `http://localhost:3000` in your browser. You will see the document rendered with:

- **Playfair Display** headings
- **Source Serif 4** body text at a comfortable 72-character line length
- **JetBrains Mono** for any code blocks
- A dark/light theme toggle in the top-right corner

## Step 3: Flag Some Content

In the browser, select the text "The timeline assumes a single developer working part-time." A floating toolbar appears below your selection. Type a comment like "Is this still accurate?" and click **Flag**.

The tool writes a flag tag directly into your `plan.md` file:

```markdown
The timeline assumes a single developer working part-time. <flag:1>Comment: Is this still accurate?</flag>
```

The page reloads automatically (via WebSocket live reload), and the flagged text is now highlighted with a warm underline. The flag sidebar on the right shows:

```
FLAGS
#1 line 11
"Is this still accurate?"
```

Add another flag: select "Consider whether the scope is realistic given the timeline." and comment "This contradicts Phase 1 estimate." Now your file has two flags.

## Step 4: Export Flags

Back in the terminal, extract your flags:

```bash
previewf flags ./plan.md
```

Output:

```
Flags in plan.md:

  #1 (line 11): Is this still accurate?
    Context: The timeline assumes a single developer working part-time.

  #2 (line 15): This contradicts Phase 1 estimate.
    Context: Consider whether the scope is realistic given the timeline.
```

For machine-readable output, use the `--json` flag:

```bash
previewf flags ./plan.md --json
```

```json
{
  "file": "plan.md",
  "flags": [
    {
      "id": 1,
      "line": 11,
      "text": "The timeline assumes a single developer working part-time.",
      "comment": "Is this still accurate?"
    },
    {
      "id": 2,
      "line": 15,
      "text": "Consider whether the scope is realistic given the timeline.",
      "comment": "This contradicts Phase 1 estimate."
    }
  ]
}
```

## Step 5: Pipe to an LLM

The JSON output is designed for piping to LLMs. For example:

```bash
previewf flags ./plan.md --json | pbcopy
# Then paste into Claude, ChatGPT, etc. with a prompt like:
# "Review each flag and suggest how to address it."
```

Or with a CLI tool:

```bash
previewf flags ./plan.md --json | \
  jq -r '.flags[] | "Flag #\(.id) on line \(.line): \(.comment)\nContext: \(.text)\n"'
```

## Step 6: View in the Terminal

If you prefer to stay in the terminal:

```bash
previewf view ./plan.md
```

This renders the markdown with ANSI colors, bold headings, and flag annotations displayed as `[FLAG #1: Is this still accurate?]` in a distinct color.

## Step 7: Serve a Directory

If you have a directory full of markdown and HTML files:

```bash
previewf serve ./docs/
```

This shows a file listing at `http://localhost:3000` with:

- Markdown files (clickable, opens in the annotated viewer)
- HTML files (clickable, opens as raw preview)
- Flag counts next to each markdown file

## What Just Happened

In this quickstart, you:

1. Served a markdown file with `previewf serve` and got a typeset browser view
2. Flagged content via the browser UI, which wrote `<flag:N>` tags into the source file
3. Extracted flags as both human-readable text and JSON with `previewf flags`
4. Viewed the file in the terminal with `previewf view`
5. Served a directory and browsed its contents

The key insight is that **flags live in the source file**. There is no database, no sidecar file, no proprietary format. The `<flag:N>Comment: ...</flag>` tags are inline in the markdown, which means they survive version control, grep, and any text processing tool. When comrak parses the markdown with `unsafe_` mode enabled, it passes these tags through as raw HTML, so they do not break rendering.

For deeper coverage of each feature, continue to the [Usage](../usage/serve.md) section. For understanding the architecture behind all of this, see the [Architecture](../architecture/overview.md) section.
