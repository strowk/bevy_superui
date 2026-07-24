# Website redesign — cyber/terminal theme + live counter

**Date:** 2026-07-24
**Status:** Approved (design)

## Goal

Restyle the mdBook docs site (`website/`) to match the cyber/terminal redesign
mocked up in `drafts/redesign_website.html`, and embed a **live, running compiled
`counter` WebAssembly example** on the landing page's "// LIVE PREVIEW" card.

The unpacked reference lives in `target/redesign_website_design/` (git-ignored):
`body.html` (reconstructed DOM with all inline styles + the component-logic
script), `styles.css`, `keyframes.css`, `design-notes.md`, and screenshots of the
three designed screens (`screenshot.jpeg` = landing, `redesign_docs.jpeg`,
`redesign_examples.jpeg`).

## Context

- The site is an mdBook (`website/book.toml`, theme `navy`) with:
  - `additional-css = ["theme/css/site.css"]` (already overrides navy palette +
    hides mdBook chrome on the landing via `body:has(.superui-landing)`).
  - a `mdbook-gallery` preprocessor that generates the Examples gallery from
    `examples/gallery.json`.
- Demo wasm apps are built by `tools/build-demos.sh` into `website/src/examples/<slug>/`
  (git-ignored) via `cargo build --target wasm32-unknown-unknown` → `wasm-bindgen`
  → `xtask host-page` → copy `assets/`.
- The `counter` example (`examples/counter/`) is the smallest supersolid app: a
  Bevy app that binds its window to `#superui-canvas` on wasm and renders a single
  teal "clicked N times" button. Its `assets/ui/counter/style.css` was **already**
  written to match the redesign's LIVE PREVIEW button.

## Decisions (locked)

1. **Scope:** whole site — landing + docs pages + examples gallery. `counter`
   becomes both a gallery example (its own page + card) and the landing live embed.
2. **Counter embed:** `<iframe>` embedding a minimal counter host page; **Reset
   reloads the iframe** (true full Bevy restart, wasm from HTTP cache, no re-download).
3. **Fonts:** Google Fonts CDN (`<link>`), not self-hosted.
4. **Doc/gallery top bar:** reskin mdBook's real `.menu-bar` (keep search, sidebar
   toggle, print; drop theme toggle — single dark theme). No forked `index.hbs`.
5. **Effects:** full animated background (aurora, drifting grid, scanlines, pulse,
   flicker, glow) wrapped in `prefers-reduced-motion: reduce` → static fallback.
6. Numbered "01 Installation" sections and per-code-block filename headers are
   **optional polish**, not required.

## Design tokens

Fonts: **Chakra Petch** (display/headings/logo/CTA), **JetBrains Mono**
(code/labels/nav/chips/footer), **Space Grotesk** (body).

Colors:

| role | value |
|---|---|
| teal accent / bright / deep / light | `#34e6d6` / `#4ff0e0` / `#2bd0c0` / `#7ff3e9` |
| amber accent / light | `#ffb454` / `#ffce8a` |
| syntax: purple / cyan-fn / blue-tag / amber-num | `#c58fff` / `#7ff3e9` / `#5fd7ff` / `#ffb454` |
| text: body / heading / muted | `#cdd8e6` / `#f2f8f7` / `#8a97a8`, `#5f7085` |
| bg base / panels | `#06080d→#080b12` / `rgba(9,13,20,.85)`, `rgba(13,19,28,.7)` |
| borders | teal `rgba(52,230,214,.16→.55)`; amber `rgba(255,180,84,.4)` |

Keyframes (from `keyframes.css`): `su-aurora`, `su-drift`, `su-pulse`,
`su-flicker`, `su-glow`, `su-scan`, `sc-shine`.

## Architecture — integration surface

All via mdBook extension points; **no template fork**.

| Unit | File | Responsibility |
|---|---|---|
| Font loading | `website/theme/head.hbs` (new) | `<link rel=preconnect>` + Google Fonts stylesheet for the three families/weights, injected into every `<head>`. |
| Global theme CSS | `website/theme/css/site.css` (expand) | Palette/font vars, background-layer styles + keyframes + reduced-motion guard, menu-bar reskin, sidebar, content headings, code blocks, TIP callout, prev/next, gallery cards, footer. |
| Landing CSS | `website/theme/css/landing.css` (new) | Landing-only: hero, eyebrow pill, code card, live-preview card, features grid, early-build banner, landing header/footer. |
| Site JS | `website/theme/js/site.js` (new, `additional-js`) | On every page: inject the 3 fixed bg layers; enhance `.menu-bar` (SU badge + wordmark + version chip + GitHub). On landing: manage counter iframe loading label + Reset. |
| `book.toml` | `website/book.toml` | Add `landing.css` to `additional-css`; add `additional-js = ["theme/js/site.js"]`. |
| Landing content | `website/src/index.md` (rewrite) | Full redesign markup. |
| Counter gallery entry | `examples/gallery.json` | Add a `counter` example so it gets a normal gallery page + a card in the Examples grid. |
| Counter gallery page | `website/src/examples/counter/index.html` (built) | Standard `xtask host-page` output (code viewer + banner), like other examples. |
| Counter embed host | `website/src/examples/counter/embed.html` (built) | Minimal canvas-only host page + `postMessage('superui:ready')`, used by the landing iframe. |
| Demo build | `tools/build-demos.sh` | Add `counter` to the slug list; after `xtask host-page`, also drop the hand-written `embed.html` into the output dir. |

### Component notes

**Background layers** — `site.js` prepends three `<div class="su-bg su-bg--aurora|--grid|--scan">`
to `<body>`, `position:fixed; pointer-events:none`, low z-index. CSS owns the
gradients + animations; `@media (prefers-reduced-motion: reduce)` disables
`animation` (static gradients/grid persist). Guard against double-injection on
mdBook's client-side navigation.

**Menu-bar reskin** — CSS restyles `.menu-bar` (dark, teal bottom border, mono).
`site.js` injects into `.menu-title` (or adjacent) the clip-path SU badge +
"SUPERUI / BEVY GAME UI" wordmark, and into the right cluster the pulsing
"v0.1 · EARLY BUILD" chip + a GitHub link. Existing buttons (search/sidebar/print)
are restyled, not removed; theme-toggle button hidden via CSS.

**Sidebar → "// MODULE INDEX"** — CSS styles `#sidebar .part-title` (from SUMMARY.md
part headers) as teal group eyebrows and `.chapter li > a.active` with the teal
left-border + tint.

**Content** — Chakra headings; reuse mdBook's hover clipboard button as the code
"COPY" control (restyle); `blockquote` → teal-left-border TIP callout; restyle
`.nav-chapters` prev/next.

**Gallery cards** — CSS over the generated `mdbook-gallery` markup: category
eyebrows, corner-bracket pseudo-elements, teal titles, pill badges (map the
existing `Apps` / `Stress tests` categories + tags).

**Landing** — `index.md` keeps the `.superui-landing` marker (chrome already hidden)
and renders its own header + footer. Structure: eyebrow pill → `<h1>` with teal
"your stack." span → tagline → CTAs (`docs/`, `examples/`) → code card
(traffic lights + `counter.tsx` + syntax-colored `<pre>`) → dashed divider →
live-preview region → 4-up features grid → early-build banner → footer.

### Live counter + Reset

- **Gallery entry:** add `counter` to `examples/gallery.json` (default features,
  no `build_args`) so it appears as an Examples card and gets its own page. Placed
  first as the smallest intro example (final category/order decided in the plan).
- **Build:** add `counter` to `tools/build-demos.sh` (`BUILD_ARGS[counter]=""`).
  Run `cargo build -p counter --release --target wasm32-…` → `wasm-bindgen
  --target web` into `website/src/examples/counter/`, run `xtask host-page` (the
  normal gallery page, like other examples), copy `examples/counter/assets`, then
  **also** drop the hand-written `embed.html` into the output dir.
- **Embed host page** (`website/src/examples/counter/embed.html`): a `<canvas
  id="superui-canvas">` filling the frame on a dark (`#0b1220`) background, an
  optional loading overlay, `import init from './counter.js'` (ignore winit's
  "Using exceptions for control flow" throw), and after `init()` settles,
  `window.parent.postMessage('superui:ready', '*')`. Shares `counter.js` + wasm
  with the gallery `index.html` (browser caches it).
- **Landing wiring** (`site.js`): the live-preview region contains
  `<iframe src="examples/counter/embed.html">` + a `Reset` button. Until a `superui:ready`
  message arrives from the iframe, the `// LIVE PREVIEW` eyebrow reads
  **`// LIVE PREVIEW · booting runtime…`** and a subtle overlay covers the frame;
  on ready it flips to **`// LIVE PREVIEW`** and reveals the canvas. Reset =
  `iframe.contentWindow.location.reload()` and re-arms the loading state.
- Path note: the landing is at site root, so the iframe `src` is relative
  `examples/counter/embed.html` (works under the `/bevy_superui/` site-url base).

### Deploy implication

Counter wasm is git-ignored (`/website/src/examples/*/`). Whatever publishes the
site must run `tools/build-demos.sh` (now including `counter`) **before**
`mdbook build`, or the landing counter renders blank in production. Flag/verify in
the deploy workflow.

## Testing / verification

- Manual: `bash tools/build-demos.sh counter && mdbook serve website`, open
  landing → counter loads, counts on click, Reset restarts it (observe re-init,
  no network re-download of `counter_bg.wasm` in devtools).
- Visual diff each screen (landing/docs/examples) against
  `target/redesign_website_design/*.jpeg` at 1280px.
- Check `prefers-reduced-motion` disables animation.
- Confirm search, sidebar toggle, and print still work on doc pages.
- Responsive check < 800px (hero grid + code card stack).

## Out of scope / deferred

- Numbered section counters and per-code-block filename captions (optional polish).
- Self-hosting fonts (chose CDN).
