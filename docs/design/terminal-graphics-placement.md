# How Is a Clipped Image Represented for Every Graphics Protocol?

This document defines how an Image and its resolved anchor become one
protocol-neutral `GraphicPlacement`, including viewport clipping and the
mapping from cell geometry to raster pixels.

## Decision

A placement retains four related geometries:

| Geometry | Meaning |
| --- | --- |
| Logical rectangle | The complete signed anchor rectangle in resolved-view cells, before clipping |
| Visible rectangle | The accumulated intersection that survived every enclosing clip |
| Source rectangle | The portion of the prepared raster corresponding to the visible rectangle |
| Cell-pixel geometry | The observed pixel width and height of one terminal cell, when available |

The logical and visible rectangles remain distinct. Scrolling may move the
logical origin outside the resolved view, but that displacement is still needed
to identify which part of the raster appears in the visible intersection.
When present, the visible rectangle is the true intersection of the logical
rectangle, every enclosing clip, and the complete `ResolvedView` bounds. It is
therefore contained by both the logical rectangle and the resolved view; it is
never translated onto an edge to make an outside rectangle appear visible.

## Deriving source pixels

For one axis, let:

- `L0` be the signed logical leading edge in resolved-view cells;
- `Ln` be the non-zero logical extent in cells;
- `V0` be the visible leading edge in resolved-view cells;
- `Vn` be the visible extent in cells; and
- `R` be the prepared raster extent in source pixels.

The visible interval within the logical rectangle is `D0 = V0 - L0` through
`D1 = D0 + Vn`, both measured in cells. Its raster-pixel bounds are:

```text
S0 = floor(R * D0 / Ln)
S1 = min(R, ceil(R * D1 / Ln))
```

`S0` is the source-pixel offset and `S1 - S0` is the source-pixel extent. The
same equations apply independently to the horizontal and vertical axes. The
leading edge rounds down and the trailing edge rounds up so the crop includes
every source pixel touched by the visible cell interval, including when `R`
does not divide evenly by `Ln`.

```text
logical cells:  [------------- complete image -------------]
visible cells:              [--------- clip ---------]
source raster:  [==========================================]
source crop:                 [========================]
```

An absent visible intersection or an empty logical/visible rectangle has no
source rectangle and produces no graphics command. The derivation first checks
the signed subtraction and interval addition. It performs `R * D0` and
`R * D1` in widened integer arithmetic, clamps the trailing edge to `R`, and
checks every conversion back to the public pixel type. Failure to represent any
step yields no derived source rectangle rather than wrapping.

Cell-pixel geometry is optional in the common placement. It is required to
derive the visible output pixel extent and to resample Sixel, but Kitty can crop
the prepared raster and size the placement in cells without it. Making the
observation optional keeps a missing terminal measurement from becoming a
protocol-specific illegal state in the shared scene.

For a reported non-zero cell-pixel extent `C`, the complete logical target
extent is `Tn = Ln * C` pixels. The visible interval begins at
`T0 = D0 * C` in that logical target grid and spans `Tv = Vn * C` output
pixels. `Image` construction already guarantees that `R` is non-zero, and a
visible placement guarantees that `Ln` is non-zero, so `Tn` cannot be zero.

The three target-grid multiplications are checked. Sixel iterates the visible
interval by adding each local band coordinate to `T0`; every addition is
checked instead of constructing a possibly overflowing `T0 + Tv` bound. For
each target coordinate `t`, it samples source-raster coordinate
`floor(t * R / Tn)`. The product `t * R` uses widened integer arithmetic, and
the result's conversion back to a raster index is checked. Any failed target
extent, offset, addition, or index conversion rejects the Sixel operation
instead of wrapping. This target-grid offset is not the `S0` raster crop used
by Kitty.

## Coordinate origin and multiple placements

Logical and visible cell coordinates are local to the complete `ResolvedView`.
The direct adapters map its zero-based origin directly to the terminal's
zero-based cell origin; `render_view` likewise renders at the terminal's
top-left cell and resolves against the observed terminal size. A caller using a
direct adapter must supply a `ResolvedView` already bounded to the terminal
surface it will occupy. Translating or clipping a resolved scene into a
different host surface is a separate integration contract and is not
represented by `GraphicPlacement`.

Placement identities must be unique in one resolved View because the generic
anchor map is keyed uniquely. The current graphics contract does not define a
portable z-order or blending rule for overlapping placements. Callers must not
depend on overlap ordering across Kitty and Sixel until that separate design
question is settled.

## Identity is separate from geometry

The Image placement key names one semantic occurrence in the View. The raster
asset key names immutable prepared pixel content that placements may share.
Moving or clipping an occurrence changes its geometry without changing either
identity. Replacing its prepared content changes the asset identity without
requiring a new placement identity.

This separation lets Kitty reuse an upload while a viewport changes its crop,
and lets Sixel reuse encoded bands from the same asset slice. Neither protocol
identifier becomes part of `GraphicPlacement`.

## Protocol consumption

Kitty transmits the complete RGBA asset and addresses the placement with the
source rectangle, visible cell size, and visible terminal position. Its
retained reconciliation is specified in
[`terminal-graphics-kitty-lifecycle.md`](terminal-graphics-kitty-lifecycle.md).

Sixel scales from the complete logical target pixel grid and emits only the
bands in the visible rectangle. Its target-grid coordinates therefore remain
aligned across adjacent viewport positions. Its redraw and cache rules are specified in
[`terminal-graphics-sixel-lifecycle.md`](terminal-graphics-sixel-lifecycle.md).

## Rejected alternatives

### Keep only the visible rectangle

Once the logical origin is discarded, the renderer cannot derive which source
pixels a viewport exposed. It would have to ask the application to track a
source offset, duplicating layout state outside resolution.

### Publish separate Kitty and Sixel placement types

Both protocols consume the same resolved Image occurrence. Splitting the
public scene would permit their clipping and identity rules to diverge and
would expose protocol choice above the graphics adapter.

### Store protocol state in the placement

Upload IDs, terminal placement IDs, encoded cache entries, and recovery flags
belong to renderer-owned lifecycle values. The placement is an immutable
description of the desired scene, not committed terminal state.
