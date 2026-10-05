---
title: View and layout reference
description: Complete reference for View nodes, box geometry, layout resolution, projections, anchors, and results.
---

This page is the task-oriented index to the View API. Use the generated Rust
API reference for every signature, generic bound, trait implementation, and
source link:

- [Complete `urushi` API](https://docs.rs/urushi/latest/urushi/index.html)
- [`View`](https://docs.rs/urushi/latest/urushi/enum.View.html),
  [`BlockTitle`](https://docs.rs/urushi/latest/urushi/struct.BlockTitle.html), and
  [`GridStyle`](https://docs.rs/urushi/latest/urushi/struct.GridStyle.html)
- [`BlockStyle`](https://docs.rs/urushi/latest/urushi/struct.BlockStyle.html),
  [`Border`](https://docs.rs/urushi/latest/urushi/struct.Border.html),
  [`Length`](https://docs.rs/urushi/latest/urushi/enum.Length.html), and
  [`Overflow`](https://docs.rs/urushi/latest/urushi/enum.Overflow.html)
- [`Viewport`](https://docs.rs/urushi/latest/urushi/struct.Viewport.html),
  [`Key`](https://docs.rs/urushi/latest/urushi/struct.Key.html), and
  [`ResolvedView`](https://docs.rs/urushi/latest/urushi/struct.ResolvedView.html)
- [`Available`](https://docs.rs/urushi/latest/urushi/struct.Available.html),
  [`LayoutError`](https://docs.rs/urushi/latest/urushi/struct.LayoutError.html), and
  [`LayoutErrorKind`](https://docs.rs/urushi/latest/urushi/enum.LayoutErrorKind.html)

## Block geometry

`BlockStyle::new()` starts empty. `from_text_style` installs one complete text
style for block-created cells; `text_style()` returns it.

Color, attributes, underline, hyperlink, and their resets/getters are indexed
under [BlockStyle cell appearance](/docs/core/styles/reference/#blockstyle-cell-appearance).
They style cells created by the block and do not cascade into its child.

| Area | Builders |
|---|---|
| Spacing | `padding`, `margin` and their resets |
| Border | `border`, four edge toggles, border text style/foreground/background, and resets |
| Size | `width`, `height`, min/max width/height, and resets |
| Content | `overflow`, `align`, `vertical_align`, and resets |

Every field has a corresponding `get_*` accessor. `frame_size()` returns the
fixed margin, enabled border, and padding contribution, excluding content and
content-dependent dimensions.

Built-in `Border` repertoires are `NORMAL`, `ROUNDED`, `THICK`, `DOUBLE`,
`ASCII`, and `HIDDEN`. Its fields are public for custom repertoires.

## Dimensions and alignment

- `Length::Cells(u16)` is fixed.
- `Length::Fill(non-zero weight)` divides remaining parent space.
- `Length::fill` panics on zero; `try_fill` returns `InvalidFillWeight`.
- `Align` is `Left`, `Center`, or `Right`.
- `VerticalAlign` is `Top`, `Center`, or `Bottom`.
- `Sides` stores top, right, bottom, and left spacing and accepts the supported
  scalar and tuple conversions.

`Overflow` is `Wrap` (the default) or `Clip(marker)`. Constructors `clip`,
`ellipsis`, and `clip_with` select clipping markers; `clip_marker()` returns
the optional marker.

## Grid layout

`GridStyle::new()` uses content-sized columns and no cell padding. `columns`
sets optional `Length` values by position; `cell_padding` sets default cell
padding. Reset and getter methods are available for both settings.

## View constructors

| Constructor | Result |
|---|---|
| `View::text(text, style)` | Plain text flow with one complete `TextStyle` |
| `View::styled_text(text)` | One `StyledText` flow with multiple spans |
| `View::block(style, child)` | One child in a box model |
| `View::titled_block(style, title, child)` | Block with a one-line top-border title |
| `View::row(align, children)` | Horizontal children with vertical cross-axis alignment |
| `View::column(align, children)` | Vertical children with horizontal cross-axis alignment |
| `View::grid(style, rows)` | Rectangular grid with shared column widths |
| `View::canvas(canvas)` | Finite free-positioned surface |
| `View::viewport(viewport, child)` | Finite projection of one child |
| `View::anchor_block(key, style, child)` | Block that reports its content rectangle |
| `View::titled_anchor_block(key, style, title, child)` | Titled block that reports its content rectangle |
| `View::anchor(key)` | Zero-size reported origin |
| `View::empty()` | Empty rectangle |

`View::default()` is `View::empty()`.

Text accepts newlines and tabs but not terminal controls or raw ANSI. Every
grid row must have the same number of cells. A titled block requires an enabled
top border.

## Resolution

| API | Purpose |
|---|---|
| `measure(view)` | Intrinsic `Size`; panics on layout error |
| `try_measure(view)` | Intrinsic `Size` or `LayoutError` |
| `resolve(view, available)` | One `ResolvedView` under an available area |
| `Resolver::new()` | Reusable resolver with a layout cache |
| `Resolver::resolve(view, available)` | Resolve through that cache |
| `Resolver::clear()` | Clear reusable state |

Use the [two-frame Resolver guide](/docs/core/views/#retain-repeated-resolution-across-frames)
for the stateless-versus-retained choice, viewport reprojection, invalidation,
`clear`, and host-state ownership.

### `Available`

| Value | Meaning |
|---|---|
| `Available::NONE` | Both axes unbounded |
| `Available::new(width, height)` | Optional bound per axis |
| `Available::columns(width)` | Width bounded, height unbounded |
| `Available::size(width, height)` | Both axes bounded |

`width()` and `height()` return the optional bounds.

### `ResolvedView`

| API | Meaning |
|---|---|
| `size()` | Settled cell rectangle |
| `rows()` | Rows of `StyledGrapheme` values |
| `anchors()` | Every reported `AnchoredRect` |
| `anchor(key)` | One anchor by opaque key |

`StyledGrapheme` exposes `symbol`, `width`, and `style`.

## Projection

`Projection::new(origin, boundary)` stores a signed origin and either
`ProjectionBoundary::Preserve` or `Clamp`. Accessors are `origin()` and
`boundary()`.

`Viewport::horizontal`, `vertical`, and `both` select projected axes.
`horizontal_projection()` and `vertical_projection()` return their optional
definitions. Construction always selects at least one axis.

## Block titles

`BlockTitle::new(text)` creates a left-aligned, one-line title with one preferred
blank cell on each side. Builders are `align` and `padding`; accessors are
`text`, `alignment`, and `horizontal_padding`. `&str`, `String`, and
`StyledText` convert into a title.

## Anchored geometry

`AnchoredRect` exposes its `key`, signed `x` and `y`, `width`, `height`,
`is_empty`, `is_within_resolved_view`, and optional clipped `visible` rectangle.
`VisibleRect` exposes signed position and finite dimensions.

## Layout errors

`LayoutError::axis()` identifies the failing `Axis`; `kind()` identifies the
`LayoutErrorKind`. Missing Canvas extents and missing allocations for Views
placed in Canvas are reported rather than silently choosing a size.
