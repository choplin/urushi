---
title: Current limitations
description: User-visible limitations and incomplete paths in Urushi 0.1.0.
---

Urushi 0.1.0 is an early release. The following boundaries affect application
design.

## TUI package split is not implemented yet

`urushi_tui::run` and `Runtime` are runnable in 0.1.0. That release still keeps
the TEA runtime, the Ratatui-backed frame terminal, and caller-owned Ratatui
widgets in one `urushi-tui` package. `RatatuiTerminal` uses Ratatui buffers and
cell diffing, then writes backend-independent terminal commands through a
`CellWriter`.

The target architecture moves the TEA runtime to `urushi-tui-app`, replaces
the runtime's frame-terminal abstraction with a concrete Urushi-owned `Screen`
in `urushi-tui`, and moves caller-owned Ratatui widgets to
`urushi-adapter-ratatui`. Those split packages are not yet dependency names.

In both shapes, this is separate from physical terminal connections.
`urushi-terminal` provides a native Unix backend and an optional Crossterm
adapter behind its own `TerminalBackend` contract.

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
