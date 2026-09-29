# How Does Sixel Redraw and Reuse Encoded Image Data?

This document defines how `SixelLifecycle` presents changing image scenes
without assuming portable persistent placement, while reusing bounded encoded
raster work.

## Decision

Sixel placement is immediate-mode terminal output. Every presentation emits
every visible image band in the current desired scene. The lifecycle never
assumes that a previously drawn Sixel image can be moved or partially deleted
through a portable terminal-side identity.

When a changed scene could leave old pixels behind, the host performs this
ordering:

1. clear the terminal surface;
2. redraw the complete cell frame; and
3. present every visible Sixel band.

`SixelLifecycle::clear` issues a full-screen terminal clear, moves the cursor to
the terminal's top-left cell, and flushes. It removes the current cell frame as
well as graphics pixels, but deliberately preserves encoded cache data. The
host therefore follows it with a complete cell and graphics redraw. The
lifecycle supplies only this protocol primitive: it neither compares cell
frames nor reports that a clear is needed. The host knows when removal, scroll,
resize, or another scene change can expose stale pixels and invokes `clear`
before that complete redraw.

## Cell-row bands

One visible terminal cell row becomes one Sixel band. Each band is sampled from
the complete logical target pixel grid. Its horizontal and vertical offsets are
the visible cell offsets within the logical rectangle multiplied by the
observed cell-pixel dimensions. They are target-grid pixels, not coordinates in
the prepared source raster. The sampling rule in
[`terminal-graphics-placement.md`](terminal-graphics-placement.md) maps each
target-grid pixel to a raster pixel. Adjacent vertical viewport positions
therefore name the same overlapping bands even though their terminal rows
change.

The encoded cache key contains every input that changes the result:

- raster asset identity;
- logical target width and height in pixels;
- horizontal and vertical logical-target pixel offset; and
- visible band width and height.

An unchanged key reuses scaling, quantization, and Sixel encoding, but the
encoded payload is still emitted because terminal pixels are not treated as
persistent placement state.

## Cache bounds

Retained encoded data is bounded by entry count and encoded payload bytes.
Least-recently-used entries outside the current scene are removed first. If the
visible working set itself exceeds either bound, already emitted live entries
are removed as well. Thus a large scene may need to re-encode an evicted band
on a future presentation, but retained cache state never exceeds its bounds.

Scaling and encoding necessarily allocate working data before the post-frame
retained-cache limit is enforced. The bound governs retained cache state, not
the transient memory required to encode the current visible scene.

## Failure recovery

A command failure may leave part of the scene visible. Cached encoded bands are
safe to retain because they describe immutable raster transformations, not
committed terminal state. If output may have failed after saving the cursor,
the next call restores and flushes the cursor first. It then emits the complete
desired image scene again.

Visual convergence still depends on the host clearing and redrawing the cell
frame when prior Sixel pixels may be stale. Automatic protocol selection and
the integrated cell-plus-graphics commit belong to that host, not this
lifecycle.

## Rejected alternatives

### Assign persistent placement identities to Sixel

There is no portable move or partial-delete contract comparable to Kitty's.
Relying on one would leave stale pixels on terminals that cannot honor it.

### Cache one encoded payload for the whole visible image

Changing a viewport origin would change the entire cache key and discard all
overlap. Cell-row bands preserve reuse across adjacent scroll positions.

### Drop the cache on clear

Clearing terminal pixels and invalidating immutable encoding work are different
events. Keeping the cache avoids re-encoding unchanged bands during the full
redraw that follows a clear.
