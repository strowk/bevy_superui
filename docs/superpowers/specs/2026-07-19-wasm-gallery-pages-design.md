# WASM Example Gallery on GitHub Pages — Design

**Date:** 2026-07-19
**Status:** Approved design, pending implementation plan
**Goal:** A CI pipeline that builds superui example apps to WebAssembly and publishes them as a public gallery on GitHub Pages, starting with TodoMVC and designed to grow into a multi-example showcase.

## Summary of decisions

| Decision | Choice |
| --- | --- |
| Gallery scaling | Manifest-driven, build-for-N now (matrix over `examples/gallery.json`) |
| Wasm toolchain | Manual `cargo build --target wasm32` → `wasm-bindgen --target web` → `wasm-opt -Oz` |
| Render backend | WebGL2 (broadest browser compatibility) |
| Deploy trigger | `push` to `main` + `workflow_dispatch` |
| PR CI | Out of scope for now — deploy pipeline only |
| Assembly/templating logic | A Rust `xtask` workspace crate (single toolchain, testable) |

Hot reload of HTML/CSS/JS is a **native-only** superui feature and cannot work on a static wasm build (no filesystem to watch). This is surfaced to visitors as an explicit callout, not hidden.

## 1. Published site shape

Project Pages serve under `https://<user>.github.io/<repo>/`, so **all URLs are relative**.

```
/  (repo root of the Pages site)
  index.html              gallery landing page (generated from the manifest)
  todomvc/
    index.html            wasm host page (boots wasm, owns the canvas, loading UI, native-only banner)
    todomvc.js            wasm-bindgen JS glue (--target web)
    todomvc_bg.wasm       optimized binary
    assets/
      ui/todomvc/
        index.html        the superui app's own DOM (copied verbatim)
        style.css
        app.js
  <next-example>/ ...      same shape
```

**Two distinct `index.html` roles** (a known source of confusion):
- **Host page** (`todomvc/index.html`) — generated from a shared template; boots the wasm module and hosts the canvas.
- **App DOM** (`todomvc/assets/ui/todomvc/index.html`) — the superui application content, copied unchanged from the example crate.

The host page includes `<base href="./">` so Bevy's wasm asset reader resolves relative asset fetches to `/<repo>/todomvc/assets/...`. **Risk to verify during implementation:** Bevy-on-wasm asset base-path resolution is the most common failure mode; confirm the app actually fetches `assets/ui/todomvc/*` from the correct subdirectory before declaring success.

## 2. Manifest — `examples/gallery.json`

The single edit point for adding an example:

```json
{
  "examples": [
    {
      "slug": "todomvc",
      "package": "todomvc",
      "title": "TodoMVC",
      "description": "The classic TodoMVC, authored in plain HTML/CSS/JS on superui."
    }
  ]
}
```

- `slug` — URL subdirectory and artifact name.
- `package` — cargo package name (`cargo build -p <package>`).
- `title` / `description` — shown on the gallery card and host-page banner.
- **Assets path is derived by convention** as `examples/<slug>/assets` (no field needed), keeping the manifest minimal.

Adding example #2 = append one object and ensure the crate exists. No workflow changes.

## 3. `xtask` crate

A small workspace crate `xtask` owns all templating/assembly so it is cross-platform (Windows dev + Linux CI) and unit-testable. Subcommands:

- `xtask host-page --slug <slug> --out <dir>` — render the shared host template for one example into `<out>/index.html`, substituting title, description, wasm JS/module names.
- `xtask gallery-index --out <file>` — read the manifest and render the root gallery landing page (one card per example, linking to `./<slug>/`).

Templates live under `tools/gallery/` (e.g. `host.html.tmpl`, `gallery.html.tmpl`) or embedded in the crate — implementer's choice, but the substitution logic is testable Rust either way. The wasm build itself (cargo/wasm-bindgen/wasm-opt) is orchestrated by the workflow, not xtask; xtask only handles page assembly and index generation.

## 4. Deploy workflow — `.github/workflows/deploy-pages.yml`

**Triggers:** `push` to `main`, `workflow_dispatch`.
**Permissions:** `pages: write`, `id-token: write`, `contents: read`.
**Concurrency:** single-flight group so overlapping deploys don't race.

Three jobs:

1. **discover** — read `examples/gallery.json`, emit a matrix via `fromJSON` of `{ slug, package }` objects.
2. **build** (matrix, one run per example):
   - Install `wasm32-unknown-unknown` target and the toolchain.
   - Restore cache with `Swatinem/rust-cache`.
   - `cargo build -p <package> --target wasm32-unknown-unknown --release`.
   - `wasm-bindgen --target web --out-dir <stage>/<slug> --out-name <slug> <wasm>`.
   - `wasm-opt -Oz` the emitted `*_bg.wasm` in place.
   - `cargo run -p xtask -- host-page --slug <slug> --out <stage>/<slug>`.
   - Copy `examples/<slug>/assets` → `<stage>/<slug>/assets`.
   - Upload `<stage>/<slug>` as an artifact named for the slug.
3. **assemble-and-deploy** — download all example artifacts into `dist/`, run `cargo run -p xtask -- gallery-index --out dist/index.html`, then `actions/upload-pages-artifact` (path `dist/`) → `actions/deploy-pages`.

The matrix is intentional headroom: overkill for one example, free scaling to a full gallery.

## 5. Footguns designed around (must be handled, not hand-waved)

- **`file_watcher` breaks the wasm build.** `examples/todomvc/Cargo.toml` currently sets `bevy = { features = ["file_watcher"] }` unconditionally; the `notify` crate underneath does not build on wasm. Split bevy deps by target:
  - native: `features = ["file_watcher"]` (preserves hot reload)
  - wasm: `features = ["webgl2"]`
  This split *is* the technical reason hot reload is native-only.
- **wasm-bindgen version match.** `wasm-bindgen-cli` must exactly match the `wasm-bindgen` crate version Bevy pulls, or linking fails. CI pins/derives the exact version from `Cargo.lock` rather than installing "latest".
- **Canvas configuration.** For a clean web layout, `main.rs` needs a wasm-only window config, e.g. `Window { canvas: Some("#superui-canvas".into()), fit_canvas_to_parent: true, .. }`, guarded by `#[cfg(target_arch = "wasm32")]`. The host page provides the matching `<canvas id="superui-canvas">`.
- **Binary size / load time.** Bevy wasm binaries are large (tens of MB). Mitigate with release profile, `wasm-opt -Oz`, and a loading spinner in the host page so visitors see progress rather than a blank canvas.

## 6. "No hot reload on wasm" messaging

Both the gallery card and each host page carry a short banner:

> ▶ This is a static WebAssembly build. Live hot-reload of HTML/CSS/JS is a **native-only** superui feature — `git clone … && cargo run -p todomvc` to try it.

This frames the limitation as a deliberate native capability rather than a silent gap.

## 7. First deliverable & scope

- Only `todomvc` is wired end-to-end.
- Manifest, workflow, xtask, and templates are all N-ready from day one.
- Definition of done: a green deploy publishing a working TodoMVC wasm build to the Pages URL, with correct asset loading (§1 risk verified) and the native-only banner visible.
- Adding example #2 later is a one-object manifest edit.

## Out of scope

- PR CI (fmt/clippy/test) — may be added later as a separate workflow.
- WebGPU backend.
- Any attempt to make hot reload work on wasm.
- Thumbnails/screenshots on gallery cards (can be added to the manifest later).

## Prerequisites (user-owned, outside CI)

- Create the GitHub repo and push.
- Settings → Pages → Source = "GitHub Actions".
