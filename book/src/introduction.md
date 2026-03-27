# Introduction

**previewf** is a personal developer tool for previewing and annotating markdown files. It is a single Rust binary that serves markdown and HTML files on localhost with rich typography, supports inline flagging of content for LLM-driven plan review, provides terminal-based markdown viewing, and exports flags as structured JSON.

This book is the comprehensive reference for the project. It covers everything from installation and daily usage to the architectural decisions that shaped the codebase, the data flow through every subsystem, and the testing strategy that keeps it reliable.

## What previewf Does

At its core, previewf solves a specific workflow problem: when you are reviewing a markdown document -- a design spec, an implementation plan, meeting notes -- you want to:

1. **Read it comfortably.** Not in a code editor with monospace font at 80 columns, but in a beautifully typeset browser view with serif fonts, proper heading hierarchy, and syntax-highlighted code blocks.

2. **Annotate it inline.** Select a passage, leave a comment, and have that annotation persisted directly in the source file as a machine-readable tag. No separate annotation database, no proprietary format -- just a tag in the markdown that any tool can grep for.

3. **Export annotations for LLMs.** Pipe your flags to an LLM as structured JSON and ask it to address each one. The flag format is designed to be both human-readable in the source file and machine-parseable for automation.

4. **Preview in the terminal.** When you are SSH'd into a server or working in a terminal-only environment, render the markdown with color, bold, and inline flag highlights right in your shell.

## The Tool in Action

```bash
# Serve a directory of markdown files with live reload
previewf serve ./docs/

# Serve a single file on a custom port
previewf serve ./plan.md --port 8080

# View a markdown file in the terminal
previewf view ./plan.md

# Extract all flags as human-readable text
previewf flags ./plan.md

# Extract all flags as JSON (pipe to jq, LLMs, etc.)
previewf flags ./plan.md --json
```

When you run `previewf serve ./docs/`, you get a browser-based reading experience:

```
+----------------------------------------------------------+
|  previewf    ~/docs/plan.md              [theme] [3]     |  <- top bar
+-------------------------------+--------------------------+
|                               |                          |
|   # Implementation Plan       |  FLAGS                   |
|                               |                          |
|   Body text in Source Serif,  |  #1 line 14              |
|   max-width 72ch for optimal  |  "need to rethink..."    |
|   reading line length.        |                          |
|                               |  #2 line 28              |
|   Flagged text highlighted    |  "contradicts S3"        |
|   with warm underline +       |                          |
|   subtle left-border glow.    |  + Add flag              |
|                               |                          |
+-------------------------------+--------------------------+
|  previewf v0.1.0 . watching . connected                  |  <- status bar
+----------------------------------------------------------+
```

The flag sidebar shows all annotations. Clicking a sidebar item scrolls to the flag in the document. Selecting text in the document opens a floating toolbar to create a new flag. When the source file changes on disk, the browser reloads automatically via WebSocket.

## Who This Is For

previewf is a personal developer tool. It is built for one person's workflow but designed well enough that others can use it, extend it, or learn from it.

It also serves as a **Rust learning project**. The codebase intentionally exercises a range of Rust patterns: async with tokio, web serving with axum, AST manipulation with comrak, the builder pattern, custom error types with thiserror, and test-driven development with nextest and insta.

## How This Book Is Organized

The book is structured in layers of increasing depth:

- **Getting Started** covers installation and a five-minute quickstart. Enough to be productive immediately.

- **Concepts** explains the flag system in detail and the design decisions behind the project -- why Rust, why each crate, why certain architectural trade-offs.

- **Usage** is the reference manual for each subcommand: `serve`, `view`, `flags`, and the theme/typography system.

- **Architecture** is the deep dive. Module responsibilities, entry points, code flow with traced examples, error handling strategy, and the testing approach.

- **Scenarios** walks through complete end-to-end workflows: serving a directory, flagging content, terminal preview, and live reload. Each scenario traces the full path from user action to system response.

- **Development** covers contributing guidelines and the CI/CD pipeline.

- **Roadmap** lists planned future features.

## A Note on the Codebase

This documentation describes the project as designed and planned. It references code structures, module boundaries, and function signatures that follow the implementation plan. The design spec and implementation plan are the authoritative sources; this book is the narrative companion that explains the "why" behind each decision and the "how" of each subsystem.
