# WASM Example Gallery on GitHub Pages — Design

**Date:** 2026-07-19
**Status:** Approved design, pending implementation plan
**Goal:** A CI pipeline that builds superui example apps to WebAssembly and publishes them as a public GitHub Pages gallery, each demo showing the running app **alongside its source code**. Ships with five examples across two categories.

## Summary of decisions

| Decision | Choice |
| --- | --- |
| Gallery scaling | Manifest-driven, build-for-N (matrix over `examples/gallery.json`) |
| Examples shipped | `todomvc`, `todomvc_supersolid`, `game_menu` (Apps); `citadel`, `horde` (Stress tests) |
| Categories | `category` field per example groups cards under headings; optional `tags` render as per-card badges (e.g. horde → "Playable game") |
| Wasm toolchain | Manual `cargo build --target wasm32` → `wasm-bindgen --target web` → `wasm-opt -Oz` |
| Render backend | WebGL2 (broadest browser compatibility) |
| Deploy trigger | `push` to `main` + `workflow_dispatch` |
| PR CI | Out of scope for now — deploy pipeline only |
| Assembly/templating logic | A Rust `xtask` workspace crate (single toolchain, testable) |
| Code viewer | Live app + tabbed source panel, responsive split; vendored (no-CDN) highlighter |
| Code shown | **Authored** source (`.tsx`/`.css`/`.html`, or `.js` for the plain example); generated/tooling files hidden |
| Demo routing | Per-app stable URL at `/<slug>/`; no tab/file hash state. README table hand-written |

Hot reload of the UI is a **native-only** superui feature and cannot work on a static wasm build (no filesystem to watch). This is surfaced to visitors as an explicit callout, not hidden.

## 0. Example inventory (what actually ships)

Four of five examples are authored in **Supersolid TSX**: an `app.tsx` that a host-side `build.rs` transpiles to `app.generated.js` (the file the wasm app actually loads at runtime). The fifth is plain HTML/CSS/JS. All five already carry the wasm `getrandom` shim and keep `file_watcher` native-only, so they compile to `wasm32-unknown-unknown`.

| slug | package | kind | authored source shown | category | tags |
| --- | --- | --- | --- | --- | --- |
| `todomvc` | todomvc | plain HTML/CSS/JS | `index.html`, `style.css`, `app.js` | Apps | — |
| `todomvc_supersolid` | todomvc_supersolid | Supersolid TSX | `app.tsx`, `index.html`, `style.css` | Apps | — |
| `game_menu` | game_menu | Supersolid TSX | `app.tsx`, `index.html`, `style.css` | Apps | — |
| `citadel` | citadel | Supersolid TSX | `app.tsx`, `index.html`, `theme.css` | Stress tests | — |
| `horde` | horde | Supersolid TSX | `app.tsx`, `index.html`, `components.css`, `theme.css` | Stress tests | Playable game |

The two TodoMVCs are intentionally both present: same app, different authoring style, different source on display. `horde` is grouped under Stress tests but is also **an actual playable game** (survivors-like: move, shoot, level up), so it carries a "Playable game" badge to set it apart from the pure benchmark sims.

**Stress-test caveat:** `citadel` and `horde` are deliberately heavy (large sims, many reactive nodes). In-browser they produce large wasm binaries and may run at reduced frame rates on weaker machines — that is the point of the category, and the gallery says so.

## 1. Published site shape

Project Pages serve under `https://<user>.github.io/<repo>/`, so **all URLs are relative**.

```
/  (repo root of the Pages site)
  index.html              gallery landing page (category sections, generated from the manifest)
  vendor/
    highlight.min.js      vendored syntax highlighter (shared, no CDN)
    highlight.css         highlighter theme
  todomvc/
    index.html            wasm host page: split live-app + tabbed code viewer, loading UI, native-only banner
    todomvc.js            wasm-bindgen JS glue (--target web)
    todomvc_bg.wasm       optimized binary
    assets/ui/todomvc/    the app's files, copied verbatim (index.html, style.css, app.js)
  todomvc_supersolid/
    index.html  todomvc_supersolid.js  todomvc_supersolid_bg.wasm
    assets/ui/todomvc_supersolid/   app.tsx, app.generated.js, index.html, style.css, …
  game_menu/ …  citadel/ …  horde/ …    same shape
```

**Two distinct `index.html` roles** (a known source of confusion):
- **Host page** (`<slug>/index.html`) — generated from a shared template; boots the wasm module, hosts the canvas, and renders the code viewer.
- **App DOM** (`<slug>/assets/ui/<slug>/index.html`) — the superui application content, copied unchanged from the example crate. For TSX examples this is a tiny mount root (e.g. `<div id="root"></div>`).

The host page includes `<base href="./">` so Bevy's wasm asset reader resolves relative asset fetches to `/<repo>/<slug>/assets/...`, and the code viewer resolves `../vendor/...` to the shared highlighter. **Risk to verify during implementation:** Bevy-on-wasm asset base-path resolution is the most common failure mode; confirm the app actually fetches `assets/ui/<slug>/*` from the correct subdirectory before declaring success.

## 2. Manifest — `examples/gallery.json`

The single edit point for adding an example:

```json
{
  "examples": [
    { "slug": "todomvc", "package": "todomvc", "category": "Apps",
      "title": "TodoMVC (HTML/CSS/JS)",
      "description": "The classic TodoMVC authored in plain HTML/CSS/JS on superui." },
    { "slug": "todomvc_supersolid", "package": "todomvc_supersolid", "category": "Apps",
      "title": "TodoMVC (Supersolid TSX)",
      "description": "The same TodoMVC authored in Solid-style .tsx with the supersolid framework." },
    { "slug": "game_menu", "package": "game_menu", "category": "Apps",
      "title": "Game Menu",
      "description": "A sci-fi game menu with multiple screens and a tab switcher, in supersolid TSX." },
    { "slug": "citadel", "package": "citadel", "category": "Stress tests",
      "title": "Citadel",
      "description": "An economy/among-buildings sim UI — a reactive-node stress test." },
    { "slug": "horde", "package": "horde", "category": "Stress tests",
      "title": "Horde", "tags": ["Playable game"],
      "description": "A survivors-like game (move, shoot, level up) with a reactive HUD under an enemy swarm — also a UI stress test." }
  ]
}
```

- `slug` — URL subdirectory and artifact name.
- `package` — cargo package name (`cargo build -p <package>`).
- `category` — gallery section heading. Sections render in first-seen order.
- `tags` *(optional, default `[]`)* — short labels rendered as badges on the card (e.g. `["Playable game"]`).
- `title` / `description` — shown on the gallery card and host-page banner.
- `build_args` *(optional, default `""`)* — extra flags appended to the wasm `cargo build` (e.g. `--no-default-features`). Not set for any example initially; it is the documented fallback if a stress-test's `bevy_dev_tools` (FPS overlay) fails to compile on wasm.
- **Assets path is derived by convention** as `examples/<slug>/assets`.

Adding an example = append one object and ensure the crate exists and compiles to wasm. No workflow changes; the code viewer picks up its authored source files automatically (§4).

## 3. In-page code viewer

Each demo page shows the **running app and its source side by side** — the point of the whole gallery: to show how little, clean code drives the app.

- **Authored source, not generated output.** For the plain `todomvc`, the files shown are exactly the files the app loads — drift-proof. For the TSX examples, the viewer shows the **authored `app.tsx`** (the source of truth), which `build.rs` transpiles to the `app.generated.js` the app actually runs. The generated JS, TypeScript declaration shims (`*.d.ts`), `tsconfig.json`, and dotfiles are **hidden** — they are noise, not authored source.
- **Selection rule (automatic).** `xtask` enumerates `examples/<slug>/assets/ui/<slug>/` and keeps a file iff its extension is one of `tsx, jsx, ts, js, css, html/htm` AND it is not a dotfile, not `*.generated.js`, not `*.d.ts`, not `tsconfig.json`. Display order: `.tsx/.jsx` → `.html` → `.css` → `.ts` → `.js`, ties alphabetical. So a TSX example leads with `app.tsx`; the plain example leads with `index.html`.
- **Layout (responsive split):** live app canvas on one side, a code panel on the other with one tab per kept source file. On narrow screens it collapses to top-level **Demo / Code** tabs.
- **Highlighting:** a **vendored** highlighter (highlight.js common bundle — includes `typescript`, `xml`, `css`, `javascript`) committed under `tools/gallery/vendor/` and published to `/vendor/`. No CDN. `.tsx`/`.ts` map to the `typescript` grammar, `.html` to `xml`, `.css` to `css`, `.js`/`.jsx` to `javascript`.
- **Runtime vs shown files both deploy.** The whole `assets/` dir is copied into the site, so both the shown `app.tsx` (fetched by the viewer) and the runtime `app.generated.js` (loaded by Bevy) are present. Because `build.rs` writes `app.generated.js` into the assets tree during `cargo build`, the copy must happen **after** the build.

## 4. `xtask` crate

A small workspace crate `xtask` owns all templating/assembly so it is cross-platform (Windows dev + Linux CI) and unit-testable. Subcommands:

- `xtask host-page --slug <slug> --out <dir>` — render the shared host template for one example into `<out>/index.html`, substituting title/slug/wasm-name, and embedding the enumerated authored-source list (§3) so the code viewer knows what to fetch and how to label/highlight it.
- `xtask gallery-index --out <file>` — read the manifest and render the root gallery landing page, grouping cards into `<section>`s by `category` (first-seen order), each card linking to `./<slug>/` and rendering its `tags` as badge chips.

Templates and the vendored highlighter live under `tools/gallery/`. The wasm build itself (cargo/wasm-bindgen/wasm-opt) is orchestrated by the workflow, not xtask. Source enumeration, host rendering, and gallery grouping are covered by unit tests.

## 5. Deploy workflow — `.github/workflows/deploy-pages.yml`

**Triggers:** `push` to `main`, `workflow_dispatch`.
**Permissions:** `pages: write`, `id-token: write`, `contents: read`.
**Concurrency:** single-flight group so overlapping deploys don't race.

Three jobs:

1. **discover** — read `examples/gallery.json`, emit a matrix via `fromJSON` of `{ slug, package, build_args }` (build_args defaults to `""`).
2. **build** (matrix, one run per example):
   - Install `wasm32-unknown-unknown` target and toolchain; restore `Swatinem/rust-cache` (per-slug key).
   - Derive the exact `wasm-bindgen` version from `cargo metadata` (the lockfile is gitignored) and install the matching `wasm-bindgen-cli` + `binaryen` via `taiki-e/install-action`.
   - `cargo build -p <package> --release --target wasm32-unknown-unknown <build_args>`.
   - `wasm-bindgen --target web --out-dir <stage>/<slug> --out-name <slug>` → `wasm-opt -Oz` in place.
   - `cargo run -p xtask -- host-page --slug <slug> --out <stage>/<slug>`.
   - Copy `examples/<slug>/assets` → `<stage>/<slug>/assets` (**after** the build, so `app.generated.js` exists).
   - Upload `<stage>/<slug>` as an artifact.
3. **assemble-and-deploy** — download all example artifacts into `dist/`, copy `tools/gallery/vendor/` → `dist/vendor/`, run `cargo run -p xtask -- gallery-index --out dist/index.html`, then `actions/upload-pages-artifact` (path `dist/`) → `actions/deploy-pages`.

## 6. Footguns designed around (must be handled, not hand-waved)

- **`file_watcher` breaks the wasm build.** Only `examples/todomvc/Cargo.toml` still needs the fix (its `bevy` line enables `file_watcher` unconditionally; `notify` doesn't build on wasm). The four TSX examples already gate `file_watcher` to native (either behind `hmr` or a `cfg(not(wasm))` target table). Fix todomvc by target-splitting: native → `file_watcher`, wasm → `webgl2`.
- **WebGL2 feature.** Each example needs the `webgl2` bevy feature on wasm. todomvc gets it in its target-split; the TSX examples get it added to their existing `[target.'cfg(target_arch = "wasm32")'.dependencies]` block.
- **Canvas config, per example, via a uniform helper.** Each `main.rs` routes its primary window through a local `web_window(Window) -> Window` helper that, on wasm only, sets `canvas: Some("#superui-canvas".into())` and `fit_canvas_to_parent: true`. Examples that already build a `WindowPlugin` (citadel) wrap their existing `Window`; the others add a `.set(WindowPlugin { primary_window: Some(web_window(Window::default())), .. })`. On native `web_window` is the identity, so no behavior change. The host page provides the matching `<canvas id="superui-canvas">`.
- **wasm-bindgen version match.** `wasm-bindgen-cli` must exactly match the `wasm-bindgen` crate version; CI derives it from `cargo metadata` at runtime (lockfile gitignored), not "latest".
- **Binary size / load time.** Bevy wasm binaries are large (tens of MB; the stress tests larger still). Mitigate with release + `wasm-opt -Oz` + a loading indicator in the app pane. The gallery flags the stress tests as heavy.

## 7. "No hot reload on wasm" messaging

Both the gallery (stress-test caveat + a general note) and each host page carry a short banner:

> ▶ This is a static WebAssembly build of {{TITLE}}. Live hot-reload of the UI is a **native-only** superui feature — clone the repo and run `cargo run -p {{SLUG}}` (add `--features hmr` for the supersolid TSX examples) to edit HTML/CSS/TSX live.

The code viewer beside it shows exactly which files you'd be editing.

## 8. URL routing & README links

Every demo is deep-linkable at a stable path — inherent to the §1 layout:

```
https://<user>.github.io/<repo>/<slug>/   →  /todomvc/  /todomvc_supersolid/  /game_menu/  /citadel/  /horde/
```

- **The slug is a permanent URL contract.** Renaming a published slug breaks external links; treat as a breaking change.
- **No per-view routing.** Tab/file selection is not encoded in the URL (out of scope).
- **README table is hand-written**, grouped to mirror the gallery categories, using the `/<slug>/` pattern.

## 9. First deliverable & scope

- All five examples wired end-to-end.
- Manifest, workflow, xtask, templates, and vendored highlighter are N-ready.
- Definition of done: a green deploy publishing all five wasm builds; each demo renders (app pane) with correct asset loading (§1 risk verified) and shows its authored source (TSX where applicable) with highlighting; the gallery groups them under Apps / Stress tests; native-only banner visible.
- Adding example #6 later is a one-object manifest edit.

## Out of scope

- PR CI (fmt/clippy/test) — may be added later as a separate workflow.
- WebGPU backend.
- Any attempt to make hot reload work on wasm.
- Showing generated/tooling files (`app.generated.js`, `*.d.ts`, `tsconfig.json`) or Rust host source in the viewer — the viewer shows authored UI source only.
- Per-view deep-linking (URL hash for tab/file) — per-app `/<slug>/` routing only.
- Auto-generated / CI-committed README table — the table is hand-curated.
- Thumbnails/screenshots on gallery cards (can be added to the manifest later).

## Prerequisites (user-owned, outside CI)

- Repo created at `https://github.com/strowk/bevy_superui.git`; **Pages source set to "GitHub Actions"** (done).
- Publishing from a private repo is enabled on the account (confirmed).
