# bevy superui

Bevy SuperUI is a crate for bevy to write game UI's using browser-like HTML/CSS/JS stack coupled with first class support for solid-style TSX components and powerful hot reload.

It is built on top of bevy_ui (inheriting some of its limitations) and incorporates somewhat modified bevy_flair for CSS support.

The goal of this projct is to provide the best possible developer experience for writing game UI's in bevy with a focus on rapid iteration and compatibility with existing web development knowledge and practices.

## Status

This is in very early stages of development, but technically some working examples are already available.

The code is mostly AI generated and is not yet reviewed as such, so it is not guaranteed to be correct or safe. Use at your own risk.
Most of API surface can be expected to be relied upon though, because I am more or less trying to support API's that are already known in web development, however a certain flux is expected at this stage.

## License

Bevy SuperUI is dual-licensed under either

- MIT License ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)

at your option.

This means you can select the license you prefer. This dual-licensing approach is the de-facto standard in the Rust and Bevy ecosystems.

Portions of this repository are derived from [`bevy_flair`](https://github.com/eckz/bevy_flair) (the vendored crates under `crates/bevy_flair_*`), which is itself dual-licensed under MIT OR Apache-2.0. Copyright over those portions remains with the original authors; see the upstream project for details.

### Your contributions

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual-licensed as above, without any additional terms or conditions.

## Live examples

Each example is compiled to WebAssembly and published on GitHub Pages, showing the
running app beside its authored source (TSX where applicable).

**Apps**

| Example | Live demo | Description |
| --- | --- | --- |
| TodoMVC (HTML/CSS/JS) | [Open](https://strowk.github.io/bevy_superui/examples/todomvc/) | Classic TodoMVC in plain HTML/CSS/JS |
| TodoMVC (Supersolid TSX) | [Open](https://strowk.github.io/bevy_superui/examples/todomvc_supersolid/) | The same app authored in Solid-style .tsx |
| Game Menu | [Open](https://strowk.github.io/bevy_superui/examples/game_menu/) | Multi-screen sci-fi game menu in supersolid TSX |

**Stress tests** (deliberately heavy — may run slowly in-browser)

| Example | Live demo | Description |
| --- | --- | --- |
| Citadel | [Open](https://strowk.github.io/bevy_superui/examples/citadel/) | Economy sim UI — reactive-node stress test |
| Horde | [Open](https://strowk.github.io/bevy_superui/examples/horde/) | Survivors-like **playable game** + reactive-HUD stress test |

> ▶ These are static wasm builds. **Hot reload of the UI is native-only** —
> `git clone` and `cargo run -p <example>` (add `--features hmr` for the supersolid
> TSX examples) to edit HTML/CSS/TSX live.

## Documentation site

The full site (landing, docs, and the examples gallery) is a single mdBook
project under `website/`, deployed to GitHub Pages by the `Deploy Pages`
workflow.

Run it locally:

```bash
cargo install mdbook        # once
mdbook serve website        # live-reload at http://localhost:3000
```

The gallery index is generated from `examples/gallery.json` by the
`mdbook-gallery` preprocessor (built automatically via `cargo run` during the
mdBook build). Per-example wasm demos are built only in CI; the
`/examples/<slug>/` links 404 under local `mdbook serve`, which is expected.

## Deploying the gallery (maintainers)

To add an example: create the crate under `examples/<slug>/` (wasm-buildable, with a
`web_window` canvas hook), then append one object to `examples/gallery.json`. The slug
becomes its permanent URL — don't rename a published slug.

