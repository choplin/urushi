---
title: Current limitations
description: User-visible limitations and incomplete paths in Urushi 0.1.0.
---

Urushi 0.1.0 is an early release. The following boundaries affect application
design.

## A low-level frame loop owns its session

`urushi-tui` provides synchronous frame presentation, not terminal lifecycle
or input. A caller using `Screen` directly must enter and restore its terminal
session, read events, handle resizes, and decide when to draw. Choose
`urushi-tui-app` when the TEA runtime should own those responsibilities.

## Cursor placement in Ratatui

A resolved Urushi view does not report a caret position. An application that
renders an editable control through Ratatui must compute and place the cursor
itself. Anchors can report component regions, but they are not a cursor API.

## ANSI inside Ratatui buffers

Text stored in a Ratatui `Buffer` is plain. The Urushi Ratatui adapter does not
interpret escape sequences embedded in text. Represent styling with
`TextStyle` and `BlockStyle` values.

## Inline prompt resize

The primary terminal buffer may reflow previous rows after a resize. The
default prompt behavior returns `RunError::Resized` instead of erasing a region
whose location is no longer known. The opt-in redraw policy clears the complete
visible viewport, including content outside the prompt.

## Alternate-screen prompts are not public yet

The prompt architecture supports choosing between an inline region and an
alternate-screen viewport without changing the form or field model. In 0.1.0,
only the inline presentation is exposed through the public form API.

## Graphics are stateless

The one-shot graphics path supports Kitty and Sixel output with fallback text.
It does not own retained uploads, deletion, scrolling slices, repaint policy,
or recovery from protocol-specific draw failures.

## Terminal feature support varies

Colors, attributes, underline forms, underline colors, hyperlinks, and graphics
protocols depend on detected terminal capabilities. Applications must not rely
on a specific decoration as the only carrier of meaning.

## API stability

The project is pre-stable. Public APIs may change before 1.0. Pin compatible
versions across all Urushi crates and review release notes before upgrading.
