# List Marker Layout

How the canonical List presentation turns nesting depth and per-item markers
into horizontal origins. Enumerator inputs and evaluation are defined in
[List Enumerator Contract](list-enumerator.md); width-dependent row creation is
defined in [List Width Resolution](list-width-resolution.md).

## One local track per sibling group

Every visible sibling group owns one marker track. Its width is the greatest
terminal-cell width of the group's normalized markers:

```text
track(group) = max(width(marker(item)))
```

Each marker is right-aligned within that track. The alignment cells are layout
geometry rather than spaces prepended to the marker string. The complete track
on the first visual row receives the enumerator role style, including otherwise
empty alignment cells, so backgrounds, underline, attributes, and hyperlinks
remain continuous.

Tracks are local. Separate sibling groups at the same depth may have different
content origins, and a wide marker in one group does not widen an unrelated
group.

## Nesting is a fixed presentation step

`ListPresentation` owns `nesting_indent`, measured in terminal cells and
defaulting to two. For an item in a group at `depth`, the horizontal origins
are:

```text
track_x      = saturating_mul(depth, nesting_indent)
marker_x     = saturating_add(track_x,
                              track(group) - width(marker))
content_x    = saturating_add(track_x, track(group))
```

The first visual row draws the marker at `marker_x` and content at `content_x`.
Every continuation row begins at the same `content_x` and has no marker. A
zero-cell nesting step is valid when a caller wants recursive traversal without
additional horizontal displacement.

The fixed step prevents a parent's changing marker width from redefining its
children's indentation. A child marker can therefore begin to the left of a
wide parent's content:

```text
1000. parent
      continuation
  • child
```

This is hanging-list geometry. It is not a connector channel and has no
indenter callback, indenter style, or Indenter role. Connector topology belongs
to a Tree presentation and can be rasterized as a `LineNetwork`; List nesting
needs only an origin.

All coordinate multiplication and addition saturates. This applies even when
the public indentation or semantic depth is extreme, so debug and release
builds preserve the same monotonic placement behavior. The subtraction in
`marker_x` is exact because the group track is never narrower than one of its
markers.

## Why not one global track or padded marker strings

A global track makes unrelated nesting levels and sibling groups move when one
marker changes. Padding callback results instead turns alignment into text:
the padding acquires marker semantics, must be recomputed when display width
changes, and obscures the prefix cost from layout. A local geometric track
aligns the items that readers compare while keeping marker content observable
as supplied.
