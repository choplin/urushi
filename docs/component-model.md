# Component Model

This document defines the boundary between reusable semantic components,
their presentations, the renderer-neutral [`View`](view-model.md) tree, and the
resolved scene. The reasoning behind the boundary lives in
[`design/component-presentation.md`](design/component-presentation.md).

## Components become views through presentations

A semantic component describes what exists. A concrete presentation describes
one way to turn that meaning into layout primitives, including an
area-dependent Canvas:

```text
semantic data + presentation [+ optional frame input]
                    --compose--> View --resolve(Available)--> ResolvedView
```

For the canonical presentation of a common component, `Theme` provides the
short call:

```rust
let list = List::new()
    .item("Define the API")
    .item(ListItem::new("Implement").items(["model", "view"]));

let view = theme.list(&list);
```

The shortcut delegates to the complete presentation object. A caller reaches
that object when one use needs local variation:

```rust
let numbered = theme
    .components()
    .list()
    .clone()
    .enumerator(arabic_enumerator);

let view = numbered.compose(&list);
```

This ordinary `compose` call borrows presentation and data, performs no
terminal I/O, and receives no `Available` area. It returns a `View`; it does not
render output. The presentation may build a tree from closed built-in
primitives or bind an owned frame into a Canvas item with one Canvas-wide
intrinsic sizing value. `compose` names the lowering responsibility, not one
shared signature: a component that needs
selection, a visible origin, or another fact about the current frame may borrow
an additional component-specific presentation input. `urushi-tui` owns and
updates that state; composition only reads its current value.

## The four layers

| Layer | Owns | Does not own |
| --- | --- | --- |
| Semantic data | Content, relationships, hierarchy, domain visibility, and semantic inspection | Presentation styles, layout primitives, available area, interaction state, terminal capabilities, or output lifecycle |
| Concrete presentation | One structural presentation, its logical styles, markers and other presentation policy, and lowering semantic data plus any borrowed frame input into a `View` | Component content, available area, state transitions, writers, or event loops |
| `View` | Renderer-neutral built-in layout and drawing primitives, including Canvas, and their logical styles | Component-specific variants or inspectable semantic roles, terminal capabilities, or application workflow |
| `ResolvedView` | The resolved rectangle of styled graphemes and reported placements | Semantic data, primitive nodes, terminal capability decisions, or output ownership |

The presentation is the only layer that understands component meaning. It may
finish that interpretation while constructing built-in primitives, or bind its
data, styles, and policy into an owned Canvas item whose later drawing depends
on the final local size. The corresponding Canvas sizing value participates in
layout before any item draws. Both remain opaque to `View`: the resolver uses
generic Canvas contracts and never inspects a header, branch, selection, or
component kind. The exact contract is [`design/canvas.md`](design/canvas.md).

An immutable, component-specific snapshot such as selection, expansion,
visible origin, cursor, or camera is an input to that lowering operation, not a
fifth architectural layer. The application owns its lifetime and transitions;
the presentation only borrows what it needs to describe one frame.

## Area-independent composition

A presentation states layout intent without deciding an area-dependent
result. It must not accept terminal width, pre-wrap content for an eventual
share, align columns by inserting measured spaces, or repeat glyphs to bake a
line of a chosen width into a text leaf.

The area reaches the tree only through `resolve`. Built-in primitive rules and
Canvas intrinsic sizing then decide widths, wrapping, clipping, alignment, and
placement. The sizing value receives only the selected local width after its
siblings share their parent; the presentation's `compose` operation never
receives the root area. This keeps the same composed component valid at the
root, inside a `Row`, and under different backends and available areas.

Intrinsic, area-independent work remains valid during composition. A
presentation may normalize a marker to one line or choose a primitive from a
semantic role. It may not compute the final geometry that only sibling sharing
or the available area can determine.

## Styles and presentations are different values

The `...Style` suffix is reserved for declarative values that do not interpret
semantic component data. `TextStyle`, `BlockStyle`, and `GridStyle` configure
the primitives they are attached to.

A type that interprets semantic data and constructs primitives is named
`...Presentation`. The built-in canonical types are
`ListPresentation`, `TreePresentation`, `TablePresentation`,
`SummaryPresentation`, and `WarningPresentation`. `ComponentTheme` stores
their theme-derived defaults alongside shared component role styles.

Each presentation is an independent concrete type. `compose` is a naming and
responsibility convention, not a shared trait or universal signature. Internal
reuse does not create a public `Presentation`, `PresentationInput`, or
`Component` abstraction. Canvas's private intrinsic measurement capability is
different: it is invoked only after a presentation has bound its distinct
inputs into an owned value.

## Canonical and alternate presentations

An unqualified name such as `TreePresentation` denotes the canonical
presentation Urushi ships for that component. A structurally different
presentation receives a structural name and is introduced only together with
an implementation:

```rust
let ordinary = theme.tree(&tree);
let vertical = VerticalTreePresentation::from_theme(&theme).compose(&tree);
```

Both consume the same `Tree` data and produce `View` values. Neither is a
subtype of the other. Appearance shared by two real presentations may be
extracted later; a hypothetical second presentation does not justify a public
style split or trait today.

The same rule scales to a future graph component. Graph topology and content
remain semantic data, while concrete choices such as layered or explicitly
positioned presentation use named types such as `LayeredGraphPresentation` or
`PositionedGraphPresentation`. They may use the built-in Canvas, but `View`
gains neither a `Graph` node nor graph
interaction semantics. No
`theme.graph(&graph)` shortcut exists until Urushi has chosen and shipped one
canonical graph presentation.

## Theme shortcuts

`Theme` exposes short methods for canonical presentations expected in ordinary
use:

```rust
theme.list(&list);
theme.tree(&tree);
theme.table(&table);
theme.summary(&summary);
theme.warning(&warning);
```

These methods add no second implementation. Each delegates to the corresponding
presentation in `theme.components()`. Named alternate presentations and local
customization stay explicit through `compose`.

## Component classification

`List`, `Tree`, `Table`, `Summary`, and `Warning` are semantic data. Their
presentation types own every conversion into `View`, including the current
canonical visual structure. A table header is Table meaning until
`TablePresentation` lowers it; a list marker and a tree branch are presentation
policy until their presentation expresses them as generic primitives.

The canonical Table presentation binds an owned Table snapshot, its styles,
and its drawing policy into a Canvas item. The item derives the Canvas's one
intrinsic sizing value from that same bound frame. During `resolve`, sizing
reports column requirements and height at the selected width; after the final
viewport is known, the item records cell and rule commands. It is not a Grid
and the core resolver cannot inspect the bound Table or presentation.
Presentations that need no specialized area-dependent algorithm continue to
compose ordinary primitive trees.

Shared recursive traversal, marker normalization, or CJK handling may remain
private implementation. Promote a shared public contract only when external
callers need generic code over multiple component models.
