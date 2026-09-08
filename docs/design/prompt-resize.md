# Prompt Resize Ownership

How each prompt display mode reacts to a terminal resize, why inline recovery
cannot safely infer a new physical region, and which apparent recovery
mechanisms are outside the portable contract.
[`inline-prompt-rendering.md`](../inline-prompt-rendering.md) defines the two
display modes and their visible lifecycle contracts.

## The rule

### Inline emits no row-recovery plan

An inline region's saved origin and owned row count describe physical terminal
state. Once the primary buffer has been resized, neither identifies the rows to
which the terminal reflowed the prompt. The resize event therefore emits no
cursor movement, erasure, separator, or replacement prompt block derived from
the previous region.

The primary terminal buffer owns whatever soft-wrap reflow it applies to the
displayed snapshot. Urushi retains the logical form state, but the inline mode
does not promise a deterministic physical layout after resize or treat
application width calculations as authority over the terminal's rows.

Resize events may be coalesced above the render plan to avoid redundant
observations. Coalescing is only an optimization: correctness does not depend
on a quiet period because no delivered resize invokes row recovery.

### Alternate screen redraws its viewport

An alternate-screen prompt owns the whole temporary viewport. On resize it
discards the previous physical frame, resolves the current form against the
latest viewport, and redraws the screen. Absolute positions are safe within
that frame because primary-buffer content does not share the surface.

Leaving the alternate screen restores the primary buffer and every terminal
mode acquired by the prompt session. Resize does not change that cleanup
obligation.

### Terminal extensions are optional

Semantic-zone OSC sequences may let a supporting terminal improve navigation
or redraw behavior, but they are not an addressing contract. The portable
renderer cannot query a marker's reflowed coordinates or replace a region by
marker identifier. A terminal-specific backend may use such extensions later;
neither display mode depends on them for correctness.

## Why inline does not recover the old region

Recovering the region would require identifying the prompt's reflowed origin
and every row it still owns. The portable terminal interface supplies neither.
Erasing an inferred extent could destroy output outside the prompt. Leaving
prompt output visible is bounded damage; guessed erasure is invisible and
unbounded.

Application-side display width is not a substitute for terminal knowledge.
Urushi's width model can differ from the terminal's, and resizing can move the
old origin above the addressable viewport. A calculated row delta can therefore
land outside the prompt even when the logical form state is intact.

## Rejected alternatives

**Abandon and re-establish an inline region after resize.** This leaves one old
prompt block for every resize that reaches the renderer. Debouncing reduces how
many blocks appear, and a separator makes their boundary clearer, but neither
removes the stale content. They address frequency and presentation rather than
the cause.

**Recover a saved physical origin after resize.** A saved cursor position and a
cursor-position report are physical coordinates. They do not identify which
reflowed rows belong to the old prompt, so using them as a logical anchor would
restore guessed erasure.

**Derive the origin from an application-side logical cursor offset.** Width
mismatch and an origin outside the addressable viewport can make a calculated
row delta land outside the prompt. This is not a safe portable recovery
mechanism without an additional capability contract.

**Always use the alternate screen.** Full viewport ownership gives reliable
reflow, but removes the active form from ordinary terminal context and
scrollback. That cost belongs to an explicit mode rather than every blocking
prompt.
