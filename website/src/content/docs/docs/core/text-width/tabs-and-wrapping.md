---
title: Configure tabs and wrapping
description: Choose tab expansion and understand text behavior under finite width.
---

`View::text` accepts horizontal tabs and uses the default four-cell tab policy.
Construct `StyledText` when another fixed-width replacement is required.

This example displays an eight-cell tab replacement with a visible arrow, then
resolves the View at 12 columns so the following word wraps:

```rust
use std::error::Error;

use urushi::{
    Available, RenderSettings, StyledText, TabPolicy, TextStyle, View, render,
    resolve,
};

fn main() -> Result<(), Box<dyn Error>> {
    let text = StyledText::new(
        "name\tstatus\n日本語\tready",
        TextStyle::new(),
    )
    .tab_policy(TabPolicy::with_marker(8, "→")?);
    let resolved = resolve(
        &View::styled_text(text),
        Available::columns(12),
    )?;
    let output = render(&resolved, &RenderSettings::default());

    // Omit right-side layout padding in this demonstration.
    for line in output.lines() {
        println!("{}", line.trim_end());
    }
    Ok(())
}
```

```text title="Rendered output"
name→
status
日本語→
ready
```

The arrow occupies the first cell of each eight-cell replacement; spaces fill
the other seven. At the 12-column limit, `status` and `ready` move to the next
row. The three Japanese graphemes occupy six cells and remain whole.

`TabPolicy::spaces(width)` uses only spaces. `TabPolicy::with_marker` returns
`InvalidTabMarker` when the marker contains a control character, contains a
zero-width grapheme, or is wider than the replacement. An empty marker becomes
the equivalent spaces-only policy. Calling `reset_tab_policy` returns to the
default four-cell replacement.

```rust
use urushi::{InvalidTabMarker, StyledText, TabPolicy, TextStyle};

let spaces = StyledText::new("a\tb", TextStyle::new())
    .tab_policy(TabPolicy::spaces(3));
let reset = spaces.clone().reset_tab_policy();
let error = TabPolicy::with_marker(1, "→→").unwrap_err();

assert_eq!(spaces.get_tab_policy().unwrap().width(), 3);
assert!(reset.get_tab_policy().is_none());
assert_eq!(error, InvalidTabMarker::TooWide { width: 1, marker_width: 2 });
```

```text title="Policy results"
spaces(3):       a···b
reset_tab_policy: a····b
with_marker(1, "→→"): TooWide { width: 1, marker_width: 2 }
```

Here `·` marks a cell containing a space; the renderer emits an ordinary
blank cell, not the dot.

View resolution expands tabs before measurement and wrapping. Direct
`render_text` output preserves source tabs because it deliberately bypasses
View layout and accepts the destination terminal's tab-stop behavior.

Wrapping happens only under finite width. Explicit newlines always begin a new
row; ordinary wrapping prefers valid text boundaries and never splits a
grapheme.
