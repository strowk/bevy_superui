# superui_html

HTML5 document parsing for [bevy_superui].

Parses an HTML string into a [`superui_dom`] tree via `html5ever`. Knows nothing
about Bevy or JavaScript, and is headless-testable.

## Part of bevy_superui

This is an internal crate of [bevy_superui]. Most projects depend on the
[`superui`] umbrella crate rather than this one directly.

- Docs and live demos: <https://strowk.github.io/bevy_superui/>
- Source: <https://github.com/strowk/bevy_superui>

## License

Dual-licensed under either MIT or Apache-2.0, at your option.

[bevy_superui]: https://github.com/strowk/bevy_superui
[`superui`]: https://crates.io/crates/superui
[`superui_dom`]: https://crates.io/crates/superui_dom
