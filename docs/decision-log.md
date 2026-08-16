# Decision Log

One row per decision made or changed, newest first. The linked document holds
the current rule and rationale; because those documents are rewritten in place,
this table is the record of when each decision was made and what it replaced.

| Date | Decision | Recorded in |
| --- | --- | --- |
| 2026-08-16 | Closed the remaining canonical-form duplication by normalization rather than by type: fold an underline colour equal to the foreground, once the style is final, and fold only what is inert. | [`design/inline-prompt-rendering.md`](design/inline-prompt-rendering.md) |
| 2026-08-16 | Chose a prompt's horizontal cursor window before Resolve, replacing the withdrawn decision to resolve a prompt unbounded in width — an unbounded axis is a measurement, so it costs every row its overflow policy. | [`design/inline-prompt-rendering.md`](design/inline-prompt-rendering.md) |
| 2026-08-16 | Stated the no-solver boundary as "a decided size is never revised, and no node is assembled twice" rather than "no node is laid out twice", so that repeating a pure measurement of a subtree — which a `Column` needs to divide its height — is inside the model. | [`view-model.md`](view-model.md) |
| 2026-08-16 | Made the clip marker a parameter of `Overflow::Clip` instead of a separate `Ellipsis` policy, so the glyph and its cell cost are the application's choice. | [`design/view-block-model.md`](design/view-block-model.md) |
| 2026-08-16 | Made the available area an input to layout: the `Cells`/`Fill` length vocabulary over one outer box, layout-participating min/max bounds, application-chosen overflow, and frames that always close — replacing post-hoc cropping by `max_width` and `Limits`. | [`design/view-block-model.md`](design/view-block-model.md) |
| 2026-08-15 | Defined the target inline prompt rendering architecture, shared with noctui. | [`inline-prompt-rendering.md`](inline-prompt-rendering.md) |
| 2026-08-15 | Split presentation into `TextStyle` and `BlockStyle` and adopted the four-node `View` tree resolving to one rectangle for all backends. | [`design/view-block-model.md`](design/view-block-model.md) |
| 2026-08-14 | Separated component data from component style: borrowed style plus borrowed data compose a `View`, with no output ownership. | [`design/component-data-and-style.md`](design/component-data-and-style.md) |
| 2026-08-13 | Adopted an immutable, backend-neutral style value model with a closed property vocabulary and generic `add`/`remove`; no patch or inheritance. | [`design/style-value-model.md`](design/style-value-model.md) |
| 2026-08-13 | Defined the target TEA-style TUI runtime layered above Ratatui, extending `urushi-tui` without making the view model depend on Ratatui. | [`tui-architecture.md`](tui-architecture.md) |
