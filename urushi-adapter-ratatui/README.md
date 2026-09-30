# urushi-adapter-ratatui

Stateless Ratatui adapters for Urushi views and logical styles.

```toml
[dependencies]
urushi = "0.1.0"
urushi-adapter-ratatui = "0.1.0"
```

The application retains ownership of its Ratatui terminal and event loop. This crate resolves Urushi views into caller-owned `Buffer` regions and converts the supported logical style subset without moving Ratatui types into Urushi core.

The 0.1 API is in early development. See [Choose a terminal surface](https://github.com/choplin/urushi#choose-a-terminal-surface) and [docs.rs](https://docs.rs/urushi-adapter-ratatui).

Licensed under the MIT License.
