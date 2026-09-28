# Terminal Graphics Boundary

This document answers where image components and terminal graphics protocols
live, and how they use Urushi layout without making images part of core.

## Decision

The separate `urushi-graphics` crate owns the complete image-specific surface:

- `Image`, checked prepared RGBA, pixel and cell dimensions, fallback text,
  and independent asset and placement identities;
- `ImagePresentation` and the conversion from image meaning into an Urushi
  `View`;
- lookup of resolved image placements; and
- Kitty, Sixel, backend selection, and reusable protocol-specific lifecycle
  machinery.

Core `urushi` owns none of those types and has no graphics command or image
variant. It supplies the existing generic anchored-region contract. An
`ImagePresentation` composes a fixed rectangle whose internal Canvas owns an
immutable image snapshot and records an empty `AnchorBlock` plus clipped
fallback text. The fallback's minimum content width therefore cannot expand the
requested cell size. Consequently, ordinary cell renderers display the fallback
without knowing an image exists, while the graphics adapter can recover its own
Canvas item type without adding a graphics attachment or Image variant to core.

After the complete surrounding `View` resolves, `urushi-graphics` matches every
owned Image placement identity to the corresponding `AnchoredRect`. That
rectangle is the renderer-neutral placement used by a graphics backend. An
ordinary caller passes only the composed View and terminal connection to
`render_view`; it does not retain a parallel image list, resolve the View, look
up anchors, inspect capabilities, construct protocol commands, or access the
physical backend itself.

The simple graphics adapters are stateless. For every non-empty image whose
complete anchor survived every enclosing resolved rectangle, Kitty transmits
the full 32-bit RGBA asset and displays it at that cell rectangle. Sixel first
resamples that raster to the placement's pixel extent using terminal-reported
cell geometry, then encodes and displays it. Neither adapter caches uploads nor
deletes old placements.

The adapter accepts `urushi_terminal::CommandWriter`, not a physical backend or
`std::io::Write`. It owns Kitty parameters and Base64 chunking, then submits
validated APC payloads; `urushi-terminal` owns cursor commands, control-string
framing, flushing, and physical output. Sixel uses the corresponding generic
DCS transport. Neither protocol becomes a terminal-foundation image type.

`render_view` is the ordinary one-shot entry point. It queries geometry and
confirmed capabilities, resolves and writes the complete View, then prefers
Kitty, followed by Sixel when cell-pixel geometry is also available. The
already-written fallback cells remain the final representation when neither
protocol can be used. `render_resolved_images` is the low-level overlay path for
a host that already owns resolution, capability observation, and cell output.
Direct protocol adapters remain public for callers that have selected one.

## Why this is a separate crate

An `image` module inside core would still make core publish image data and
presentation contracts. Keeping the package separate enforces the intended
dependency direction: `urushi-graphics` depends on `urushi` and the generic
`urushi-terminal` command vocabulary, while core remains usable without image
data, Base64, Kitty, Sixel, or a graphics lifecycle.

Kitty and Sixel are alternate encodings of the same resolved image placement,
not TUI concepts. They therefore share `urushi-graphics`; neither belongs to
the Ratatui adapter and runtime in `urushi-tui`. Terminal observation and the
capability vocabulary remain in `urushi-terminal`. `TerminalCapabilities`
reports protocols independently through `supports_graphics` only after the
terminal answers the corresponding Kitty or primary-device-attributes query;
`TERM` and terminal-family names are not evidence. The graphics layer decides
which confirmed encoder to use.

## Why core anchors are sufficient

An anchor already reserves a generic region, follows every layout offset, and
reports signed geometry without knowing what an external adapter draws there.
Adding a second graphics-specific placement channel to `View` or `ResolvedView`
would duplicate that contract and make core own an image extension point.

The Image component hides the association: its placement key is both semantic
Image data and the Anchor key emitted by its presentation. Consumers compose a
normal `View`; only `urushi-graphics` inspects its own Canvas item and performs
the key lookup after resolution.

## Deferred lifecycle

Upload reuse, deletion, movement, partial source rectangles, protocol-specific
repaint, and failure recovery require knowledge of prior frames and the selected
terminal. Reusable protocol state machines belong in `urushi-graphics`, but the
host that owns a redraw lifecycle owns their state: `urushi-tui` for a TUI and
`urushi-prompt` for a prompt. Each integration is optional behind that host's
graphics feature. One-shot callers retain no state, and core `urushi`,
`ImagePresentation`, `View`, and `ResolvedView` remain independent of the
lifecycle.
