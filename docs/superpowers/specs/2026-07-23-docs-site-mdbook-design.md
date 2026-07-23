# superui official site — single mdBook driver (landing + docs + gallery)

**Date:** 2026-07-23
**Status:** Approved design, pre-implementation

## Goal

Turn the existing static WebAssembly examples gallery into a full **official
website** for superui: a landing/front-door page, a documentation section, and
the examples gallery — all deployed to GitHub Pages exactly as the gallery is
today. Only a couple of docs pages are authored now; the site structure is built
now so content can be filled in later without further rework.

## Key decisions (from brainstorming)

- **Stay Rust-native / zero-Node.** No Next.js/fumadocs toolchain.
- **mdBook drives the *entire* site.** One `mdbook serve website` gives live
  reload for landing, docs, and gallery index in a single dev loop — this is the
  primary reason for the mdBook-centric design over an xtask-driven one.
- **The book output *is* `dist/`.** No separate site-vs-docs split.
- **URL layout:** `/` landing, `/docs/…` docs, `/examples/` gallery index,
  `/examples/<slug>/` per-demo wasm pages. (Existing `/<slug>/` demo links move
  under `/examples/` and are updated in the README.)
- **Landing is a chrome-light front door:** a `theme/index.hbs` override tags the
  root page with a `landing` body class; CSS hides the sidebar/chrome and styles
  a bespoke hero for that page only. Every other page keeps normal mdBook chrome,
  sidebar, and search.
- **Gallery index via an mdBook preprocessor** so `gallery.json` edits stay in
  the live-reload loop.

## Verified assumptions

- mdBook's built-in **`index` preprocessor converts `README.md` → `index.html`**,
  so `src/examples/README.md` yields the `/examples/` section-index URL.
- Preprocessors "modify the raw Markdown before it gets sent to the renderer" and
  support custom markers (like the built-in `{{#include}}`), which is exactly the
  mechanism the gallery injection uses.
- mdBook cleans its build-output directory on each build. Therefore CI **must run
  `mdbook build` first, then overlay the CI-built demo folders** into
  `dist/examples/<slug>/`. (Asserting "build first" is safe regardless of clean
  behavior.) The design does **not** rely on mdBook copying stray HTML from
  `src/`.

## Architecture

```
website/                     ← the one mdBook project (book output = dist/)
  book.toml                  site-url = "/bevy_superui/"; preprocessor + theme config
  src/
    index.md                 → /            landing (chrome-light hero)
    docs/…                   → /docs/…       docs chapters
    examples/README.md       → /examples/    gallery index, contains {{#gallery}} marker
  theme/
    index.hbs                override: adds `landing` body class on the root page
    css/… (additional-css)   palette match (dark #1e1e28 family) + hide chrome on .landing
tools/gallery-preproc/        ← mdBook preprocessor: expands {{#gallery}} from examples/gallery.json
xtask (host.rs / sources.rs)  ← unchanged: still generates per-demo host pages for CI
```

### 1. Landing (`/`)
`src/index.md` holds an HTML hero authored inline:
- Tagline (from README: browser-like HTML/CSS/JS + Solid-style TSX with hot
  reload, for Bevy game UI).
- CTA buttons → **Docs** and **Examples**.
- A small TSX counter snippet (highlighted; mdBook has built-in highlighting).
- 3–4 feature cards (HTML/CSS/JS stack · Solid-style TSX · hot reload · runs on
  bevy_ui).
- An "early stage / experimental" status note (mirrors README).
- Footer: dual-license + GitHub link.

A minimal `theme/index.hbs` override adds a `landing` body class when the page is
the book root; `theme` CSS keyed on `body.landing` hides the sidebar/top-bar
chrome and styles the hero. All other pages are untouched.

### 2. Docs (`/docs/…`)
Normal mdBook chapters — nav, search, prev/next come free. Initial `SUMMARY.md`:
- **Introduction** (new) — what superui is + status.
- **Getting Started** (new) — add the crate, minimal app, pointer to examples.
- **Reference** — existing markdown **moved** into `website/src/docs/reference/`:
  - CSS (`docs/support/css.md`)
  - HTML (`docs/support/html.md`)
  - JS / DOM (`docs/support/js-dom.md`)
  - Keyed / performance (`docs/superui/keyed.md`)

The top-level `docs/` folder keeps internal specs (`superpowers/`) and
`future_backlog/`, which stay unpublished.

### 3. Gallery index (`/examples/`)
`src/examples/README.md` contains a `{{#gallery}}` marker. A small Rust
preprocessor (`tools/gallery-preproc`) reads `examples/gallery.json` and expands
the marker into the category/card grid — the same HTML `xtask/src/gallery.rs`
produces today, but as a **fragment** (no standalone `<html>`/`<style>` shell,
since it now lives inside the themed book page). Card links stay `./<slug>/` →
`/examples/<slug>/`.

### 4. Demo pages (`/examples/<slug>/`)
Mechanism unchanged from today: `xtask host-page` generates each host page and CI
builds the wasm. Only the output location changes — the CI **assemble** step
overlays each demo (plus `vendor/`) into `dist/examples/<slug>/` **after**
`mdbook build` writes landing + docs + gallery index. Full-screen apps, no book
chrome. Existing relative paths still resolve because the whole subtree just moves
under `/examples/`:
- host `../vendor/` → `/examples/vendor/`
- host `./<slug>.js` → `/examples/<slug>/<slug>.js`

### Theming / consistency
One palette in the theme CSS (gallery's dark `#1e1e28` family) covers the whole
book, so landing, docs, and gallery index are automatically consistent.
`book.toml` `site-url = "/bevy_superui/"` fixes the GH-Pages project base-path for
all mdBook internal links — no per-page base juggling.

## Changes vs. the earlier xtask-driven plan

- xtask is **no longer the site driver**. It keeps only `host-page` + `sources`
  (per-demo pages). The renderer logic in `gallery.rs` **moves into the
  preprocessor** and emits a fragment. The previously-considered
  `landing`/`docs-theme`/shared-nav xtask work is **dropped**.
- `tools/gallery/gallery.html.tmpl` (the standalone gallery shell) is retired; the
  per-demo `host.html.tmpl` and `vendor/` are retained.

## CI (`deploy-pages.yml`)

- **build** job: unchanged per-example wasm build; artifacts unchanged.
- **assemble-and-deploy**:
  1. Install `mdbook` (taiki-e/install-action) + build the `gallery-preproc` bin.
  2. `mdbook build website -d dist` → writes landing, docs, gallery index.
  3. Overlay demo artifacts into `dist/examples/<slug>/` and `dist/examples/vendor/`.
  4. `actions/upload-pages-artifact` + `deploy-pages` as today.
- README live-demo links updated to `/examples/<slug>/`.

> Note: gallery-deploy is under active repair in parallel. Rebase this wiring onto
> whatever assemble/path changes land there; do not disturb that work.

## Testing

- **Preprocessor unit tests:** `gallery.json` → expected card-grid fragment (port
  the existing `gallery.rs` test), category first-seen order, badges, no leftover
  `{{#gallery}}`.
- Existing `sources.rs` / `host.rs` tests stay green.
- CI `mdbook build` fails the deploy if the book or preprocessor breaks.
- Local check: `mdbook serve website`.

## Deliberate scope cuts (YAGNI)

- No docs versioning.
- mdBook's built-in search is sufficient — no custom search tuning.
- Demos remain CI-only (no local wasm build), same as today; `/examples/<slug>/`
  links 404 under local `mdbook serve`, which is acceptable.
