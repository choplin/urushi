# urushi-graphics

Image components and Kitty/Sixel terminal graphics adapters for Urushi.

```toml
[dependencies]
urushi = "0.1.1"
urushi-graphics = "0.1.1"
urushi-terminal = "0.1.1"
```

The crate keeps image data and protocol state outside Urushi core. An `ImagePresentation` contributes renderer-neutral placement and text fallback to a `View`; one-shot and retained adapters select positively confirmed Kitty or Sixel capabilities.

The 0.1 API is in early development. See [Choose a terminal surface](https://github.com/choplin/urushi#choose-a-terminal-surface) and [docs.rs](https://docs.rs/urushi-graphics).

Licensed under the MIT License.
