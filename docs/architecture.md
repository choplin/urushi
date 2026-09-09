# Architecture

This document is the starting point for developers changing Urushi. It covers
the workspace and module responsibilities, the dependency direction between
them, the rendering flows, terminal ownership, and the invariants those
boundaries rest on.

Urushi spans plain CLI output, blocking interactive prompts, and a full-screen
TUI runtime layered on Ratatui. The [`README`](../README.md#concept) explains
why one library covers all three; this document explains how those surfaces
share a foundation without being forced into one rendering model or one
terminal lifecycle. Each surface's own design is a separate document:
[`inline-prompt-rendering.md`](inline-prompt-rendering.md) for the prompt's
rendering and terminal-surface choice, and
[`tui-architecture.md`](tui-architecture.md) for the full-screen application
runtime.

What follows states rules. The reasoning behind a rule, and the alternatives it
was chosen over, live under [`design/`](design/);
[`decision-log.md`](decision-log.md) is the history of the decisions those
documents record.

## Architecture at a glance

Concrete component presentations compose semantic data into a renderer-neutral
`View`. Terminal capabilities are applied only at the output boundary.

```text
SemanticTokens --> Theme --> ComponentTheme --> concrete Presentation
                                                       |
Semantic component data -------------------------------+-- compose
Optional component-specific frame input ---------------+     |
                                                             v
 View (built-ins and Canvas items) --> resolve --> ResolvedView + TerminalProfile --> AnsiRenderer --> ANSI text
         |
         +--> StderrTerminal (owns renderer/profile) --> stable stderr output
```

The core [`View`](../urushi/src/view/model.rs) is a component-agnostic layout
tree. Its built-in vocabulary is `Text`, `Block`, `Row`, `Column`, `Grid`,
`Canvas`, and the keyed `AnchorBlock` form. Canvas carries one sizing mode and
ordered, comparable items that record renderer-neutral drawing commands after
its finite size is known. The tree carries logical
[`TextStyle`](../urushi/src/style/text.rs) and
[`BlockStyle`](../urushi/src/style/block.rs) values rather than
terminal-resolved ANSI strings. Component presentations stop at this boundary.
[`resolve`](../urushi/src/view/resolve.rs) turns the tree into one `ResolvedView`
rectangle of styled graphemes, and
[`AnsiRenderer`](../urushi/src/render/ansi.rs) applies a
[`TerminalProfile`](../urushi/src/terminal/profile.rs) to it and serializes the
result. A presentation either lowers component meaning into built-in nodes or
binds its component snapshot and policy into an intrinsically sized Canvas.
Neither `View` nor `resolve` can inspect whether a tree or item came from a
List, Table, Tree, or future Graph.

Primitive styling splits in two, and geometry belongs to only one half: a
`TextStyle` is everything a terminal can express about a run of text, and a
`BlockStyle` is a rectangle plus the style filling it.
[`component-model.md`](component-model.md) defines the semantic-to-primitive
composition boundary, [`view-model.md`](view-model.md) defines the primitive
tree and layout pass, and [`style-model.md`](style-model.md) defines the style
values themselves.

Direct box-model rendering of static content is the single-block case of that
same pass:

```text
plain text + BlockStyle --> BlockStyle::render --> RenderedBlock
```

`BlockStyle::render` resolves a `Block` containing uniformly styled text with
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
- concrete component presentations, whose `compose` operations turn semantic
  data and any borrowed component-specific frame input into built-in `View`
  nodes or bound Canvas items without receiving an available area;
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
| Plain CLI output | Direct box-model `BlockStyle::render`; concrete component presentations composing `View`; `AnsiRenderer`; `StderrTerminal`; optional spinner and progress-bar lifecycles | The application owns its command workflow and stdout policy. `StderrTerminal` and progress handles own the stderr resources they acquire. |
| Interactive prompt | `Form` / `Group`; typed `Input`, `Select`, and `Confirm`; synchronous validation; selectable inline or alternate-screen presentation; terminal session setup and cleanup | `urushi-prompt` owns the blocking prompt session and the resources it acquires. The application owns when the form runs and what submitted values mean. |
| Full-screen TUI | `urushi-tui`: the runtime and its `ratatui` adapter — logical-style conversion, widgets that resolve a `View` and draw it into a Ratatui `Buffer`, and the cell-writing path the runtime's renderer takes with a view it resolved itself | The `urushi-tui` runtime owns event delivery, frame scheduling, terminal entry and restoration. The application owns its model, update, and view. |

The surfaces are intentionally partial. Sharing the foundation does not require
one surface to adopt another's application model or lifecycle, so the flows are
deliberately related but not identical:

```text
application semantics
        |
        v
SemanticTokens --> Theme --> ComponentTheme / logical styles
                                      |
             +------------------------+-------------------------------------+
             |                                |                             |
             v                                v                             v
     plain CLI layer                  prompt layer                 Ratatui adapter
 data + Presentation / text      Form / Group / Field             application view
             |                                |                             |
  View or BlockStyle::render        View + prompt stages        BlockStyle / widget
             |                                |                             |
 AnsiRenderer / terminal       selected renderer + session       caller-owned Buffer
```

This split prevents visual consistency from turning into lifecycle coupling.
For example, a prompt and a Ratatui screen may resolve the same
`PromptOptionSelected` role and use the same CJK width rules, but the prompt
still owns validation and its selected terminal surface, while the full-screen
application runtime owns event processing and frame rendering. Likewise, plain
CLI output can use the same theme without entering raw mode or starting an
event loop.

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
| [`urushi-prompt`](../urushi-prompt/) | Typed input, select, and confirm forms; prompt state transitions; inline and alternate-screen presentations; terminal session setup and cleanup. | `urushi` |
| [`urushi-tui`](../urushi-tui/) | The Ratatui backend adapter in [`ratatui`](../urushi-tui/src/ratatui/) — style conversion, widgets, and the cell-writing path they share with the renderer — and the full-screen TUI runtime behind the default-on `runtime` Cargo feature. | `urushi` |

`urushi-prompt` owns interactive prompt behavior. The core crate must not gain
prompt-specific navigation, validation, cursor, or form-submission policy merely
to share styling.

## Core module responsibilities

| Module | Responsibility | Internal dependencies |
| --- | --- | --- |
| [`style`](../urushi/src/style/) | Colors, border glyphs, box spacing and alignment, the text `TextStyle` and the geometry-bearing `BlockStyle`, and the direct block render entry point. | `text`, `view` |
| [`text`](../urushi/src/text/) | Plain-text values, including grapheme-aligned `StyledText`, plus display-width measurement and cell-aware word/CJK wrapping. | `style` |
| [`theme`](../urushi/src/theme/) | Semantic color tokens, reusable component roles, canonical component presentations, application role resolution, and explicit light/dark selection. | `style`, `component` |
| [`view`](../urushi/src/view/) | The renderer-neutral `View` tree, the one layout pass in its three phases — width, height, assembly — behind `measure` / `resolve` (`Size`, `Available`, `StyledGrapheme`, `ResolvedView`), and composition of already-rendered `RenderedBlock` values. | `style`, `text` |
| [`component`](../urushi/src/component/) | Reusable semantic data and the independent concrete presentations that compose it into primitive `View` trees. | `theme`, `view`, `text` |
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
canonical component presentations and those presentations derive their
logical styles from the theme.
`render` and `terminal` do so because `AnsiRenderer` applies a
`TerminalProfile` and `StderrTerminal` owns an `AnsiRenderer`.

Width and wrapping policy must remain shared. A component or renderer should
not introduce a private definition of CJK display width.

Themes describe meaning. They do not detect `NO_COLOR`, inspect TTY state, emit
ANSI, or retain an output writer.

Reusable components separate owned semantic data — `List`, `Tree`, `Table`,
`Summary`, and `Warning` — from concrete presentations that compose it into a
primitive `View`. The contract they follow is defined in
[`component-model.md`](component-model.md).

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

### Presentations stop at View

A reusable component owns semantic data and normalization. A concrete
presentation owns one structural interpretation and composes a `View` without
receiving `Available`; it may also borrow component-specific immutable input
describing the current frame. It either constructs built-in nodes or binds an
owned frame into a Canvas item and supplies one Canvas-wide intrinsic sizing
value. The application owns the frame input's transitions, and `resolve` alone
decides area-dependent geometry before asking Canvas items to draw. Neither
layer may write to stderr, choose live mode, construct an Indicatif object, or
depend on Ratatui. The boundary and sizing invariants are defined in
[`design/canvas.md`](design/canvas.md).

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

### A prompt chooses its terminal surface

An interactive prompt draws inline by default. Its logical region participates
in the primary buffer and scrollback, and terminal soft wrapping owns reflow.
Because that reflow makes the old region unlocatable, an inline prompt returns
a resize error by default. A caller may instead authorize clearing the visible
primary-buffer viewport and redrawing there; this destructive policy does not
clear scrollback or enter the alternate screen.
Callers that need complete application-controlled layout across resize may
instead select an alternate-screen presentation, which owns and redraws the
whole temporary viewport. The two presentations share form behavior, but their
surface-specific positioning options do not mix.
[`inline-prompt-rendering.md`](inline-prompt-rendering.md) defines both display
contracts and the default inline rendering path; its linked design topics hold
the exact resize and ownership rules.

### External backends stay behind adapters

Ratatui types stay in `urushi-tui`; Indicatif types stay in the private core
adapter. Public `urushi` surfaces use Urushi-owned concepts: progress types
expose messages, positions, and semantic completion operations, not Indicatif
templates, draw targets, or tick configuration, and `TerminalProfile` resolves
capabilities before data reaches the private backend. This keeps backend
replacement local and prevents backend lifecycle rules from becoming core
application contracts.

Replacement is not hypothetical. Urushi is built toward owning the layer that
writes to and reads from the terminal, because a rule this documentation states
about that layer — which stream a request goes to, which reader consumes a
reply, when the terminal is entered and restored — can only be guaranteed by
the code that performs it. An adapted backend is a means to that layer, not the
definition of it.

One rule follows for every document here: **a design states what the terminal
layer must do, never what a current backend happens to do.** Where a backend
cannot express a requirement, the requirement is still what gets written down.
The deviation is recorded at the code that deviates, so a reader of that code
sees it, and in the issue tracker, so it is scheduled. Softening a rule to
match a backend removes the only record of what the backend owes, and makes it
permanent by making it invisible.

## Architectural invariants

Changes must preserve these invariants unless the architecture itself is being
changed deliberately and this document is updated in the same change:

1. `style` and `text` remain independent from I/O and external rendering
   backends.
2. Themes contain semantic choices but no terminal detection or writer state.
3. A reusable component owns semantic data; only a concrete presentation
   composes that data and any borrowed current-frame input into a primitive
   `View`, and neither performs output or owns state transitions.
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
