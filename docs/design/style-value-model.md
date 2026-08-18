# Styles as Effective Values

Urushi's style contract, defined in [`style-model.md`](../style-model.md),
shapes a style as an immutable collection of effective presentation values with
a closed property vocabulary, rather than a patch, an instruction list, or a
completed ANSI state transition. The reasons follow.

## Why effective values, not instructions

A style whose entries are effective values means one thing wherever it is read:
a renderer consumes the values present and emits whatever its backend needs to
realize them. A style whose entries are instructions — "turn bold off" — only
means something relative to a prior state, so every reader must agree on what
that state is.

Removing a modifier from an immutable value therefore removes the value; it
does not preserve an ANSI off-code instruction. A renderer that maintains prior
terminal state diffs the previous and next effective styles itself.
`TextStyle::paint` and `BlockStyle::render` surround emitted styling with a
final ANSI reset. Between the two, no stored removal instruction is needed
anywhere.

## Why a closed vocabulary and one generic `remove`

Urushi exposes a closed enum pair per type and one generic `remove`, while
retaining named builders for common construction. Lip Gloss, which likewise
treats a style as an immutable value containing a set of rules, instead tracks
property presence separately and exposes many property-specific `Unset*`
methods. The enums make the complete property vocabulary discoverable and give
generic code an exhaustive match, and one
`remove` cannot drift from a parallel family of `Unset*` methods. The named
builders are thin wrappers over `add` for the same reason: Urushi avoids two
entry points that define two behaviors.

## Why an underline is a value rather than a flag and a color

The property vocabulary is not merely a list of what a terminal can express; it
decides whether two styles with the same appearance are the same value. A run's
style is the unit a redraw compares, so a vocabulary admitting two spellings of
one appearance makes every frame redraw rows that did not change.

An underline is where the naive vocabulary fails twice. An `UNDERLINED`
modifier flag spells Select Graphic Rendition (SGR) `4`, which is SGR `4:1`, so
a separate shape property would give a single underline two spellings; and an
underline color paints nothing on a run with no underline, so a free-standing
color property would be invisible in the output while still making two values
unequal. Both are closed by making the underline one optional value that owns
its shape and its color — which is why `Modifier` lost its underline flag
rather than gaining a sibling shape property.

What a single field cannot decide — an underline color equal to the foreground —
is left to the canonical fold. The division between the two — what the type
forbids and what the fold normalizes away — is the general rule here:
unrepresentability wherever one field decides, normalization only where a
comparison between fields is required.

## Why rapid blink is not in the vocabulary

SGR parameter 6, rapid blink, is left out for the same reason. Xterm-family
terminals draw it exactly as SGR 5, so it would be distinguishable as a value
and indistinguishable on screen — one appearance with two values, admitted at
the vocabulary level.

## Why normalization folds only what is inert

Two runs can look identical while holding different values, and the type rules
out most of that: one underline value rather than a modifier bit plus a color, a
color type with no reset spelling, one modifier set rather than an add set and a
subtract set. That is why no separate "effective" style type is needed alongside
`TextStyle`. What the type cannot rule out is duplication *between* fields, and
there the design normalizes instead.

The admission rule is narrow on purpose. A value may be folded away when its removal
cannot change the output whatever the terminal does — an underline color on a run
with no underline paints nothing, on every terminal. An equivalence that holds
only because a terminal is assumed to implement an attribute a particular way is
not grounds for a fold.

Reversed video is the case this excludes, and it is why the rule is phrased
around inertness rather than around appearance. Swapping a foreground and a
background looks the same as setting reverse, so an appearance-based rule would
admit it. But a dim attribute may apply to the declared foreground or to the
effective one depending on the terminal, so rewriting one form into the other
can change what is drawn. The selection row of a list is exactly where the two
spellings meet and exactly where redraws are most frequent, so the temptation is
real; it is still refused.

When the fold happens is settled separately. Folding after the style is final,
rather than as it is built, is forced by the style being an immutable value with
a builder: any value folded earlier is restored by the next call that changes
the foreground. A profile stage is one instance of "final", not the reason for
the rule. Capability degradation is why that stage matters at all: two colors a
terminal profile collapses to the same output are equal on screen and unequal in
a logical style, so a consumer that compares runs for equality must resolve
against the profile before it compares.

One residue is not closable. When the foreground is absent, its concrete color
is the terminal's default, unknown here, so an underline color equal to it
cannot be recognized. This weakens only one of the two things a canonical form
buys — not redrawing a row that has not changed. The other, the same input
always yielding the same value, holds regardless, because the rule is fixed. A
canonical form owes determinism, not minimality.

## Why no patch operation

Urushi has no patch operation, and modifiers are one set rather than an
add set and a subtract set. Reusable changes are ordinary
`fn(TextStyle) -> TextStyle` transforms, and no Urushi boundary requires a
separately inspectable patch value. A patch data type would be introduced only
for a concrete need to serialize or inspect such changes.

The absence of a patch representation is also what keeps parent-to-child style
inheritance out of the view model, as
[`view-block-model.md`](view-block-model.md) records.
