# Architecture

This document is the starting point for developers changing Urushi. It covers
the workspace and module responsibilities, the dependency direction between
them, the rendering flows, terminal ownership, and the invariants those
boundaries rest on.

Urushi spans plain CLI output, inline interactive prompts, and a full-screen
TUI runtime layered on Ratatui. The [`README`](../README.md#concept) explains
why one library covers all three; this document explains how those surfaces
share a foundation without being forced into one rendering model or one
terminal lifecycle. Each surface's own design is a separate document:
[`inline-prompt-rendering.md`](inline-prompt-rendering.md) for the prompt's
render path, [`tui-architecture.md`](tui-architecture.md) for the full-screen
runtime.

What follows states rules. The reasoning behind a rule, and the alternatives it
was chosen over, live under [`design/`](design/);
[`decision-log.md`](decision-log.md) is the history of the decisions those
documents record.

## Architecture at a glance

Reusable components produce a renderer-neutral `View`. Terminal capabilities
are applied only at the output boundary.

```text
SemanticTokens --> Theme --> ComponentStyles
                                  |
Component props ------------------+
        |
        v
       View --> resolve --> ResolvedView + TerminalProfile --> AnsiRenderer --> ANSI text
         |
         +--> StderrTerminal (owns renderer/profile) --> stable stderr output
```

The core [`View`](../urushi/src/view/model.rs) is a tree of four nodes — `Text`,
`Block`, `Row`, and `Column` — carrying logical
[`TextStyle`](../urushi/src/style/text.rs) and
[`BlockStyle`](../urushi/src/style/block.rs) values rather than
terminal-resolved ANSI strings. Components stop at this boundary.
[`resolve`](../urushi/src/view/resolve.rs) turns the tree into one `ResolvedView`
rectangle of styled graphemes, and
[`AnsiRenderer`](../urushi/src/render/ansi.rs) applies a
[`TerminalProfile`](../urushi/src/terminal/profile.rs) to it and serializes the
result.

Presentation splits in two, and geometry belongs to only one half: a `TextStyle`
is everything a terminal can express about a run of text, and a `BlockStyle` is
a rectangle plus the style filling it. [`view-model.md`](view-model.md) defines
the view tree and the layout pass; [`style-model.md`](style-model.md) defines
the style values themselves.

Direct box-model rendering of static content is the single-block case of that
same pass:

```text
plain text + BlockStyle --> BlockStyle::render --> RenderedBlock
```

`BlockStyle::render` resolves `Block(style, Text(content, style.text))` with
unbounded `Available`, so there is one implementation of the box model in the
workspace. [`RenderedBlock`](../urushi/src/view/rendered.rs) carries the size it
was measured at; `join_horizontal` and `join_vertical` compose such blocks
without re-measuring escape sequences. The reasoning behind this shape is
recorded in [`design/view-block-model.md`](design/view-block-model.md).

Terminal and Ratatui integrations sit outside these semantic types:

```text
Progress state --> Urushi progress lifecycle --> private Indicatif adapter

Resolved TextStyle   --> RatatuiStyle
View + Rect          --> ViewWidget    --> caller-owned Ratatui Buffer
BlockStyle + text    --> RatatuiWidget --> ViewWidget's path
```

The Ratatui adapter computes no geometry. A target `Rect` becomes `Available`,
`resolve` runs the same layout pass inside that area, and the adapter converts
each grapheme and its logical style into cells, so the two backends cannot
disagree about a rectangle.

## Shared foundation and surface layers

Urushi's shared foundation is a set of presentation contracts, not a universal
widget tree or event loop. An application can define visual meaning once and
carry it across terminal surfaces while each surface retains the interaction
and lifecycle rules appropriate to it.

The contracts shared across surfaces are:

- [`TextStyle`](../urushi/src/style/text.rs) and
  [`BlockStyle`](../urushi/src/style/block.rs), which describe logical text and
  box-model presentation without owning terminal state;
- [`Theme`](../urushi/src/theme/definition.rs),
  [`SemanticTokens`](../urushi/src/theme/tokens.rs), and
  [`ComponentRole`](../urushi/src/theme/role.rs), which give presentation a
  reusable semantic vocabulary;
- the [`text`](../urushi/src/text/) implementation, which supplies one
  cell-aware definition of plain-text display width and wrapping; and
- [`TerminalProfile`](../urushi/src/terminal/profile.rs), which resolves the
  same logical styles for the capabilities of the actual output surface.

`View` belongs to that foundation as well. Every surface goes through the same
`resolve` and draws only the `ResolvedView` it produces — the Ratatui adapter as
much as the ANSI renderer — so the surfaces share the layout pass itself and
not merely its model.

The prompt shares `View` and that layout pass for its Resolve stage, and adds
prompt-specific Frame, Plan, and Execute stages above it. `View` models no text
cursor, so the prompt's cursor, viewport, help and error priority, and cleanup
state live in a prompt-specific runtime rather than in a view.
[`inline-prompt-rendering.md`](inline-prompt-rendering.md) defines that render
path.

The surfaces above the foundation, and the layer Urushi provides for each, are:

| Surface | Urushi-provided layer | Lifecycle owner |
| --- | --- | --- |
| Plain CLI output | Direct box-model `BlockStyle::render`; reusable components producing `View`; `AnsiRenderer`; `StderrTerminal`; optional spinner and progress-bar lifecycles | The application owns its command workflow and stdout policy. `StderrTerminal` and progress handles own the stderr resources they acquire. |
| Interactive prompt | `Form` / `Group`; typed `Input`, `Select`, and `Confirm`; synchronous validation; the prompt-specific render stages; inline redraw; terminal session setup and cleanup | `urushi-prompt` owns the blocking prompt session and the resources it acquires. The application owns when the form runs and what submitted values mean. |
| Full-screen TUI | `urushi-tui`: the runtime and its `ratatui` adapter — logical-style conversion, widgets that resolve a `View` and draw it into a Ratatui `Buffer`, and the cell-writing path the runtime's renderer takes with a view it resolved itself | The `urushi-tui` runtime owns event delivery, frame scheduling, terminal entry and restoration. The application owns its model, update, and view. |

The surfaces are intentionally partial. Sharing the foundation does not require
one surface to adopt another's application model or lifecycle, so the flows are
deliberately related but not identical:

```text
application semantics
        |
        v
SemanticTokens --> Theme --> ComponentRole --> logical TextStyle
                                              |
             +--------------------------------+-----------------------------+
             |                                |                             |
             v                                v                             v
     plain CLI layer                  prompt layer                 Ratatui adapter
 Component props / text          Form / Group / Field             application view
             |                                |                             |
  View or BlockStyle::render        View + prompt stages        BlockStyle / widget
             |                                |                             |
 AnsiRenderer / terminal        inline renderer + session        caller-owned Buffer
```

This split prevents visual consistency from turning into lifecycle coupling.
For example, a prompt and a Ratatui screen may resolve the same
`PromptOptionSelected` role and use the same CJK width rules, but the prompt
still owns validation and inline cursor restoration, while the full-screen
runtime owns event processing and frame rendering. Likewise, plain CLI output
can use the same theme without entering raw mode or starting an event loop.

## Extending a theme

The same separation governs how an application adds its own meaning.
Applications may extend a `Theme` with domain-specific roles by implementing
`TextThemeRole` or `BlockThemeRole` for their own role type, which derives the
style from the theme each time it is asked instead of freezing it at
construction time. Workflow meaning — command phases, product-specific selection
states — belongs in those application roles rather than in an expansion of
Urushi's common roles or in a surface adapter. The conventions for writing such
a role are documented with the extension point itself, in
[`urushi/src/theme/mod.rs`](../urushi/src/theme/mod.rs).

## Workspace responsibilities

| Crate | Responsibility | Dependencies within the workspace |
| --- | --- | --- |
| [`urushi`](../urushi/) | Logical styles, themes, renderer-neutral views and components, output adapters, terminal capability resolution, and the progress lifecycle. Stderr ownership and live progress sit behind the optional `terminal` Cargo feature. | None |
| [`urushi-prompt`](../urushi-prompt/) | Typed input, select, and confirm forms; prompt state transitions; inline drawing; terminal session setup and cleanup. | `urushi` |
| [`urushi-tui`](../urushi-tui/) | The Ratatui backend adapter in [`ratatui`](../urushi-tui/src/ratatui/) — style conversion, widgets, and the cell-writing path they share with the renderer — and the full-screen TUI runtime behind the default-on `runtime` Cargo feature. | `urushi` |

`urushi-prompt` owns interactive prompt behavior. The core crate must not gain
prompt-specific navigation, validation, cursor, or form-submission policy merely
to share styling.

## Core module responsibilities

| Module | Responsibility | Internal dependencies |
| --- | --- | --- |
| [`style`](../urushi/src/style/) | Colors, border glyphs, box spacing and alignment, the text `TextStyle` and the geometry-bearing `BlockStyle`, and the direct block render entry point. | `text`, `view` |
| [`text`](../urushi/src/text/) | Plain-text display-width measurement and cell-aware word/CJK wrapping, over the `PrintableText` / `PrintableLines` types that carry the plain-text domain. | None |
| [`theme`](../urushi/src/theme/) | Semantic color tokens, reusable component roles and styles, application role resolution, and explicit light/dark selection. | `style`, `component` |
| [`view`](../urushi/src/view/) | The renderer-neutral `View` tree, the one layout pass in its three phases — width, height, assembly — behind `measure` / `resolve` (`Size`, `Available`, `StyledGrapheme`, `ResolvedView`), and composition of already-rendered `RenderedBlock` values. | `style`, `text` |
| [`component`](../urushi/src/component/) | Reusable semantic components that return `View`: summaries, warnings, owned lists, owned trees, and owned tables. | `theme`, `view`, `text` |
| [`render`](../urushi/src/render/) | Translation of renderer-neutral views to ANSI text. | `style`, `view`, `terminal/profile` |
| [`terminal`](../urushi/src/terminal/) | Terminal capability detection, color degradation, stderr ownership, output-mode selection, and the progress lifecycle. | `style`, `theme`, `view`, `render` |

The dependency direction runs from I/O and adapters toward semantic modules:

- `style` and `text` do not depend on themes, components, renderers, or terminal
  lifecycle;
- `theme` assigns semantic meaning to styles but does not inspect a terminal;
- `view` does not choose a renderer or own terminal state;
- renderers translate Urushi values into a backend representation and do not
  own application workflows;
- `terminal` owns physical output and live-region lifecycle, not application
  labels such as command introductions or final outcomes;
- prompt and application code compose these capabilities at their own entry
  points.

Three pairs reference each other by design. `style` and `view` do so because
`BlockStyle::render` is the single-block case of the view's layout pass rather
than a second box model. `theme` and `component` do so because `theme` stores
the component style values and `component` resolves its roles through `theme`.
`render` and `terminal` do so because `AnsiRenderer` applies a
`TerminalProfile` and `StderrTerminal` owns an `AnsiRenderer`.

Width and wrapping policy must remain shared. A component or renderer should
not introduce a private definition of CJK display width.

Themes describe meaning. They do not detect `NO_COLOR`, inspect TTY state, emit
ANSI, or retain an output writer.

Reusable components separate owned, presentation-neutral data — `List`,
`Tree`, `Table` — from the component style that composes it into a `View`. The
contract they follow is defined in [`component-model.md`](component-model.md).

## Core contracts

### TextStyle remains logical until an output boundary

`Theme` and `View` retain logical `TextStyle` values. `TerminalProfile` applies
the writer-specific ANSI policy and color fidelity at an output boundary. Detect
a profile for the writer that will receive the result; do not reuse stdout's
profile for stderr or a Ratatui surface.

The prompt's view is the one deliberate exception: its runs carry
profile-resolved styles from the moment the view is built, because rows there
are compared for equality to decide whether to redraw — see
[`design/style-canonical-form.md`](design/style-canonical-form.md). This does
not relax the contract for `Theme` or `View`.

The immutable value model, closed property vocabulary, and generic
`add`/`remove` operations are specified in
[`style-model.md`](style-model.md).

`BlockStyle::render` is also an output boundary. It renders one block directly
and does not inspect terminal capabilities by itself. Callers that need
capability degradation call `TerminalProfile::resolve_block_style` first.

### Components stop at View

A reusable component owns semantic props, normalization, and component-local
layout. Its output is a `View`. It must not write to stderr, choose live mode,
construct an Indicatif object, or depend on Ratatui.

Application-specific workflow chrome remains in the application: an application
composes its own command start and finish lines rather than Urushi growing
generic `Intro` and `Outro` components.

### Output resources have explicit owners

`AnsiRenderer` is a pure translator. `StderrTerminal` owns stderr output,
terminal-width detection, and the live/plain decision. `Spinner` and
`ProgressBar` own cleanup of their active live region and leave a stable result
line when work finishes or is interrupted.

Non-TTY output and `TERM=dumb` use append-only plain output. Machine-readable
stdout remains separate from human-facing progress on stderr.

### A prompt owns a region of rows, never the screen

An interactive prompt draws inline. It never enters the alternate screen and
never clears the terminal. Instead, it owns a *region*: a run of rows anchored
at the cursor position it saves on its first draw. Only rows inside that region
may be erased or rewritten. Terminal content above the origin, and below the
last reserved row, belongs to whatever produced it.
[`inline-prompt-rendering.md`](inline-prompt-rendering.md) defines how the
region is claimed, released, and recovered after a failed write.

### External backends stay behind adapters

Ratatui types stay in `urushi-tui`; Indicatif types stay in the private core
adapter. Public `urushi` surfaces use Urushi-owned concepts: progress types
expose messages, positions, and semantic completion operations, not Indicatif
templates, draw targets, or tick configuration, and `TerminalProfile` resolves
capabilities before data reaches the private backend. This keeps backend
replacement local and prevents backend lifecycle rules from becoming core
application contracts.

## Architectural invariants

Changes must preserve these invariants unless the architecture itself is being
changed deliberately and this document is updated in the same change:

1. `style` and `text` remain independent from I/O and external rendering
   backends.
2. Themes contain semantic choices but no terminal detection or writer state.
3. A reusable component returns `View` and performs no output.
4. `AnsiRenderer` applies `TerminalProfile` at the output boundary.
5. Display width and wrapping use the shared `text` implementation; rendered
   output is measured only by `RenderedBlock::from_ansi`.
6. Stderr lifecycle and live/plain selection have one owner:
   `StderrTerminal`.
7. Public progress APIs do not expose Indicatif representations or controls.
8. Live and plain progress use the same semantic theme roles.
9. `urushi-tui` widgets write only to the buffer supplied by the caller;
   terminal lifecycle, event delivery, and frame scheduling belong to the
   runtime, never to a widget.
10. Prompt-specific state, cursor behavior, and terminal cleanup remain in
    `urushi-prompt`, not the core component model.
