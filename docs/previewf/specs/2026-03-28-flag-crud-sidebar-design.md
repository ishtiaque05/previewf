# Flag CRUD & Sidebar Fix — Design Spec

**Date:** 2026-03-28
**Branch:** `feature/flag-crud-sidebar` (worktree)
**Status:** Approved

---

## Problem

1. **Bug:** Flags injected into markdown files do not appear in the right sidebar ("FLAGS" pane says "No flags in this document" even when flags exist in the file).
2. **Missing feature:** No ability to delete flags.
3. **Missing feature:** No ability to edit/modify flag comments.
4. **UX issue:** Adding a flag triggers `window.location.reload()`, which loses scroll position and flashes the page.

## Design Decisions

- Flags are markdown-only (no JSON/HTML flag support).
- Delete is immediate (no confirmation dialog).
- Edit is inline in the sidebar (input field replaces comment text, Save/Cancel buttons).
- All mutations (add/edit/delete) follow the same pattern: server-side write, then partial sidebar refresh via API — no full page reloads.
- The document body updates via the existing file watcher + WebSocket live reload.
- Flag count badge updates live after every mutation.

---

## 1. Bug Fix: Sidebar Not Populating

### Symptom
`initFlagSidebar()` in `app.js` queries `.flag[data-flag-id]` in the DOM to populate the sidebar. On markdown views with flags, the sidebar shows "No flags in this document."

### Investigation Required
Trace the pipeline end-to-end:
1. Verify `inject_flag()` writes valid `<flag:N>Comment: ...</flag>` tags to the markdown file.
2. Verify `render_html()` in `markdown.rs` calls `render_flags()` which converts flag tags to `<span class="flag" data-flag-id="N">` HTML.
3. Verify the rendered HTML is placed inside `#document` in the template.
4. Verify `initFlagSidebar()` runs after the DOM is ready and queries the correct selectors.

### Fix
Patch whatever step in the pipeline is broken. The expected result: after a flag is added to a markdown file, both the inline marker and the sidebar item appear.

---

## 2. New API Endpoints

### `DELETE /flag/{id}/{*filepath}`

Remove a flag by ID from a markdown file. The ID comes before the wildcard filepath to avoid Axum's greedy wildcard consuming the ID segment.

**Route:** `DELETE /flag/{id}/{*filepath}`
**Path parameters:**
- `id` — flag ID (u32)
- `filepath` — relative path to the markdown file

**Behavior:**
1. Validate file is markdown (400 if not).
2. Resolve path via `resolve_path()` (404 if traversal or not found).
3. Acquire per-file mutex lock.
4. Read file content.
5. Call `remove_flag(content, id)` to strip the `<flag:{id}>...</flag>` tag.
6. Write modified content back to disk.
7. Return 200 OK.

**Errors:**
- 400: Not a markdown file.
- 404: File not found, or flag ID not found in file.
- 500: File write failure.

### `PUT /flag/{id}/{*filepath}`

Update a flag's comment.

**Route:** `PUT /flag/{id}/{*filepath}`
**Path parameters:**
- `id` — flag ID (u32)
- `filepath` — relative path to the markdown file

**Request body:**
```json
{
  "comment": "updated comment text"
}
```

**Behavior:**
1. Validate file is markdown (400 if not).
2. Resolve path via `resolve_path()` (404 if traversal or not found).
3. Acquire per-file mutex lock.
4. Read file content.
5. Call `update_flag_comment(content, id, new_comment)` to replace the comment (with sanitization).
6. Write modified content back to disk.
7. Return 200 OK.

**Errors:**
- 400: Not a markdown file, or empty comment.
- 404: File not found, or flag ID not found in file.
- 500: File write failure.

---

## 3. New Functions in `flags.rs`

### `remove_flag(content: &str, id: u32) -> Result<String, PreviewError>`

- Scan content for `<flag:{id}>...</flag>` using the existing `FLAG_RE` regex.
- Remove the matched tag from the line (preserve the rest of the line content).
- Return error if flag ID not found.

### `update_flag_comment(content: &str, id: u32, new_comment: &str) -> Result<String, PreviewError>`

- Scan content for `<flag:{id}>...</flag>`.
- Replace the tag with `<flag:{id}>Comment: {sanitized_comment}</flag>`.
- Use the existing `sanitize_comment()` function.
- Return error if flag ID not found.

---

## 4. Sidebar UI Changes

### Flag Item Layout

Each flag item in the sidebar gets an action row:

```
+----------------------------------+
| Flag #1                          |
| This needs attention             |
| [Edit] [Delete]                  |
+----------------------------------+
```

- **Edit button:** Small text button. Clicking replaces the comment with an inline `<input>` pre-filled with current comment text. Shows "Save" and "Cancel" buttons. Enter saves, Escape cancels.
- **Delete button:** Small text button. Clicking immediately fires DELETE request, then refreshes sidebar.

### `refreshFlagSidebar()` — New Helper Function

Central function called after every mutation (add/edit/delete):

1. Fetch `GET /flags/{filepath}` to get the current flag list as JSON.
2. Clear `#flag-list` container.
3. Rebuild flag items from the JSON response (not from DOM queries).
4. Re-bind click-to-scroll handlers for bidirectional navigation.
5. Update `#flag-count` badge with the new count.

This replaces the current pattern of querying `.flag` elements in the DOM.

### Changes to Existing Add Flow

In `submitFlag()`, replace:
```javascript
window.location.reload();
```
with:
```javascript
refreshFlagSidebar();
```

This preserves scroll position and makes add consistent with edit/delete.

---

## 5. Styling

New CSS additions in `style.css`:

- `.flag-item-actions` — action row container (flex, gap, right-aligned or left-aligned)
- `.flag-action-btn` — small button style for Edit/Delete (subtle, matches theme)
- `.flag-action-btn:hover` — accent color on hover
- `.flag-action-btn-delete:hover` — red/warning color on hover
- `.flag-edit-input` — inline edit input field styling
- `.flag-edit-actions` — Save/Cancel button row during edit mode

All styles must work in both light and dark themes using existing CSS custom properties.

---

## 6. Data Flow

All three operations follow the same pattern:

```
User action (add/edit/delete)
  -> HTTP request to server (POST/PUT/DELETE)
  -> Server modifies markdown file on disk
  -> Server returns 200
  -> JS calls refreshFlagSidebar()
     -> GET /flags/{filepath}
     -> Rebuild sidebar from JSON
     -> Update badge count
  -> File watcher detects change
  -> WebSocket sends "reload"
  -> Document body re-renders with updated inline flag markers
```

---

## 7. Files Changed

| File | Changes |
|------|---------|
| `src/flags.rs` | Add `remove_flag()`, `update_flag_comment()` + tests |
| `src/server.rs` | Add `DELETE /flag/{filepath}/{id}` and `PUT /flag/{filepath}/{id}` routes + handlers |
| `assets/app.js` | Add `refreshFlagSidebar()`, rework `initFlagSidebar()` with edit/delete buttons, replace `reload()` in `submitFlag()` |
| `assets/style.css` | Styles for action buttons, inline edit input, edit-mode state |
| `tests/flags_test.rs` | Tests for `remove_flag()` and `update_flag_comment()` |
| `tests/server_test.rs` | Integration tests for DELETE and PUT endpoints |
| Bug fix file(s) | TBD after diagnosing the sidebar population bug |

---

## 8. Out of Scope

- Flags on JSON or HTML files.
- Multi-line text selection for flag creation.
- Drag-and-drop reordering of flags.
- Flag categories or priority levels.
- Undo/redo for flag operations.
- Confirmation dialogs on delete.
