# Scenario: Serving a Single File

This walkthrough traces what happens when you run `previewf serve ./plan.md` on a single markdown file rather than a directory.

## Setup

You have a single file:

```markdown
# Implementation Plan

## Phase 1: Foundation

Set up the project structure with error handling and CLI scaffolding.
This phase should take approximately two weeks.

## Phase 2: Core Features

Implement the markdown rendering pipeline and flag system.
The timeline assumes a single developer working part-time.
```

No flags yet.

## Step 1: Command Execution

```bash
previewf serve ./plan.md
```

clap parses this into:

```rust
Commands::Serve {
    path: PathBuf::from("./plan.md"),
    port: 3000,
}
```

The `ServerBuilder` constructs a config with this path. Note that the path is a file, not a directory.

## Step 2: Server Startup

The server starts identically to directory mode: bind to port 3000, create the router, print the startup message:

```
Serving ./plan.md on http://localhost:3000
```

## Step 3: Browser Opens the Root URL

The user navigates to `http://localhost:3000/`.

### The Redirect

In `index_handler`, the first thing checked is whether the base path is a file:

```rust
if base_path.is_file() {
    let filename = base_path.file_name().unwrap_or_default().to_string_lossy();
    return axum::response::Redirect::to(&format!("/view/{}", filename)).into_response();
}
```

Since `./plan.md` is a file:
1. Extract filename: `"plan.md"`
2. Redirect to: `/view/plan.md`
3. Return HTTP 303 See Other (axum's default redirect)

The browser follows the redirect and requests `GET /view/plan.md`.

### Why a Redirect Instead of Direct Rendering

The redirect exists so that all markdown viewing goes through the `/view/:path` route. This keeps the routing consistent: the same handler, the same template, the same JavaScript. If we rendered the file directly at `/`, the URL in the browser would be `/` instead of `/view/plan.md`, which would break relative paths and create an inconsistency in the flag creation flow (which POSTs to `/flag/:path`).

## Step 4: Document Rendering

The `view_handler` receives `filepath = "plan.md"`.

### Path resolution in single-file mode

```rust
fn resolve_path(base: &Path, relative: &str) -> PathBuf {
    if base.is_file() {
        base.to_path_buf()    // Returns "./plan.md" regardless of `relative`
    } else {
        base.join(relative)
    }
}
```

In single-file mode, `resolve_path` ignores the `relative` parameter and returns the base path directly. This means `/view/plan.md` and `/view/anything.md` would both resolve to `./plan.md`. This is a simplification that works because in single-file mode, there is only one file to serve.

### Rendering pipeline

The rendering proceeds identically to directory mode:

1. Read `./plan.md`
2. comrak parses to HTML (no flags yet, so no flag tags to process)
3. syntect processes code blocks (none in our example)
4. Flag span rendering (nothing to convert)
5. Template substitution

### Browser display

The user sees the plan rendered with:
- "Implementation Plan" as a large heading in Playfair Display
- "Phase 1: Foundation" and "Phase 2: Core Features" as sub-headings
- Body text in Source Serif 4
- An empty flag sidebar (collapsed because there are no flags)
- The status bar showing "connected"

## Step 5: Creating the First Flag

The user reads the plan and notices that "two weeks" for Phase 1 seems ambitious. They select the text "This phase should take approximately two weeks." in the browser.

### Text selection

The JavaScript `mouseup` handler detects the selection:

```javascript
documentEl.addEventListener('mouseup', function() {
    var selection = window.getSelection();
    var text = selection.toString().trim();
    // text = "This phase should take approximately two weeks."

    if (text.length > 0) {
        var rect = selection.getRangeAt(0).getBoundingClientRect();
        showToolbar(rect.left + window.scrollX, rect.bottom + window.scrollY + 8);
    }
});
```

A floating toolbar appears below the selected text with a comment input field and a "Flag" button.

### Flag submission

The user types "Is this realistic for one person?" and clicks Flag:

```javascript
fetch('/flag/' + encodeURIComponent('plan.md'), {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
        comment: 'Is this realistic for one person?',
        selected_text: 'This phase should take approximately two weeks.'
    }),
});
```

### Server processing

The `flag_post_handler` receives the request:

1. **Read file:** `std::fs::read_to_string("./plan.md")`

2. **Find line:** Search for "This phase should take approximately two weeks." -- found on line 6.

3. **Inject flag:**
   ```rust
   inject_flag(&content, 6, "Is this realistic for one person?")
   ```

   `next_flag_id` finds no existing flags, returns 1.

   Line 6 changes from:
   ```
   This phase should take approximately two weeks.
   ```
   To:
   ```
   This phase should take approximately two weeks. <flag:1>Comment: Is this realistic for one person?</flag>
   ```

4. **Write file:** `std::fs::write("./plan.md", new_content)`

5. **Broadcast:** `state.reload_tx.send(())`

### Page reload

The WebSocket client receives "reload" and calls `location.reload()`. The page re-renders with:

- The flagged text highlighted with a warm yellow background
- Flag sidebar now showing: `#1 line 6 "Is this realistic for one person?"`
- Flag count badge in the top bar showing `1`

## Step 6: Creating a Second Flag

The user selects "The timeline assumes a single developer working part-time." on line 11 and comments "This contradicts the two-week estimate."

### Server processing

1. `next_flag_id` finds flag 1, returns 2
2. Line 11 gets: `<flag:2>Comment: This contradicts the two-week estimate.</flag>`
3. File is written, broadcast sent, page reloads

### Updated file on disk

```markdown
# Implementation Plan

## Phase 1: Foundation

Set up the project structure with error handling and CLI scaffolding.
This phase should take approximately two weeks. <flag:1>Comment: Is this realistic for one person?</flag>

## Phase 2: Core Features

Implement the markdown rendering pipeline and flag system.
The timeline assumes a single developer working part-time. <flag:2>Comment: This contradicts the two-week estimate.</flag>
```

### Updated browser display

The sidebar now shows both flags. Clicking flag #1 in the sidebar scrolls to line 6 and highlights it. Clicking flag #2 scrolls to line 11.

## Step 7: Exporting Flags

Without stopping the server, in another terminal:

```bash
previewf flags ./plan.md --json
```

Output:

```json
{
  "file": "plan.md",
  "flags": [
    {
      "id": 1,
      "line": 6,
      "text": "This phase should take approximately two weeks.",
      "comment": "Is this realistic for one person?"
    },
    {
      "id": 2,
      "line": 11,
      "text": "The timeline assumes a single developer working part-time.",
      "comment": "This contradicts the two-week estimate."
    }
  ]
}
```

This reads the same file on disk. Both the running server and the `flags` command see the same data because the file is the single source of truth.

## Differences from Directory Mode

| Aspect | Directory Mode | Single File Mode |
|--------|---------------|-----------------|
| Root URL (`/`) | File listing | Redirect to `/view/<filename>` |
| Path resolution | `base_dir.join(relative)` | Always returns the file path |
| Available files | All `.md` and `.html` in directory | Only the one file |
| File watcher scope | Recursive on directory | Single file |
| Use case | Browsing a collection | Focused review of one document |

## When to Use Single File Mode

Single file mode is best when you:

- Want to review one specific document
- Are working on a plan or spec and want quick flag-and-review cycles
- Do not need the directory listing overhead
- Want to share a URL with someone on the same network (they go to `http://your-ip:3000/` and see the document directly)
