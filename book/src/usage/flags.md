# Flags Command

The `flags` command extracts all flag annotations from a markdown file and outputs them as either human-readable text or structured JSON. It is the bridge between previewf's annotation system and external tools, especially LLMs.

## Basic Usage

```bash
# Human-readable output
previewf flags ./plan.md

# JSON output (for piping to tools)
previewf flags ./plan.md --json
```

## Command Syntax

```
previewf flags <PATH> [OPTIONS]

Arguments:
  <PATH>    Markdown file to extract flags from

Options:
      --json    Output as JSON
  -h, --help    Print help
```

## Human-Readable Output

Without the `--json` flag, the output is formatted for human consumption:

```bash
previewf flags ./plan.md
```

```
Flags in plan.md:

  #1 (line 11): Is this still accurate?
    Context: The timeline assumes a single developer working part-time.

  #2 (line 15): contradicts section 1
    Context: Consider whether the scope is realistic given the timeline.

  #3 (line 20): first issue
    Context: Multiple flags on one line.

  #4 (line 20): second issue
    Context: Multiple flags on one line.
```

Each flag shows:

- **ID**: The numeric identifier (`#1`)
- **Line number**: Where the flag appears in the source file
- **Comment**: The annotation text
- **Context**: The line content with flag tags stripped

If the file has no flags:

```
Flags in plan.md:

  No flags found.
```

## JSON Output

With `--json`, the output is structured JSON designed for machine consumption:

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
      "comment": "contradicts section 1"
    }
  ]
}
```

### JSON Schema

The `FlagReport` structure serializes to:

| Field | Type | Description |
|-------|------|-------------|
| `file` | `string` | The filename (relative path as provided) |
| `flags` | `array` | Array of flag objects |

Each flag object:

| Field | Type | Description |
|-------|------|-------------|
| `id` | `number` | Unique flag ID within the file |
| `line` | `number` | 1-indexed line number in the source |
| `text` | `string` | Line content with flag tags removed |
| `comment` | `string` | The annotation comment |

## Piping to LLMs

The JSON output is specifically designed for LLM consumption. Here are practical workflows:

### Direct Copy-Paste

```bash
previewf flags ./plan.md --json | pbcopy
```

Then paste into an LLM conversation with a prompt like:

> Here are the flags from my implementation plan. For each flag, explain the issue and suggest a concrete fix:
>
> [paste JSON]

### With jq for Formatting

```bash
previewf flags ./plan.md --json | \
  jq -r '.flags[] | "Flag #\(.id) on line \(.line):\n  Comment: \(.comment)\n  Context: \(.text)\n"'
```

Output:

```
Flag #1 on line 11:
  Comment: Is this still accurate?
  Context: The timeline assumes a single developer working part-time.

Flag #2 on line 15:
  Comment: contradicts section 1
  Context: Consider whether the scope is realistic given the timeline.
```

### Count Flags

```bash
previewf flags ./plan.md --json | jq '.flags | length'
```

### Filter Flags by Keyword

```bash
previewf flags ./plan.md --json | jq '.flags[] | select(.comment | contains("section"))'
```

### Batch Processing

Process all markdown files in a directory:

```bash
for f in docs/*.md; do
  echo "=== $f ==="
  previewf flags "$f" --json | jq '.flags | length'
done
```

## How It Works Internally

The `flags` command follows this path:

```
main.rs: Commands::Flags { path, json }
    |
    v
std::fs::read_to_string(&path)  -- read the file
    |
    v
extract_flags(&content)  -- regex scan for <flag:N> tags
    |
    v
FlagReport { file: path, flags }  -- wrap in report struct
    |
    +-- if json:
    |       serde_json::to_string_pretty(&report)  -- serialize
    |       println!("{}", json_str)
    |
    +-- else:
            format_flags_text(&report)  -- human format
            print!("{}", text)
```

The entire operation is synchronous file I/O plus regex matching. It does not start a web server, file watcher, or any async machinery.

## Error Handling

- **File not found**: Returns an error with the file path:
  ```
  Error: Cannot read file: ./missing.md
  ```

- **Empty file**: Returns a report with zero flags (not an error).

- **File with no flags**: Returns a report with an empty flags array (JSON) or "No flags found." (text).

- **Binary file or non-text**: `std::fs::read_to_string` will fail with a UTF-8 decoding error. The error message will indicate the file path.

## Comparison with Server Flag Endpoint

The `flags` command and the `/flags/:path` server endpoint produce the same output:

```bash
# These produce identical JSON:
previewf flags ./plan.md --json
curl http://localhost:3000/flags/plan.md
```

The difference is that the `flags` command works without a running server. It reads the file directly from disk. The server endpoint reads the file relative to the served directory.

## Design Philosophy

The `flags` command embodies the Unix philosophy: do one thing (extract flags), produce structured output (JSON), and compose with other tools (jq, pbcopy, LLMs). It is deliberately simple -- no filtering options, no sorting, no aggregation. Those operations belong in downstream tools like jq.

The human-readable format exists because not every invocation needs to be piped. Sometimes you just want to see "what flags are in this file?" at a glance. The `--json` flag switches between the two modes.
