# Terminal Graphics

Terminal graphics are an optional rendering layer over Urushi's ordinary
renderer-neutral View. An application composes an `Image` through
`ImagePresentation`; core layout reserves and clips its region with the same
Canvas and anchor machinery used by other extensions. After resolution,
`urushi-graphics` joins that geometry to the prepared raster and translates the
result for Kitty or Sixel. Core never acquires an image node or a graphics
protocol state.

```text
Image + ImagePresentation
          |
          | compose fallback cells and a generic anchor
          v
         View --------------------------+
          |                              |
          | resolve                      | recover immutable Image snapshots
          v                              |
     ResolvedView                        |
          |                              |
          +----------+-------------------+
                     |
                     | pair by placement identity
                     v
              GraphicPlacement
              - logical cell rectangle
              - visible cell intersection
              - source raster crop
              - observed cell-pixel size
                     |
              +------+------+
              |             |
              v             v
            Kitty         Sixel
```

## The placement is the shared scene

`GraphicPlacement` is the protocol-neutral hinge between layout and graphics.
It keeps the complete signed logical rectangle even when a viewport clips the
image, and separately keeps the visible intersection. From those two cell
rectangles and the prepared raster it derives the source-pixel crop. When the
terminal reports cell-pixel dimensions, the same placement also describes the
visible output extent in pixels.

Kitty and Sixel consume this one placement rather than defining separate public
scenes. Protocol identifiers and source offsets do not enter the application
model, while core layout remains unaware that an anchor will later carry an
image. The exact geometry and clipping rules are defined in
[`design/terminal-graphics-placement.md`](design/terminal-graphics-placement.md).

## Rendering paths

`render_view` is the ordinary one-shot path. It resolves and writes the cell
view, discovers Image snapshots embedded in that View, and selects a positively
confirmed graphics protocol. Kitty is preferred; Sixel is used when it is
confirmed and cell-pixel geometry is available. When neither can be used, the
already-rendered fallback cells remain visible.

Interactive hosts may retain protocol-specific lifecycle values:

- `KittyLifecycle` retains terminal uploads and placement identities. A clipped
  or scrolled image updates its crop and position without retransmitting RGBA.
- `SixelLifecycle` treats terminal pixels as immediate-mode output. It emits the
  complete visible image scene each time, but reuses encoded cell-row bands.

`urushi-graphics` defines these lifecycle types and their state machines; the
renderer that survives successive frames owns each runtime instance. A
lifecycle owns no application model, terminal connection, scheduling, or session.
The host is responsible for ordering cell output and graphics output as one
presentation. Kitty's retained-state rules are defined in
[`design/terminal-graphics-kitty-lifecycle.md`](design/terminal-graphics-kitty-lifecycle.md);
Sixel's redraw and cache rules are defined in
[`design/terminal-graphics-sixel-lifecycle.md`](design/terminal-graphics-sixel-lifecycle.md).

## Ownership boundaries

`urushi-graphics` owns Image data, presentation, placement lookup, protocol
encoding, and reusable graphics lifecycle machinery. Core `urushi` owns generic
View composition, anchors, clipping, and resolution. `urushi-terminal` owns
terminal observations, validated commands, APC/DCS framing, and physical
output. A prompt or TUI host owns any retained graphics lifecycle and its
coordination with cell frames.

This dependency direction keeps applications that do not use images free of
image data and protocol machinery. The precise package and transport decision
is recorded in
[`design/terminal-graphics-boundary.md`](design/terminal-graphics-boundary.md).
