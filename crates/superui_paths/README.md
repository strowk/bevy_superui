# superui_paths

The forward-slash asset-path convention shared by [bevy_superui]'s `superui`
(runtime resolution) and `supersolid` (build-time output).

Zero dependencies, so both — including the wasm build of `superui` — can depend
on it without pulling in `oxc`.

## Part of bevy_superui

This is an internal crate of [bevy_superui]. Most projects depend on the
[`superui`] umbrella crate rather than this one directly.

- Docs and live demos: <https://strowk.github.io/bevy_superui/>
- Source: <https://github.com/strowk/bevy_superui>

## License

Dual-licensed under either MIT or Apache-2.0, at your option.

[bevy_superui]: https://github.com/strowk/bevy_superui
[`superui`]: https://crates.io/crates/superui
