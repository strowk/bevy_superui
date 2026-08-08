# Website blueprint restyle — design

Date: 2026-08-07
Status: approved, ready for planning

## Problem

The website has three visually unrelated skins:

1. **Home** (`website/src/index.md`) renders its own `<header class="su-lhead">` and
   `<footer>`, and hides mdBook's chrome entirely via `body:has(.superui-landing)`.
2. **Docs / gallery** use mdBook's own `.menu-bar`, reskinned by `theme/js/site.js`.
3. **Demo pages** (`tools/gallery/host.html.tmpl`) are standalone generated HTML with a
   dark-purple palette that shares nothing with either.

So the header changes when you move from home to docs, and a demo page looks like a
different product. Separately, the current "cyber" theme (near-black `#080b12`, Chakra
Petch / Space Grotesk / JetBrains Mono, rounded corners) is being replaced by the
"blueprint" style in `drafts/new_style/SuperUI Docs v3.dc.html`.

## Goals

- One header, byte-identical across home, docs, gallery and demo pages, whose geometry
  does not shift when the sidebar opens or closes.
- Adopt the blueprint visual language closely.
- Keep our own `logo.svg` as the mark (not the draft's "SU" box).
- Keep mdBook's search and sidebar-toggle controls in the header.
- Demo pages share the design.

## Non-goals

- Restructuring how demos are built or deployed.
- Changing docs prose. Content stays; only its presentation changes.
- Supporting multiple colour themes. The site ships one dark theme, as today.

## Decisions

These were settled during brainstorming and constrain everything below.

| # | Decision | Rationale |
|---|---|---|
| D1 | **Visual style, plain labels.** Adopt the blueprint visuals, but keep honest labels: HOME / DOCS / EXAMPLES, a normal sidebar, normal callouts. No "Sheet 1", no "SHEET INDEX", no invented page numbers, no "NOTE TO FITTER". | Drafting vocabulary hides what things are, and page numbers would have to be faked. |
| D2 | **No sidebar on home; its toggle is hidden there.** Everything else in the header is identical. | Home is a full-bleed marketing page. A toggle for a sidebar that does not exist is a dead control. |
| D3 | **Header becomes `position: fixed`, full-width, above both sidebar and content.** | mdBook's menu bar normally lives *inside* the content column, so its left edge jumps by the sidebar's width whenever the sidebar toggles — a far bigger inconsistency than D2's missing icon. |
| D4 | **Demo pages use the draft's drawing + spec-column layout**, not a reskin of today's 50/50 split. | Gives the running app real room; the stress-test examples are cramped today. |
| D5 | **Moderate motif in the docs body**: teal rule under H1, CSS-counter numbered H2 chips, bracketed callouts, square code panels. Not heavy (no per-section dimension lines or corner brackets on every figure), not light (not palette-only). | Motif should be felt without fighting long technical prose. |
| D6 | **Shared assets live in `website/src/assets/`** and are consumed by both the book and the demo pages. | See "Why `src/assets/`" below. |
| D7 | **No `//` prefixes anywhere.** They are a code-comment idiom from the cyber theme. | A drafting sheet does not annotate itself in C syntax. |

## Why `src/assets/`

mdBook 0.5 content-hashes `additional-css` / `additional-js`: the built filenames are
`theme/css/site-94d99c16.css` and `theme/js/site-2ab7aca7.js`. A standalone demo page
therefore cannot link the book's theme files by any stable path.

mdBook does copy non-markdown files under `src/` to the output **verbatim and unhashed**.
Verified empirically against mdBook 0.5.4: `website/src/assets/blueprint.css` lands at
`/assets/blueprint.css`, and `{{ path_to_root }}` in `head.hbs` resolves correctly at
every depth (`assets/…` at the root, `../../assets/…` under `docs/concepts/`).

Two alternatives were rejected:

- **Duplicating the styles** into a self-contained `demo.css` — the palette, header markup
  and footer would exist twice and drift on the first colour tweak.
- **Making each demo a real mdBook chapter** that iframes the wasm app — everything shared
  for free, including working search, but the app ends up nested in an iframe, which breaks
  keyboard focus and pointer capture for `horde`, and it would restructure the CI overlay.

## Design tokens

| Token | Value | Use |
|---|---|---|
| `--su-bg` | `#0d2b40` | page ground |
| `--su-bg-deep` | `#071a28` | code wells |
| `--su-panel` | `rgba(9,32,48,.5)` | cards, sidebar |
| `--su-teal` | `#34e6d6` | accent, active state, links |
| `--su-teal-bright` | `#5cf2e4` | solid-button hover |
| `--su-orange` | `#ff8a6b` | warnings, early-build note |
| `--su-amber` | `#ffc46b` | numeric literals in code |
| `--su-heading` | `#f2f9fc` | headings |
| `--su-text` | `#dce9f2` | body |
| `--su-body` | `#a9c4d6` | secondary body |
| `--su-muted` | `#7fa3bd` | labels |
| `--su-dim` | `#5c7f99` | faint labels |
| `--su-line` | `rgba(220,233,242,.28)` | borders |
| `--su-line-soft` | `rgba(220,233,242,.16)` | hairlines |
| `--font-display` | Saira Condensed | headings, nav, buttons — uppercase, tracked |
| `--font-body` | Archivo | prose |
| `--font-mono` | IBM Plex Mono | labels, code |

`border-radius: 0` everywhere. That single change carries most of the drafting feel.

Fonts are declared once, as an `@import` at the top of `blueprint.css`, so the book and the
demo pages cannot drift. `head.hbs` keeps the two `preconnect` hints.

### Label idioms (replacing `//`)

- **Short teal tick then tracked mono caps** — `▬ RENDERS THROUGH BEVY_UI`. For labels
  floating in content.
- **Tracked Saira caps over a hairline rule** — for the sidebar header, which reads
  plain `CONTENTS`.

Today's four `//` usages are removed: `// MODULE INDEX` (sidebar), `// LIVE PREVIEW ·
booting runtime…` (home), `01 // DOM` (feature tags, which become circular numbered
badges), and `// build a12f · 2026` (footer, which folds into the title block's `REV` cell).

## Shared chrome

### Background

Three fixed layers behind everything, replacing today's aurora/grid/scanline:

1. slow radial teal/blue wash (`bp-wash`, 26s),
2. 22px fine grid,
3. 110px coarse grid.

All are `pointer-events: none` and suppressed under `prefers-reduced-motion`.

### Header

```
┌───────────────────────────────────────────────────────────────────────┐
│ ☰  [logo] SUPERUI        HOME   DOCS   EXAMPLES      🔍  [ SOURCE ↗ ] │
│           MADE FOR BEVY          ────                                 │
└───────────────────────────────────────────────────────────────────────┘
```

- `logo.svg` + Saira wordmark `SUPERUI` + mono strapline `MADE FOR BEVY`.
- Tabs are the draft's full-height shape; the active one carries a 2px teal underline and
  a faint teal wash. Active state is derived from the URL, so EXAMPLES stays lit on a demo
  page.
- `--menu-bar-height: 64px`. The bar is `position: fixed` and full-width (D3); the sidebar
  gets `top: 64px`; mdBook's `#mdbook-menu-bar-hover-placeholder` is hidden and `.page`'s
  compensating negative `margin-block-start` removed in favour of `padding-top`. The fixed
  prev/next arrows also gain `top: 64px` so they clear the bar.
- Inner content is capped at 1520px and centred by a `padding-inline` of
  `max(clamp(14px,3vw,26px), calc((100vw - 1520px) / 2))`. The existing "centre the whole
  app" gutter logic in `site.css` is retained with `--su-app-max` retuned 1600px → 1520px,
  so the header, the sidebar and the article all share one gutter and the brand aligns with
  the sidebar's left edge on wide screens.
- Flex order: sidebar toggle (`.left-buttons`, far left) → brand → tabs → `margin-left:auto`
  → search → SOURCE. JS relocates `#mdbook-search-toggle` out of `.left-buttons` into the
  right cluster. The theme picker stays hidden.
- On home, the sidebar toggle is hidden (D2).
- The current header's `v0.1 · EARLY BUILD` chip is **removed**; that information now lives
  in the footer title block's `REV` cell. The GitHub link keeps its icon but is relabelled
  `SOURCE ↗` and becomes a solid teal button.

**Responsive.** The header must never wrap, because `--menu-bar-height` is load-bearing —
the sidebar's `top` and `.page`'s `padding-top` are both derived from it, so a wrapped bar
would overlap the content. Instead, elements drop out by breakpoint, and
`--menu-bar-height` is restated per breakpoint if the remaining content needs a different
height:

| Width | Header contents |
|---|---|
| ≥ 1080px | everything |
| 768–1079px | drop the `MADE FOR BEVY` strapline |
| < 768px | also drop the wordmark (logo only) and shorten `SOURCE ↗` to its icon |

Verification at 360px must confirm no horizontal scroll and no wrap.

### Footer

The draft's title block: a five-cell mono strip — `PROJECT / DRAWN BY / SCALE / REV /
LICENCE` — with hairline dividers, injected on every page including demos.

### `blueprint.js`

One implementation, used by the book and by demo pages. It:

1. injects the three background layers;
2. builds the header into mdBook's `.menu-bar` (book) or a `[data-su-header]` placeholder
   (demos), resolving links against `path_to_root`;
3. marks the active tab from the URL;
4. relocates mdBook's search button (book only, guarded);
5. builds the footer;
6. derives the docs eyebrow (below);
7. wires the home live-counter iframe;
8. keeps `suppressLandingPrev` — home is a prefix chapter, so mdBook wires the first docs
   chapter's "previous" arrow back to it, which is not something to page into.

Every step is idempotent and no-ops when its target is absent, so the same file is safe on
all four page kinds.

## Page designs

### Home

Loses its bespoke `<header>` and `<footer>`; keeps the `.superui-landing` marker that hides
the sidebar and lets content go full-bleed.

- Two-column hero. A dimension-rule strip reads `RENDERS THROUGH BEVY_UI` between end ticks.
- Headline adopts the draft's copy — *"Game UI you already know how to write."*, last line
  in teal — replacing "Game UI that speaks your stack", because it says something concrete.
- Solid-teal `READ THE DOCS` and ghost `SEE EXAMPLES`; mono stack strip
  `RUST + BEVY_UI + WASM + NATIVE`.
- Right column: framed code panel with corner brackets. **We keep the real wasm counter
  iframe** (`examples/counter/embed.html`) rather than the draft's hand-faked inline
  counter — it is a live build of the actual example. The draft's dashed divider stays,
  relabelled honestly.
- Four numbered callout cards, each with a circular teal number badge hanging off the
  top-left corner.
- Early-build warning as the orange left-rail note.

### Docs

**Sidebar.** `CONTENTS` in tracked Saira caps over a hairline rule. Part titles in mono
teal. Chapter links carry a dotted leader rule running to the right edge — the drawing-index
feel without invented page numbers. Active item keeps its teal left border and wash.

**Article.** A mono eyebrow above the H1 reads `SECTION · <part name>`, derived by
`blueprint.js` from the active sidebar entry's preceding `.part-title` — no markdown edits,
no hardcoding, and absent (not broken) when there is no part title. H1 sits over the
teal-to-hairline rule. H2s get auto-numbered `01`, `02` chips from a CSS counter scoped to
`main`'s **direct children**, so gallery category headings and nested headings are
unaffected. Blockquotes become the teal `i`-rail callout. Code panels get square borders
with COPY on a header bar. Prev/next go mono. `--content-max-width` widens 750px → 900px to
suit the body size already in use.

### Examples gallery

Category headings become label + dashed rule + count. Cards go square with corner brackets
and gain a "plate" strip at the top — grid ground with a crosshair reticle and the package
slug in mono — above title, description and badges.

This needs a markup change in `tools/mdbook-gallery/src/gallery.rs`: a `card-plate` div and
a wrapped card body. The existing constraint holds — **no `<h3>`/`<p>` inside the card
anchor**, because mdBook post-processes headings into anchored links and a nested `<a>`
splits the card apart. Its unit test moves with the markup.

### Demo pages

`tools/gallery/host.html.tmpl` is rewritten around D4. It declares
`var path_to_root = "../../"` before loading the same `assets/blueprint.css` and
`assets/blueprint.js` the book uses, plus `assets/demo.css`.

```
┌─────────────────────────────────────────────────────────────────────┐
│  [logo] SUPERUI      HOME   DOCS   EXAMPLES          [ SOURCE ↗ ]   │
├─────────────────────────────────────────────────────────────────────┤
│  ← EXAMPLES / Apps / Game Menu                                      │
│                                                                     │
│  [VIEW][PARTS]      WASM · game_menu   [⤢ FULL SHEET]               │
│  ┌───────────────────────────────────┐   ┌───────────────────────┐  │
│  │┌ VIEWPORT — APP MOUNTS HERE     ┐ │   │ Apps                  │  │
│  │                                   │   │ GAME MENU             │  │
│  │            (the app)              │   │ ━━━━━─────────────────│  │
│  │                                   │   │ description…          │  │
│  │└                    BEVY · WGPU ┘ │   │ [tsx] [signals]       │  │
│  └───────────────────────────────────┘   ├───────────────────────┤  │
│                                          │ ! │ hot reload is     │  │
│                                          │   │ native only       │  │
│                                          ├───────────────────────┤  │
│                                          │ $ cargo run -p …      │  │
│                                          │ [  VIEW SOURCE ↗  ]   │  │
│                                          └───────────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
```

- Grid is `minmax(0,1.62fr) minmax(268px,1fr)`, collapsing to one column below ~900px.
- `⤢ FULL SHEET` switches to a stacked layout with the viewport at `calc(100vh - 250px)`.
- VIEW is the framed canvas with corner brackets, grid ground and the corner labels
  `VIEWPORT — APP MOUNTS HERE` / `BEVY · WGPU`. The loader becomes the draft's rotating
  radar with a sweep bar, replacing today's plain "Loading…" line.
- PARTS renders `window.__SOURCES__` as a **flat** file list with coloured language badges
  beside a code pane — flat because that is what the data is; no invented directory nesting.
- The old mobile-only Demo/Code switch is removed; VIEW/PARTS works at every width.
- `tools/gallery/vendor/highlight.css` is retinted to the blueprint palette.

**Search is omitted on demo pages.** They are standalone files outside the book, with no
search index and no `searcher.js`. mdBook's searcher only auto-opens for a *non-empty*
`?search=` term, so there is no honest way to wire the button through, and a search box that
cannot search is worse than no box. Same principle as D2.

**Template placeholders.** The spec column needs the category and description, so
`{{CATEGORY}}` and `{{DESCRIPTION}}` join the existing `{{TITLE}}` / `{{SLUG}}` /
`{{WASM_JS}}` / `{{SOURCES_JSON}}`. `xtask/src/host.rs` fills them from fields that already
exist on `Example`; badges come from `tags`. Its test moves with it.

## Cascade order

mdBook emits `head.hbs` **before** its own `chrome.css`, so a `<link>` to `blueprint.css`
there would lose every specificity tie to mdBook's chrome. Instead `theme/css/site.css`
opens with:

```css
@import url("../../assets/blueprint.css");
```

`site.css` sits in the "Custom theme stylesheets" block after `chrome.css`, and content
hashing changes only its filename, not its directory, so the relative path holds.

JS has no cascade problem: `head.hbs` gets
`<script defer src="{{ path_to_root }}assets/blueprint.js"></script>`, which runs after
mdBook's `book.js` (a classic script at end of body executes during parsing; deferred
scripts run after parsing, before `DOMContentLoaded`). `blueprint.js` is nevertheless
defensive about the sidebar being populated, since `toc.js` upgrades a custom element.

## File map

**New**

| File | Purpose |
|---|---|
| `website/src/assets/blueprint.css` | fonts, tokens, background, header, footer, buttons, panels, badges — shared |
| `website/src/assets/blueprint.js` | background/header/footer builder, active tab, search relocation, docs eyebrow, live counter — shared |
| `website/src/assets/demo.css` | demo drawing frame, spec column, parts pane |

**Changed**

| File | Change |
|---|---|
| `website/theme/css/site.css` | rewritten — mdBook chrome only; imports `blueprint.css` |
| `website/theme/css/landing.css` | rewritten — home only |
| `website/theme/head.hbs` | preconnects + `blueprint.js` |
| `website/book.toml` | drop `additional-js` |
| `website/src/index.md` | rewritten — no bespoke header/footer |
| `tools/gallery/host.html.tmpl` | rewritten |
| `tools/gallery/vendor/highlight.css` | retinted |
| `tools/mdbook-gallery/src/gallery.rs` | card plate markup + test |
| `xtask/src/host.rs` | two new placeholders + test |

**Deleted**

| File | Reason |
|---|---|
| `website/theme/js/site.js` | moved to `website/src/assets/blueprint.js` |

## Verification

- `cargo test -p mdbook-gallery -p xtask` for the two Rust changes.
- Build the book with **mdBook 0.5.4**, matching what CI's `taiki-e/install-action` pulls.
  Not the 0.4.47 on the PATH: it serializes the book as `sections` rather than `items`,
  which crashes the shiki preprocessor, and its element ids lack the `mdbook-` prefix.
  `npm ci` in `website/tools/mdbook-shiki` is a prerequisite.
- Set `CARGO_TARGET_DIR` to the main checkout's `target/` so the worktree does not grow its
  own copy — the gallery preprocessor is a Rust crate, so building the book compiles it.
- Screenshot home, a docs page, the gallery and a demo page at desktop and mobile widths.
  Confirm the header is identical across the first three, including with the sidebar
  toggled, and that no horizontal scroll appears at 360px.

## Risks

| Risk | Mitigation |
|---|---|
| Fixing the menu bar fights mdBook layout rules we have not seen (mobile breakpoints, `.sidebar-resizing`). | Verify at 360px / 768px / 1440px / 2560px with the sidebar both open and closed before calling it done. |
| A wrapped header would desynchronise `--menu-bar-height` from the real bar height, overlapping content. | Elements drop out by breakpoint rather than wrapping; see "Responsive". |
| The `@import` adds a serial CSS fetch. | Accepted: correctness of cascade order beats ~1 round trip on a docs site. |
| The docs eyebrow depends on mdBook's sidebar DOM shape. | Render nothing when the expected structure is absent, rather than guessing. |
| A future mdBook release renames ids again (0.4 → 0.5 already did). | Keep id lookups tolerant of both spellings, as the current `site.js` already does. |
