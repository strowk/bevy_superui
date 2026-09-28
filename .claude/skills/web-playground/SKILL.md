---
name: web-playground
description: Use when adding a new in-browser playground example (a live-editable .tsx/.css superui demo) to the bevy_superui docs, embedding a playground on another docs page, or changing the playground host page, editor, or build wiring.
---

# Web playground: live-editable demos in the docs

## Overview

A web playground embeds a running superui demo in the docs with its `.tsx`/`.css`
in a CodeMirror editor: edit, press Run (or Ctrl+Enter), and the demo hot-reloads
in-browser — `.tsx` state-preserving, `.css` live re-cascade. The transpiler
(`oxc`) and the class-utility generator (`encre-css`) run in wasm, driving
superui's native hot-reload seam.

A docs page embeds it as one lazy `<iframe>` pointing at a generated host page.
The host page is reusable across examples and pages.

## The pieces

| Piece | Where | Role |
| --- | --- | --- |
| Seam | `crates/superui_playground_web` | wasm exports `apply_source` / `apply_utilities` / `poll_diagnostics` + `PlaygroundBridgePlugin`; drives the hot-reload seam |
| Example | `examples/<slug>` | a normal example with a `playground` feature (see `counter`, `styling_showcase`) |
| Host template | `tools/gallery/playground.html.tmpl` | the `<canvas>` + editor page; tokens `{{SLUG}}`/`{{WASM_JS}}`/`{{SOURCES_JSON}}` |
| Generator | `xtask playground-page --slug <s> --out <dir>` | fills the template (manifest-free; no `gallery.json` entry needed) |
| Shared assets | `website/src/assets/playground.{css,js}` | editor bootstrap, Run wiring, layout, theming — authored once, all playgrounds share it |
| Editor | `tools/gallery/vendor/codemirror/` | CodeMirror 5 vendored (committed here; mirror under `website/src/examples/vendor/` is gitignored) |
| Build | `tools/build-demos.sh` | builds each slug's wasm + generates its host page into `website/src/examples/<slug>/` |
| Embed | a docs `.md` | hand-written `<iframe>` in a window-frame wrapper |

## Add a new playground example

1. **Crate.** Copy `examples/counter/{Cargo.toml,src/main.rs,build.rs,.gitignore}`
   to `examples/<slug>/`. Rename `counter` → `<slug>`; change the asset dir
   string to `ui/<slug>`. Keep the `playground` feature verbatim:
   ```toml
   playground = ["superui/transpiler", "superui/hmr", "dep:superui_playground_web", "superui_playground_web/utilities"]
   ```
   `main.rs` gates two things on `#[cfg(feature = "playground")]`: the
   `watch_for_changes_override: Some(true)` in `web_asset_plugin`, and adding
   `PlaygroundBridgePlugin`. Workspace `members` globs `examples/*` — no edit.

2. **Assets** under `examples/<slug>/assets/ui/<slug>/`: `index.html` (a Model 2
   manifest — `<link rel=stylesheet href=style.css>` + `<script type=module
   src=app.tsx>` + `<div id="root">`), `app.tsx`, `style.css`. One `.tsx`
   module only (the transpiler strips cross-module imports). Respect flair-0.6
   CSS limits and **do not add** the utilities `@import` (see Gotchas).

3. **Build wiring — two places:**
   - **Deploy (required):** add the slug to the `playgrounds` array in
     `examples/gallery.json`. The deploy workflow builds every `playgrounds`
     entry with `--features playground` and `xtask playground-page`, overlaying
     `playground.html` + wasm under `dist/examples/<slug>/`. Without this the
     docs iframe 404s in production. (`playgrounds` is separate from `examples`,
     so it is not a gallery card.)
   - **Local serve:** in `tools/build-demos.sh` add
     `[<slug>]="--features playground"` to `BUILD_ARGS`, `[<slug>]=1` to the
     `PLAYGROUND` map (routes it to `playground-page`), and add `<slug>` to the
     default `slugs=( … )` list.

4. **Generate + verify:** `bash tools/build-demos.sh <slug>`, then
   `mdbook serve website`. Commit the crate + assets + the `build-demos.sh`
   edit; the wasm/host page under `website/src/examples/<slug>/` is gitignored.

Native dev loop (live `.tsx` in a window, no browser): `cargo run -p <slug> --features hmr`.

**Prerequisite:** a playground demo must be a single supersolid `.tsx` module. An
example whose UI is native Rust, or split across imported `.tsx` files, is not
playground-able until its UI is a single `app.tsx`.

**Retrofitting an existing example** (e.g. making a gallery demo editable):
don't copy `counter` wholesale — instead add to the example's own `Cargo.toml`
the `playground` feature line above and the optional `superui_playground_web`
dep, add the two `#[cfg(feature = "playground")]` gates to its `main.rs`
(mirroring `counter`), and do step 3 (`build-demos.sh`). Its assets already
exist; only confirm they meet the constraints above (single module, no
`@import`, flair-0.6 CSS).

## Embed on a docs page

Hand-write the wrapper + iframe in the `.md` (no preprocessor). Copy the block
in `website/src/docs/concepts/styling.md`; change the name, and fix the iframe
`src` for the page's depth:

```html
<div class="su-playground-embed su-playground-embed--stack">
  <div class="su-playground-bar">
    <span class="su-playground-dot"></span>
    <span class="su-playground-name">My playground</span>
    <span class="su-playground-live">Live &middot; edit &amp; run</span>
  </div>
  <iframe src="../../examples/<slug>/playground.html?layout=stack"
          title="Live playground" loading="lazy"></iframe>
</div>
```

- **Layout:** `?layout=stack` = canvas band above a full-width editor (best in a
  narrow docs column); omit it or use `?layout=split` for side-by-side. The
  `su-playground-embed--stack` wrapper class sets the taller height — pair it
  with `?layout=stack`.
- **Path:** `../../examples/…` is correct from `/docs/concepts/`; adjust the
  `../` depth per page.
- The `.su-playground-*` styles (frame, bar, height) live in
  `website/theme/css/site.css`.

## How Run works (in `playground.js`)

Order is load-bearing: `apply_source("style.css", …)` → `apply_utilities([tsx,
html])` → `apply_source("app.tsx", …)`, auto-run once after boot. Transpile
diagnostics come back from `apply_source`; CSS parse + JS runtime errors arrive
via the `poll_diagnostics` interval. The console clears on each manual Run.

## Gotchas (each of these has bitten)

- **Artifacts are gitignored.** `.gitignore` ignores `website/src/examples/*/`
  and `.../vendor/`. No demo commits its wasm — `build-demos.sh` regenerates it.
  Commit source + the script only.
- **mdBook needs a restart to see example changes.** It copies `src/examples/`
  only at startup, and browsers cache the iframe's assets hard. After
  regenerating: restart `mdbook serve`, then DevTools → Network → **Disable
  cache** (or hard-refresh the `playground.html` URL directly). `.md`/`site.css`
  changes show on a normal refresh; template/`playground.{css,js}` changes do not.
- **No `@import` in the demo's `style.css`.** flair's asset loader fails the
  *whole* sheet at mount when the imported file is absent (nothing runs
  `write_generated` here), and the playground's inline parser rejects `@import`.
  Utilities come from `apply_utilities`.
- **flair 0.6 CSS limits:** no `cursor`, no `filter`, `border-width: 0` (not
  `border: none`), no `font-family` naming an unloaded font (voids the sheet),
  and no *logical* padding/margin shorthands — `px-4`/`py-2` are rejected; use
  `pl-4 pr-4 pt-2 pb-2`.
- **Verify utility classes with `superui_css_utilities::expand()`**, not a
  `cargo build | grep skip` (the utility oracle is a runtime system, so the
  compile-time grep never fires).
- **The utility scanner reads visible text.** Comments and labels are scanned, so
  a bare Tailwind display word (`inline`, `block`, `flex`, `grid`, `hidden`) in
  prose emits a spurious diagnostic. The styling demo labels the technique
  "style attribute", not "inline style", for this reason.
- **Editor highlighting is theme-class-agnostic** (`.CodeMirror .cm-*` in
  `playground.css`) on purpose — do not scope it to a `cm-s-*` theme name, or a
  cached JS/CSS version skew blanks the colors.
- **Canvas focus.** winit auto-focuses the canvas, which steals the host page's
  keyboard (arrow-key nav). The template neutralizes the load-time grab and
  re-enables `focus()` on `pointerenter` — do not replace it with a plain
  `focus()`; picking needs canvas focus, so clicks would stop working.

## Deploy

`.github/workflows/deploy-pages.yml` builds every `gallery.json` `playgrounds`
entry with `--features playground` + `xtask playground-page` and overlays it
under `dist/examples/<slug>/`. `playground.{css,js}`/`blueprint.css` ride along
in `dist/assets/` and the CodeMirror vendor in `dist/examples/vendor/`, so the
docs iframe resolves. Adding a playground is only live once its slug is in
`playgrounds`.
