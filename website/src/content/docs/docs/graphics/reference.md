---
title: Graphics reference
description: Complete reference for image values, selection, rendering, and protocol lifecycle APIs.
---

This page indexes every public graphics API. Use the generated Rust API for
exact signatures, trait implementations, and source links:

- [Complete `urushi-graphics` API](/api/urushi_graphics/index.html)
- [`Image`](/api/urushi_graphics/struct.Image.html) and
  [`ImagePresentation`](/api/urushi_graphics/struct.ImagePresentation.html)
- [`GraphicsPreference`](/api/urushi_graphics/enum.GraphicsPreference.html) and
  [`GraphicsSelection`](/api/urushi_graphics/enum.GraphicsSelection.html)
- [`KittyLifecycle`](/api/urushi_graphics/kitty/struct.KittyLifecycle.html) and
  [`SixelLifecycle`](/api/urushi_graphics/sixel/struct.SixelLifecycle.html)

## Image data and placement

| Type | Public API |
|---|---|
| `PixelSize` | `new`, `width`, `height` |
| `PixelPosition` | `new`, `x`, `y` |
| `CellSize` | `ZERO`, `new`, `width`, `height` |
| `RgbaRaster` | `key`, `size`, `bytes` |
| `Image` | `rgba`, `fallback`, `key`, `raster`, `get_fallback`, `placement` |
| `ImagePresentation` | `new`, `from_theme`, `get_fallback_style`, `fallback_style`, `compose` |
| `GraphicPlacement` | `key`, `raster`, `logical_origin`, `logical_size`, `visible_origin`, `visible_size`, `cell_pixels`, `source_offset`, `source_size`, `visible_pixels` |
| Free function | `image_placements(view, resolved, cell_pixels)` |

`Image::rgba` returns `InvalidRgbaRaster::Empty`, `ByteLength`, or
`ByteLengthOverflow` when its source cannot be a non-empty `width × height × 4`
RGBA raster. Placement and asset keys are independent `urushi::Key` values.

## Defaults

| Value | Default behavior |
|---|---|
| `Image::rgba(...)` | Uses `[image]` as its text fallback until `.fallback(...)` replaces it |
| `ImagePresentation::new()` / `default()` | Uses a plain `TextStyle` for fallback cells |
| `GraphicsPreference::default()` | `Auto`: Kitty, then usable Sixel, then text |
| `KittyLifecycle::default()` / `SixelLifecycle::default()` | Same empty retained state as `new()` |

## Selection

| API | Meaning |
|---|---|
| `GraphicsPreference::Auto` | Prefer Kitty, then usable Sixel, then text |
| `GraphicsPreference::Kitty` | Require positively confirmed Kitty support |
| `GraphicsPreference::Sixel` | Require Sixel support and cell-pixel geometry |
| `GraphicsPreference::Text` | Always keep text fallback |
| `GraphicsSelection` | Selected `Kitty`, `Sixel`, or `Text` path |
| `select_graphics` | Resolve a preference from capabilities and optional cell-pixel size |
| `GraphicsUnavailable` | `UnsupportedProtocol` or `MissingCellPixelGeometry` |

## Stateless rendering

| API | Ownership contract |
|---|---|
| `render_view` | Query, resolve, write cells, select a protocol, and overlay all images once |
| `render_resolved_images` | Overlay caller-supplied images on a caller-resolved View using automatic selection |
| `kitty::render_kitty` | Emit one stateless Kitty presentation |
| `sixel::render_sixel` | Emit one stateless Sixel presentation |

`render_view` returns `Option<TerminalGraphicsProtocol>` because a successful
text-fallback presentation uses no graphics protocol.

## Retained and cached lifecycles

| Type | Public API | Contract |
|---|---|---|
| `kitty::KittyLifecycle` | `new`, `present`, `clear` | Reconcile retained uploads and placements; cleanup is retryable after partial failure |
| `sixel::SixelLifecycle` | `new`, `present`, `clear` | Cache encoding, emit every visible band per presentation, and clear the cell surface before host redraw |

Both lifecycle types implement `Default`. They own protocol state, not a
terminal connection, session, event loop, View resolution policy, or frame
scheduling.

## Runtime integration

With the `urushi-tui-app/graphics` Cargo feature, that crate re-exports
`GraphicsPreference` and adds `Runtime::graphics(preference)`. The application
must still depend on `urushi-graphics` to construct `Image` values. See
[Crates and features](/docs/reference/crates/#urushi-tui-app) for the dependency
contract.
