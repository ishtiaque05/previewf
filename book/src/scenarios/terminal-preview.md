# Scenario: Terminal Preview

This walkthrough traces what happens when you use `previewf view` to render a markdown file directly in the terminal, covering the rendering pipeline, flag display, and practical usage patterns.

## The Setup

You are SSH'd into a remote server and need to review a flagged markdown file. No browser available. The file is `plan.md`:

```markdown
# Deployment Plan

## Prerequisites

- Docker 24+ installed
- Kubernetes cluster accessible
- Helm 3.12+ installed

## Step 1: Build Images

Build the application image:

\```bash
docker build -t app:v1.2.0 .
docker push registry.example.com/app:v1.2.0
\```

## Step 2: Database Migration

Run the migration <flag:1>Comment: test this on staging first</flag> before deploying:

\```sql
ALTER TABLE users ADD COLUMN last_login TIMESTAMP;
CREATE INDEX idx_users_last_login ON users(last_login);
\```

## Step 3: Deploy

Apply the Helm chart:

\```bash
helm upgrade --install app ./charts/app \
  --set image.tag=v1.2.0 \
  --set replicas=3
\```

## Rollback

If issues arise, rollback <flag:2>Comment: need to define "issues" more precisely</flag> immediately:

\```bash
helm rollback app 1
\```
```

## Step 1: Run the Command

```bash
previewf view ./plan.md
```

## Step 2: What Happens Inside

### CLI parsing

clap matches `Commands::View { path: "./plan.md" }`.

### File reading

```rust
let content = std::fs::read_to_string(&path)
    .with_context(|| format!("Cannot read file: {}", path.display()))?;
```

The entire file is read into a `String`. This is synchronous and fast (the file is a few KB).

### Flag preparation

Before termimad processes the markdown, flag tags are converted to a terminal-friendly format:

```rust
let prepared = prepare_flags_for_terminal(&content);
```

The regex `<flag:(\d+)>Comment:\s*(.+?)</flag>` matches two flags:

| Original | Replacement |
|----------|-------------|
| `<flag:1>Comment: test this on staging first</flag>` | `**[FLAG #1:** test this on staging first**]**` |
| `<flag:2>Comment: need to define "issues" more precisely</flag>` | `**[FLAG #2:** need to define "issues" more precisely**]**` |

After preparation, the relevant lines look like:

```
Run the migration **[FLAG #1:** test this on staging first**]** before deploying:
```

```
If issues arise, rollback **[FLAG #2:** need to define "issues" more precisely**]** immediately:
```

### termimad rendering

```rust
let skin = MadSkin::default();
skin.term_text(&prepared).to_string()
```

termimad processes the prepared markdown:

1. **Headings** are rendered in bold, possibly with color (depending on terminal capabilities)
2. **Lists** are rendered with bullet characters
3. **Code blocks** are rendered with a distinct background or indentation
4. **Bold text** (the `**...**` around flags) is rendered with ANSI bold attribute
5. **Text wrapping** respects the terminal width

### Output

```rust
print!("{}", rendered);
```

The ANSI-formatted string is printed to stdout. The terminal interprets the escape codes.

## Step 3: What the User Sees

The terminal displays something like (exact formatting depends on terminal capabilities):

```
 Deployment Plan
 ================

 Prerequisites
 --------------
 * Docker 24+ installed
 * Kubernetes cluster accessible
 * Helm 3.12+ installed

 Step 1: Build Images
 ---------------------
 Build the application image:

 ┌─────────────────────────────────────────────┐
 │ docker build -t app:v1.2.0 .                │
 │ docker push registry.example.com/app:v1.2.0 │
 └─────────────────────────────────────────────┘

 Step 2: Database Migration
 ---------------------------
 Run the migration [FLAG #1: test this on staging first] before deploying:

 ┌────────────────────────────────────────────────────────────┐
 │ ALTER TABLE users ADD COLUMN last_login TIMESTAMP;         │
 │ CREATE INDEX idx_users_last_login ON users(last_login);    │
 └────────────────────────────────────────────────────────────┘

 Step 3: Deploy
 ---------------
 Apply the Helm chart:

 ┌─────────────────────────────────────────────┐
 │ helm upgrade --install app ./charts/app \   │
 │   --set image.tag=v1.2.0 \                 │
 │   --set replicas=3                          │
 └─────────────────────────────────────────────┘

 Rollback
 ---------
 If issues arise, rollback [FLAG #2: need to define "issues" more
 precisely] immediately:

 ┌───────────────────────┐
 │ helm rollback app 1   │
 └───────────────────────┘
```

The flags appear as bold bracketed text: `[FLAG #1: test this on staging first]`. They are visually distinct from the surrounding prose because of the bold rendering and the `[FLAG #N:]` prefix.

## Practical Usage Patterns

### Piping to a Pager

For long documents, pipe to `less` with ANSI color support:

```bash
previewf view ./long-document.md | less -R
```

The `-R` flag tells `less` to interpret ANSI escape sequences, preserving colors and bold text. You can scroll through the document with standard `less` keybindings.

### Watching for Changes

While `previewf view` does not have built-in live reload, you can combine it with `watch`:

```bash
watch -n 2 previewf view ./plan.md
```

This re-renders the file every 2 seconds. If someone edits the file, the terminal display updates. Note that `watch` clears the screen on each refresh, so you lose your scroll position.

For a more sophisticated approach on macOS:

```bash
fswatch -o ./plan.md | xargs -n1 -I{} previewf view ./plan.md
```

This uses `fswatch` to trigger a re-render only when the file changes.

### Quick Flag Check

Sometimes you just want to see if a file has flags without opening a browser:

```bash
previewf view ./plan.md | grep "FLAG #"
```

Output:

```
 Run the migration [FLAG #1: test this on staging first] before deploying:
 If issues arise, rollback [FLAG #2: need to define "issues" more
```

### Comparing Two Files

```bash
diff <(previewf view ./plan-v1.md) <(previewf view ./plan-v2.md)
```

This compares the rendered terminal output of two documents. Note that the ANSI escape codes are included in the diff, which can make it noisy. For a cleaner diff, strip ANSI codes:

```bash
diff <(previewf view ./plan-v1.md | sed 's/\x1b\[[0-9;]*m//g') \
     <(previewf view ./plan-v2.md | sed 's/\x1b\[[0-9;]*m//g')
```

## Limitations and Workarounds

### No flag creation

The `view` command is read-only. To create flags, you need either the browser UI (`previewf serve`) or manual editing (add `<flag:N>Comment: text</flag>` tags by hand).

For manual flag creation in the terminal:

```bash
# Find the next flag ID
previewf flags ./plan.md --json | jq '.flags | map(.id) | max + 1'
# Output: 3

# Edit the file and add: <flag:3>Comment: your comment</flag>
vi ./plan.md
```

### No syntax highlighting in code blocks

termimad provides basic code block rendering (boxed/indented) but not language-aware syntax highlighting. The code appears as monospace text without color-differentiated keywords and strings.

This is a deliberate trade-off: integrating syntect's terminal output into termimad's layout would require significant complexity for marginal benefit in a read-only terminal view.

### Terminal width dependency

termimad wraps text to the terminal width. If your terminal is narrow (under 60 columns), the output may be cramped. You can set a wider terminal or redirect to a file:

```bash
previewf view ./plan.md > rendered.txt
```

The output file will contain ANSI escape codes. To view it later:

```bash
cat rendered.txt  # in a terminal that supports ANSI
```

Or to strip ANSI codes for a plain-text version:

```bash
previewf view ./plan.md | sed 's/\x1b\[[0-9;]*m//g' > plain.txt
```

## When to Use Terminal Preview vs Browser Preview

| Situation | Recommended |
|-----------|-------------|
| Quick file inspection | `previewf view` |
| Detailed review with flagging | `previewf serve` |
| Remote/SSH session | `previewf view` |
| Sharing with someone on the network | `previewf serve` |
| Checking flag status | `previewf view` (or `previewf flags`) |
| Reading a long document | `previewf serve` (better typography) |
| CI/CD output | `previewf view` (no browser needed) |
