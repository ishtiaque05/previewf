# Roadmap

This chapter lists features that are planned for future development but are explicitly out of scope for the initial release (v0.1.0). They are listed here to document intent and to prevent scope creep during initial development.

## Planned Features

### CLI Editing Mode

**What:** Create, modify, and resolve flags from the terminal without opening a browser.

**Why:** For terminal-only workflows (SSH sessions, CI pipelines), the current `view` command is read-only. Users must manually edit the file to add or remove flag tags.

**Possible implementation:**

```bash
# Add a flag interactively
previewf flag add ./plan.md --line 15 --comment "needs clarification"

# Remove a flag
previewf flag resolve ./plan.md --id 3

# Resolve all flags
previewf flag resolve-all ./plan.md
```

This would extend the CLI with a `flag` subcommand that has its own sub-subcommands. The underlying `inject_flag` and `extract_flags` functions already exist; this feature is primarily a CLI interface addition.

### HTML File Flagging

**What:** Support flagging annotations on HTML files in addition to markdown files.

**Why:** Currently, HTML files are served via the `/raw/:path` route without any annotation support. Users with mixed markdown/HTML documentation cannot flag their HTML files.

**Challenges:** HTML files are not parsed by comrak, so the flag injection strategy would be different. The flag tags would need to be inserted as HTML comments or custom data attributes. The rendering pipeline would need a separate path for HTML files.

### Multiple Flag Categories

**What:** Support different types of annotations beyond the generic `Comment:` type.

**Possible categories:**

```markdown
<flag:1>Comment: general observation</flag>
<flag:2>Todo: implement error handling here</flag>
<flag:3>Question: why was this approach chosen?</flag>
```

**Why:** Different annotations serve different purposes. A "todo" flag means "something needs to be done," a "question" flag means "I need clarification," and a "comment" flag means "here is my observation." Categorization would enable filtering and prioritization.

**Impact on format:** The regex would need to be updated to capture the category prefix. The Flag struct would gain a `category` field. The sidebar could group flags by category with different colors.

### Flag Resolution Tracking

**What:** Track whether flags have been resolved, when, and by whom.

**Possible format:**

```markdown
<flag:1 resolved="2026-04-01">Comment: this was addressed in commit abc123</flag>
```

**Why:** Currently, resolving a flag means deleting the tag entirely. There is no record that a flag existed and was addressed. Resolution tracking would maintain an audit trail.

**Trade-offs:** Adding attributes to the flag tag increases format complexity. The regex becomes more involved. The file gets cluttered with resolved flags. An alternative is to move resolved flags to a separate section at the end of the file.

### Homebrew Formula

**What:** Publish a Homebrew formula so macOS users can install with `brew install previewf`.

**Why:** Homebrew is the standard package manager for developer tools on macOS. A formula makes installation trivial and enables automatic updates.

**Prerequisites:** Stable release with reliable binary builds. The Homebrew formula needs to download the binary from a GitHub Release URL and verify its checksum.

### Custom CSS Themes

**What:** Allow users to provide custom CSS that overrides the default theme.

**Possible implementation:**

```bash
previewf serve ./docs/ --theme ./my-theme.css
```

The custom CSS would be loaded after the default CSS, allowing overrides of any CSS custom property.

**Why:** While the editorial design works well for most reading scenarios, some users may prefer different fonts, colors, or layout parameters.

### PDF Export

**What:** Export the rendered markdown as a PDF file.

```bash
previewf export ./plan.md --format pdf --output plan.pdf
```

**Why:** PDFs are useful for sharing documents with people who do not have previewf installed, for archiving, and for printing.

**Challenges:** PDF generation from HTML requires a rendering engine (headless Chrome, wkhtmltopdf, or a Rust PDF library). This adds significant complexity and binary size. May be better implemented as an external tool that captures the browser output.

## Priority and Sequencing

The features are roughly ordered by expected value and implementation complexity:

| Feature | Value | Complexity | Priority |
|---------|-------|-----------|----------|
| CLI editing mode | High (completes terminal workflow) | Low | First |
| Multiple flag categories | Medium (better organization) | Low | Second |
| Flag resolution tracking | Medium (audit trail) | Medium | Third |
| Homebrew formula | Medium (easier installation) | Low | Fourth |
| HTML file flagging | Low (niche use case) | Medium | Fifth |
| Custom CSS themes | Low (personal preference) | Low | Sixth |
| PDF export | Low (alternative tools exist) | High | Last |

## What Is NOT on the Roadmap

These features have been explicitly considered and rejected:

- **Multi-user collaboration.** previewf is a personal tool. Real-time collaboration would require a server component, user authentication, and conflict resolution. Use Google Docs or Notion for that.

- **Cloud hosting / deployment.** previewf runs on localhost. Deploying to a cloud server would require authentication, HTTPS, and security hardening. Use dedicated documentation platforms for public content.

- **WYSIWYG editing.** previewf is a previewer, not an editor. The source file is edited in your preferred text editor. The browser view is read-only (except for flag creation).

- **Plugin system.** The codebase is small enough that any extension can be made as a direct code change. A plugin system would add abstraction overhead without corresponding benefit at this scale.

- **Database backend.** Flags are stored in the source file, period. This is a deliberate constraint, not a limitation. A database would add deployment complexity and break the "flags are in the file" invariant that makes the tool simple and reliable.
