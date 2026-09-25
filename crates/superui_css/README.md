# superui_css

The CSS layer for [bevy_superui]: an in-tree fork of `bevy_flair` 0.8 (Bevy 0.19)
re-exported behind an HTML-shaped surface.

The fork matches real HTML element/attribute/class/id and
`:hover`/`:focus`/`:checked` selectors; this crate bundles it into one plugin
(`SuperUiCssPlugin`) and adds an HTML tag-name interner.

## Part of bevy_superui

This is an internal crate of [bevy_superui]. Most projects depend on the
[`superui`] umbrella crate rather than this one directly.

- Docs and live demos: <https://strowk.github.io/bevy_superui/>
- Source: <https://github.com/strowk/bevy_superui>

## License

Dual-licensed under either MIT or Apache-2.0, at your option.

[bevy_superui]: https://github.com/strowk/bevy_superui
[`superui`]: https://crates.io/crates/superui
