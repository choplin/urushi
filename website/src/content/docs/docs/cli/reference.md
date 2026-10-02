---
title: CLI reference
description: Look up static-output helpers, rendering settings, CLI roles, and the built-in CLI presentations.
---

Use this page after choosing a workflow from the [CLI overview](/docs/cli/).
Core `View`, style, theme, component, and text-width APIs have their own
references under **Presentation model**.

## Standard-stream helpers

| Function | Stream | Adds newline | Input |
|---|---|---:|---|
| `print` | stdout | no | `StyledText` |
| `println` | stdout | yes | `StyledText` |
| `eprint` | stderr | no | `StyledText` |
| `eprintln` | stderr | yes | `StyledText` |
| `print_view` | stdout | no | `View` |
| `println_view` | stdout | yes | `View` |
| `eprint_view` | stderr | no | `View` |
| `eprintln_view` | stderr | yes | `View` |

Text helpers preserve the source line and tab structure. View helpers resolve
layout against the detected terminal width before rendering. Attached terminals
use detected capabilities; redirected streams use unlimited width and plain
text. A non-empty `NO_COLOR` disables color but retains supported non-color
attributes.

All helpers return `std::io::Result<()>` and keep one standard-stream lock while
writing the rendered bytes and any requested trailing newline. This does not
promise one operating-system write or atomic output: newline helpers may write
the newline separately, and `write_all` may retry. The helpers do not enter raw
mode, read input, or manage a cursor-driven screen.

## Explicit layout and rendering

| API | Purpose |
|---|---|
| `measure(&view)` | Return the natural size of a view. |
| `resolve(&view, available)` | Produce a `ResolvedView` for a known cell area. |
| `render(&resolved, &settings)` | Serialize a resolved rectangle to a `String`. |
| `render_text(&text, &settings)` | Serialize `StyledText` without box-model layout. |
| `Available::NONE` | No width or height constraint. |
| `Available::columns(n)` | Limit layout to `n` columns. |

`RenderSettings::default()` selects no escape-sequence-producing features.
`RenderSettings::all()` selects true color, every text attribute, every
underline style, underline color, and hyperlinks. `From<TerminalCapabilities>`
copies the terminal's confirmed feature maximum.

| Feature axis | Setter | Reset | Getter |
|---|---|---|---|
| Color | `color_level` | `reset_color_level` | `get_color_level` |
| Attributes | `attributes` | `reset_attributes` | `get_attributes` |
| Underline styles | `underline_styles` | `reset_underline_styles` | `get_underline_styles` |
| Underline colors | `underline_colors` | `reset_underline_colors` | `get_underline_colors` |
| Hyperlinks | `hyperlinks` | `reset_hyperlinks` | `get_hyperlinks` |

`resolve_text_style` narrows one logical `TextStyle` to these settings. Use it
only when implementing a serializer; ordinary output should render the whole
resolved view.

`UnderlineStyleSet` is the value accepted by `underline_styles`. It provides
`SINGLE`, `DOUBLE`, `CURLY`, `DOTTED`, and `DASHED` constants together with
`empty`, `all`, `union`, `contains`, and `|` composition.

`TerminalTextStyle::from(&text_style)` lowers a logical style into a physical
`TerminalStyle` plus an optional OSC 8 `Hyperlink`. It does not negotiate
capabilities: an adapter or custom serializer must first call
`RenderSettings::resolve_text_style`, then inspect `style()` and `hyperlink()`.
Ordinary CLI output should use `render`, `render_text`, or the standard-stream
helpers instead.

## `urushi-cli` data

| Type | Constructor and builders | Accessors |
|---|---|---|
| `Summary` | `new(title)`, `field(label, value)` | `title`, `fields` |
| `SummaryField` | `new(label, value)` | `label`, `value` |
| `Warning` | `new(title, message)` | `title`, `message` |

All strings are plain text. Apply styling through a presentation or
`CliTheme`; do not embed ANSI sequences.

## CLI theme and presentations

`CliTheme::from_theme(&theme)` derives four roles:

| `CliRole` | Core source | Used for |
|---|---|---|
| `Body` | `ComponentRole::Body` | Field values and warning messages |
| `Muted` | `ComponentRole::Muted` | Rails, spacing, and summary labels |
| `Accent` | `ComponentRole::Accent` | Summary glyph and title |
| `Warning` | `theme.tokens().warning` | Warning glyph and title |

`get_style(role)` returns a role's style. `style(role, text_style)` returns an
updated theme and also updates the built-in presentations that consume the
role.

| API | Result |
|---|---|
| `summary(&Summary)` | Canonical summary `View` |
| `warning(&Warning)` | Canonical warning `View` |
| `summary_presentation()` | Borrow the canonical `SummaryPresentation` |
| `warning_presentation()` | Borrow the canonical `WarningPresentation` |
| `SummaryPresentation::new(muted, accent, body)` | Independent summary policy |
| `WarningPresentation::new(muted, warning, body)` | Independent warning policy |
| `compose(&data)` | Renderer-neutral `View` from either presentation |

In 0.1.0, `Summary` and `Warning` are the complete set of built-in
`urushi-cli` presentations. Delivery to stdout, stderr, a logger, or another
writer remains application policy.

## Related references

- [Styles reference](/docs/core/styles/reference/)
- [View and layout reference](/docs/core/views/reference/)
- [Components reference](/docs/core/components/reference/)
- [Rendering behavior](/docs/reference/rendering/)
