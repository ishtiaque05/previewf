# Scenario: Flagging Workflow

This walkthrough traces the complete end-to-end flagging workflow: reviewing a document, creating flags, exporting them, sending them to an LLM, and resolving the feedback. This is the primary use case previewf was built for.

## The Setup

You have written a design spec and want to review it before sharing with the team. The document is `docs/api-design.md`:

```markdown
# API Design Specification

## Authentication

All API endpoints require Bearer token authentication.
Tokens expire after 24 hours and must be refreshed via the /auth/refresh endpoint.

## Rate Limiting

Rate limits are set to 1000 requests per minute per API key.
Exceeding the limit returns a 429 status code with a Retry-After header.

## Endpoints

### GET /users

Returns a paginated list of users. Default page size is 50.
Supports filtering by role, status, and creation date.

### POST /users

Creates a new user. Requires admin role.
The request body must include email, name, and role fields.

### DELETE /users/:id

Permanently deletes a user and all associated data.
This action cannot be undone.

## Error Handling

All errors return a JSON body with `code`, `message`, and `details` fields.
Internal server errors (500) include a request ID for debugging.
```

## Phase 1: Reading and Flagging

### Start the server

```bash
previewf serve docs/api-design.md
```

Open `http://localhost:3000` in the browser. The document renders with editorial typography.

### Flag 1: Authentication concern

While reading, you notice that 24-hour token expiry might be too long for a security-sensitive API. Select "Tokens expire after 24 hours" and add the comment: "24h is too long for admin tokens. Consider 1h for admin, 24h for regular users."

What happens on disk:

```markdown
Tokens expire after 24 hours and must be refreshed via the /auth/refresh endpoint. <flag:1>Comment: 24h is too long for admin tokens. Consider 1h for admin, 24h for regular users.</flag>
```

### Flag 2: Rate limiting question

The 1000 req/min limit seems arbitrary. Select "1000 requests per minute per API key" and comment: "What's the basis for this number? Need load testing data."

### Flag 3: Pagination concern

The 50-item default page size might be too large. Select "Default page size is 50" and comment: "50 is high for mobile clients. Consider 20 with a max of 100."

### Flag 4: Destructive action warning

The DELETE endpoint permanently deletes data with no undo. Select "This action cannot be undone" and comment: "Need soft-delete instead. Permanent deletion should require a separate confirmation step."

### Flag 5: Error handling gap

The error handling section does not mention validation errors. Select "All errors return a JSON body" and comment: "What about validation errors (422)? Need to specify field-level error format."

### Current state

The document now has 5 flags. The sidebar shows all of them with their IDs and comments. The file on disk has 5 `<flag:N>` tags embedded inline.

## Phase 2: Exporting for Review

### Human-readable export

```bash
previewf flags docs/api-design.md
```

```
Flags in api-design.md:

  #1 (line 5): 24h is too long for admin tokens. Consider 1h for admin, 24h for regular users.
    Context: Tokens expire after 24 hours and must be refreshed via the /auth/refresh endpoint.

  #2 (line 9): What's the basis for this number? Need load testing data.
    Context: Rate limits are set to 1000 requests per minute per API key.

  #3 (line 14): 50 is high for mobile clients. Consider 20 with a max of 100.
    Context: Returns a paginated list of users. Default page size is 50.

  #4 (line 23): Need soft-delete instead. Permanent deletion should require a separate confirmation step.
    Context: This action cannot be undone.

  #5 (line 27): What about validation errors (422)? Need to specify field-level error format.
    Context: All errors return a JSON body with code, message, and details fields.
```

This is useful for a quick summary or for pasting into a chat message.

### JSON export for LLM

```bash
previewf flags docs/api-design.md --json > /tmp/flags.json
```

```json
{
  "file": "api-design.md",
  "flags": [
    {
      "id": 1,
      "line": 5,
      "text": "Tokens expire after 24 hours and must be refreshed via the /auth/refresh endpoint.",
      "comment": "24h is too long for admin tokens. Consider 1h for admin, 24h for regular users."
    },
    {
      "id": 2,
      "line": 9,
      "text": "Rate limits are set to 1000 requests per minute per API key.",
      "comment": "What's the basis for this number? Need load testing data."
    },
    {
      "id": 3,
      "line": 14,
      "text": "Returns a paginated list of users. Default page size is 50.",
      "comment": "50 is high for mobile clients. Consider 20 with a max of 100."
    },
    {
      "id": 4,
      "line": 23,
      "text": "This action cannot be undone.",
      "comment": "Need soft-delete instead. Permanent deletion should require a separate confirmation step."
    },
    {
      "id": 5,
      "line": 27,
      "text": "All errors return a JSON body with code, message, and details fields.",
      "comment": "What about validation errors (422)? Need to specify field-level error format."
    }
  ]
}
```

## Phase 3: LLM Review

### Prompt construction

Combine the original document with the flags and send to an LLM:

```bash
cat docs/api-design.md
echo "---"
echo "Here are the review flags for this document:"
previewf flags docs/api-design.md --json
echo "---"
echo "For each flag, analyze the concern and propose specific changes to the document text."
```

### LLM response (example)

The LLM might respond with:

> **Flag #1 (line 5): Token expiry**
> Replace "Tokens expire after 24 hours" with:
> "Standard tokens expire after 24 hours. Admin tokens expire after 1 hour. Both can be refreshed via /auth/refresh."
>
> **Flag #2 (line 9): Rate limiting basis**
> Add a section: "Rate limits are derived from our P99 load testing results (Dec 2025): peak sustained load of 800 req/min with 95th percentile response time under 200ms."
>
> **Flag #3 (line 14): Page size**
> Replace "Default page size is 50" with:
> "Default page size is 20. Maximum page size is 100, configurable via ?limit= parameter."
>
> **Flag #4 (line 23): Soft delete**
> Replace the DELETE section with:
> "Soft-deletes a user (sets status to 'deleted', data retained for 30 days). Requires admin role. For permanent deletion, use DELETE /users/:id/purge with additional confirmation header."
>
> **Flag #5 (line 27): Validation errors**
> Add: "Validation errors return 422 with a `fields` array: `[{"field": "email", "code": "invalid_format", "message": "..."}]`"

## Phase 4: Resolving Flags

After reviewing the LLM's suggestions, update the document and remove the flag tags.

### Manual resolution

Open `docs/api-design.md` in your editor. For each flag:

1. Read the LLM's suggestion
2. Decide whether to accept, modify, or reject it
3. Update the text
4. Delete the `<flag:N>Comment: ...</flag>` tag

For example, change line 5 from:

```markdown
Tokens expire after 24 hours and must be refreshed via the /auth/refresh endpoint. <flag:1>Comment: 24h is too long for admin tokens. Consider 1h for admin, 24h for regular users.</flag>
```

To:

```markdown
Standard tokens expire after 24 hours. Admin tokens expire after 1 hour. Both can be refreshed via the /auth/refresh endpoint.
```

### Verify all flags are resolved

```bash
previewf flags docs/api-design.md
```

```
Flags in api-design.md:

  No flags found.
```

The document is clean. If you are still running the server, the browser live-reloads to show the updated document with no flags and an empty sidebar.

## The Complete Cycle

```
1. WRITE        Write or receive a document
                    |
                    v
2. PREVIEW      previewf serve ./doc.md
                Browser renders with editorial typography
                    |
                    v
3. FLAG         Read carefully, select text, add comments
                Flags are written into the source file
                    |
                    v
4. EXPORT       previewf flags ./doc.md --json
                Structured data for downstream tools
                    |
                    v
5. REVIEW       Pipe JSON to LLM or share with team
                Get suggestions for each flag
                    |
                    v
6. RESOLVE      Update document, remove flag tags
                Verify: previewf flags shows "No flags found"
                    |
                    v
7. COMMIT       git add doc.md && git commit
                Clean document, flags resolved
```

## Why This Workflow Works

**Flags stay with the document.** There is no separate annotation database to synchronize. The flags are in the file, so they appear in git diffs, survive branches and merges, and are visible to anyone who opens the file.

**The export format is LLM-friendly.** JSON with clear field names (`id`, `line`, `text`, `comment`) is exactly what LLMs are good at processing. You can ask the LLM to address each flag systematically.

**Resolution is manual and intentional.** There is no "resolve all" button. You read each suggestion, decide what to do, and edit the document. This keeps the human in the loop.

**The tool gets out of the way.** previewf does not modify your workflow beyond adding the flag tags. You still use your preferred editor, your preferred version control, your preferred LLM. previewf is a thin annotation layer on top of markdown.

## Advanced: Batch Flagging Across Files

For a multi-file review:

```bash
# Export flags from all markdown files
for f in docs/*.md; do
    echo "### $(basename $f)"
    previewf flags "$f" --json
    echo ""
done
```

Or to get a total flag count:

```bash
total=0
for f in docs/*.md; do
    count=$(previewf flags "$f" --json | jq '.flags | length')
    echo "$f: $count flags"
    total=$((total + count))
done
echo "Total: $total flags"
```
