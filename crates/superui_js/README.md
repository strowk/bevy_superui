# superui_js

The JS engine boundary for [bevy_superui].

The engine runs the framework-free JS shadow DOM (`js/dom.js`): author and
reactive JS mutate a JS-side tree that records primitive ops. Once per frame the
host decodes that batch so the bridge can replay it onto the render mirror. Knows
nothing about Bevy, and is headless-testable.

The backend is chosen at compile time via two mutually-exclusive features:
`engine-v8` (default; native, via `deno_core`/V8) and `engine-web` (wasm, runs in
the browser's own JS engine).

## Part of bevy_superui

This is an internal crate of [bevy_superui]. Most projects depend on the
[`superui`] umbrella crate rather than this one directly.

- Docs and live demos: <https://strowk.github.io/bevy_superui/>
- Source: <https://github.com/strowk/bevy_superui>

## License

Dual-licensed under either MIT or Apache-2.0, at your option.

[bevy_superui]: https://github.com/strowk/bevy_superui
[`superui`]: https://crates.io/crates/superui
