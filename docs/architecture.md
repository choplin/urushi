# Architecture

This document is the starting point for developers changing Urushi. It covers
the workspace and module responsibilities, the dependency direction between
them, the rendering flows, terminal ownership, and the invariants those
boundaries rest on.

Urushi spans plain CLI output, blocking interactive prompts, and full-screen
TUI applications. The [`README`](../README.md#concept) explains
why one library covers all three; this document explains how those surfaces
share a foundation without being forced into one rendering model or one
terminal lifecycle. Each surface's own design is a separate document:
[`cli-presentation.md`](cli-presentation.md) for opinionated non-interactive
CLI presentation,
[`inline-prompt-rendering.md`](inline-prompt-rendering.md) for the prompt's
rendering and terminal-surface choice, and
[`tui-architecture.md`](tui-architecture.md) for the full-screen application
runtime.

What follows states rules. The reasoning behind a rule, and the alternatives it
was chosen over, live under [`design/`](design/);
[`decision-log.md`](decision-log.md) is the history of the decisions those
documents record.

The exact terminal observation and output contract is recorded in
[`design/terminal-output.md`](design/terminal-output.md). Optional terminal
background observation and theme selection are recorded separately in
[`design/terminal-background.md`](design/terminal-background.md).

## Architecture at a glance

The root flow begins with a renderer-neutral `View`, resolves it into cells,
then serializes those cells for output.

```text
View + Available
  |
  | resolution
  | - resolve (direct, stateless)
  | - Resolver::resolve (optional retained evaluation)
  v
ResolvedView
  |
  | render(RenderSettings)
  v
String
  |
  | write
  v
std::io::Write
```

The core [`View`](../urushi/src/view/model.rs) is a component-agnostic layout
tree. Its built-in vocabulary is `Text`, `Block`, `Row`, `Column`, `Grid`,
`Canvas`, `Viewport`, and the keyed `AnchorBlock` form. Canvas carries one sizing mode and
ordered, comparable items that record renderer-neutral drawing commands after
its finite size is known. The tree carries logical
[`TextStyle`](../urushi/src/style/text.rs) and
[`BlockStyle`](../urushi/src/style/block.rs) values rather than
terminal-resolved ANSI strings. Component presentations stop at this boundary.
The direct [`resolve`](../urushi/src/view/resolve.rs) function turns the tree
into one `ResolvedView` rectangle of styled graphemes without retained
evaluation state. Callers that evaluate successive immutable View snapshots
may instead keep an optional `Resolver`, which produces the same rectangle
while reusing unchanged materialized subtree output, including settled
Viewport content. Its retained state never chooses layout; the current View and
available area remain the complete layout input.
The stateless
[`render`](../urushi/src/render/ansi.rs) function applies explicit
[`RenderSettings`](../urushi/src/render/settings.rs) and serializes the result.
A presentation either lowers component meaning into built-in nodes or
binds its component snapshot and policy into an intrinsically sized Canvas.
Neither `View` nor `resolve` can inspect whether a tree or item came from a
List, Table, Tree, or future Graph.

Terminal graphics use that same core without entering it. `urushi-graphics`
owns Image data and presentations, composes fallback cells inside generic
anchored regions, then pairs the resolved anchors with Image assets for Kitty
or Sixel output. [`terminal-graphics.md`](terminal-graphics.md) defines that
subsystem and links to its exact design contracts.

Primitive styling splits in two, and geometry belongs to only one half: a
`TextStyle` is everything a terminal can express about a run of text, and a
`BlockStyle` is a rectangle plus the style filling it.
[`component-model.md`](component-model.md) defines the semantic-to-primitive
composition boundary, [`view-model.md`](view-model.md) defines the primitive
tree and layout pass, and [`style-model.md`](style-model.md) defines the style
values themselves.

Static output is a composition of orthogonal stages:

```text
View + Available --> resolve --> ResolvedView
ResolvedView + RenderSettings --> render --> String --> std::io::Write
```

Source-preserving styled text has a deliberately separate path:

```text
StyledText + RenderSettings --> render_text --> String --> std::io::Write
```

This path performs no layout. Tabs and line boundaries reach the output as
authored; callers that need computable geometry compose a `View` and use the
resolved path above.

Composition happens in `View` before layout. Rendered strings do not re-enter
the layout model. The reasoning behind the view/block boundary is recorded in
[`design/view-block-model.md`](design/view-block-model.md).

The optional `urushi-adapter-ratatui` integration sits outside these semantic
types:

```text
Resolved TextStyle   --> RatatuiStyle
View + Rect          --> ViewWidget    --> caller-owned Ratatui Buffer
BlockStyle + text    --> RatatuiWidget --> ViewWidget's path
```

The adapter computes no geometry. A target `Rect` becomes `Available`,
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
- `urushi-terminal`, which defines workspace-independent style and geometry
  primitives, commands, events, queries, raw-mode control, and session
  restoration. Passive handle inspection observes terminal attachment and
  size; a bidirectional backend obtains rendering capabilities and an optional
  RGB background from terminal replies without choosing prompt or TUI policy.
  Core `urushi` re-exports the style primitives, so ordinary styling APIs do
  not expose the lower crate as a second vocabulary.

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
| Plain CLI output | Core `View`, `resolve`, `render`, and static-write helpers; [`urushi-cli`](cli-presentation.md) adds opinionated Summary and Warning presentation | The application owns its command workflow. Standard-stream convenience functions own only one static write. |
| Interactive prompt | `Form` / `Group`; typed `Input`, `Select`, and `Confirm`; synchronous validation; selectable inline or alternate-screen presentation; terminal session setup and cleanup | `urushi-prompt` owns the blocking prompt session and the resources it acquires. The application owns when the form runs and what submitted values mean. |
| Full-screen frame presentation | `urushi-tui`: a synchronous `Screen` and draw-scoped `Frame` over Urushi-owned cell buffers and diffing | The caller owns the loop and terminal session; `Screen` owns working and committed frame state and one output transaction. |
| Full-screen application | `urushi-tui-app`: the TEA application value and runtime above `urushi-tui` | The application runtime owns event delivery, frame scheduling, terminal entry and restoration. The application owns its model, update, and view. |
| Ratatui integration | `urushi-adapter-ratatui`: style conversion and widgets that resolve a `View` into a caller-owned Ratatui `Buffer` | The Ratatui application owns its loop, buffer, terminal, and session. |

The surfaces are intentionally partial. Sharing the foundation does not require
one surface to adopt another's application model or lifecycle, so the flows are
deliberately related but not identical:

```text
application semantics
        |
        v
SemanticTokens --> Theme --> ComponentTheme / logical styles
                         |            |
                         |            +---------------------------------------------+
                         v                                                          |
                      CliTheme                                                      |
                         |                                                          |
             +-----------+--------------------+-------------------------------------+
             |                                |                                     |
             v                                v                                     v
     plain CLI layer                  prompt layer                         Ratatui adapter
 data + Presentation / text      Form / Group / Field                     application view
             |                                |                                     |
             View                   View + prompt stages                BlockStyle / widget
             |                                |                                     |
 resolve / render / output      selected renderer + session       caller-owned Buffer
```

This split prevents visual consistency from turning into lifecycle coupling.
For example, a prompt and a Ratatui screen may resolve the same
`PromptOptionSelected` role and use the same CJK width rules, but the prompt
still owns validation and its selected terminal surface, while
`urushi-tui-app` owns event processing and frame rendering for a TEA
application. Likewise, plain
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
| [`urushi-derive`](../urushi-derive/) | Procedural derives for typed component data. | None within the workspace |
| [`urushi-terminal`](../urushi-terminal/) | Workspace-independent terminal contracts and inspection: style and geometry primitives, commands, dependency-free ANSI encoding, events, queries, session restoration, terminal/non-terminal classification, visible size, and capabilities. | None within the workspace |
| [`urushi`](../urushi/) | Logical styles, themes, renderer-neutral views and components, layout, static ANSI rendering policy, and standard-stream output convenience. | `urushi-derive`, `urushi-terminal` |
| [`urushi-cli`](../urushi-cli/) | Opinionated semantic summaries and warnings for human-facing, non-interactive CLI output. | `urushi` |
| [`urushi-graphics`](../urushi-graphics/) | Image data and presentations, resolved anchor-to-image placement, terminal graphics encoders, and reusable graphics lifecycle machinery. Retaining that state belongs to an opted-in host runtime. | `urushi`, `urushi-terminal` |
| [`urushi-prompt`](../urushi-prompt/) | Typed input, select, and confirm forms; prompt state transitions; inline and alternate-screen presentations; terminal session setup and cleanup. | `urushi`, `urushi-terminal` |
| [`urushi-tui`](../urushi-tui/) | Synchronous full-screen cell presentation: draw-scoped frames, Urushi-owned buffers and diffing, transactional output, and failed-output recovery. | `urushi`, `urushi-terminal` |
| `urushi-tui-app` | TEA-style full-screen applications: model ownership, delivery, effects, subscriptions, scheduling, rendering into `urushi-tui`, terminal input, session restoration, and optional graphics presentation. | `urushi`, `urushi-terminal`, `urushi-tui`; optionally `urushi-graphics` |
| `urushi-adapter-ratatui` | Optional Ratatui integration: logical-style conversion, stateless widgets, resolved-cell writing, and anchor translation for a caller-owned Ratatui buffer. | `urushi` |

The packages share one release version and are published in dependency order;
[`releasing.md`](releasing.md) describes the maintainer workflow.

`urushi-prompt` owns interactive prompt behavior. The core crate must not gain
prompt-specific navigation, validation, cursor, or form-submission policy merely
to share styling.

## Core module responsibilities

| Module | Responsibility | Internal dependencies |
| --- | --- | --- |
| [`style`](../urushi/src/style/) | Re-exports shared terminal style primitives and builds logical `TextStyle`, border glyphs, box spacing and alignment, and geometry-bearing `BlockStyle` from them. | `text`, `urushi-terminal` |
| [`text`](../urushi/src/text/) | Plain-text values, including grapheme-aligned `StyledText`, plus display-width measurement and cell-aware word/CJK wrapping. | `style` |
| [`theme`](../urushi/src/theme/) | Semantic color tokens, reusable component roles, canonical component presentations, application role resolution, and explicit light/dark selection. | `style`, `component` |
| [`view`](../urushi/src/view/) | The renderer-neutral `View` tree and the one layout pass in its three phases — width, height, and assembly — behind `measure`, direct `resolve`, and optional retained `Resolver::resolve` (`Size`, `Available`, `StyledGrapheme`, `ResolvedView`). Stateless Canvas assembly rasterizes and immediately composes one command at a time after sizing; a retained resolver may privately reuse equivalent evaluation artifacts. | `style`, `text` |
| [`component`](../urushi/src/component/) | Reusable semantic data and the independent concrete presentations that compose it into primitive `View` trees. | `theme`, `view`, `text` |
| [`render`](../urushi/src/render/) | Feature selection, run coalescing and scope ordering, and translation of a `ResolvedView` to ANSI text through `urushi-terminal`'s encoder. | `style`, `view`, `urushi-terminal` |
| [`output`](../urushi/src/output.rs) | Standard-stream convenience: detection, width selection, rendering policy, and one static write. | `view`, `render`, `urushi-terminal` |

## TUI crate responsibilities

| Crate | Responsibility | Internal dependencies |
| --- | --- | --- |
| `urushi-tui` | Defines `Screen` and draw-scoped `Frame`; owns cell storage, wide-grapheme ownership, committed and working buffers, diffing, output commit, and recovery. | `urushi`, `urushi-terminal` |
| `urushi-tui-app` | Exposes the application, effect, subscription, admission, and blocking runtime values; owns source execution, delivery ordering, frame scheduling, view resolution, terminal input, and session restoration. | `urushi`, `urushi-terminal`, `urushi-tui`, Tokio |
| `urushi-adapter-ratatui` | Converts logical styles and resolved views to Ratatui cells and provides stateless widgets for caller-owned Ratatui loops. | `urushi`, Ratatui |

The reason these are package boundaries rather than Cargo features is that they
serve three independent consumers. A caller-owned full-screen loop needs the
synchronous frame engine without an async executor; a TEA application needs
the application runtime; and a Ratatui application needs only the foreign
buffer adapter. The detailed ownership and rejected combined forms are defined
in [`design/tui-crate-boundaries.md`](design/tui-crate-boundaries.md).

The dependency direction runs from I/O and adapters toward semantic modules:

- `style` and `text` do not depend on themes, components, renderers, terminal
  lifecycle, or a backend adapter; style uses only backend-independent
  primitives from `urushi-terminal`;
- `theme` assigns semantic meaning to styles but does not inspect a terminal;
- `view` does not choose a renderer or own terminal state;
- renderers translate Urushi values into a backend representation and do not
  own application workflows;
- `output` owns one static standard-stream write, while prompt and
  `urushi-tui-app` own the interactive terminal resources and redraw
  lifecycles they acquire;
- prompt and application code compose these capabilities at their own entry
  points.

`theme` and `component` reference each other because `theme` stores
canonical component presentations and those presentations derive their
logical styles from the theme. Terminal inspection and lifecycle remain below
rendering, so neither capability detection nor standard handles enter the
style and view model; backend-independent colors, attributes, and underlines
are shared values rather than converted copies.

Width and wrapping policy must remain shared. A component or renderer should
not introduce a private definition of CJK display width.

Themes describe meaning. They do not detect `NO_COLOR`, inspect TTY state, emit
ANSI, or retain an output writer. When presentation should adapt to the
terminal background, the caller queries its connection, resolves an explicit
`ThemeMode`, constructs the application or form with that stable choice, and
passes the same connection to the interactive surface.

Reusable core components separate owned semantic data — `List`, `Tree`, and
`Table` — from concrete presentations that compose it into a primitive `View`.
[`urushi-cli`](cli-presentation.md) applies the same contract to its `Summary`
and `Warning` data and presentations without making their CLI visual language
part of core. `urushi-graphics` applies it to Image without making image or
terminal-protocol APIs part of core. The general contract is defined in
[`component-model.md`](component-model.md).

## Core contracts

### TextStyle remains logical until an output boundary

`Theme` and `View` retain logical `TextStyle` values. `RenderSettings` selects
the color level, text attribute set, underline styles and color, and hyperlink support
at an output boundary. It is explicit input to `render` and defaults to dumb
plain output.

The prompt's view is the one deliberate exception: its runs carry
settings-resolved styles from the moment the view is built, because rows there
are compared for equality to decide whether to redraw — see
[`design/style-canonical-form.md`](design/style-canonical-form.md). This does
not relax the contract for `Theme` or `View`.

The immutable effective-value model and its typed, named operations are specified in
[`style-model.md`](style-model.md).

### Presentations stop at View

A reusable component owns semantic data and normalization. A concrete
presentation owns one structural interpretation and composes a `View` without
receiving `Available`; it may also borrow component-specific immutable input
describing the current frame. It either constructs built-in nodes or binds an
owned frame into a Canvas item and supplies one Canvas-wide intrinsic sizing
value. The application owns the frame input's transitions, and `resolve` alone
decides area-dependent geometry before asking Canvas items to draw. Neither
layer may write to a terminal, choose a live-output mode, own execution state,
or depend on Ratatui. The boundary and sizing invariants are defined in
[`design/canvas.md`](design/canvas.md).

Application-specific workflow chrome remains in the application: an application
composes its own command start and finish lines rather than Urushi growing
generic `Intro` and `Outro` components.

Execution-linked indicators follow the same ownership rule. The host that owns
the running operation also owns its progress state, animation timing, redraw
scheduling, and terminal cleanup. The core crate does not create a progress
thread, timer, or live terminal region of its own.

### Output resources have explicit owners

`urushi-terminal::detect` is the observation part of the lower terminal layer.
It observes one supplied handle and distinguishes a
non-terminal from a terminal whose capability set is empty; a terminal size
query failure is an error. `print_view` and `eprint_view` inspect their own
target and use the detected width. Redirected View output uses unbounded layout
and dumb settings; `print` and `eprint` write `StyledText` without layout.

Non-TTY output and passive output-handle inspection use append-only plain
output. Machine-readable stdout remains separate from human-facing diagnostics
on stderr. Interactive backends can enable richer output only from confirmed
terminal query responses. A background query likewise requires a bidirectional
connection to the actual destination; passive inspection and static
standard-stream helpers never infer a theme.

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

### Physical terminals, frame presentation, and adapters stay distinct

The backend-independent command, event, query, raw-mode, session-restoration,
and terminal-geometry contracts live in `urushi-terminal`. It contains no
frame, buffer, cell-diff, or presentation model. `urushi-tui` owns `Screen`,
`Frame`, its cells and rectangles, and the working and committed buffers. A
`Screen` computes its own diff and lowers changed cells to
`urushi_terminal::Command` values. `TerminalBackend` describes one interactive
connection that owns input, output, process modes, and queries; its component
traits remain usable independently for tests and non-interactive output.
Queries take mutable access because an implementation may have to write a
request and consume its response from the same connection.

`backend::ansi::AnsiWriter` owns dependency-free SGR and OSC 8 byte spelling as
well as the remaining output protocol. Static rendering uses its scoped style
and hyperlink operations, while `AnsiWriter` command output uses the same
implementation and the Crossterm adapter delegates OSC 8 fallback encoding to
it. `backend::native::NativeTerminal` owns `/dev/tty`, Unix raw mode, window
queries, input decoding, and that encoder as one concrete interactive
connection. Crossterm remains an optional cross-platform adapter, and its types
stay inside `backend::crossterm`. The crate root exposes only Urushi-owned
contracts and values.
`RenderSettings` selects output features before data reaches an adapter.
Backend replacement therefore remains local, and backend lifecycle rules do
not become core application contracts.

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

Prompt renderers hold presentation state but receive the connection that
performs each draw; they do not own a second physical writer.

The inverse mistake is also forbidden: the Urushi contract does not reproduce
a backend library command-for-command. It models coherent terminal domains and
preserves the information within each owned domain. Adapters lower those
semantic operations to their library or platform primitives. Current call-site
counts neither define nor narrow this contract.

## Architectural invariants

Changes must preserve these invariants unless the architecture itself is being
changed deliberately and this document is updated in the same change:

1. `style` and `text` remain independent from I/O and external rendering
   backends.
2. Themes contain semantic choices but no terminal detection or writer state.
3. A reusable component owns semantic data; only a concrete presentation
   composes that data and any borrowed current-frame input into a primitive
   `View`, and neither performs output or owns state transitions.
4. `render` consumes only a `ResolvedView`, `render_text` consumes only a
   `StyledText`, and both require explicit `RenderSettings`.
5. Display width and wrapping use the shared `text` implementation; rendered
   strings are not a layout input.
6. Workspace-independent terminal contracts and target-specific inspection are
   centralized in `urushi-terminal`.
7. `urushi-tui` owns backend-independent full-screen frame state and diffing;
   `urushi-tui-app` owns terminal lifecycle, event delivery, and frame
   scheduling; `urushi-adapter-ratatui` writes only to the Ratatui buffer its
   caller supplies.
8. Prompt-specific state, cursor behavior, and terminal cleanup remain in
    `urushi-prompt`, not the core component model.
