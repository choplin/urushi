# urushi-terminal

Backend-independent terminal contracts and target-specific inspection for the Urushi ecosystem.

```toml
[dependencies]
urushi-terminal = "0.1.1"
```

The default build exposes commands, events, sessions, styles, and capability
queries without depending on another Urushi crate. On Unix it also exposes the
native backend. Its dependency-free ANSI writer is the shared owner of SGR and
OSC 8 byte encoding for static and interactive output. Enable the `crossterm`
feature for the portable Crossterm adapter used by prompt and TUI entry points.

The 0.1 API is in early development. See [docs.rs](https://docs.rs/urushi-terminal) and the [workspace architecture](https://github.com/choplin/urushi/blob/main/docs/architecture.md).

Licensed under the MIT License.
