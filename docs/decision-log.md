# Decision Log

One row per decision made or changed, newest first. The linked document holds
the current rule and rationale; because those documents are rewritten in place,
this table is the record of when each decision was made and what it replaced.

| Date | Decision | Recorded in |
| --- | --- | --- |
| 2026-08-16 | Made the available area an input to layout: the `Cells`/`Fill` length vocabulary over one outer box, layout-participating min/max bounds, application-chosen overflow, and frames that always close — replacing post-hoc cropping by `max_width` and `Limits`. | [`design/view-block-model.md`](design/view-block-model.md) |
| 2026-08-15 | Defined the target inline prompt rendering architecture, shared with noctui. | [`inline-prompt-rendering.md`](inline-prompt-rendering.md) |
| 2026-08-15 | Split presentation into `TextStyle` and `BlockStyle` and adopted the four-node `View` tree resolving to one rectangle for all backends. | [`design/view-block-model.md`](design/view-block-model.md) |
| 2026-08-14 | Separated component data from component style: borrowed style plus borrowed data compose a `View`, with no output ownership. | [`design/component-data-and-style.md`](design/component-data-and-style.md) |
| 2026-08-13 | Adopted an immutable, backend-neutral style value model with a closed property vocabulary and generic `add`/`remove`; no patch or inheritance. | [`design/style-value-model.md`](design/style-value-model.md) |
| 2026-08-13 | Defined the target TEA-style TUI runtime layered above Ratatui, extending `urushi-tui` without making the view model depend on Ratatui. | [`tui-architecture.md`](tui-architecture.md) |
