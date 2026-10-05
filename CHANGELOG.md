# Changelog

All notable changes to Urushi are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- Centralized SGR and OSC 8 encoding in `urushi-terminal`; static and
  interactive output now share escaping and sequence generation, while
  hyperlink getters return their logical, unescaped values.

### Fixed

- Prevented idle terminal input polling from delaying initial TUI surface
  registration and rendering.

## [0.1.0] - 2026-09-30

### Added

- Composable styles, themes, components, renderer-neutral views, layout, and
  terminal output in the core `urushi` crate.
- Derive macros for typed component data, including `TableRow`, in
  `urushi-derive`.
- Human-facing CLI presentations and typed interactive prompts that share the
  core theme and terminal contracts.
- Synchronous TUI rendering, a TEA-style application runtime, and an adapter
  for caller-owned Ratatui buffers.
- Kitty and Sixel terminal graphics with capability-based selection and text
  fallback.
- Shared terminal commands, events, geometry, capability detection, and
  session restoration across the prompt, TUI, and graphics surfaces.

[0.1.0]: https://github.com/choplin/urushi/releases/tag/v0.1.0
