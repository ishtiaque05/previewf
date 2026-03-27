# Themes and Typography

previewf's browser interface is designed with an editorial aesthetic: warm colors, serif typography, and a reading-optimized layout. This chapter covers the theme system, the typography choices, and the technical implementation.

## Theme System

previewf ships with two themes: **Warm Paper** (light) and **Midnight Ink** (dark).

### Warm Paper (Light Theme)

The light theme uses a warm cream background instead of harsh white, and a soft black for text instead of pure `#000000`. This reduces eye strain during extended reading sessions.

```
Background:         #FAF8F5  (warm cream)
Surface:            #FFFFFF  (white cards/panels)
Text:               #2D2D2D  (soft black)
Muted text:         #6B6B6B  (gray)
Accent:             #C45B28  (burnt orange)
Flag background:    #FFF3CD  (warm yellow)
Flag border:        #E6A817  (gold)
Code background:    #F5F2EF  (warm off-white)
Links:              #1A6B4F  (forest green)
Sidebar:            #F0EDE8  (warm gray)
```

### Midnight Ink (Dark Theme)

The dark theme uses a deep navy rather than pure black. Navy dark themes are softer on the eyes and create a sense of depth.

```
Background:         #1A1A2E  (deep navy)
Surface:            #16213E  (darker navy)
Text:               #E8E6E3  (warm white)
Muted text:         #8B8FA3  (muted lavender)
Accent:             #E8845A  (coral)
Flag background:    #3D2E1F  (dark amber)
Flag border:        #D4952B  (gold)
Code background:    #0F0F23  (near-black navy)
Links:              #4ECDC4  (teal)
Sidebar:            #12122A  (deep navy)
```

### Theme Detection and Switching

On page load, the theme is determined in this order:

1. **localStorage**: If the user has previously toggled the theme, that preference is stored in `localStorage` under the key `previewf-theme` and is used.
2. **System preference**: If no stored preference exists, `window.matchMedia('(prefers-color-scheme: dark)')` is checked.
3. **Default**: If the media query is not supported, the light theme is used.

The theme toggle button in the top bar switches between light and dark, stores the choice in `localStorage`, and applies it immediately by setting `data-theme` on the `<html>` element.

### CSS Implementation

All theme colors are defined as CSS custom properties on the `[data-theme]` selector:

```css
[data-theme="light"] {
    --bg: #FAF8F5;
    --bg-surface: #FFFFFF;
    --text: #2D2D2D;
    --text-muted: #6B6B6B;
    --accent: #C45B28;
    --flag-bg: #FFF3CD;
    --flag-border: #E6A817;
    --code-bg: #F5F2EF;
    --link: #1A6B4F;
    --sidebar-bg: #F0EDE8;
}

[data-theme="dark"] {
    --bg: #1A1A2E;
    --bg-surface: #16213E;
    --text: #E8E6E3;
    --text-muted: #8B8FA3;
    --accent: #E8845A;
    --flag-bg: #3D2E1F;
    --flag-border: #D4952B;
    --code-bg: #0F0F23;
    --link: #4ECDC4;
    --sidebar-bg: #12122A;
}
```

All styling rules reference these variables (`color: var(--text)`, `background: var(--bg)`), so a theme switch is just a CSS variable change with no JavaScript DOM manipulation beyond setting the `data-theme` attribute.

Theme transitions are animated with `300ms` ease on all color properties, preventing a jarring flash when switching.

## Typography

### Font Stack

| Role | Primary Font | Fallback | Source |
|------|-------------|----------|--------|
| Headings | Playfair Display | Georgia, serif | Google Fonts |
| Body text | Source Serif 4 | Charter, Bitstream Charter, serif | Google Fonts |
| Code / monospace | JetBrains Mono | Menlo, Consolas, monospace | Google Fonts |
| Flag comments | DM Sans | system-ui, -apple-system, sans-serif | Google Fonts |

### Why These Fonts

**Playfair Display for headings.** Playfair is a transitional serif with high contrast between thick and thin strokes. It commands attention without being heavy. At large sizes (h1, h2), the contrast creates a clear visual hierarchy that draws the eye to section boundaries.

**Source Serif 4 for body text.** Source Serif is Adobe's answer to the question "what if Georgia were designed for modern screens?" It has generous x-height (the height of lowercase letters relative to capitals), open counters (the spaces inside letters like 'e' and 'a'), and carefully tuned spacing. These properties make it highly legible at body text sizes (16-18px). It is available in multiple weights, including a lighter weight (300) for less important text and a bold weight (700) for emphasis.

**JetBrains Mono for code.** JetBrains Mono was designed specifically for reading code. It has clear distinctions between similar characters (`0` vs `O`, `1` vs `l` vs `I`), optional ligatures for common programming constructs (`->`, `=>`, `!=`), and a generous character width that makes code scannable.

**DM Sans for flag comments.** Flag annotations are metadata, not document content. Using a sans-serif font for flags creates a clear visual distinction: "this is an annotation by a human reviewer, not part of the document." DM Sans is clean, geometric, and highly legible at small sizes.

### Font Loading

All four fonts are loaded from Google Fonts via `<link>` tags in the HTML template headers:

```html
<link href="https://fonts.googleapis.com/css2?family=Playfair+Display:wght@400;700;900&family=Source+Serif+4:ital,wght@0,300;0,400;0,600;0,700;1,400&family=JetBrains+Mono:wght@400;700&family=DM+Sans:wght@400;500;700&display=swap" rel="stylesheet">
```

The `display=swap` parameter means text is immediately visible in the fallback font, then swaps to the web font once loaded. This prevents a flash of invisible text (FOIT).

### Line Length

The document column is constrained to `max-width: 72ch`. This is measured in the `ch` unit, which equals the width of the `0` character in the font. For Source Serif 4 at body text size, 72 characters per line falls within the optimal range of 60-75 characters for sustained reading.

The 72-character constraint is applied to the `<article class="document">` element. The sidebar sits outside this constraint. On narrow screens (below 900px), the sidebar collapses and the document takes the full width.

## Diff Coloring

Code blocks with the `diff` language tag receive special git-style coloring:

```diff
- old line that was removed
+ new line that was added
@@ -1,3 +1,3 @@ hunk header
  unchanged context line
```

### Color Values

| Element | Light Mode | Dark Mode |
|---------|-----------|-----------|
| Added line background | `#DAFBE1` (soft green) | `#1B3829` (deep green) |
| Added line text | `#1A7F37` | `#3FB950` |
| Removed line background | `#FFE2DD` (soft red) | `#3D1F1F` (deep red) |
| Removed line text | `#CF222E` | `#F85149` |
| Hunk header background | `#EDE8FD` (soft purple) | `#2D1F4E` (deep purple) |
| Hunk header text | `#6639BA` | `#BC8CFF` |

### How Diff Detection Works

In `src/markdown.rs`, the `highlight_code_blocks` function checks the language tag of each code block. If the language is `diff`, it routes to `render_diff_block` instead of syntect:

```rust
if lang == "diff" {
    return render_diff_block(&code);
}
```

The `render_diff_block` function classifies each line by its prefix:

- Lines starting with `+` (but not `+++`): `diff-added`
- Lines starting with `-` (but not `---`): `diff-removed`
- Lines starting with `@@`: `diff-hunk`
- All others: `diff-context`

Each line is wrapped in a `<span>` with the appropriate class, and the CSS applies the colors.

## Syntax Highlighting

Code blocks with language tags are highlighted server-side by syntect using the "base16-ocean.dark" theme. This theme works with both light and dark CSS themes because the highlighted code sits inside a code block with its own background color.

Supported languages include all those in syntect's default bundle: Rust, Python, JavaScript, TypeScript, Go, Ruby, Java, C, C++, SQL, YAML, JSON, TOML, Bash, and many more.

### How Highlighting Works

1. comrak renders code blocks as `<pre><code class="language-X">...</code></pre>`
2. The `highlight_code_blocks` function in `src/markdown.rs` matches this pattern with a regex
3. For each match, syntect parses the code using the syntax definition for language X
4. syntect produces HTML with `<span style="color:#...">` elements for each token
5. The original `<pre><code>` is replaced with the highlighted version

## Layout

The page layout uses a two-column design:

```
+----------------------------------------------------------+
|  top bar (logo, filepath, theme toggle, flag count)      |
+-------------------------------+--------------------------+
|                               |                          |
|  document column              |  flag sidebar             |
|  (max-width: 72ch, centered) |  (slides in from right)  |
|                               |                          |
+-------------------------------+--------------------------+
|  status bar (version, watch status, connection)          |
+----------------------------------------------------------+
```

### Responsive Behavior

At screen widths below 900px:

- The sidebar collapses (hidden by default, toggle to show)
- The document column takes the full width
- The top bar stacks vertically if needed

## Animations

All animations use CSS transitions and keyframes:

| Animation | Duration | Trigger |
|-----------|----------|---------|
| Page load content fade-in | 200ms stagger per section | Page load |
| Flag highlight on hover | 300ms ease-in | Mouse hover over flag |
| Theme switch | 300ms ease on all colors | Theme toggle click |
| WebSocket reconnect | Pulse/throb animation | Connection state change |
| File list item slide-in | 50ms stagger per item | Directory listing load |

Animations are CSS-only (no JavaScript animation libraries). The `prefers-reduced-motion` media query is respected: when the user has requested reduced motion in their OS settings, animations are disabled.
