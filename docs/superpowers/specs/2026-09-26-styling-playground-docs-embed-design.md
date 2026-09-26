# Styling playground: a live, editable demo embedded in the docs

- **Date:** 2026-09-26
- **Status:** Approved design, pending implementation plan
- **Scope:** a new `examples/styling_showcase` playground example; a reusable
  playground host page (`tools/gallery/playground.html.tmpl` + an `xtask
  playground-page` subcommand + shared `playground.css`/`playground.js` + vendored
  CodeMirror); an `<iframe>` embed on `website/src/docs/concepts/styling.md`.
- **Deliberately local-first:** the demo runs under `mdbook serve` with committed
  artifacts and is verified in a real browser. Wiring the CI/deploy matrix is a
  labeled follow-up (see Non-goals).

## Why

The styling docs page (`website/src/docs/concepts/styling.md`) explains three ways to
style a superui UI — inline `style`, authored CSS, and utility classes — as prose.
The playground seam (the original spec's sub-project **A**,
`2026-07-24-web-playground-01-transpile-hotreload-seam-design.md`) is landed on `main`:
`superui_playground_web` exports `apply_source` / `apply_utilities` /
`poll_diagnostics`, and `examples/counter` is the reference wiring. This spec spends
that seam on the first real docs embed — a live, editable demo at the end of the
styling page where a reader edits the `.tsx`/`.css` and sees the three techniques
change in-browser, with signal state preserved across `.css` edits.

This is the original effort's sub-project **B** (the playground page), narrowed to one
docs page and one new example, and built so a second embed elsewhere is one command
plus one snippet rather than a copy-paste.

## Product decisions (fixed during brainstorming)

- **Embedding:** an `<iframe>` to a generated per-example host page — the pattern the
  landing counter already uses (`web-embed.html` → `embed.html`, lazy-loaded, posts
  `superui:ready`). No new mdBook preprocessor. Reuse elsewhere = paste the iframe
  snippet (fixing relative depth) and generate that example's host page.
- **Scope:** local-first. Build the wasm by hand, commit the artifacts under
  `website/src/examples/styling_showcase/`, verify in a browser. CI is a follow-up.
- **Editor:** a rich editor — CodeMirror **5**, vendored as static files (no bundler),
  mirroring how `highlight.js` is already vendored under
  `website/src/examples/vendor/`.
- **Layout:** canvas and editor **side-by-side** inside the iframe (the existing
  Examples host page toggles VIEW/PARTS — one or the other; this shows both at once),
  stacking vertically on narrow screens.
- **Demo concept:** three sibling "chips" in a row, each labeled with the technique
  that styles it, plus one shared reactive signal so state-preservation across a `.css`
  edit is visible.

## Non-goals

- **CI / deploy rollout.** Extending `.github/workflows/deploy-pages.yml` to build
  `styling_showcase --features playground` and ship it, and deciding whether it also
  appears in the Examples gallery grid, are the original effort's sub-project **C** and
  a separate task.
- **A generic mdBook shortcode/preprocessor** for playground embeds. The iframe snippet
  is hand-written per page for now.
- **Changes to the playground seam.** `apply_source` / `apply_utilities` /
  `poll_diagnostics` and the bridge are consumed as-is.
- **Module resolution / multi-file imports.** The example stays single-entry, single
  `.tsx` module (the transpiler strips cross-module imports).

## The example: `examples/styling_showcase`

A new example crate that mirrors `examples/counter`'s playground wiring.

### Crate wiring (copied from `counter`)

`Cargo.toml`:

```toml
[features]
hmr = ["superui/hmr", "bevy/file_watcher"]
playground = ["superui/transpiler", "superui/hmr", "dep:superui_playground_web", "superui_playground_web/utilities"]

[dependencies]
superui_playground_web = { path = "../../crates/superui_playground_web", version = "0.3.5", optional = true }
```

`src/main.rs` reuses counter's two-place gate verbatim: `web_asset_plugin` forces
`watch_for_changes_override: Some(true)` under `#[cfg(feature = "playground")]` (the HMR
gate needs `watching = true`; no file watcher runs on wasm — the bridge fires
`AssetEvent::Modified` itself), and `PlaygroundBridgePlugin` is added under the same
cfg. `setup` spawns a `Camera2d` and `SuperUiRoot::from_asset_dir("ui/styling_showcase",
&assets)`. `web_window` binds the primary window to `#superui-canvas`.

A `build.rs` mirroring counter's (host-only pre-transpile of `app.tsx` →
`.superui/build/app.js`) is included so `cargo run -p styling_showcase` works natively
without `--features hmr`. It is a dev convenience, not used by the playground build
(which loads `.tsx` live via `live_source()`).

The `playground` feature deliberately omits `bevy/file_watcher` (there is no watcher on
wasm). Normal builds stay oxc-free — guarded by `cargo tree -p styling_showcase --target
wasm32-unknown-unknown -i oxc` being empty without `--features playground`.

### Assets: `assets/ui/styling_showcase/`

`index.html` (Model 2 manifest, same shape as counter):

```html
<html>
  <head>
    <link rel="stylesheet" href="style.css">
    <script type="module" src="app.tsx"></script>
  </head>
  <body>
    <div id="root"></div>
  </body>
</html>
```

`app.tsx` — a single module (imports stripped), one `Row` component rendering three
labeled chips plus a shared control:

- A shared signal, e.g. `const [n, setN] = createSignal(0)` with a small +/− control.
  Its value is what must survive a `.css` edit.
- **Chip A — inline `style`:** `<div class="chip" style={`width: ${120 + n()*8}px`}>`
  with an "inline style" label. Demonstrates the runtime-computed case: the width is
  driven by the signal, so no utility class or stylesheet rule could carry it. Editing
  `app.tsx` → state-preserving reload.
- **Chip B — authored CSS:** `<div class="chip chip-authored">` with an "authored CSS"
  label, styled entirely by a `.chip-authored` rule in `style.css`. Editing `style.css`
  → live restyle, signal preserved.
- **Chip C — utility classes:** `<div class="chip flex items-center px-4 py-2 rounded
  bg-slate-800 text-teal-300">` (final class list finalized in implementation against
  the class-utilities catalog) with a "utility classes" label. Regenerated in-browser by
  `apply_utilities`.

`style.css` — within flair-0.6 limits (the constraints documented in counter's
`style.css`: no `cursor`, no `filter`, `border-width: 0` not `border: none`, no
`font-family` referencing an unloaded font). Contains `#root` layout, a base `.chip`
rule, the `.chip-authored` rule (the live-restyle target), the `+/-` control styling,
and a `:hover` (nodes spawn with a `Hovered` component). It carries the
`@import ".superui/build/utilities.generated.css";` line on line 1 for native
consistency; that import is inert in the playground (the inline CSS parser does not
support `@import`), where utilities come from `apply_utilities` — see call order below.

Authoring gotcha to respect: control-flow (`<For>`/`<Show>`) is not needed here; the
demo is static structure, so no bare-control-flow wrapping concerns apply.

## Reusable playground host page

### Split of responsibilities

- **Shared, authored once:**
  - `website/src/assets/playground.js` — the editor bootstrap, tab switching, Run
    wiring, and diagnostics polling. Reads a slug and the initial file contents from a
    `window.__PLAYGROUND__` object the generated page injects.
  - `website/src/assets/playground.css` — the split-pane layout, blueprint-theme chrome,
    editor/console styling, responsive stacking.
  - `website/src/examples/vendor/codemirror/` — CodeMirror 5 bundle + `javascript`,
    `css`, `jsx` modes and one theme, as static files. Source of truth
    `tools/gallery/vendor/codemirror/` (mirrors the existing `tools/gallery/vendor`
    highlight.js vendoring; CI already copies `tools/gallery/vendor` →
    `dist/examples/vendor`).
- **Per-example, generated:** `playground.html` — a thin page carrying only `{{SLUG}}`
  (for the wasm-bindgen import) and `{{SOURCES_JSON}}` (the initial file contents). It
  references the shared assets and vendored CodeMirror by relative path.

### Generation: `xtask playground-page`

A new subcommand alongside the existing `host-page`, reusing `xtask/src/sources.rs` to
enumerate/classify the authored files under `examples/<slug>/assets/ui/<slug>/` into the
`{{SOURCES_JSON}}` array, and substituting `{{SLUG}}`:

```
cargo run -p xtask -- playground-page --slug styling_showcase --out website/src/examples/styling_showcase
```

Template `tools/gallery/playground.html.tmpl`, structured like `host.html.tmpl` but for
the playground: a `<canvas id="superui-canvas">`, the editor pane, and the wasm loader.
The wasm import matches host.html.tmpl's pattern:

```html
<script type="module">
  import init, { apply_source, apply_utilities, poll_diagnostics } from './{{WASM_JS}}';
  window.__PLAYGROUND__ = { slug: "{{SLUG}}", sources: {{SOURCES_JSON}},
                            apply_source, apply_utilities, poll_diagnostics };
  init().catch(/* ignore winit control-flow throw */).finally(() => { /* remove loader; postMessage superui:ready */ });
</script>
<script src="../assets/playground.js" type="module"></script>
```

### Run wiring (call order is load-bearing)

`playground.js` follows the manual's rules exactly, or utilities/authored CSS drop for a
frame:

1. Seed authored CSS: `apply_source("style.css", cssBuffer)`.
2. Regenerate utilities from **all** class-bearing buffers:
   `apply_utilities(JSON.stringify([tsxBuffer, htmlBuffer]))`.
3. Re-transpile + hot-swap: `apply_source("app.tsx", tsxBuffer)`.

Run once after `init()` resolves (so the first paint has utilities + authored CSS), then
again on each Run-button click. `poll_diagnostics()` on a ~500ms interval appends
transpile diagnostics, CSS parse errors, and JS runtime errors to the console pane.

Editable tabs: `app.tsx` and `style.css` (the two styling surfaces). `index.html` is not
exposed as an editor tab (editing it is a full remount / state loss, and it is not a
styling surface). Pre-mount edits are dropped with a diagnostic; the auto-run fires after
mount.

### Loader + parent messaging

The page shows a loader until the (large — playground wasm adds ~730 KiB gz transpiler +
~140 KiB gz utilities over a normal demo) wasm boots, then removes it and posts
`superui:ready` to the parent, matching `embed.html`, so the docs-page frame can reveal
it smoothly.

## Embedding on `styling.md`

At the end of the page (after the existing content), a short lead-in tying the demo to
the three techniques already described, then a raw-HTML block:

```html
<div class="su-playground-embed">
  <iframe src="../../examples/styling_showcase/playground.html"
          title="Live styling playground" loading="lazy"></iframe>
</div>
```

The relative path resolves from `/bevy_superui/docs/concepts/styling.html` to
`/bevy_superui/examples/styling_showcase/playground.html`. A `.su-playground-embed` rule
(fixed height, blueprint border, responsive) is added to `website/theme/css/site.css`
(or `assets/blueprint.css`, chosen in implementation to match where comparable
demo-embed rules already live). `loading="lazy"` defers the heavy wasm until the reader
scrolls to it.

## Build & local serve

Scripted, per the manual's recipe:

```bash
cargo build -p styling_showcase --release --target wasm32-unknown-unknown --features playground
wasm-bindgen --no-typescript --target web \
  --out-dir website/src/examples/styling_showcase --out-name styling_showcase \
  target/wasm32-unknown-unknown/release/styling_showcase.wasm
# confirm exports survived DCE:
grep -oE "apply_source|apply_utilities|poll_diagnostics" website/src/examples/styling_showcase/styling_showcase.js | sort -u
wasm-opt -Oz --enable-reference-types --enable-bulk-memory \
  -o website/src/examples/styling_showcase/styling_showcase_bg.wasm \
  website/src/examples/styling_showcase/styling_showcase_bg.wasm
cargo run -p xtask -- playground-page --slug styling_showcase --out website/src/examples/styling_showcase
cp -r examples/styling_showcase/assets website/src/examples/styling_showcase/assets
mdbook serve website   # open the styling page, scroll to the embed
```

Committed under `website/src/examples/styling_showcase/` (the repo already commits
per-example wasm copies for local serve): `styling_showcase.js`, `styling_showcase_bg.wasm`,
`playground.html`, and `assets/`. Shared `playground.css`/`playground.js` and the
CodeMirror vendor go in their shared locations above.

## Verification

- **Bridge seam:** already covered by `superui_playground_web`'s native tests; unchanged.
- **`cargo tree` guard:** oxc absent from a normal `styling_showcase` wasm build, present
  with `--features playground`.
- **Browser smoke (automated via chrome-devtools MCP):** serve, load the styling page,
  scroll to the embed, screenshot. Then, in the editor:
  - increment the shared signal (Chip A grows), edit `style.css` (e.g. change
    `.chip-authored` background), Run → screenshot shows Chip B restyled **and** the
    signal value unchanged (state preserved across CSS restyle).
  - edit `app.tsx` (e.g. add a chip or change a label), Run → screenshot shows the
    structural change applied.
  - introduce a deliberate syntax error in `app.tsx`, Run → the console pane shows a
    transpile diagnostic and the demo stays alive.
- **No native regression:** `cargo run -p styling_showcase` (native) still renders; the
  build.rs pre-transpile path is exercised.

## Risks & watch-items

- **Recursive-descent parser stack on wasm.** `styling_showcase` is small, so mount-time
  transpile is a non-issue (same as counter).
- **CodeMirror 5 JSX mode fidelity.** TSX highlighting via the `jsx` mode is approximate;
  acceptable for a docs demo. If it misbehaves, fall back to the `javascript` mode.
- **First-load latency.** The playground wasm is large and `http.server`/`mdbook serve`
  is slow on first load; `loading="lazy"` and the loader mitigate the perceived cost.
- **Call-order regressions.** If authored styles flash-disappear for a frame, the seed
  order (authored CSS before `apply_utilities`) is wrong — the single most likely bug,
  called out in the manual.

## Ordering

1. Example crate + assets (`styling_showcase`), building natively.
2. Vendor CodeMirror; author `playground.css` / `playground.js`.
3. Template + `xtask playground-page`.
4. Build the playground wasm; generate + commit artifacts.
5. Iframe embed + `.su-playground-embed` CSS on the styling page.
6. Browser verification via chrome-devtools MCP.
