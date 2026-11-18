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
actively developed Bevy 0.20 line (the `0.4.x` series); the `0.3.x` (Bevy 0.19) line
is a maintained branch (`release/bevy-0.19`), and the `0.1.x` (Bevy 0.17) and `0.2.x`
(Bevy 0.18) lines are frozen maintenance mirrors. This file follows the `main`
lineage only; see the [GitHub releases](https://github.com/strowk/bevy_superui/releases)
for the maintenance lines.

## [Unreleased]

### Added

- Cross-file module imports: `import { X } from "./other"` resolves, loads, and
  bundles the whole import graph instead of stripping the import. Relative
  specifiers (`./widget`, extension optional — `.tsx`/`.ts`/`.js`, then
  `/index.*`) and bare packages (resolved from a sibling `superui_modules/`
  directory, which no tooling populates yet) both resolve; each module's
  `export` / `export default` is wired through a CommonJS-style registry, so one
  UI can split across sibling files — one module per screen, shared widgets and
  data in their own files. Editing any file in the graph, not just the entry,
  hot-reloads. `export * from` re-exports and dynamic `import()` warn and are
  skipped; `import.meta` and `package.json` `main`/`module` resolution are
  unsupported. An unresolved module import warns and the bundle still builds
  (runtime/HMR and build), but is fatal in the end-to-end test engine.
- Module `.css` imports: a `.css` imported by any module in the graph is
  applied after the entry's `<link>` stylesheet, so an equal-specificity module
  rule wins the cascade. Editing it hot-reloads like the rest of the graph.
- Browser-faithful mouse events: `mouseover`/`mouseout` (bubbling),
  `mouseenter`/`mouseleave` (non-bubbling), and `mousedown`/`mouseup`/`mousemove`,
  each carrying `clientX`/`clientY`/`button`/`buttons`. `document` and `window`
  are now real event targets (`addEventListener`/`removeEventListener`) and sit
  atop the bubble chain for every bubbling event, so a drag started with
  `mousedown` on an element can keep tracking `mousemove`/`mouseup` at
  `document`/`window` — with `event.target` still the node under the pointer —
  even after the cursor leaves that element. A new `chest_inventory` example
  (`examples/inventory`) demonstrates chest-to-inventory drag-and-drop built on
  these primitives.
- Modifier keys on events: mouse events (`click`, `mousedown`/`up`/`move`,
  `mouseover`/`out`/`enter`/`leave`) and keyboard events (`keydown`/`keyup`)
  now carry `shiftKey`/`ctrlKey`/`altKey`/`metaKey`, sourced from live key
  state at input time — so a handler can branch on Shift/Ctrl/Alt/Cmd, e.g.
  `transfer(e.shiftKey ? 100 : 1)` or Shift-click to quick-move. Non-input
  events (`change`/`input`/`focus`/`blur`) carry none, matching the browser.
  The `chest_inventory` example gains Shift-click quick-transfer.
- HMR now preserves module-top-level signals across a hot reload, not just
  signals created inside components. A `createSignal` at the top level of a
  module (e.g. app-wide navigation state held outside the component tree) keeps
  its current value when you edit and reload, instead of resetting to its
  initial value. State is keyed per module path, so each file in an import graph
  — including nested imported modules — rehydrates its own top-level signals.
  Only signals created during a module's top-level execution are preserved; a
  signal first created later (e.g. deferred from an event handler) still resets.
  Active whenever HMR is on; the shipped (non-HMR) and wasm paths are unaffected.

## [0.4.0] - 2026-10-10

### Changed

- Target Bevy 0.20; rebased the Flair fork onto bevy_flair 0.9.

## [0.3.6] - 2026-10-10

### Added

- `<img>` tag: reconciled into a bevy_ui `ImageNode`. `src` resolves relative to
  the document's own directory (recorded on `UiRuntime` as its base dir, the same
  way CSS and JS links already resolve), loads as a `Handle<Image>`, and is
  load-guarded so a stable `src` loads only once. Sizing is CSS-driven
  (`NodeImageMode::Auto`); `object-fit` is intentionally out of scope until
  bevy_ui gains fit modes.
- `cargo superui test`: the end-to-end test runner is now included as a subcommand 
  of the CLI.
- `UiRuntime::take_errors`: drains uncaught JavaScript evaluation errors captured
  since the last call. Previously these were logged and discarded; now apps and
  tooling can read them to surface a failing author script.
- `transpiler` cargo feature on `superui`: compiles the oxc `.tsx` transpiler into
  the build (including wasm), enabling in-browser transpilation for live
  playgrounds.
- `page.emit(name, value)` in the end-to-end test engine: a spec can deliver a
  game→UI bridge event to the UI's `bevy.on(name, …)` handlers (the counterpart
  of the game's `commands.trigger`), so a headless test with no game side can
  drive and assert the UI's reaction to bridge data.
- `.json` imports: `import x from "./data.json"` — default, `import * as x`, and
  `import { x }` all bind the whole value — inlines the file's parsed value as
  a top-level `const x`, resolved relative to the importing file. Only the
  whole value binds; picking a sub-path or member out of the JSON is out of
  scope, and other cross-file imports are still stripped. A missing or invalid
  file is fatal in the end-to-end test engine (the spec fails to transpile)
  and a skipped-binding warning everywhere else (runtime/HMR and build);
  editing the JSON hot-reloads the `.tsx` that imports it.

### Changed

- `cargo-superui` parses its arguments with `argh` instead of hand-rolled flag
  parsing.
- Test engine `--ui` mode: specs are shown as a tree with their tests nested
  underneath; each spec header carries a status icon (spinner while running,
  check/cross once finished, hollow dot when not yet run) and each test a
  pass/fail icon. Time-travel now records a frame per step, so the slider moves
  the rendered image together with the DOM, and selecting a test jumps to its
  final step. The shell starts at a slightly larger zoom.

### Fixed

- The test engine now loads images, fonts, and class utilities, so
  screenshot-based end-to-end tests render the same as the running app.

## [0.3.5] - 2026-09-25

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

[Unreleased]: https://github.com/strowk/bevy_superui/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/strowk/bevy_superui/compare/v0.3.6...v0.4.0
[0.3.6]: https://github.com/strowk/bevy_superui/compare/v0.3.5...v0.3.6
[0.3.5]: https://github.com/strowk/bevy_superui/compare/v0.3.4...v0.3.5
[0.3.4]: https://github.com/strowk/bevy_superui/compare/v0.3.3...v0.3.4
[0.3.3]: https://github.com/strowk/bevy_superui/compare/v0.3.2...v0.3.3
[0.3.2]: https://github.com/strowk/bevy_superui/compare/v0.3.0...v0.3.2
[0.3.0]: https://github.com/strowk/bevy_superui/releases/tag/v0.3.0
