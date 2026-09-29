# How Does Kitty Reconcile Retained Image State?

This document defines how `KittyLifecycle` reuses uploads and placement
identities across frames, and how it recovers when terminal output fails.

## Decision

`KittyLifecycle` is an opt-in retained renderer primitive. One terminal
presentation owns one lifecycle shared by all Image components in that
presentation. The lifecycle owns protocol identifiers and reconciliation state,
but no terminal connection, session, scheduling, application model, View, or
`ResolvedView`.

The ordinary one-shot adapter remains stateless. It transmits the complete RGBA
asset and displays the current visible crop in one operation.

## Assets and placements

Asset identity selects one terminal upload. Placement identity selects one
visible terminal placement. Each presentation derives the desired placements
from the current View's immutable Image snapshots and the shared clipped
geometry defined in
[`terminal-graphics-placement.md`](terminal-graphics-placement.md).

When the same image moves or a viewport changes its clip, the lifecycle keeps
the asset ID and placement ID. It sends a placement update containing the new
terminal position, source offset, source extent, and visible cell size; it does
not retransmit RGBA. Hiding an image deletes its placement while its upload may
remain available for later reuse.

Replacing an occurrence's asset reuses its placement identity, uploads the new
asset when necessary, and retires the superseded upload once no visible
placement references it. Several placements may reference one upload.

## Upload cache

Unused uploads form a cache bounded by both entry count and total RGBA bytes.
Least-recently-used unreferenced uploads are evicted when either bound is
exceeded. An asset whose last placement adopted a replacement is retired once
unused; an asset adopted again before retirement becomes ordinarily cacheable.

## Commit and failure recovery

Reconciliation first computes a candidate logical state and its ordered
protocol operations. The lifecycle commits that state only after every command
and the final flush succeed. Physical output is not atomic: a failure may leave
old uploads, newly allocated uploads, or a displaced cursor in the terminal.

After such a failure, the next operation restores and flushes the cursor when
necessary, deletes every identifier that this lifecycle committed or may have
allocated in the failed candidate, and reconstructs the complete desired scene
with fresh identifiers. It does not issue a terminal-global delete and does not
assume that the previous committed state still describes the terminal.

`clear` applies the same conservative deletion to resize invalidation and
shutdown. Failed cleanup remains pending and is retried later. Resolver reuse
and Kitty reconciliation remain independent: a host may retain both, but each
has its own inputs, validity, commit, and recovery rules.

## Rejected alternatives

### Re-upload on every move or clip

The raster is immutable and identified independently from its occurrence.
Retransmitting it on viewport scroll discards the protocol's retained-asset
advantage and makes work proportional to image bytes instead of placement
metadata.

### Commit state as commands are written

A later command or flush may fail after the terminal accepted earlier writes.
Incremental logical commits would claim a terminal state that cannot be known
and make the following differential update unsafe.

### Trust the previous state after failure

Terminal output provides no transaction rollback. Conservative deletion and
full reconstruction are required to converge from every partial-write point.
