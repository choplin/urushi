# Prompt Field Presentation

An inline prompt has three distinct presentation units. A public `Group`
chooses the current navigation page, an internal field presentation keeps one
field's semantic parts together, and Resolve produces the final drawable rows.
Keeping those units separate lets normal layout and viewport degradation use
the same field body without adding prompt concepts to the generic `View` model.

## The field boundary

Each runtime field produces this internal shape:

```text
FieldPresentation {
  body    : View
  help    : View?
  regions : [{ role, anchor }]
  cursor  : Key?
}
```

`body` is the ordinary full layout. Its anchors identify Question,
Description, Control, Focus, and Error regions. The region table gives those
opaque keys prompt meaning; `View` and Resolve continue to treat every anchor
as an ordinary keyed box. A zero-sized cursor anchor identifies the terminal
cursor independently of the Focus region.

The current Group contributes all of its field presentations. Each body is
resolved exactly once against the drawing width. Frame then maps the resolved
anchor rectangles to row ranges, preserving both field boundaries and wrapped
region boundaries. Help stays outside `body` as the active-field footer.

## Viewport degradation

When the complete Group fits, Frame returns every resolved row unchanged. When
height is constrained, it applies these priorities:

1. retain the active Focus or cursor row;
2. retain the active Error when present;
3. retain the complete active Question when it fits with the higher-priority
   rows;
4. for a multi-row Control, fill remaining control capacity with rows nearest
   the focused row;
5. add optional Description and Help content; and
6. add non-active fields only as complete fields.

A Question is never partially retained. Without an error, two available rows
show a one-row Question with its focus row; one row shows only the focus row.
If a wrapped Question cannot fit with the focus row, it is omitted whole. With
an error, two rows show Error and focus; a third row restores a one-row
Question. A multi-row Select always retains the focused choice and uses spare
control rows for nearby choices.

The chosen rows keep their resolved horizontal positions and source order.
Frame selects rows only; it neither measures nor reflows them. Back navigation
is unchanged because Group and field state transitions happen before
presentation.

## Confirm alignment

Confirm uses a borderless one-column Grid. Its header cell is left-aligned and
contains Question and optional Description; its control cell applies
`button_alignment`. The shared intrinsic column is therefore the wider of the
header and button row. Left, Center, and Right work whether the header,
description, or buttons are widest, without printable-width calculations or a
synthetic padding view.

## Rejected shapes

A grouped flat-row model preserves row tags but cannot express Confirm's
shared-width alignment through ordinary layout. Prebuilding full, compact, and
minimal views duplicates field layout and resolves it repeatedly. Keeping the
old flat model loses field boundaries, while adding Question or Error variants
to `View` would make prompt-only policy part of the generic layout vocabulary.
