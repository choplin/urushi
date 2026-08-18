# Component Data and Style

[`component-model.md`](../component-model.md) defines the contract this
document argues for: reusable components separate semantic data from
presentation, and a component style composes a `View` from component data.

## When to introduce a component style

Introduce a dedicated component style when either condition holds:

- callers need to build or reuse the data independently of its presentation;
  or
- callers need to configure or reuse the presentation policy independently,
  including both default component styles and per-use variations.

Nested inheritance, custom markers, and component-specific layout are strong
signals that presentation is an independent policy. Simple, short-lived
components may continue to accept `ComponentStyles` directly when neither the
data nor presentation has an independent reuse requirement.

Do not introduce a dedicated style merely for symmetry. When uncertain, ask two
questions: may callers reasonably build the data before choosing a theme, and
may they show the same data in two different ways?

## Why similar components do not share a public data model

Similar components do not need to share a public data model. Public aliases or
traits couple their future evolution even when their current implementation is
identical.

What may be shared is genuinely common implementation, kept private. A private
trait or generic function may own recursive traversal, marker alignment,
multiline continuation, or CJK cell-width handling while each component keeps
independent public data and callback types.

Promote a shared contract to a public trait only when external callers need to
write generic code over multiple component models. Internal deduplication alone
does not justify a public abstraction.

## Why `…Style` and not `…Renderer`

A component style composes a `View`; it does not render output. Names such as
`ListStyle` and `TreeStyle` therefore fit this role better than `ListRenderer`
or `TreeRenderer`.
