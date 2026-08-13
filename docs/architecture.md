# Architecture

This document is the starting point for developers changing Urushi. It
describes the architecture implemented in this repository today: workspace and
module responsibilities, dependency direction, rendering flows, terminal
ownership, and the tests that protect those boundaries.

Urushi supports styled static output, renderer-neutral line components,
line-oriented prompts, optional live progress, and optional Ratatui adaptation.
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

Optional terminal and Ratatui integrations sit outside these semantic types:

```text
Progress state --> Urushi progress lifecycle --> private indicatif adapter

Resolved Style --> RatatuiStyle
Style + text  --> RatatuiWidget --> caller-owned Ratatui Buffer
```

## Workspace responsibilities

| Crate | Responsibility | Dependencies within the workspace |
| --- | --- | --- |
| [`urushi`](../urushi/) | Logical styles, themes, renderer-neutral views and components, output adapters, terminal capability resolution, and optional progress lifecycle. | None |
| [`urushi-prompt`](../urushi-prompt/) | Typed input, select, and confirm forms; prompt state transitions; inline drawing; terminal session setup and cleanup. | `urushi` |

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
| [`component`](../urushi/src/component/) | Reusable semantic components that return `View`; currently summaries and warnings. | `theme`, `view`, `text` |
| [`render`](../urushi/src/render/) | Translation to output technologies: ANSI text and optional Ratatui styles/widgets. | `style`, `view`, `terminal/profile`, `text` |
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
- [`style/layout.rs`](../urushi/src/style/layout.rs) owns `Align` and `Sides`.
- [`style/logical.rs`](../urushi/src/style/logical.rs) owns the `Style` builder,
  box-model rules, and direct string rendering.
- [`text/width.rs`](../urushi/src/text/width.rs) owns visible cell measurement.
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

Current components receive `ComponentStyles` and a display width explicitly.
They may perform component-specific layout such as summary label alignment, but
they do not resolve terminal capabilities or emit output.

### Renderers

- [`render/ansi.rs`](../urushi/src/render/ansi.rs) resolves each logical span for
  a `TerminalProfile` and serializes the `View`.
- [`render/ratatui/style.rs`](../urushi/src/render/ratatui/style.rs) converts the
  stylable subset and border colors to Ratatui types.
- [`render/ratatui/widget.rs`](../urushi/src/render/ratatui/widget.rs) draws the
  Urushi box model into a caller-provided Ratatui buffer.

The Ratatui integration is an adapter, not a TUI framework. It does not own
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

### External backends stay behind adapters

Ratatui and Indicatif types stay in their adapter modules. Public Urushi
surfaces use Urushi-owned concepts. This keeps backend replacement local and
prevents backend lifecycle rules from becoming application contracts.

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
9. Ratatui adapters write only to the buffer supplied by the caller and do not
   own a TUI runtime.
10. Prompt-specific state, cursor behavior, and terminal cleanup remain in
    `urushi-prompt`, not the core component model.

## Common change paths

### Add a reusable component

1. Add one concept-focused file under `urushi/src/component/`.
2. Accept semantic props, `ComponentStyles`, and explicit layout constraints as
   needed.
3. Return `View` without choosing an output writer or backend.
4. Test semantic edge cases and CJK width behavior at the component boundary.
5. Re-export the component from `component/mod.rs` and the crate root when it
   is part of the public API.

### Add or replace an output adapter

1. Put backend-specific types in a private module under `render/` or
   `terminal/`.
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

1. Keep style/color conversion in `render/ratatui/style.rs`.
2. Keep box-model buffer drawing in `render/ratatui/widget.rs`.
3. Preserve the caller's ownership of terminal setup, event processing, state,
   layout orchestration, and frame rendering.
4. Test narrow areas, CJK clipping, border colors, and terminal-profile
   degradation.

## Verification map

| Guarantee | Primary evidence |
| --- | --- |
| Box model, ANSI scopes, wrapping, alignment, and CJK width | [`urushi/tests/render.rs`](../urushi/tests/render.rs), [`urushi/tests/join.rs`](../urushi/tests/join.rs) |
| Terminal detection and style degradation | [`urushi/tests/terminal_profile.rs`](../urushi/tests/terminal_profile.rs), tests beside `terminal/profile.rs` |
| Theme roles resolve consistently for terminal and Ratatui output | [`urushi/tests/theme_terminal_render.rs`](../urushi/tests/theme_terminal_render.rs), [`urushi/tests/theme_ratatui_render.rs`](../urushi/tests/theme_ratatui_render.rs) |
| Progress messages share theme behavior across live and stable output | tests beside [`terminal/progress/mod.rs`](../urushi/src/terminal/progress/mod.rs) |
| Prompt submission, viewport behavior, and cleanup | tests beside [`urushi-prompt/src/runtime.rs`](../urushi-prompt/src/runtime.rs) |
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
- the Ratatui integration is limited to style conversion and box-model widget
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
