# superui

Browser-like HTML/CSS/JS + Solid-style TSX UI for [Bevy](https://bevyengine.org/).

`superui` is the umbrella plugin of [bevy_superui]. It bundles the CSS engine and
the DOM-to-ECS bridge, registers the `.html`/`.js` asset loaders, mounts authored
UI, and hot-reloads it. This is the crate to depend on to build UI with superui.

You author interfaces the way you would for the web — an `index.html`, a
stylesheet, and reactive `.tsx` components — and superui renders them with
`bevy_ui`, styles them with a CSS engine (a fork of `bevy_flair`), and runs their
logic in an embedded JavaScript engine.

## Getting started

See the guide for setup and a first UI:

- Getting started: <https://strowk.github.io/bevy_superui/docs/getting-started.html>
- Docs and live demos: <https://strowk.github.io/bevy_superui/>
- Source: <https://github.com/strowk/bevy_superui>

## License

Dual-licensed under either MIT or Apache-2.0, at your option.

[bevy_superui]: https://github.com/strowk/bevy_superui
