# Style Canonical Form

The vocabulary of [`style-model.md`](../style-model.md) is chosen so that one
appearance has one value, but one duplication survives it: an underline color
equal to the foreground. This file defines the fold that closes it, the rule
for admitting any future fold, the canonical form of the styled runs a prompt
frame emits, and the reasoning behind each.

## The rule

The duplication is *between* fields, so no signature makes it unrepresentable,
and it is closed by normalization, applied to both style types alike:

> Fold an underline color to *absent* when the foreground is a concrete color
> and the underline color is **the same value**. The values are compared as
> values: a palette red and a true-color red look different on screen and must
> not be folded together.

An underline is drawn in the foreground color unless one is set, so stating the
color a run already has changes nothing but the bytes.

The fold applies once the style is final. `RenderSettings::resolve_text_style`
applies it as its last step, after degradation,
because degradation is what makes two logical colors equal; a style that has
passed through render settings is therefore canonical, and no separate normalizing
call is part of the public API.

The rule for admitting any future fold is narrow:

> **Fold only what is inert.** A value may be dropped when doing so cannot
> change the output, whatever the terminal does. An equivalence that holds only
> because a terminal is assumed to implement an attribute a particular way is
> not a fold.

One case is not closable: when the foreground is absent, its concrete color is
the terminal's default and unknown here, so an underline color equal to it
cannot be recognized. A canonical form owes determinism, not minimality.

### Runs have a canonical form

A prompt frame compares rows to decide whether to redraw, and that comparison
breaks if the same visible row can be represented by more than one sequence of
runs. The Frame stage of [`prompt-render-plan.md`](prompt-render-plan.md)
therefore emits runs in a canonical form:

- adjacent runs with equal styles are merged, greedily and left to right;
- no run is empty; and
- a run's style is the style **as it will be emitted** — resolved under the
  selected `RenderSettings`, in a representation where equal appearance means equal
  value, and in the canonical form defined above.

Run aggregation is where the form is established, because rows are aggregated
from grapheme-level content on each frame. This is also why the prompt's view
carries settings-resolved styles from the moment it is built, several stages
above the writer — the one deliberate exception to the rule that `TextStyle`
stays logical until an output boundary.

## Why normalization folds only what is inert

Two runs can look identical while holding different values, and the type rules
out most of that: one underline value rather than an attribute bit plus a color, a
color type with no reset spelling, one attribute set rather than an add set and a
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
the foreground. Applying selected render settings is one instance of "final",
not the reason for the rule. Capability degradation is why that stage matters at
all: two colors the settings collapse to the same output are equal on screen and
unequal in a logical style, so a consumer that compares runs for equality must
apply the settings before it compares.

One residue is not closable. When the foreground is absent, its concrete color
is the terminal's default, unknown here, so an underline color equal to it
cannot be recognized. This weakens only one of the two things a canonical form
buys — not redrawing a row that has not changed. The other, the same input
always yielding the same value, holds regardless, because the rule is fixed. A
canonical form owes determinism, not minimality.

## Why runs, and why runs are canonical

Rows hold runs rather than individual cells: the plan writes whole rows and
never addresses a cell, so per-grapheme granularity would cost an allocation
per character on the redraw path without being used. Grapheme-level resolution
remains internal to Resolve.

A row is the unit the plan compares to decide whether to redraw, so equal
appearance has to mean equal value at the row level too; merging equal
neighbours and forbidding empty runs is what makes two renderings of the same
row the same sequence.
