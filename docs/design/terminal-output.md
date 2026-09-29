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
semantics cannot be silently assigned to `None`. Passive inspection has no
input path on which to receive protocol replies. It therefore reports the
terminal's size with an empty capability set rather than interpreting `TERM`,
`COLORTERM`, or a terminal-family name. Failure to query the size of a handle
already identified as a terminal is an I/O error.

Capabilities describe only features positively confirmed by a bidirectional
terminal query or supplied by an explicitly configured backend. The native
backend queries Kitty graphics, primary device attributes for Sixel, and
XTGETTCAP's specified `Co` and `RGB` queries for color fidelity. A missing,
negative, malformed, or timed-out response does not enable the feature. Text
attributes, underline variants, underline color, and hyperlinks remain disabled
because the supported query protocols provide no portable positive answer for
them. The set remains feature-granular so an explicitly configured backend can
supply positive knowledge from its platform. Kitty and Sixel are independent
flags because a terminal may support both.
Environment preferences such as `NO_COLOR` are not capabilities.

A bidirectional `TerminalQuery` may also observe the terminal's optional RGB
background. That result is presentation input rather than a rendering
capability, so it is not stored in `TerminalInfo` or `TerminalCapabilities`.
Its one-shot query, error mapping, and use in explicit theme selection are
defined in [`terminal-background.md`](terminal-background.md).

## Selection and rendering

`RenderSettings` describes what one render operation selects. Its axes mirror
terminal capabilities, but it is a separate type because a caller may narrow a
detected maximum. `RenderSettings::default()` selects no escape-sequence
features. `RenderSettings::from(info.capabilities())` supports the common case,
and consuming property setters let the caller narrow individual axes.

The ordinary layout and rendering entry points are pure, stateless functions:

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

Repeated viewport projection may instead opt into the stateful `Resolver`
defined by [`resolution-reuse.md`](resolution-reuse.md). It has the same layout
inputs and output as free `resolve`; only private evaluation lifetime differs.
The standard-stream convenience path remains stateless and never constructs
one implicitly.

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

Interactive extension protocols do not use that raw-writer path. A
`CommandWriter` accepts validated APC and DCS payloads and its backend supplies
the ECMA-48 framing. An extension crate therefore owns protocol encoding while
the terminal connection remains the only owner of physical output. The exact
graphics use of this transport is defined in
[`terminal-graphics-boundary.md`](terminal-graphics-boundary.md).

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
