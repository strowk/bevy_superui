# superui_bridge

The single coupling point between the web world (arena DOM + JS + flair CSS) and
Bevy's ECS, for [bevy_superui].

It owns the per-frame reconciler (DOM → `bevy_ui` entities), the input → DOM-event
seam, and the `window.bevy` bridge.

## Part of bevy_superui

This is an internal crate of [bevy_superui]. Most projects depend on the
[`superui`] umbrella crate rather than this one directly.

- Docs and live demos: <https://strowk.github.io/bevy_superui/>
- Source: <https://github.com/strowk/bevy_superui>

## License

Dual-licensed under either MIT or Apache-2.0, at your option.

[bevy_superui]: https://github.com/strowk/bevy_superui
[`superui`]: https://crates.io/crates/superui
