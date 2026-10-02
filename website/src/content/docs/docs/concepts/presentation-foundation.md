---
title: Presentation model
description: Find the Urushi concept that owns each part of terminal presentation.
---

Urushi uses the same presentation concepts for plain output, prompts, and
full-screen TUIs. The concepts below match the sections in this sidebar: start
with the term that describes what you need to control.

<div class="overview-catalog">
  <a href="/docs/core/views/">
    <pre>View
├─ heading
└─ row → panels</pre>
    <strong>View</strong>
    <span>The shared presentation value.</span>
  </a>
  <a href="/docs/core/styles/">
    <pre><span class="demo-accent">accent</span>
<span class="demo-success">success</span>
<strong>bold</strong> · <em>italic</em></pre>
    <strong>Styles</strong>
    <span>How terminal cells look.</span>
  </a>
  <a href="/docs/core/layout/">
    <pre>╭ A ╮╭── B ──╮
│   ││       │
╰───╯╰───────╯</pre>
    <strong>Layout</strong>
    <span>How Views receive and occupy space.</span>
  </a>
  <a href="/docs/core/themes/">
    <pre><span class="demo-accent">accent role</span>
<span class="demo-warning">warning role</span>
same meaning</pre>
    <strong>Themes</strong>
    <span>One semantic visual language.</span>
  </a>
  <a href="/docs/core/components/">
    <pre>Name    State
core    <span class="demo-success">ready</span>
prompt  ready</pre>
    <strong>Components</strong>
    <span>Semantic lists, tables, trees, and scrollbars.</span>
  </a>
  <a href="/docs/core/canvas/">
    <pre>┌──┬──┐
│  ├──┤
└──┴──┘</pre>
    <strong>Canvas</strong>
    <span>Coordinates, overlap, and connected lines.</span>
  </a>
  <a href="/docs/core/text-width/">
    <pre>8 cells
日本語
status</pre>
    <strong>Text width</strong>
    <span>Grapheme-safe measurement, tabs, and wrapping.</span>
  </a>
</div>

[Build and render your first shared View →](/docs/core/views/#quickstart)

These are cooperating parts of one model, not competing ways to build an
interface. Choose the card that names the result you want; each page begins
with a runnable example and continues into the relevant settings.

## How the concepts fit together

A `View` is the common value at the center of the model. A component keeps
semantic data separate from its presentation, and a theme picks that
presentation and the styles for semantic roles. Both produce Views without
choosing a terminal backend.

Styles determine how the View's cells look. Layout determines how Views occupy
space and relate to one another. Text width determines how their content is
measured in terminal cells. Canvas is a specialized View for content whose
position or overlap must be expressed in those cells explicitly.

The completed View can then be resolved for an ordinary CLI write, a prompt,
Urushi's full-screen runtime, or a caller-owned Ratatui buffer. Those surfaces
have different event and output lifecycles, but they consume the same
presentation model.

[Terminal graphics](/docs/graphics/) extend that model at its anchor boundary:
an image reserves space as a View, then Kitty or Sixel pixels overlay the
resolved region. Graphics are therefore not another layout primitive, and
Canvas remains cell-space drawing rather than a pixel rasterizer.
