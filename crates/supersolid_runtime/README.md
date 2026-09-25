# supersolid_runtime

The Supersolid reactive core: Solid-like fine-grained signals, effects, memos,
lifecycle, and context, authored in JS and run via [`superui_js`].

Bevy-free and wasm-clean — unlike the [`supersolid`] transpiler crate, this runs
on every target. Only the author API is published on `globalThis`; the graph
internals stay closured.

## Part of bevy_superui

This is an internal crate of [bevy_superui]. Most projects depend on the
[`superui`] umbrella crate rather than this one directly.

- Docs and live demos: <https://strowk.github.io/bevy_superui/>
- Source: <https://github.com/strowk/bevy_superui>

## License

Dual-licensed under either MIT or Apache-2.0, at your option.

[bevy_superui]: https://github.com/strowk/bevy_superui
[`superui`]: https://crates.io/crates/superui
[`superui_js`]: https://crates.io/crates/superui_js
[`supersolid`]: https://crates.io/crates/supersolid
