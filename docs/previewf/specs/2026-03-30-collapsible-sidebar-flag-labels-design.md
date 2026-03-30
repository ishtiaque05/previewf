# Collapsible Sidebar & Flag Labels

## Overview

Two features for the flag sidebar:
1. **Collapsible sidebar** — collapses to a narrow icon strip with flag count badge, expands on click
2. **Flag labels** — categorize flags with predefined labels (Bug, Todo, Question, Note, Style) or custom freeform labels

## Flag Syntax

The label replaces the fixed `Comment:` prefix:

```
<flag:1>Bug: off-by-one error</flag>
<flag:1>Todo: add validation</flag>
<flag:1>Question: should this be async?</flag>
<flag:1>Comment: general note</flag>         ← default
<flag:1>Perf: custom freeform label</flag>   ← custom labels
```

Existing `Comment:` flags require no migration — they parse naturally as label="Comment".

## Data Model

### Flag struct

```rust
pub struct Flag {
    pub id: u32,
    pub line: usize,
    pub context: String,
    pub label: String,
    pub comment: String,
}
```

### Regex

`FLAG_RE` changes from:
```
<flag:(\d+)>Comment:\s*(.+?)</flag>
```
to:
```
<flag:(\d+)>(\w+):\s*(.+?)</flag>
```

Group 1: ID, Group 2: label, Group 3: comment.

### Predefined Labels

| Label    | Color  | CSS Variable    |
|----------|--------|-----------------|
| Comment  | Gray   | `--label-comment`  |
| Bug      | Red    | `--label-bug`      |
| Todo     | Blue   | `--label-todo`     |
| Question | Orange | `--label-question` |
| Note     | Green  | `--label-note`     |
| Style    | Purple | `--label-style`    |
| Custom   | Pink   | `--label-custom`   |

Color mapping lives in CSS, not in markdown. Unknown labels use the custom/pink fallback.

## Collapsible Sidebar

### Behavior

- Chevron toggle in sidebar header collapses to a 36px icon strip
- Strip shows: expand chevron + flag count badge
- Click strip to expand
- Collapse state persisted in `localStorage` (key: `previewf-sidebar-collapsed`)
- Smooth CSS transition (~200ms)
- Document area flexes to fill space (already `flex: 1`)

### CSS States

```
.sidebar                → expanded (280px, current behavior)
.sidebar.collapsed      → collapsed (36px icon strip)
```

## Flag Creation Toolbar

### Label Picker

Row of clickable label pills above the comment input:

```
[Comment] [Bug] [Todo] [Question] [Note] [Style] [Custom...]
```

- "Comment" pre-selected by default
- Single-select: clicking a pill deselects the previous one
- "Custom..." replaces the pill row with a text input (Enter confirms, Escape cancels)

### POST Body

```json
{
  "comment": "description text",
  "selected_text": "the highlighted text",
  "label": "Bug"
}
```

`label` is optional — defaults to "Comment" if omitted.

## API Changes

### Modified Endpoints

- `POST /flag/{*filepath}` — accepts optional `label` field in body
- `PUT /flag/{id}/{*filepath}` — accepts optional `label` field to change the label
- `GET /flags/{*filepath}` — `Flag` JSON response now includes `label` field

### Request/Response

POST/PUT request body:
```json
{ "comment": "text", "selected_text": "...", "label": "Bug" }
```

GET response (existing shape, new field):
```json
{
  "file": "readme.md",
  "flags": [
    { "id": 1, "line": 5, "context": "...", "label": "Bug", "comment": "off-by-one" }
  ]
}
```

## Sidebar Display

### Expanded

Each flag item shows:
- Flag ID + colored label badge pill
- Comment text
- Edit / Delete buttons

Label badge uses `data-label` attribute for CSS color mapping:
```html
<span class="flag-label" data-label="bug">Bug</span>
```

### Collapsed (Icon Strip)

- 36px wide
- Expand chevron at top
- Flag count badge (orange circle with number)

### Edit Mode

Edit mode adds a label picker (same pill row as creation toolbar) so the label can be changed inline alongside the comment.

## CLI Output

```
$ previewf flags readme.md
Flags in readme.md:

  #1 [Bug] (line 5): off-by-one error
  #2 [Todo] (line 12): add validation

$ previewf flags readme.md --json
{ "file": "readme.md", "flags": [...] }
```

## Backend Functions

### Modified

- `extract_flags()` — parse label from new regex group 2
- `inject_flag()` — accept `label` parameter, use as prefix instead of "Comment"
- `update_flag_comment()` — accept optional `label` parameter to update both
- `format_flags_text()` — include label in output: `#1 [Bug] (line 5): text`
- `flag_handler` (POST) — read `label` from request body
- `update_flag_handler` (PUT) — read optional `label` from request body

### Unchanged

- `remove_flag()` — regex already captures any prefix, no change needed
- `flags_handler` (GET) — just returns the Flag structs which now include label
- `delete_flag_handler` — unaffected

## Scope Boundaries

**In scope:**
- Collapsible sidebar with localStorage persistence
- Label system with 6 predefined + custom freeform
- Label picker in creation toolbar and edit mode
- Colored badge display in sidebar
- Updated CLI output
- Updated regex and Flag struct

**Out of scope:**
- Filtering/sorting flags by label
- Label statistics/counts in sidebar header
- Custom color assignment for custom labels
- Label autocomplete from previously used custom labels
