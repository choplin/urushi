# urushi-tui

Synchronous full-screen presentation for Urushi.

```toml
[dependencies]
urushi = "0.1.0"
urushi-tui = "0.1.0"
```

`Screen` and draw-scoped `Frame` own Urushi cell buffers, diffing, and transactional terminal output. The caller still owns application state, input, terminal queries, raw mode, session restoration, and the event loop. The default `crossterm` feature re-exports the portable backend.

The 0.1 API is in early development. See [Choose a terminal surface](https://github.com/choplin/urushi#choose-a-terminal-surface) and [docs.rs](https://docs.rs/urushi-tui).

Licensed under the MIT License.
