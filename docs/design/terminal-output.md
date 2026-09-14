# Terminal Output

How terminal observation, layout, rendering, and writing remain orthogonal while
the ordinary stdout and stderr calls stay convenient.

## Foundation

`urushi-terminal` owns observation of one supplied output handle:

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
and consuming `with_*` methods let the caller narrow individual axes.

Layout and rendering are pure, stateless functions:

```rust
resolve(&view, available) -> Result<ResolvedView, LayoutError>
render(&resolved, &settings) -> String
```

`Available` is only layout input. `RenderSettings` is only serialization input.
Neither function detects a terminal or writes bytes, and rendering never
recomputes geometry.

## Standard-stream convenience

`print`, `println`, `eprint`, and `eprintln` inspect the exact standard stream
they write. A terminal supplies a width-only `Available` and settings derived
from its capabilities; `NO_COLOR` narrows only the color level. A non-terminal
uses `Available::NONE` and dumb settings, producing an unbounded plain dump.
The functions do not flush static output automatically.

An arbitrary writer does not require a terminal-named wrapper. The caller uses
`urushi_terminal::detect` when the writer is inspectable, chooses `Available`
and `RenderSettings`, then combines `resolve`, `render`, and `std::io::Write`.
That lower path supports both redirected-output intents: an unbounded plain dump
or output deliberately rendered as if it targeted a known terminal.

## Boundaries

Terminal inspection stays below `urushi`, so prompt and TUI crates may use it
directly without routing through the core renderer. Renderer types do not own
writers or detected terminal state. Rendered strings do not carry geometry and
cannot be joined after rendering; composition is represented in `View` before
layout.
