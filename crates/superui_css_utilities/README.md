# superui_css_utilities

The pure, native-only core of [bevy_superui]'s class utilities
(Tailwind-compatible utility classes).

It turns a set of utility class names into a generated CSS string, using flair's
own parser as the oracle for what is supported: each class is rendered to CSS via
[`encre-css`](https://docs.rs/encre-css), then probed through a headless CSS app.
Classes flair accepts are kept; the rest are dropped with a diagnostic. There is
no hand-maintained property allow-list — flair decides.

## Part of bevy_superui

This is an internal crate of [bevy_superui]. Most projects depend on the
[`superui`] umbrella crate rather than this one directly.

- Docs and live demos: <https://strowk.github.io/bevy_superui/>
- Source: <https://github.com/strowk/bevy_superui>

## License

Dual-licensed under either MIT or Apache-2.0, at your option.

[bevy_superui]: https://github.com/strowk/bevy_superui
[`superui`]: https://crates.io/crates/superui
