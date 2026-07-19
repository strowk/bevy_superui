# bevy superui

Bevy SuperUI is a crate for bevy to write game UI's using browser-like HTML/CSS/JS stack coupled with first class support for solid-style TSX components and powerful hot reload.

It is built on top of bevy_ui (inheriting some of its limitations) and incorporates somewhat modified bevy_flair for CSS support.

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

