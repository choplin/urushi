# Architecture

This document is the starting point for developers changing Urushi. It
describes the architecture implemented in this repository today: workspace and
module responsibilities, dependency direction, rendering flows, terminal
ownership, and the tests that protect those boundaries.

The [`README`](../README.md#concept) explains why Urushi spans plain CLI output,
interactive prompts, and full-screen TUIs. This document explains how those
surfaces share a foundation without forcing them into one rendering model or
terminal lifecycle.

Urushi supports styled static output, renderer-neutral line components,
line-oriented prompts, optional live progress, and Ratatui adaptation through
`urushi-tui`.
It does not provide a general full-screen TUI runtime. Ratatui application
state, event loops, layout orchestration, and frame scheduling remain owned by
the consuming application. [`tui-architecture.md`](tui-architecture.md)
describes the target architecture for adding that runtime without changing the
boundaries documented here prematurely.

## Architecture at a glance

Reusable line components produce a renderer-neutral `View`. Terminal
capabilities are applied only at the output boundary.

```text
SemanticTokens --> Theme --> ComponentStyles
                                  |
Component props ------------------+
        |
        v
       View + TerminalProfile --> AnsiRenderer --> ANSI text
         |
         +--> StderrTerminal (owns renderer/profile) --> stable stderr output
```

The core [`View`](../urushi/src/view/model.rs) is an ordered collection of
styled lines and spans. It contains logical [`Style`](../urushi/src/style/logical.rs)
values rather than terminal-resolved ANSI strings. Components stop at this
boundary. [`AnsiRenderer`](../urushi/src/render/ansi.rs) applies a
[`TerminalProfile`](../urushi/src/terminal/profile.rs) to each span and converts
the result to text.

Urushi also retains a direct box-model rendering path for static content:

```text
plain text + Style --> Style::render --> ANSI-capable String
```

This path owns wrapping, alignment, padding, margins, and borders for one
styled block. It is separate from `View`, whose current model represents lines
that a component has already composed.

Terminal and Ratatui integrations sit outside these semantic types:

```text
Progress state --> Urushi progress lifecycle --> private indicatif adapter

Resolved Style --> RatatuiStyle
Style + text  --> RatatuiWidget --> caller-owned Ratatui Buffer
```

## Shared foundation and surface layers

Urushi's shared foundation is a set of presentation contracts, not a universal
widget tree or event loop. An application can define visual meaning once and
carry it across terminal surfaces while each surface retains the interaction
and lifecycle rules appropriate to it.

The contracts shared across surfaces are:

- [`Style`](../urushi/src/style/logical.rs), which describes logical color,
  modifiers, and box-model presentation without owning terminal state;
- [`Theme`](../urushi/src/theme/definition.rs),
  [`SemanticTokens`](../urushi/src/theme/tokens.rs), and
  [`ComponentRole`](../urushi/src/theme/role.rs), which give presentation a
  reusable semantic vocabulary;
- the [`text`](../urushi/src/text/) implementation, which supplies one
  ANSI-aware, cell-aware definition of visible width and wrapping; and
- [`TerminalProfile`](../urushi/src/terminal/profile.rs), which resolves the
  same logical styles for the capabilities of the actual output surface.

`View` is not in this lowest common set. It is the renderer-neutral line model
used by reusable line components and ANSI output. The prompt needs cursor,
viewport, help/error priority, and final cleanup information that `View` does
not model, while Ratatui applications render into a cell buffer. Those surfaces
therefore share the contracts above but use surface-specific view and runtime
types.

The implemented layers above the foundation are:

The plain CLI and interactive prompt rows describe working, dogfooded paths,
not placeholders for a future unified UI. They are intentionally partial:
sharing the foundation does not require either surface to wait for the TUI
runtime or to adopt its application model.

| Surface | Urushi-provided layer | Lifecycle owner | Current status |
| --- | --- | --- | --- |
| Plain CLI output | Direct box-model `Style::render`; reusable components producing `View`; `AnsiRenderer`; `StderrTerminal`; optional spinner and progress-bar lifecycles | The application owns its command workflow and stdout policy. `StderrTerminal` and progress handles own the stderr resources they acquire. | Implemented and dogfooded. Static output, themed line components, terminal degradation, and live/plain progress are covered by repository tests and runnable examples; public progress is also exercised by Agentlog dogfood. |
| Interactive prompt | `Form` / `Group`; typed `Input`, `Select`, and `Confirm`; synchronous validation; a prompt-specific view; inline redraw; terminal session setup and cleanup | `urushi-prompt` owns the blocking prompt session and resources it acquires. The application owns when the form runs and what submitted values mean. | Implemented and dogfooded. The core prompt flow, viewport behavior, validation, cancellation, and cleanup are covered by tests and runnable terminal examples. |
| Full-screen TUI | `urushi-tui` logical-style conversion and box-model widgets that draw into a caller-provided Ratatui `Buffer` | Today, the consuming Ratatui application owns state, events, layout orchestration, frame scheduling, and terminal lifecycle. | Adapter implemented in a provisional crate boundary. A general Urushi TUI runtime is not implemented; [`tui-architecture.md`](tui-architecture.md) defines its target architecture. |

The resulting flows are deliberately related but not identical:

```text
application semantics
        |
        v
SemanticTokens --> Theme --> ComponentRole --> logical Style
                                              |
             +--------------------------------+-----------------------------+
             |                                |                             |
             v                                v                             v
     plain CLI layer                  prompt layer                 Ratatui adapter
 Component props / text          Form / Group / Field             application view
             |                                |                             |
     View or Style::render              PromptView                  Style / widget
             |                                |                             |
 AnsiRenderer / terminal        inline renderer + session        caller-owned Buffer
```

This split prevents visual consistency from turning into lifecycle coupling.
For example, a prompt and a Ratatui screen may resolve the same
`PromptOptionSelected` role and use the same CJK width rules, but the prompt
still owns validation and inline cursor restoration, while the Ratatui
application owns full-screen event processing and frame rendering. Likewise,
plain CLI output can use the same theme without entering raw mode or starting
an event loop.

Applications may extend a `Theme` with domain-specific roles. They should keep
workflow meaning, such as command phases or product-specific selection states,
in that application extension rather than expanding Urushi's common roles or
moving application state into a surface adapter.

## Workspace responsibilities

| Crate | Responsibility | Dependencies within the workspace |
| --- | --- | --- |
| [`urushi`](../urushi/) | Logical styles, themes, renderer-neutral views and components, output adapters, terminal capability resolution, and optional progress lifecycle. | None |
| [`urushi-prompt`](../urushi-prompt/) | Typed input, select, and confirm forms; prompt state transitions; inline drawing; terminal session setup and cleanup. | `urushi` |
| [`urushi-tui`](../urushi-tui/) | Ratatui style conversion and box-model widgets; provisional owner of future full-screen TUI concerns. | `urushi` |

`urushi-prompt` owns interactive prompt behavior. The core crate must not gain
prompt-specific navigation, validation, cursor, or form-submission policy merely
to share styling.

## Core module responsibilities

| Module | Responsibility | Internal dependencies |
| --- | --- | --- |
| [`style`](../urushi/src/style/) | Colors, border glyphs, box spacing and alignment, logical style values, and direct box-model string rendering. | `text` |
| [`text`](../urushi/src/text/) | ANSI-aware visible-width measurement and cell-aware word/CJK wrapping. | None |
| [`theme`](../urushi/src/theme/) | Semantic color tokens, reusable component roles and styles, typed application extensions, and explicit light/dark selection. | `style` |
| [`view`](../urushi/src/view/) | Renderer-neutral `Span`, `Line`, and `View` values, plus composition of already-rendered string blocks. | `style`, `text` |
| [`component`](../urushi/src/component/) | Reusable semantic components that return `View`; currently summaries, warnings, owned lists, and owned trees. | `theme`, `view`, `text` |
| [`render`](../urushi/src/render/) | Translation of renderer-neutral views to ANSI text. | `style`, `view`, `terminal/profile` |
| [`terminal`](../urushi/src/terminal/) | Terminal capability detection, color degradation, stderr ownership, output-mode selection, and optional progress lifecycle. | `style`, `theme`, `view`, `render` |

The intended dependency direction is from I/O and adapters toward semantic
modules:

- `style` and `text` do not depend on themes, components, renderers, or terminal
  lifecycle;
- `theme` assigns semantic meaning to styles but does not inspect a terminal;
- `view` does not choose a renderer or own terminal state;
- a reusable component returns `View` and does not write output;
- renderers translate Urushi values into a backend representation and do not
  own application workflows;
- `terminal` owns physical output and live-region lifecycle, not application
  labels such as command introductions or final outcomes;
- prompt and application code compose these capabilities at their own entry
  points.

The `mod.rs` files are the source of truth for the implemented module graph.
Keep this table consistent with them.

## Responsibilities within modules

Files separate concepts or external change drivers without creating additional
crate boundaries.

### Style and text

- [`style/color.rs`](../urushi/src/style/color.rs) owns color representation and
  SGR encoding.
- [`style/border.rs`](../urushi/src/style/border.rs) owns border character sets.
- [`style/layout.rs`](../urushi/src/style/layout.rs) owns `Align`,
  `VerticalAlign`, and `Sides`.
- [`style/property.rs`](../urushi/src/style/property.rs) owns the closed generic
  property vocabulary used by `Style::add` and `Style::remove`.
- [`style/logical.rs`](../urushi/src/style/logical.rs) owns the `Style` builder,
  box-model rules, and direct string rendering.
- [`text/width.rs`](../urushi/src/text/width.rs) owns visible cell measurement
  and ANSI/grapheme-safe row truncation.
- [`text/wrap.rs`](../urushi/src/text/wrap.rs) owns word and hard wrapping.

Width and wrapping policy must remain shared. A component or renderer should
not introduce a private definition of CJK display width.

### Theme

- [`theme/tokens.rs`](../urushi/src/theme/tokens.rs) defines semantic colors.
- [`theme/role.rs`](../urushi/src/theme/role.rs) defines common component roles
  and typed role resolution.
- [`theme/component_styles.rs`](../urushi/src/theme/component_styles.rs) maps
  common roles to logical styles.
- [`theme/definition.rs`](../urushi/src/theme/definition.rs) owns `Theme`, typed
  extensions, `ThemeSet`, and explicit scheme selection.

Themes describe meaning. They do not detect `NO_COLOR`, inspect TTY state, emit
ANSI, or retain an output writer.

### View and components

- [`view/model.rs`](../urushi/src/view/model.rs) owns `Span`, `Line`, and `View`.
- [`view/join.rs`](../urushi/src/view/join.rs) composes strings that have already
  been rendered into rectangular blocks.
- each file under [`component`](../urushi/src/component/) owns one reusable
  component and its conversion to `View`.

Components either receive `ComponentStyles` directly or use a dedicated style
value supplied by it. When layout requires a display width, the caller supplies
that constraint explicitly. Components may perform component-specific layout
such as summary label alignment or branch composition, but they do not resolve
terminal capabilities or emit output.

`List` and `Tree`, including their recursive item and node types, are owned
presentation-neutral data. `ListStyle` and `TreeStyle` own the corresponding
semantic styles and marker policies, receive the data model, and compose a
renderer-neutral `View`. `ComponentStyles::list` and `ComponentStyles::tree`
provide global defaults; a caller can clone either value for local presentation
changes. List and Tree keep independent public models and callback positions,
while a private `Traversable` contract shares recursive layout, multiline
continuation, marker alignment, and display-width handling. The reusable
criteria for this separation are defined in
[`design/component-data-and-style.md`](design/component-data-and-style.md).

### Renderers

- [`render/ansi.rs`](../urushi/src/render/ansi.rs) resolves each logical span for
  a `TerminalProfile` and serializes the `View`.
- [`urushi-tui/src/style.rs`](../urushi-tui/src/style.rs) converts the
  stylable subset and border colors to Ratatui types.
- [`urushi-tui/src/widget.rs`](../urushi-tui/src/widget.rs) draws the
  Urushi box model into a caller-provided Ratatui buffer.

`urushi-tui` is provisionally an adapter, not yet a TUI framework. It does not own
application state, input handling, navigation, an event loop, terminal entry or
restoration, or frame scheduling. Those concerns are intentionally left for the
separate TUI design work and consuming applications.

### Terminal and progress

- [`terminal/profile.rs`](../urushi/src/terminal/profile.rs) detects ANSI and
  color capabilities for a specific writer and resolves logical styles.
- [`terminal/palette.rs`](../urushi/src/terminal/palette.rs) owns deterministic
  xterm palette conversion.
- [`terminal/stderr.rs`](../urushi/src/terminal/stderr.rs) owns stderr writes,
  terminal width, and live-versus-plain output selection.
- [`terminal/progress/spinner.rs`](../urushi/src/terminal/progress/spinner.rs)
  and [`bar.rs`](../urushi/src/terminal/progress/bar.rs) own Urushi's public
  progress states and lifecycle.
- [`terminal/progress/view.rs`](../urushi/src/terminal/progress/view.rs) owns the
  stable renderer-neutral progress lines.
- [`terminal/progress/indicatif_backend.rs`](../urushi/src/terminal/progress/indicatif_backend.rs)
  is a private live-region adapter.

Indicatif is an implementation detail. Public progress types expose messages,
positions, and semantic completion operations; they do not expose Indicatif
types, templates, draw targets, or tick configuration. Live and plain messages
both use `ComponentRole::Body`, and terminal capability resolution happens
before data reaches the private backend. Replacing Indicatif should therefore
not require changes in consumers such as Agentlog.

## Core contracts

### Style remains logical until an output boundary

`Theme` and `View` retain logical `Style` values. `TerminalProfile` applies the
writer-specific ANSI policy and color fidelity at an output boundary. Detect a
profile for the writer that will receive the result; do not reuse stdout's
profile for stderr or a Ratatui surface.

The immutable value model, closed property vocabulary, and generic
`add`/`remove` operations are specified in
[`style-model.md`](style-model.md).

`Style::render` is also an output boundary. It renders one box-model block
directly and does not inspect terminal capabilities by itself. Callers that
need capability degradation resolve the style first.

### Components stop at View

A reusable component owns semantic props, normalization, and component-local
layout. Its output is a `View`. It must not write to stderr, choose live mode,
construct an Indicatif object, or depend on Ratatui.

Application-specific workflow chrome remains in the application. For example,
Agentlog composes command start and finish lines locally rather than adding
generic `Intro` and `Outro` components to Urushi.

### Output resources have explicit owners

`AnsiRenderer` is a pure translator. `StderrTerminal` owns stderr output,
terminal-width detection, and the live/plain decision. `Spinner` and
`ProgressBar` own cleanup of their active live region and leave a stable result
line when work finishes or is interrupted.

Non-TTY output and `TERM=dumb` use append-only plain output. Machine-readable
stdout remains separate from human-facing progress on stderr.

### A prompt owns a region of rows, never the screen

An interactive prompt draws inline. It never enters the alternate screen and
never clears the terminal. Instead it owns a *region*: a run of rows anchored at
the cursor position it saves on its first draw. Only rows inside that region may
be erased or rewritten. Terminal content above the origin, and below the last
reserved row, belongs to whatever produced it.

The region is governed by three rules:

- **Claiming.** The region grows only downward, and only by scrolling new rows
  into existence with bare line feeds before re-anchoring the origin. It never
  shrinks during a session, because rows already scrolled into existence cannot
  be given back.
- **Releasing.** A submitted prompt keeps its final rows and moves the terminal
  below them, so the answered prompt stays in the scrollback. Every other
  outcome — cancellation, error, panic — erases the region and returns to the
  origin, leaving no trace.
- **Recovery.** Terminal writes can fail midway. The renderer therefore claims
  pessimistically and commits optimistically: before the first write of a redraw
  it records every row that redraw *could* touch, and it records anchoring and
  reservation only once the corresponding command has been written. Cleanup
  after a failure then erases the whole partially drawn view rather than the
  part that happened to succeed.

`urushi-prompt` splits this across three crate-private stages so each can be
reasoned about separately: `runtime::layout` resolves a view against the
terminal box, `runtime::inline_plan` turns that plus the previous presentation
into a list of terminal commands and their recovery checkpoints, and
`runtime::crossterm_executor` is the only stage that touches a writer. The
command vocabulary is Urushi's own, not crossterm's, so the same region
semantics can back a different execution environment.

### External backends stay behind adapters

Ratatui types stay in `urushi-tui`; Indicatif types stay in the private core
adapter. Public `urushi` surfaces use Urushi-owned concepts. This keeps backend
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
5. Visible width and wrapping use the shared `text` implementation.
6. Stderr lifecycle and live/plain selection have one owner:
   `StderrTerminal`.
7. Public progress APIs do not expose Indicatif representations or controls.
8. Live and stable progress use the same semantic theme roles.
9. `urushi-tui` adapters write only to the buffer supplied by the caller and do not
   own a TUI runtime.
10. Prompt-specific state, cursor behavior, and terminal cleanup remain in
    `urushi-prompt`, not the core component model.

## Common change paths

### Add a reusable component

1. Add one concept-focused file under `urushi/src/component/`.
2. Keep reusable data models presentation-neutral. Accept `ComponentStyles`
   directly for simple components, or add a dedicated component style value
   when data and presentation need independent reuse.
3. Return `View` without choosing an output writer or backend.
4. Put shared recursive layout behind a private contract unless multiple public
   component models genuinely need the same public abstraction.
5. Test semantic edge cases and CJK width behavior at the component boundary.
6. Re-export the component from `component/mod.rs` and the crate root when it
   is part of the public API.

### Add or replace an output adapter

1. Put backend-specific types in the crate that owns that output technology;
   core ANSI and terminal adapters remain under `render/` or `terminal/`.
2. Accept Urushi-owned styles, views, profiles, or semantic state at the
   boundary.
3. Keep backend templates, errors, and lifecycle handles out of public types.
4. Preserve terminal capability and theme semantics before translating to the
   backend representation.
5. Run consumer dogfood tests to verify that the public boundary remained
   stable.

### Change terminal capability behavior

1. Change detection and style resolution in `terminal/profile.rs`.
2. Change palette conversion only in `terminal/palette.rs`.
3. Verify TTY, non-TTY, `TERM=dumb`, `NO_COLOR`, and explicit profile behavior.
4. Check ANSI, progress, prompt, and Ratatui consumers for consistent
   degradation.

### Change progress behavior

1. Keep semantic lifecycle changes in `spinner.rs` or `bar.rs`.
2. Keep stable output composition in `view.rs`.
3. Keep Indicatif-specific redraw mechanics in `indicatif_backend.rs`.
4. Apply theme roles before crossing into the backend.
5. Verify both live and append-only modes, cleanup, interruption, and the
   Agentlog dogfood integration.

### Change Ratatui support

1. Keep style/color conversion in `urushi-tui/src/style.rs`.
2. Keep box-model buffer drawing in `urushi-tui/src/widget.rs`.
3. Preserve the caller's ownership of terminal setup, event processing, state,
   layout orchestration, and frame rendering.
4. Test narrow areas, CJK clipping, border colors, and terminal-profile
   degradation.

## Verification map

| Guarantee | Primary evidence |
| --- | --- |
| Box model, ANSI scopes, wrapping, alignment, and CJK width | [`urushi/tests/render.rs`](../urushi/tests/render.rs), [`urushi/tests/join.rs`](../urushi/tests/join.rs) |
| Terminal detection and style degradation | [`urushi/tests/terminal_profile.rs`](../urushi/tests/terminal_profile.rs), tests beside `terminal/profile.rs` |
| Theme roles resolve consistently for terminal and Ratatui output | [`urushi/tests/theme_terminal_render.rs`](../urushi/tests/theme_terminal_render.rs), [`urushi-tui/tests/theme_ratatui_render.rs`](../urushi-tui/tests/theme_ratatui_render.rs) |
| Progress messages share theme behavior across live and stable output | tests beside [`terminal/progress/mod.rs`](../urushi/src/terminal/progress/mod.rs) |
| Prompt submission, viewport behavior, and cleanup | tests beside [`urushi-prompt/src/runtime.rs`](../urushi-prompt/src/runtime.rs) |
| Prompt owned-region command order and recovery checkpoints | plan assertions beside [`urushi-prompt/src/runtime.rs`](../urushi-prompt/src/runtime.rs), against [`runtime/inline_plan.rs`](../urushi-prompt/src/runtime/inline_plan.rs) |
| Public progress behavior remains usable by a real consumer | Agentlog dogfood unit and CLI integration tests using the global Cargo patch configuration |

Use the Nix development environment for repository checks:

```sh
nix develop -c cargo fmt --all -- --check
nix develop -c cargo test --workspace --all-features
nix develop -c cargo clippy --workspace --all-targets --all-features -- -D warnings
```

When a change affects a dogfooded public API, also run the consumer's tests and
Clippy with `config-urushi-dev.toml`.

## Current limitations

These are descriptions of the current implementation, not commitments to a
future roadmap:

- `View` is an ordered line/span representation, not a general layout tree or
  canonical resolved cell grid;
- component-specific width handling occurs while constructing a `View`;
- `Style::render` and `View` rendering are separate composition paths;
- live progress currently uses a private Indicatif backend;
- `urushi-tui` is currently limited to style conversion and box-model widget
  drawing;
- full-screen TUI architecture and runtime policy are intentionally outside the
  scope of this document;
- `urushi-prompt` uses a prompt-specific internal view and renderer because it
  requires cursor, viewport, help/error prioritization, and cleanup semantics
  that the core `View` does not currently model.

## Keeping this document current

Update this document in the same change whenever code changes a module
responsibility, dependency direction, public rendering flow, terminal resource
owner, or architectural invariant. Ordinary implementation details that remain
inside an existing boundary do not require an architecture update.

Keep this document focused on the implementation that exists today. Put API
details next to their modules and keep unimplemented plans out of the
architecture description.
