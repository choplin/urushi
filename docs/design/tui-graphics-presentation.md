# How Does the TUI Commit Cells and Terminal Graphics Together?

This document defines protocol selection, frame ordering, logical commit, and
failure recovery when the optional `urushi-tui-app` graphics integration is
enabled.

## Selection

`GraphicsPreference` is `Auto`, `Kitty`, `Sixel`, or `Text`. The runtime queries
positive terminal capability evidence once, after entering the session and
before starting the input reader. `Auto` chooses confirmed Kitty first,
confirmed Sixel second only when uniform character-cell pixel geometry is
available, and otherwise text. An explicit Kitty or Sixel request that cannot
be satisfied is a startup error; it is not silently weakened.

Kitty is preferred because it separates retained uploads from placements. A
move or viewport crop can update placement metadata without retransmitting the
raster, and an individual placement or asset can be deleted. Sixel has no
portable equivalent identity or deletion operation, so its integration is an
immediate-mode full redraw. Explicit Sixel remains useful as a compatibility
override and for protocol testing, not as the ordinary choice when both are
available.

The accepted `Surface` observation is also the presentation snapshot. When an
application subscribes to `Surface`, its cell size and optional cell-pixel
geometry reach the application and draw worker from the same `Sync`
acceptance; graphics cannot use geometry from a newer resize than the model
whose `View` is being drawn. Without that subscription, the terminal source
still accepts resize geometry and requests a presentation-only redraw, so
graphics never remain at stale geometry merely because the application has no
surface message.

## One logical frame

The runtime resolves the application `View` once for the selected surface.
Image fallback cells and protocol-neutral image placements come from that same
resolved value. `Screen::draw_with` then performs one logical transaction:

1. write the required cell changes and cursor request;
2. reconcile and write the selected graphics scene;
3. flush; and
4. commit the working cell buffer only after every preceding step succeeds.

Kitty reconciliation commits its own candidate state only after its commands
and flush succeed. Thus a successful draw advances both retained baselines; a
failure advances neither cell state nor a knowingly incomplete Kitty state.

Every Sixel frame first invalidates the physical cell baseline. `Screen`
clears the surface and writes the complete cell frame, then `SixelLifecycle`
emits every visible image band. Its bounded encoded-band cache survives the
clear, but terminal pixels do not. This ordering also removes stale pixels
after scroll, image removal, or replacement.

Resize first clears protocol-specific state, replaces the cell buffers, and
draws against the accepted new `Surface`. Shutdown clears the selected
protocol before the terminal session restores its modes.

## Failure recovery

A cell-output failure uses `Screen`'s existing rule: the physical baseline is
unknown and the next draw clears and reconstructs it.

A graphics-output failure takes a stronger path in the same draw worker:

1. retain the original graphics error;
2. attempt protocol-specific cleanup;
3. disable graphics for the rest of the presentation;
4. resolve the same `View` again for text fallback;
5. clear and redraw the complete cell frame; and
6. report the original error after the fallback frame has committed.

Applications that subscribe to terminal errors can diagnose the failure and
continue with text output. Without that subscription, the runtime retains its
general policy of returning a draw error. A cleanup or fallback-redraw failure
is appended to the reported error without replacing the original cause.
Protocol lifecycle state is retained after failed cleanup so the next text
frame and shutdown can try cleanup again.

Physical output is never claimed to be atomic. These rules provide logical
commit and deterministic convergence from any observed partial-write point.

## Rejected alternatives

### Fall through from failed Kitty output to Sixel in the same frame

A partial Kitty write may have created uploads or placements whose exact
terminal state is unknown. Switching protocols adds another partially visible
scene before cleanup and weakens the convergence rule. Runtime failure falls
back to cells; Sixel is the automatic choice only when Kitty was unavailable
at selection time.

### Let Image own probing, output, or protocol state

An Image is renderer-neutral component data. It cannot coordinate cell commit,
terminal resize, one physical writer, or session cleanup. The presentation
worker is the smallest owner that already has all of those responsibilities.

### Commit cells before graphics output

Doing so would leave the cell diff baseline ahead of a failed logical frame.
Recovery would then need to guess which cells to resend while also repairing
unknown graphics state. The extension output therefore runs inside the
`Screen` transaction before cell commit.
