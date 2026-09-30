# urushi

Composable styling, layout, semantic components, and terminal rendering for Rust CLIs.

```toml
[dependencies]
urushi = "0.1.0"
```

Urushi keeps logical styles and `View` values independent from terminal output, with CJK-aware measurement and one renderer-neutral layout pass. It can feed plain CLI output, `urushi-prompt`, the native Urushi TUI stack, or `urushi-adapter-ratatui`.

The 0.1 API is in early development and may change without notice. See the [workspace quickstart](https://github.com/choplin/urushi#quickstart) for the first runnable example and [docs.rs](https://docs.rs/urushi) for the API.

Licensed under the MIT License.
