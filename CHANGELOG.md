# Changelog

All notable changes to bevy_superui are recorded here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

`bevy_superui` does not follow semantic versioning. 
Major and minor versions would follow bevy versions with these rules:
Major is 0 as long as bevy is `0.x`. Minor is the bevy minor version minus 16, i.e `bevy 0.19` -> `superui 0.3.x`.
When bevy reaches 1.0, superui will follow Major/Minor versioning exactly copying bevy's version.
Patch version is the only one incremented when superui releases changes that do not require a bevy version bump.
This means increase in major/minor version in superui will always indicate a bevy version bump and no other changes, except that needed to support the new bevy version.
Increase in patch version in superui will indicate all other changes, i.e new superui features, bug fixes, etc, that do not require a bevy version bump.

`bevy_superui` ships one release line per supported Bevy version. `main` tracks the
actively developed Bevy 0.19 line (the `0.3.x` series); the `0.1.x` (Bevy 0.17)
and `0.2.x` (Bevy 0.18) lines are frozen maintenance mirrors. This file follows
the `main` lineage only; see the [GitHub releases](https://github.com/strowk/bevy_superui/releases)
for the maintenance lines.

## [Unreleased]

### Added

- `<input type=range>` range slider: draggable and keyboard-controllable (arrow
  keys, Home/End), with `min`, `max`, `step`, and `value` attributes (value
  clamped to range; defaults to the midpoint). Fires DOM `input` while
  interacting and `change` on commit, and `.value` is readable and writable from
  JS. Renders a browser-like slider by default, fully overridable via the new
  `::slider-track`, `::slider-fill`, and `::slider-thumb` pseudo-elements.
- Text `<input>` backed by Bevy `EditableText`: real text editing with a visible
  text cursor. Placeholder text fades to the input's text color.
- `<textarea>` support as a multiline `EditableText`.
- Tailwind-compatible class utilities: utility classes resolved at build time,
  documented in the website CSS reference.
- `widgets_showcase` example demonstrating a text input, checkbox, and range
  slider, with a screenshot-based end-to-end test.

### Changed

- Keyboard focus and Tab navigation are unified on Bevy's `InputFocus`.
- Bevy 0.19 is the only actively developed line; the 0.17 and 0.18 lines are
  maintenance-only.

### Fixed

- Editable `<input>`/`<textarea>` fields no longer lose focus on mouse release. A
  field with no JS listener was treated as non-interactive and let the
  release-click fall through to the layer behind it, which stole focus on
  mouse-up; editable controls now block those picks. Button-family controls
  (`<button>`, `<input type=button|submit|reset|image>`) stay pass-through unless
  they carry a listener.
- Fixing teardown: a mounted UI is correctly cleaned up when its root entity is despawned.

## [0.3.4] - 2026-09-20

### Added

- Mouse-wheel scrolling for `overflow` nodes.

### Changed

- Swapped the JavaScript engine from Boa to V8 (native builds) and the browser's
  engine (wasm). The first native build downloads V8, so initial compilation
  needs network access.

### Fixed

- Reordering of keyed rows.

## [0.3.3] - 2026-08-14

### Added

- `PickingPolicy::Solid` to make a mounted UI block the layers behind it, for
  modals and menus.

### Fixed

- A mounted UI no longer blocks the host app's picking by default.

## [0.3.2] - 2026-08-14

### Fixed

- HMR re-exec collision via IIFE scoping.
- Reactive row-index accessors.
- Stack overflow on deeply nested JSX.

## [0.3.0] - 2026-07-26

### Added

- New engine and parser crates.

### Changed

- Target Bevy 0.19; rebased the Flair fork onto bevy_flair 0.8.0.

[Unreleased]: https://github.com/strowk/bevy_superui/compare/v0.3.4...HEAD
[0.3.4]: https://github.com/strowk/bevy_superui/compare/v0.3.3...v0.3.4
[0.3.3]: https://github.com/strowk/bevy_superui/compare/v0.3.2...v0.3.3
[0.3.2]: https://github.com/strowk/bevy_superui/compare/v0.3.0...v0.3.2
[0.3.0]: https://github.com/strowk/bevy_superui/releases/tag/v0.3.0
