# Terminal Output

How terminal observation, layout, rendering, and writing remain orthogonal while
the ordinary stdout and stderr calls stay convenient.

## Observation within the terminal foundation

This design topic covers the observation part of `urushi-terminal`; it does not
limit the crate's wider ownership of backend-independent terminal contracts.
`urushi-terminal` observes one supplied output handle:

```rust
pub enum TerminalDetection {
    Terminal(TerminalInfo),
    NonTerminal,
}

pub struct TerminalInfo {
    size: TerminalSize,
    capabilities: TerminalCapabilities,
}
```

`NonTerminal` is explicit rather than an absent `TerminalInfo`, so redirect
semantics cannot be silently assigned to `None`. `TERM=dumb` remains a terminal:
its size is available for layout, while its capability set is empty. Failure to
query the size of a handle already identified as a terminal is an I/O error.

Capabilities describe the features the detector can safely establish for the
terminal. Unknown features remain disabled, so the reported set is a
conservative safe set for automatic output rather than an optimistic list of
escape sequences. The set is feature-granular: color fidelity, text attributes,
underline shapes, underline color, and hyperlinks are independent axes.
Environment preferences such as `NO_COLOR` are not capabilities.

## Selection and rendering

`RenderSettings` describes what one render operation selects. Its axes mirror
terminal capabilities, but it is a separate type because a caller may narrow a
detected maximum. `RenderSettings::default()` selects no escape-sequence
features. `RenderSettings::from(info.capabilities())` supports the common case,
and consuming property setters let the caller narrow individual axes.

Layout and rendering are pure, stateless functions:

```rust
resolve(&view, available) -> Result<ResolvedView, LayoutError>
render(&resolved, &settings) -> String
render_text(&styled_text, &settings) -> String
```

`Available` is only layout input. `RenderSettings` is only serialization input.
None of these functions detects a terminal or writes bytes, and rendering never
recomputes geometry. `render_text` deliberately bypasses geometry: it preserves
tabs and line boundaries in the source, while `render` accepts only a rectangle
whose tabs have already been replaced during resolution.

## Standard-stream convenience

`print`, `println`, `eprint`, and `eprintln` inspect the exact standard stream
they write and serialize `StyledText` without resolving layout. Tabs and source
line boundaries therefore retain their ordinary terminal-owned behavior.

`print_view`, `println_view`, `eprint_view`, and `eprintln_view` perform the
same exact-stream detection and feature selection for `View`, then resolve it.
A terminal supplies a width-only `Available`; a non-terminal uses
`Available::NONE`, producing an unbounded plain dump. Naming the layout-bearing
operation keeps width-dependent resolution visible at the call site instead of
hiding the semantic difference behind an argument type. `NO_COLOR` narrows only
the color level in both paths. The functions do not flush static output
automatically.

An arbitrary writer does not require a terminal-named wrapper. The caller uses
`urushi_terminal::detect` when the writer is inspectable, chooses `Available`
and `RenderSettings`, then combines `resolve`, `render`, and `std::io::Write`.
That lower path supports both redirected-output intents: an unbounded plain dump
or output deliberately rendered as if it targeted a known terminal.

## Boundaries

All `urushi-terminal` contracts stay below `urushi` and have no workspace
dependencies. Terminal inspection therefore remains directly usable by prompt
and TUI crates without routing through the core renderer. Renderer types do not
own writers or detected terminal state; an interactive surface passes its one
terminal connection into each render operation. Rendered strings do not carry
geometry and cannot be joined after rendering; composition is represented in `View`
before layout. The TUI frame and terminal contracts and the shared session
guard are specified in
[`tui-terminal-ownership.md`](tui-terminal-ownership.md).
