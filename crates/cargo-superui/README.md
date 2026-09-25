# cargo-superui

A cargo subcommand that scaffolds superui editor types and `tsconfig` into a
project, for [bevy_superui].

`cargo superui install` projects the supersolid `.d.ts` modules into a gitignored
`superui_modules/` directory and wires up `tsconfig` path resolution, giving
`.tsx` authoring IntelliSense in your editor.

## Install

```sh
cargo install cargo-superui
cargo superui install
```

## Part of bevy_superui

This is a tool for [bevy_superui].

- Docs and live demos: <https://strowk.github.io/bevy_superui/>
- Source: <https://github.com/strowk/bevy_superui>

## License

Dual-licensed under either MIT or Apache-2.0, at your option.

[bevy_superui]: https://github.com/strowk/bevy_superui
