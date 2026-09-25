# supersolid

The Solid-style TSX transpiler for [bevy_superui]: transpiles `.tsx`/`.ts` to
plain JavaScript.

It strips TypeScript types (via `oxc`) and performs reactivity-aware element-walk
JSX lowering to the `$ss` runtime ABI (see [`supersolid_runtime`]). Bevy-free;
the asset loader lives in `superui` (native-only) so `oxc` never enters a wasm
build.

## Part of bevy_superui

This is an internal crate of [bevy_superui]. Most projects depend on the
[`superui`] umbrella crate rather than this one directly.

- Docs and live demos: <https://strowk.github.io/bevy_superui/>
- Source: <https://github.com/strowk/bevy_superui>

## License

Dual-licensed under either MIT or Apache-2.0, at your option.

[bevy_superui]: https://github.com/strowk/bevy_superui
[`superui`]: https://crates.io/crates/superui
[`supersolid_runtime`]: https://crates.io/crates/supersolid_runtime
