# Why Does Terminal Graphics Live Outside Core?

This document answers why Image components, resolved image placement, and
Kitty/Sixel adapters live in `urushi-graphics` instead of core `urushi`, the
terminal foundation, or an application runtime.

## Decision

The separate `urushi-graphics` crate owns the complete image-specific surface:

- `Image`, checked prepared RGBA, source-raster dimensions, requested cell
  dimensions, fallback text, and independent asset and placement identities;
- `ImagePresentation`, which lowers image meaning into an ordinary View;
- the association between immutable Image snapshots and resolved anchors; and
- Kitty and Sixel encoding plus the types and state machines for reusable
  protocol-specific lifecycles.

Core `urushi` owns generic composition and geometry only. It supplies Canvas,
`AnchorBlock`, `AnchoredRect`, clipping, and resolution without an Image variant
or graphics attachment. `urushi-terminal` owns terminal geometry and capability
observations, validated commands, APC/DCS framing, flushing, and physical
output; it does not own raster data or choose a graphics protocol.

## How the boundary works

`ImagePresentation` composes a fixed rectangle. Its Canvas item retains an
immutable Image snapshot, while an empty generic anchor records the rectangle
and clipped fallback text occupies the ordinary cell scene. After the complete
surrounding View resolves, `urushi-graphics` recovers its Image snapshots and
matches each placement identity to the corresponding `AnchoredRect`.
An Image snapshot with no matching anchor yields no graphics placement. A
generic anchor with no matching Image remains ordinary core geometry and is
ignored by graphics. `ImagePresentation` creates the supported one-to-one pair;
duplicate anchor keys violate core's rule that one key names one region.

`image_placements` exposes that same discovery and pairing operation to a host
that owns cell output. The host can therefore derive its graphics cell layer
from the visible placement rectangles without learning the private Canvas item
type or putting image meaning into core. In particular, the TUI host replaces
fallback glyphs inside those rectangles with background-only blanks when a
graphics protocol is selected.

The public one-shot `render_view` operation owns that orchestration for ordinary
callers: it queries geometry and confirmed capabilities, resolves and writes
the cell View, then prefers Kitty over Sixel. The lower-level overlay and direct
protocol adapters remain available to hosts that already own resolution,
capability observation, and cell output.

Both adapters accept `urushi_terminal::CommandWriter`, not a physical backend
or raw `std::io::Write`. The graphics crate constructs validated protocol
payloads; the terminal crate frames and writes them. This preserves one command
and connection boundary without making the terminal foundation image-aware.

## Why generic anchors are sufficient

An anchor already reserves a region, follows every layout offset, retains
signed logical geometry, and reports the intersection that survived enclosing
clips. Graphics needs exactly that layout result. A second graphics-specific
channel through View or `ResolvedView` would duplicate geometry and make core
own an extension it does not need to interpret.

The placement key connects the semantic Image snapshot to its generic anchor.
Only `urushi-graphics` knows that the association exists. The precise placement
derived from that association is defined in
[`terminal-graphics-placement.md`](terminal-graphics-placement.md).

## Rejected alternatives

### Put Image in core View

An Image node or graphics attachment would make core publish raster and
protocol-adjacent contracts even for consumers that use only text layout. The
generic Canvas and anchor extension points already carry the required meaning.

### Put encoders in `urushi-terminal`

The terminal foundation owns safe transport, not presentation meaning. Owning
prepared raster data, Image identity, or protocol-selection policy there would
couple workspace-independent terminal control to an optional component system.

### Put source offsets or protocol identifiers in the application model

Those values are derived rendering state. Storing them in application data
would split one image's meaning across the model and renderer, and would make
the model depend on the selected terminal protocol.

### Put graphics state in every runtime

Prompts and TUIs have different session and frame lifecycles. A host that opts
into retained graphics owns a runtime instance of a protocol lifecycle supplied
by `urushi-graphics`; hosts that do not opt in retain no graphics state.
