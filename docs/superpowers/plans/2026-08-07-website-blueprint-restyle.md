# Website Blueprint Restyle Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restyle the website to the blueprint design in `drafts/new_style/SuperUI Docs v3.dc.html`, with one header shared byte-for-byte across home, docs, gallery and demo pages.

**Architecture:** Shared CSS and JS live in `website/src/assets/`, which mdBook copies to the output verbatim and unhashed, so the standalone demo pages can link the same files the book uses. mdBook's menu bar is repurposed as the shared header and made `position: fixed` and full-width so its geometry never shifts. mdBook-specific chrome rules stay in `website/theme/css/site.css`, which pulls the shared sheet in via `@import` to guarantee cascade order.

**Tech Stack:** mdBook 0.5.4, handlebars templates, vanilla CSS/JS (no build step for site assets), Rust (`mdbook-gallery` preprocessor, `xtask` host-page generator), Node (Shiki preprocessor, already wired).

**Spec:** `docs/superpowers/specs/2026-08-07-website-blueprint-restyle-design.md`

## Global Constraints

- **Work in the worktree** `/home/tim/bevy_superui/.claude/worktrees/website-blueprint-restyle`. Do not `cd` to the main checkout.
- **Do not commit.** The user asked for changes to be left in the working tree. Every task ends with a verification step instead of a commit. This is a deliberate deviation from the usual commit-per-task rhythm.
- **Always `export CARGO_TARGET_DIR=/home/tim/bevy_superui/target`** before any cargo or mdbook command. The gallery preprocessor is a Rust crate, so building the book compiles it; without this the worktree grows its own multi-gigabyte `target/`.
- **Build with mdBook 0.5.4 only**, at `/tmp/claude-1000/-home-tim-bevy-superui/6dc0b566-e8ae-4579-bf7d-88828f5447e5/scratchpad/mdbook`. The `mdbook` on `PATH` is 0.4.47, which serializes the book as `sections` rather than `items` (crashing the Shiki preprocessor) and emits element ids without the `mdbook-` prefix. CI installs latest, so 0.5.4 is what production sees.
- **No rounded corners**, with one deliberate exception: the circular number badges on the
  home callout cards (`.su-num`). mdBook's twelve radius rules are zeroed by an explicit
  selector list in Task 1b — never by a blanket `* { border-radius: 0 !important }`.
- **No `//` label prefixes anywhere.** They are a cyber-theme code-comment idiom. Use a short teal tick before tracked mono caps, or tracked Saira caps over a hairline rule.
- **Labels are plain**: HOME / DOCS / EXAMPLES / CONTENTS. No "Sheet 1", "SHEET INDEX", "NOTE TO FITTER", or invented page numbers.
- **Palette** (exact values, used verbatim):
  `--su-bg:#0d2b40` `--su-bg-deep:#071a28` `--su-panel:rgba(9,32,48,.5)`
  `--su-teal:#34e6d6` `--su-teal-bright:#5cf2e4` `--su-orange:#ff8a6b` `--su-amber:#ffc46b`
  `--su-heading:#f2f9fc` `--su-text:#dce9f2` `--su-body:#a9c4d6` `--su-muted:#7fa3bd` `--su-dim:#5c7f99`
  `--su-line:rgba(220,233,242,.28)` `--su-line-soft:rgba(220,233,242,.16)`
- **Alpha literals must decompose a real token.** Plain CSS cannot apply an alpha to a hex
  token, so translucent fills are written as `rgba(...)` literals — that is expected and is
  not a violation. But the triple must be a real token's RGB: `rgba(52,230,214,X)` is
  `--su-teal`, `rgba(220,233,242,X)` is the line colour, `rgba(9,32,48,X)` is `--su-panel`,
  and `rgba(7,26,40,X)` is `--su-bg-deep` (`#071a28`). The v3 draft also used
  `rgba(6,22,34,X)` for the card plate and the demo viewport; that is 1–6/255 off
  `--su-bg-deep` per channel, imperceptible, and matches no token. Every occurrence in this
  plan has been normalised to `rgba(7,26,40,X)` — do not reintroduce the draft's variant.
- **Fonts**: `--font-display` Saira Condensed, `--font-body` Archivo, `--font-mono` IBM Plex Mono. Declared once via `@import` at the top of `blueprint.css`.
- **`--menu-bar-height: 64px`** is load-bearing: the sidebar's `top` and `.page`'s `padding-top` derive from it. The header must never wrap.

### Standard verification commands

```bash
cd /home/tim/bevy_superui/.claude/worktrees/website-blueprint-restyle
export CARGO_TARGET_DIR=/home/tim/bevy_superui/target
export MDBOOK=/tmp/claude-1000/-home-tim-bevy-superui/6dc0b566-e8ae-4579-bf7d-88828f5447e5/scratchpad/mdbook

# build the site (output: website/book/, gitignored)
(cd website && "$MDBOOK" build)

# serve for screenshots
(cd website && "$MDBOOK" serve -p 3000)

# rust tests
cargo test -p mdbook-gallery -p xtask
```

Node dependencies for the Shiki preprocessor are already installed in this worktree
(`website/tools/mdbook-shiki/node_modules`). If they go missing: `cd website/tools/mdbook-shiki && npm ci`.

---

## File Structure

**New — shared, consumed by both the book and the demo pages**

| File | Responsibility |
|---|---|
| `website/src/assets/blueprint.css` | Font import, design tokens, background layers, header, footer, buttons, panels, corner brackets, badges, code wells. Nothing mdBook-specific. |
| `website/src/assets/blueprint.js` | Builds background layers, header and footer into placeholders; active tab; mdBook search relocation; docs eyebrow; home live-counter wiring. Every step idempotent and a no-op when its target is absent. |
| `website/src/assets/demo.css` | Demo-page-only layout: drawing frame, VIEW/PARTS tabs, spec column, parts pane, full-sheet mode. |

**Modified**

| File | Responsibility after the change |
|---|---|
| `website/theme/css/site.css` | mdBook chrome only — imports `blueprint.css`, then fixed-header layout integration, sidebar, search, content typography, gallery cards, prev/next. |
| `website/theme/css/landing.css` | Home page only. |
| `website/theme/head.hbs` | Font preconnects + `blueprint.js`. |
| `website/book.toml` | Drops `additional-js`. |
| `website/src/index.md` | Home content only — no bespoke header or footer. |
| `tools/gallery/host.html.tmpl` | Demo page, drawing + spec-column layout. |
| `tools/gallery/vendor/highlight.css` | highlight.js theme retinted to the blueprint palette. |
| `tools/mdbook-gallery/src/gallery.rs` | Card markup gains a plate strip. |
| `xtask/src/host.rs` | Two new template placeholders. |
| `xtask/src/manifest.rs` | `description`/`category`/`tags` become live fields. |
| `examples/counter/web-embed.html` | Background matches the blueprint code well. |

**Deleted**

| File | Reason |
|---|---|
| `website/theme/js/site.js` | Superseded by `website/src/assets/blueprint.js`. |

---

## Task 1: Shared foundation — tokens, background, asset wiring

Establishes `src/assets/` as a real, verified delivery path and repaints the ground. Nothing
else can be checked until an asset in `src/assets/` is provably reaching the built pages.

**Files:**
- Create: `website/src/assets/blueprint.css`
- Modify: `website/theme/css/site.css` (prepend the `@import`, add the radius reset and the
  mdBook variable bridge, and **delete the entire old cyber theme** below them)
- Modify: `website/theme/head.hbs` (whole file)

**Interfaces:**
- Consumes: nothing.
- Produces: the CSS custom properties listed in Global Constraints, on `:root`; the classes
  `.su-bg`, `.su-bg--wash`, `.su-bg--grid-fine`, `.su-bg--grid-coarse`; the keyframes
  `bp-wash`, `bp-march`, `bp-plot`, `bp-sweep`. Tasks 2–7 rely on all of these.

- [ ] **Step 1: Create `website/src/assets/blueprint.css` with the font import, tokens and background layers**

```css
/* Shared blueprint design system.
   Loaded by the book (via an @import at the top of theme/css/site.css, which puts it
   after mdBook's chrome.css in the cascade) and directly by the generated demo pages
   (tools/gallery/host.html.tmpl). Must contain nothing mdBook-specific. */
@import url("https://fonts.googleapis.com/css2?family=Saira+Condensed:wght@500;600;700&family=Archivo:wght@400;500;600&family=IBM+Plex+Mono:wght@400;500;600&display=swap");

:root {
  --su-bg: #0d2b40;
  --su-bg-deep: #071a28;
  --su-panel: rgba(9, 32, 48, 0.5);
  --su-panel-solid: #092030;
  --su-teal: #34e6d6;
  --su-teal-bright: #5cf2e4;
  --su-orange: #ff8a6b;
  --su-amber: #ffc46b;
  --su-heading: #f2f9fc;
  --su-text: #dce9f2;
  --su-body: #a9c4d6;
  --su-muted: #7fa3bd;
  --su-dim: #5c7f99;
  --su-line: rgba(220, 233, 242, 0.28);
  --su-line-soft: rgba(220, 233, 242, 0.16);
  --font-display: "Saira Condensed", system-ui, sans-serif;
  --font-body: "Archivo", system-ui, sans-serif;
  --font-mono: "IBM Plex Mono", ui-monospace, Consolas, monospace;
  --su-app-max: 1520px;
  --su-edge: max(clamp(14px, 3vw, 26px), calc((100vw - var(--su-app-max)) / 2));
}

html, body { background: var(--su-bg); color: var(--su-text); font-family: var(--font-body); }

/* --- fixed background layers (injected by blueprint.js) --- */
.su-bg { position: fixed; pointer-events: none; }
.su-bg--wash {
  inset: -15%; z-index: 0; filter: blur(26px);
  background:
    radial-gradient(700px 560px at 26% 24%, rgba(52, 230, 214, 0.10), transparent 68%),
    radial-gradient(620px 520px at 76% 72%, rgba(120, 180, 235, 0.07), transparent 66%);
  animation: bp-wash 26s ease-in-out infinite;
}
.su-bg--grid-fine {
  inset: 0; z-index: 0;
  background-image:
    linear-gradient(rgba(220, 233, 242, 0.05) 1px, transparent 1px),
    linear-gradient(90deg, rgba(220, 233, 242, 0.05) 1px, transparent 1px);
  background-size: 22px 22px;
}
.su-bg--grid-coarse {
  inset: 0; z-index: 0;
  background-image:
    linear-gradient(rgba(220, 233, 242, 0.10) 1px, transparent 1px),
    linear-gradient(90deg, rgba(220, 233, 242, 0.10) 1px, transparent 1px);
  background-size: 110px 110px;
}

@keyframes bp-wash {
  0%, 100% { opacity: 0.5; transform: translate3d(0, 0, 0); }
  50% { opacity: 0.85; transform: translate3d(2%, 1.5%, 0); }
}
@keyframes bp-march { to { background-position: 14px 0; } }
@keyframes bp-plot { to { transform: rotate(360deg); } }
@keyframes bp-sweep { 0% { left: -30%; } 100% { left: 100%; } }

@media (prefers-reduced-motion: reduce) {
  .su-bg { animation: none !important; }
  * { animation-duration: 0s !important; animation-iteration-count: 1 !important; }
}
```

- [ ] **Step 1b: Add the corner reset to `website/theme/css/site.css`**

This belongs in `site.css`, not `blueprint.css`: every selector is an mdBook one, and the
demo pages have no mdBook chrome to reset. Put it directly after the Task 1 `@import`.

mdBook sets `border-radius` in exactly twelve places across `chrome.css` and `general.css`.
They are listed explicitly rather than zeroed with a blanket
`* { border-radius: 0 !important }`, so no `!important` is needed anywhere and our own
components (the callout number badges in Task 5) stay free to round themselves. `site.css`
loads after both mdBook sheets, so equal specificity wins.

```css
/* The drafting sheet has no rounded corners. These are every rule in mdBook's
   chrome.css and general.css that sets a radius (mdBook 0.5.4). If a future mdBook
   rounds something new, Task 8's visual sweep is what catches it. */
.mobile-nav-chapters,
:not(pre) > .hljs,
pre > .buttons button,
mark,
#mdbook-searchbar,
ul#mdbook-searchresults li,
.theme-popup,
#mdbook-help-popup,
kbd,
.footnote-definition > li:target::before,
.footnote-reference:target,
.tooltiptext { border-radius: 0; }
```

- [ ] **Step 2: Prepend the import to `website/theme/css/site.css`**

The very first line of the file, before any rule (CSS requires `@import` to precede all
rules). `site.css` sits in mdBook's "Custom theme stylesheets" block, after `chrome.css`,
and content hashing changes only its filename — not its directory — so this relative path
holds in the built output.

```css
/* Shared blueprint design system. Imported (not <link>ed from head.hbs) so it lands
   AFTER mdBook's chrome.css in the cascade — head.hbs is emitted before chrome.css,
   where every specificity tie would be lost. */
@import url("../../assets/blueprint.css");
```

- [ ] **Step 2b: Delete the old cyber theme from `website/theme/css/site.css`**

Everything below the radius reset is the previous theme. It must go, not merely be
overridden. Two rules in it actively defeat Task 1:

- it re-declares `--su-bg: #080b12`, `--font-display: "Chakra Petch"` and the rest of the
  palette on `:root, .navy, .light, .rust, .coal, .ayu` — **after** the `@import`, so the
  old values win and `blueprint.css` is inert;
- `body::before` paints an opaque near-black gradient over the new background layers.

It also defines `.su-brand`, `.su-logo`, `.su-word`, `.su-chip`, `.su-gh` and `.su-footer`,
which would override the shared header rules `blueprint.css` adds in Task 2 for the same
reason.

Truncate the file to just the comment, the `@import`, the radius reset, and the variable
bridge from Step 2c. Tasks 3, 4 and 6 append their sections to that clean base.

- [ ] **Step 2c: Bridge mdBook's variables onto the blueprint palette**

mdBook's own chrome (menu-bar icons, links, tables, search, footnotes) is driven by its
custom properties, not by our selectors. Map them once here so those elements match without
per-selector overrides. Names taken verbatim from mdBook 0.5.4's `variables.css` `.navy`
block — note `--sidebar-non-existant`, which is misspelled upstream and must be copied as-is
to have any effect.

```css
/* mdBook drives its built-in chrome from these; point them at the blueprint palette.
   The theme-class selectors are mdBook's own — matching them keeps our values winning
   whichever theme class ends up on <html>. */
:root, .navy, .light, .rust, .coal, .ayu {
  --bg: var(--su-bg);
  --fg: var(--su-text);
  --sidebar-bg: rgba(7, 26, 40, 0.92);
  --sidebar-fg: var(--su-text);
  --sidebar-non-existant: var(--su-dim);
  --sidebar-active: var(--su-teal);
  --sidebar-spacer: var(--su-line-soft);
  --scrollbar: rgba(52, 230, 214, 0.28);
  --icons: var(--su-muted);
  --icons-hover: var(--su-teal);
  --links: var(--su-teal);
  --inline-code-color: var(--su-teal);
  --theme-popup-bg: var(--su-panel-solid);
  --theme-popup-border: var(--su-line);
  --theme-hover: rgba(52, 230, 214, 0.08);
  --quote-bg: rgba(52, 230, 214, 0.06);
  --quote-border: rgba(52, 230, 214, 0.4);
  --warning-border: var(--su-orange);
  --table-border-color: var(--su-line-soft);
  --table-header-bg: rgba(9, 32, 48, 0.7);
  --table-alternate-bg: rgba(9, 32, 48, 0.35);
  --searchbar-border-color: var(--su-line);
  --searchbar-bg: rgba(7, 26, 40, 0.92);
  --searchbar-fg: var(--su-text);
  --searchresults-header-fg: var(--su-muted);
  --searchresults-border-color: var(--su-line-soft);
  --searchresults-li-bg: rgba(9, 32, 48, 0.5);
  --search-mark-bg: rgba(255, 196, 107, 0.22);
  --footnote-highlight: var(--su-teal);
  --overlay-bg: rgba(7, 26, 40, 0.72);
  --color-scheme: dark;

  /* The 4px rule beside a page's headers in the nav. mdBook's navy default is an
     off-palette blue (#2f6ab5); the previous theme overrode it and so must we. */
  --sidebar-header-border-color: rgba(52, 230, 214, 0.35);

  /* GitHub-style alert accents (`> [!NOTE]` and friends). Unused in the docs today,
     but mdBook 0.5 renders them and its defaults are all off-palette. */
  --blockquote-note-color: var(--su-teal);
  --blockquote-tip-color: #9de8b4;
  --blockquote-important-color: #8fb8ff;
  --blockquote-warning-color: var(--su-amber);
  --blockquote-caution-color: var(--su-orange);
}
```

The full set of names above is every custom property mdBook 0.5.4 defines in a theme block.
If a value is left unbridged, that element silently keeps mdBook's stock navy colour.

- [ ] **Step 3: Replace `website/theme/head.hbs` entirely**

```handlebars
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<script defer src="{{ path_to_root }}assets/blueprint.js"></script>
```

The font `<link>` is gone — fonts are now `@import`ed once inside `blueprint.css` so the
book and the demo pages cannot drift. The preconnects still help, since the import resolves
to the same origins. `blueprint.js` does not exist until Task 2; a 404 here is expected and
harmless for this task's verification.

- [ ] **Step 4: Build and verify the asset reaches the output at every depth**

```bash
cd /home/tim/bevy_superui/.claude/worktrees/website-blueprint-restyle
export CARGO_TARGET_DIR=/home/tim/bevy_superui/target
export MDBOOK=/tmp/claude-1000/-home-tim-bevy-superui/6dc0b566-e8ae-4579-bf7d-88828f5447e5/scratchpad/mdbook
(cd website && "$MDBOOK" build)

test -f website/book/assets/blueprint.css && echo "OK copied unhashed"
grep -o 'src="[^"]*assets/blueprint.js"' website/book/index.html
grep -o 'src="[^"]*assets/blueprint.js"' website/book/docs/concepts/signals.html
```

Expected: `OK copied unhashed`; then `src="assets/blueprint.js"` on the root page and
`src="../../assets/blueprint.js"` on the nested page — proving `path_to_root` resolves at
every depth.

- [ ] **Step 5: Confirm the ground actually repainted**

Serve and screenshot the home page. Expected: deep navy `#0d2b40` ground with a fine and a
coarse grid visible, no rounded corners anywhere. The header and content will still be the
old cyber styling — that is expected at this stage.

---

## Task 2: Shared chrome — header and footer, one implementation

**Files:**
- Create: `website/src/assets/blueprint.js`
- Modify: `website/src/assets/blueprint.css` (append header/footer/button/panel styles)
- Modify: `website/src/index.md` (delete the bespoke `<header class="su-lhead">` and `<footer class="su-footer su-lfoot">` blocks only; leave all other content untouched for now)
- Modify: `website/book.toml` (drop `additional-js`)
- Delete: `website/theme/js/site.js`

**Interfaces:**
- Consumes: tokens and `.su-bg--*` classes from Task 1.
- Produces:
  - `window.__superuiInit()` — re-runnable init, kept for parity with the old `site.js`.
  - Header DOM contract, relied on by Tasks 3, 5 and 7:
    `.su-brand` > `img.su-logo` + `.su-word` > `b` + `small`;
    `nav.su-tabs` > `a.su-tab[data-su-tab="home|docs|examples"]`, active one carries
    `.is-active`; `a.su-source`.
  - Footer DOM contract: `footer.su-titleblock` > five `div` > `span` (label) + `b` (value).
  - Placeholder contract for non-mdBook pages (used by Task 7):
    `[data-su-header]` and `[data-su-footer]`, plus a preceding
    `<script>var path_to_root = "../../";</script>`.

- [ ] **Step 1: Create `website/src/assets/blueprint.js`**

```js
// Shared site chrome for the blueprint theme.
//
// Loaded by BOTH the mdBook pages (theme/head.hbs, deferred) and the generated demo
// pages (tools/gallery/host.html.tmpl). Every step is idempotent and no-ops when its
// target is absent, so the same file is safe on all four page kinds.
//
// mdBook exposes `path_to_root` as a global; the demo template defines it by hand.
(function () {
  const ROOT = (typeof path_to_root === "string") ? path_to_root : "";

  const GITHUB = "https://github.com/strowk/bevy_superui";
  const GH_ICON =
    '<svg viewBox="0 0 16 16" width="14" height="14" fill="currentColor" aria-hidden="true">' +
    '<path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 ' +
    '0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53' +
    '.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95' +
    ' 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27' +
    ' 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95' +
    '.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0 0 16 8c0-4.42-3.58-8-8-8z"/>' +
    "</svg>";

  // ---- background ---------------------------------------------------------
  function injectBackground() {
    if (document.querySelector(".su-bg")) return;
    for (const kind of ["wash", "grid-fine", "grid-coarse"]) {
      const el = document.createElement("div");
      el.className = "su-bg su-bg--" + kind;
      document.body.insertBefore(el, document.body.firstChild);
    }
  }

  // ---- which tab is current ----------------------------------------------
  // Resolve the site root against the current URL, then look at the leading path
  // segment. Works at any depth and on the demo pages (path_to_root = "../../").
  function currentTab() {
    let rel;
    try {
      const root = new URL(ROOT || ".", location.href);
      rel = location.pathname.slice(root.pathname.length);
    } catch (_) {
      return "home";
    }
    if (rel.startsWith("docs/")) return "docs";
    if (rel.startsWith("examples/")) return "examples";
    return "home";
  }

  // ---- header -------------------------------------------------------------
  function buildHeader() {
    const bar =
      document.querySelector("[data-su-header]") ||
      document.getElementById("mdbook-menu-bar") ||
      document.getElementById("menu-bar");
    if (!bar || bar.querySelector(".su-brand")) return;

    bar.classList.add("su-bar");

    const active = currentTab();
    const tab = (key, href, label) =>
      '<a class="su-tab' + (key === active ? " is-active" : "") + '"' +
      ' data-su-tab="' + key + '" href="' + ROOT + href + '">' + label + "</a>";

    const frag = document.createElement("div");
    frag.className = "su-bar-inner";
    frag.innerHTML =
      '<a class="su-brand" href="' + ROOT + 'index.html">' +
        '<img class="su-logo" src="' + ROOT + 'logo.svg" alt="">' +
        '<span class="su-word"><b>SUPERUI</b><small>MADE FOR BEVY</small></span>' +
      "</a>" +
      '<nav class="su-tabs" aria-label="Site sections">' +
        tab("home", "index.html", "HOME") +
        tab("docs", "docs/index.html", "DOCS") +
        tab("examples", "examples/index.html", "EXAMPLES") +
      "</nav>";

    const source = document.createElement("a");
    source.className = "su-source";
    source.href = GITHUB;
    source.target = "_blank";
    source.rel = "noopener";
    source.innerHTML = GH_ICON + "<span>SOURCE ↗</span>";

    // mdBook order: .left-buttons (sidebar toggle) first, then our block, then the
    // right cluster. On a demo page there are no mdBook buttons and this is all of it.
    const left = bar.querySelector(".left-buttons");
    if (left) left.after(frag); else bar.appendChild(frag);

    const right = bar.querySelector(".right-buttons") || bar;
    right.appendChild(source);
  }

  // mdBook puts the search toggle in .left-buttons beside the sidebar toggle. The
  // blueprint header wants it in the right cluster, next to SOURCE. Book pages only.
  function relocateSearch() {
    const search =
      document.getElementById("mdbook-search-toggle") ||
      document.getElementById("search-toggle");
    const right = document.querySelector(".menu-bar .right-buttons");
    if (!search || !right || search.parentElement === right) return;
    right.insertBefore(search, right.firstChild);
  }

  // ---- footer -------------------------------------------------------------
  const TITLE_BLOCK = [
    ["PROJECT", "SUPERUI · BEVY UI", ""],
    ["DRAWN BY", "STROWK", ""],
    ["SCALE", "NOT TO SCALE", ""],
    ["REV", "R1 · 0.1 EARLY BUILD", "su-rev"],
    ["LICENCE", "MIT / APACHE-2.0", ""],
  ];

  function buildFooter() {
    if (document.querySelector(".su-titleblock")) return;
    const host =
      document.querySelector("[data-su-footer]") ||
      document.querySelector(".page") ||
      document.body;
    const footer = document.createElement("footer");
    footer.className = "su-titleblock";
    footer.innerHTML = TITLE_BLOCK
      .map(([label, value, cls]) =>
        "<div><span>" + label + "</span><b" +
        (cls ? ' class="' + cls + '"' : "") + ">" + value + "</b></div>")
      .join("");
    host.appendChild(footer);
  }

  // ---- home live counter --------------------------------------------------
  function initLandingCounter() {
    const frame = document.getElementById("su-counter-frame");
    if (!frame) return;
    const label = document.getElementById("su-live-label");
    const overlay = document.getElementById("su-live-overlay");
    const reset = document.getElementById("su-reset");

    const arming = () => {
      if (label) label.textContent = "LIVE — BOOTING RUNTIME…";
      if (overlay) overlay.classList.remove("su-hidden");
    };
    const ready = () => {
      if (label) label.textContent = "LIVE — RUNNING IN YOUR BROWSER";
      if (overlay) overlay.classList.add("su-hidden");
    };

    window.addEventListener("message", (e) => {
      if (e.source === frame.contentWindow && e.data === "superui:ready") ready();
    });
    // Fallback if the message is missed: reveal after load + a grace delay.
    frame.addEventListener("load", () => setTimeout(ready, 4000));
    if (reset) reset.addEventListener("click", () => {
      arming();
      frame.contentWindow.location.reload();
    });
    arming();
  }

  // Home is a prefix chapter, so mdBook wires the first docs chapter's "previous"
  // arrow back to it. It is a marketing page, not something to page back into.
  function suppressLandingPrev() {
    let landing;
    try { landing = new URL(ROOT + "index.html", location.href).href; }
    catch (_) { return; }
    document.querySelectorAll(".nav-chapters.previous, .mobile-nav-chapters.previous")
      .forEach((a) => {
        const href = a.getAttribute("href");
        if (!href) return;
        try { if (new URL(href, location.href).href === landing) a.remove(); }
        catch (_) {}
      });
  }

  function init() {
    injectBackground();
    buildHeader();
    relocateSearch();
    buildFooter();
    suppressLandingPrev();
    initLandingCounter();
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", init);
  } else {
    init();
  }
  window.__superuiInit = init;
})();
```

- [ ] **Step 2: Append header and footer styles to `website/src/assets/blueprint.css`**

Shared, so the demo pages get them too. Geometry only where it is not mdBook-specific —
the fixed positioning and page-offset work lands in Task 3.

```css
/* ============ shared header ============ */
.su-bar {
  display: flex; align-items: stretch;
  background: rgba(9, 32, 48, 0.88);
  border-bottom: 1px solid var(--su-line);
  padding-inline: var(--su-edge);
  min-height: var(--menu-bar-height, 64px);
}
.su-bar-inner { display: flex; align-items: stretch; gap: 20px; min-width: 0; }

.su-brand { display: flex; align-items: center; gap: 13px; text-decoration: none;
  border-bottom: none; }
.su-logo { height: 30px; width: auto; display: block;
  filter: drop-shadow(0 0 10px rgba(52, 230, 214, 0.3)); }
.su-word { line-height: 1; display: inline-flex; flex-direction: column; }
.su-word b { font-family: var(--font-display); font-weight: 700; font-size: 21px;
  letter-spacing: 0.14em; text-transform: uppercase; color: var(--su-heading); }
.su-word small { font-family: var(--font-mono); font-size: 9px; letter-spacing: 0.24em;
  color: var(--su-muted); margin-top: 4px; }

.su-tabs { display: flex; align-items: stretch; }
.su-tab {
  display: flex; align-items: center; padding: 0 18px; white-space: nowrap;
  font-family: var(--font-display); font-weight: 600; font-size: 16px;
  letter-spacing: 0.1em; text-transform: uppercase;
  color: var(--su-text); text-decoration: none;
  border-bottom: 2px solid transparent;
}
.su-tab:hover { border-bottom-color: rgba(52, 230, 214, 0.5); }
.su-tab.is-active { border-bottom-color: var(--su-teal); background: rgba(52, 230, 214, 0.07); }

.su-source {
  display: inline-flex; align-items: center; gap: 7px; align-self: center;
  font-family: var(--font-mono); font-size: 11.5px; letter-spacing: 0.1em;
  color: #0d2b40; background: var(--su-teal); padding: 9px 15px;
  text-decoration: none; border-bottom: none; white-space: nowrap;
}
.su-source:hover { background: var(--su-teal-bright); color: #0d2b40; }

/* Never wrap: --menu-bar-height drives the sidebar top offset and the page padding,
   so a wrapped bar would silently overlap the content. Drop elements instead. */
@media (max-width: 1079px) { .su-word small { display: none; } }
@media (max-width: 767px)  {
  .su-word { display: none; }
  .su-source span { display: none; }
  .su-tab { padding: 0 11px; font-size: 14px; }
  .su-bar-inner { gap: 10px; }
}

/* ============ shared footer (title block) ============ */
.su-titleblock {
  position: relative; z-index: 4; margin-top: 60px;
  border-top: 1px solid var(--su-line); background: rgba(9, 32, 48, 0.86);
  display: grid; grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
  font-family: var(--font-mono); font-size: 10px; letter-spacing: 0.14em;
}
.su-titleblock > div { padding: 13px 16px; border-right: 1px solid var(--su-line-soft); }
.su-titleblock > div:last-child { border-right: none; }
.su-titleblock span { display: block; color: var(--su-dim); font-size: 8.5px; }
.su-titleblock b { display: block; margin-top: 5px; font-weight: 400;
  letter-spacing: 0.1em; color: var(--su-heading); }
.su-titleblock b.su-rev { color: var(--su-teal); }
```

- [ ] **Step 3: Strip the bespoke header and footer from `website/src/index.md`**

Delete exactly two blocks and nothing else:
- the whole `<header class="su-lhead"> … </header>` element (currently lines 2–16),
- the whole `<footer class="su-footer su-lfoot"> … </footer>` element (currently the last
  four lines).

Keep the `<div class="superui-landing"></div>` marker on line 1 — Task 3 depends on it to
hide the sidebar and go full-bleed. Keep `<div class="su-landing-main">` and everything
inside it; Task 5 rewrites that.

- [ ] **Step 4: Drop `additional-js` from `website/book.toml` and delete the old script**

In `website/book.toml`, remove the line `additional-js = ["theme/js/site.js"]`. Leave
`additional-css` as it is. Then:

```bash
rm website/theme/js/site.js
rmdir website/theme/js
```

- [ ] **Step 5: Build and verify the header is present and identical on all three page kinds**

```bash
cd /home/tim/bevy_superui/.claude/worktrees/website-blueprint-restyle
export CARGO_TARGET_DIR=/home/tim/bevy_superui/target
export MDBOOK=/tmp/claude-1000/-home-tim-bevy-superui/6dc0b566-e8ae-4579-bf7d-88828f5447e5/scratchpad/mdbook
(cd website && "$MDBOOK" build)

# the old bespoke header must be gone from home
! grep -q 'su-lhead' website/book/index.html && echo "OK bespoke header removed"
# the old script must no longer be referenced or emitted
! grep -rq 'theme/js/site' website/book/ && echo "OK old site.js gone"
```

Then serve and check in a browser that home, `docs/`, and `examples/` each show the same
header: logo, SUPERUI/MADE FOR BEVY, HOME/DOCS/EXAMPLES with exactly one tab lit per page,
and the SOURCE button. The bar's position and the page layout will still be wrong (it is
not fixed yet) — Task 3 fixes that.

---

## Task 3: mdBook layout integration — fixed full-width header

The task that actually delivers the user's headline requirement. mdBook nests the menu bar
inside the content column, so its left edge jumps by the sidebar's width whenever the
sidebar toggles.

**Files:**
- Modify: `website/theme/css/site.css` (append; Task 1 already reduced this file to the `@import`, the radius reset and the mdBook variable bridge)

**Interfaces:**
- Consumes: `--su-edge`, `--su-app-max` and the header classes from Tasks 1–2.
- Produces: `--menu-bar-height: 64px` on `:root`, and the guarantee that `.menu-bar` is
  `position: fixed` with a constant height. Tasks 4–6 assume content starts below it.

- [ ] **Step 1: Write the layout integration rules**

These replace the old "centre the whole app" block. Note `--su-app-max` is now 1520px
(Task 1), matching the header cap so header, sidebar and article share one gutter.

```css
:root {
  --menu-bar-height: 64px;
  --content-max-width: 900px;
  --su-gutter: max(0px, calc((100vw - var(--su-app-max)) / 2));
  --su-pw-offset: calc(var(--sidebar-width, 300px) + 8px);
}

/* --- the header: fixed, full-width, above sidebar AND content --- */
/* `top: 0 !important` is load-bearing, not decoration. mdBook's book.js runs a
   `controllMenu()` hide-on-scroll routine that writes an INLINE `menu.style.top =
   scrollTop + 'px'` on every scroll event. A non-important `top: 0` loses to that
   inline style and the header walks off-screen as you scroll. An !important author
   declaration beats a non-important inline style, which pins it. */
.menu-bar {
  position: fixed !important;
  top: 0 !important; inset-inline: 0;
  margin: 0 !important;
  z-index: 200;
  height: var(--menu-bar-height);
}
/* mdBook's hover-placeholder reserves the bar's height inside the page and sits at
   z-index 101 over the top 50px, intercepting header clicks. With a fixed bar it has
   no purpose. */
#mdbook-menu-bar-hover-placeholder, #menu-bar-hover-placeholder { display: none !important; }
/* .page carries a negative top margin to compensate for that placeholder; with the
   placeholder gone the compensation must go too, replaced by real padding. */
.page { margin-block-start: 0 !important; padding-top: var(--menu-bar-height); }
/* The sidebar is fixed at top:0 by default — drop it below the bar. */
.sidebar { top: var(--menu-bar-height) !important; }
/* The fixed prev/next arrows span the full viewport height; keep them clear of the bar. */
.nav-chapters { top: var(--menu-bar-height) !important; }

/* --- centre the whole app within --su-app-max on wide screens --- */
/* Only shift the sidebar while it is visible: when collapsed mdBook slides it off by
   its own width, which would not clear the gutter, leaving a sliver on screen. */
html.sidebar-visible body:not(:has(.superui-landing)) .sidebar { left: var(--su-gutter); }
body:not(:has(.superui-landing)) .page-wrapper { margin-inline-end: var(--su-gutter); }
/* mdBook 0.5 sets the inline-start margin from an id selector, which out-specifies a
   class rule — match on the id. */
html.sidebar-visible body:not(:has(.superui-landing)) #mdbook-page-wrapper {
  margin-inline-start: calc(var(--su-gutter) + var(--su-pw-offset)); }
html:not(.sidebar-visible) body:not(:has(.superui-landing)) #mdbook-page-wrapper {
  margin-inline-start: var(--su-gutter); }
html.sidebar-visible body:not(:has(.superui-landing)) .nav-chapters.previous {
  left: calc(var(--su-gutter) + var(--su-pw-offset)); }
html:not(.sidebar-visible) body:not(:has(.superui-landing)) .nav-chapters.previous {
  left: var(--su-gutter); }
body:not(:has(.superui-landing)) .nav-chapters.next { right: var(--su-gutter); }

/* --- home: no sidebar, no sidebar toggle, full-bleed content --- */
body:has(.superui-landing) .sidebar,
body:has(.superui-landing) .sidebar-resize-handle,
body:has(.superui-landing) .nav-chapters,
body:has(.superui-landing) .mobile-nav-chapters,
body:has(.superui-landing) #mdbook-sidebar-toggle,
body:has(.superui-landing) #sidebar-toggle { display: none !important; }
body:has(.superui-landing) .page-wrapper,
body:has(.superui-landing) #mdbook-page-wrapper {
  margin-inline-start: 0 !important; margin-inline-end: 0 !important; left: 0 !important; }
body:has(.superui-landing) .content main { max-width: none; margin: 0; padding: 0; }
/* The marker itself never renders. */
.superui-landing { display: none; }

/* --- wrappers must be transparent or they cover the fixed background layers --- */
#mdbook-page-wrapper, .page-wrapper, .page, .content { background: transparent; }
.page-wrapper, .menu-bar { position: relative; z-index: 1; }

/* single dark theme: hide the theme picker */
#mdbook-theme-toggle, #mdbook-theme-list, #theme-toggle, #theme-list { display: none !important; }
/* our brand replaces the book title; our SOURCE button replaces the git icon */
.menu-bar .menu-title { display: none !important; }
.menu-bar a[title="Git repository"], #git-repository-button,
.menu-bar a[title="Print this book"], #print-button { display: none !important; }
.menu-bar .icon-button { color: var(--su-muted); }
.menu-bar .icon-button:hover { color: var(--su-teal); }
.menu-bar .left-buttons, .menu-bar .right-buttons { display: flex; align-items: center; margin: 0; }
/* push the right cluster (search + SOURCE) to the far edge */
.menu-bar .right-buttons { margin-inline-start: auto; gap: 6px; }
```

Careful: `.menu-bar { position: fixed !important }` and the later
`.page-wrapper, .menu-bar { position: relative; z-index: 1 }` both target `.menu-bar`. The
`!important` wins, but the ordering is confusing — drop `.menu-bar` from that second rule
and leave it as `.page-wrapper { position: relative; z-index: 1; }`.

- [ ] **Step 2: Build and verify the header does not move when the sidebar toggles**

```bash
cd /home/tim/bevy_superui/.claude/worktrees/website-blueprint-restyle
export CARGO_TARGET_DIR=/home/tim/bevy_superui/target
export MDBOOK=/tmp/claude-1000/-home-tim-bevy-superui/6dc0b566-e8ae-4579-bf7d-88828f5447e5/scratchpad/mdbook
(cd website && "$MDBOOK" serve -p 3000)
```

In the browser at 1440px:
1. Open `/docs/getting-started.html`. Note the exact x-position of the logo.
2. Click the sidebar toggle. The logo must not move; the sidebar slides out from under
   the bar.
3. Navigate to `/`. The logo must be at the same x-position, and the sidebar toggle must
   be absent.

- [ ] **Step 3: Verify the responsive ladder — the bar must never wrap**

At 2560px, 1440px, 768px and 360px confirm: the header stays exactly 64px tall (no
wrapping), there is no horizontal page scroll, and content is never hidden behind the bar.
At ≤1079px the `MADE FOR BEVY` strapline is gone; at ≤767px the wordmark and the SOURCE
label are gone too.

---

## Task 4: Docs body — sidebar, typography, callouts, search

**Files:**
- Modify: `website/theme/css/site.css` (append the docs rules)
- Modify: `website/src/assets/blueprint.js` (add the section eyebrow builder)

**Interfaces:**
- Consumes: the layout from Task 3.
- Produces: `.su-eyebrow` inserted as the first child of `.content main` on docs pages.

- [ ] **Step 1: Add the section eyebrow to `blueprint.js`**

Insert this function above `init()`, and add `docsEyebrow();` to `init()` after
`buildHeader();`.

```js
  // Docs pages get a mono eyebrow above the H1 reading "SECTION · <part name>",
  // derived from the active sidebar entry's preceding .part-title. Renders nothing
  // when that structure is absent rather than guessing.
  function docsEyebrow() {
    const main = document.querySelector(".content main");
    if (!main || main.querySelector(".su-eyebrow")) return;
    if (document.querySelector(".superui-landing")) return;
    const h1 = main.querySelector("h1");
    if (!h1) return;

    const active = document.querySelector(".chapter li a.active");
    if (!active) return;
    const li = active.closest("li");
    if (!li) return;

    let part = null;
    for (let n = li.previousElementSibling; n; n = n.previousElementSibling) {
      if (n.classList.contains("part-title")) { part = n.textContent.trim(); break; }
    }
    if (!part) return;

    const eyebrow = document.createElement("div");
    eyebrow.className = "su-eyebrow";
    eyebrow.textContent = "SECTION · " + part.toUpperCase();
    h1.before(eyebrow);
  }
```

The sidebar is populated by `toc.js` upgrading a custom element during parsing, so by the
time this deferred script runs the `.chapter` list exists. If it ever does not, the early
returns leave the page correct but eyebrow-less.

- [ ] **Step 2: Append the docs rules to `website/theme/css/site.css`**

```css
/* ============ sidebar ============ */
.sidebar { background: rgba(7, 26, 40, 0.92); border-right: 1px solid var(--su-line-soft); }
.sidebar .sidebar-scrollbox { padding: 20px 14px 60px; }
.sidebar .sidebar-scrollbox::before {
  content: "CONTENTS"; display: block;
  font-family: var(--font-display); font-weight: 600; font-size: 13px;
  letter-spacing: 0.16em; text-transform: uppercase; color: var(--su-heading);
  padding-bottom: 12px; margin-bottom: 8px; border-bottom: 1px solid var(--su-line);
}
.chapter li.part-title {
  font-family: var(--font-mono); font-size: 9.5px; letter-spacing: 0.2em;
  text-transform: uppercase; color: var(--su-teal); margin: 20px 0 8px; padding: 0 6px;
}
.chapter li a {
  display: flex; align-items: baseline;
  font-family: var(--font-body); font-size: 14px; color: var(--su-body);
  border-left: 2px solid transparent; padding: 6px 8px; margin-bottom: 1px;
}
/* dotted leader running to the right edge — drawing-index feel, no fake page numbers */
.chapter li a::after {
  content: ""; flex: 1; margin: 0 0 5px 7px;
  border-bottom: 1px dotted rgba(220, 233, 242, 0.3);
}
.chapter li a:hover { color: var(--su-heading); background: rgba(52, 230, 214, 0.06); }
.chapter li a.active {
  color: var(--su-heading); background: rgba(52, 230, 214, 0.10);
  border-left-color: var(--su-teal);
}

/* themed scrollbars (page + sidebar + code) */
html, .sidebar-scrollbox, .content pre.shiki {
  scrollbar-width: thin; scrollbar-color: rgba(52, 230, 214, 0.28) transparent;
}
html::-webkit-scrollbar, .sidebar-scrollbox::-webkit-scrollbar,
.content pre.shiki::-webkit-scrollbar { width: 10px; height: 10px; }
html::-webkit-scrollbar-track, .sidebar-scrollbox::-webkit-scrollbar-track,
.content pre.shiki::-webkit-scrollbar-track { background: transparent; }
html::-webkit-scrollbar-thumb, .sidebar-scrollbox::-webkit-scrollbar-thumb,
.content pre.shiki::-webkit-scrollbar-thumb {
  background-color: rgba(52, 230, 214, 0.24);
  border: 2px solid transparent; background-clip: padding-box;
}

/* ============ article typography ============ */
.content main { color: var(--su-text); counter-reset: su-h2; }
.content p, .content li, .content td, .content th, .content dd, .content dt {
  font-size: 1.7rem; line-height: 1.72; color: var(--su-text);
}
.content h1, .content h2, .content h3, .content h4 {
  font-family: var(--font-display); color: var(--su-heading);
  font-weight: 700; text-transform: uppercase; letter-spacing: 0.02em;
}
.content h1 { font-size: clamp(28px, 3.6vw, 40px); line-height: 1.06; margin-bottom: 0; }
/* teal-to-hairline rule under the H1 */
.content main > h1::after {
  content: ""; display: block; margin-top: 14px; height: 2px;
  background: linear-gradient(90deg, var(--su-teal) 0 80px, rgba(220,233,242,0.2) 80px);
}
.su-eyebrow {
  display: flex; align-items: center; gap: 10px;
  font-family: var(--font-mono); font-size: 10.5px; letter-spacing: 0.18em;
  color: var(--su-muted); margin-bottom: 12px;
}
.su-eyebrow::before { content: ""; width: 22px; height: 1px; background: var(--su-teal); }

/* Auto-numbered H2 chips. Scoped to main's DIRECT children so the gallery's category
   headings (inside section.gallery-cat) and any nested headings are untouched. */
.content main > h2 {
  display: flex; align-items: baseline; gap: 13px;
  font-weight: 600; font-size: 21px; letter-spacing: 0.1em;
  border: none; margin-top: 2em;
}
.content main > h2::before {
  counter-increment: su-h2;
  content: counter(su-h2, decimal-leading-zero);
  font-family: var(--font-mono); font-weight: 600; font-size: 14px;
  color: var(--su-teal); border: 1px solid var(--su-teal); padding: 3px 8px;
  align-self: center;
}
body:not(:has(.superui-landing)) .content a:not(.header) { color: var(--su-teal); }

/* ============ code ============ */
.content pre {
  background: rgba(7, 26, 40, 0.7); border: 1px solid var(--su-line);
}
.content pre > code { font-family: var(--font-mono); font-size: 14.5px; line-height: 1.7; }
.content pre.shiki { overflow-x: auto; }
.content pre.shiki code { background: transparent; }
.content code:not(pre code) {
  font-family: var(--font-mono); font-size: 0.9em; color: var(--su-teal);
  background: rgba(52, 230, 214, 0.09); border: 1px solid rgba(52, 230, 214, 0.24);
  padding: 1px 5px;
}
.buttons .clip-button, pre .buttons { color: var(--su-muted); }
.buttons .clip-button:hover { color: var(--su-teal); }

/* ============ blockquote → callout with an "i" rail ============ */
.content blockquote {
  position: relative; margin-inline: 0; padding: 15px 18px 15px 60px;
  border: 1px solid rgba(52, 230, 214, 0.4); border-left: none;
  background: rgba(52, 230, 214, 0.06); color: var(--su-text);
}
.content blockquote::before {
  content: "i"; position: absolute; left: 0; top: 0; bottom: 0; width: 42px;
  display: grid; place-items: center;
  border-right: 1px solid rgba(52, 230, 214, 0.34);
  font-family: var(--font-mono); font-weight: 600; font-size: 15px; color: var(--su-teal);
}

/* ============ prev/next + search ============ */
.nav-chapters { color: var(--su-muted); font-family: var(--font-mono); }
.nav-chapters:hover { color: var(--su-teal); }
.mobile-nav-chapters { color: var(--su-teal); }
#mdbook-searchbar, #searchbar {
  background: rgba(7, 26, 40, 0.92); color: var(--su-text);
  font-family: var(--font-mono); font-size: 14px;
  border: 1px solid var(--su-line); padding: 10px 14px; margin-top: 8px;
}
#mdbook-searchbar:focus, #searchbar:focus {
  outline: none; border-color: var(--su-teal);
  box-shadow: 0 0 0 2px rgba(52, 230, 214, 0.18);
}
.searchresults-header {
  font-family: var(--font-mono); font-size: 11px; letter-spacing: 0.16em;
  text-transform: uppercase; color: var(--su-muted); margin-top: 12px;
}
#mdbook-searchresults, #searchresults { border-top: 1px solid var(--su-line-soft); }
#mdbook-searchresults li, #searchresults li {
  list-style: none; padding: 8px 4px; border-bottom: 1px solid rgba(52, 230, 214, 0.08);
}
#mdbook-searchresults a, #searchresults a { color: var(--su-teal); text-decoration: none; }
#mdbook-searchresults span.teaser, #searchresults span.teaser {
  color: var(--su-muted); font-size: 13px;
}
mark { background: rgba(255, 196, 107, 0.22); color: var(--su-amber); padding: 0 2px; }
```

- [ ] **Step 3: Build and verify the docs page**

```bash
cd /home/tim/bevy_superui/.claude/worktrees/website-blueprint-restyle
export CARGO_TARGET_DIR=/home/tim/bevy_superui/target
export MDBOOK=/tmp/claude-1000/-home-tim-bevy-superui/6dc0b566-e8ae-4579-bf7d-88828f5447e5/scratchpad/mdbook
(cd website && "$MDBOOK" build && "$MDBOOK" serve -p 3000)
```

Open `/docs/getting-started.html` and confirm: eyebrow reads `SECTION · GUIDE`; the H1 sits
over a teal-to-hairline rule; H2s carry `01`, `02`, … chips numbered in document order;
blockquotes render with the teal `i` rail; the sidebar reads `CONTENTS` with dotted leaders;
code blocks keep their Shiki colours on a square navy panel. Press `s` and confirm the
search box opens and returns results.

- [ ] **Step 4: Verify the H2 counter does not leak into the gallery**

Open `/examples/index.html`. The category headings ("Apps", "Stress tests") must have **no**
numbered chip — they live inside `section.gallery-cat`, not as direct children of `main`.

---

## Task 5: Home page

**Files:**
- Modify: `website/src/index.md` (rewrite the body)
- Modify: `website/theme/css/landing.css` (rewrite)
- Modify: `examples/counter/web-embed.html:9` (background colour)

**Interfaces:**
- Consumes: the chrome from Tasks 2–3; `#su-counter-frame`, `#su-live-label`,
  `#su-live-overlay`, `#su-reset` are read by `initLandingCounter()` in `blueprint.js` and
  must keep those exact ids.

- [ ] **Step 1: Rewrite `website/src/index.md`**

```html
<div class="superui-landing"></div>
<div class="su-landing-main">
  <div class="su-hero">
    <div class="su-hero-text">
      <div class="su-tickline"><span></span>RENDERS THROUGH BEVY_UI</div>
      <h1 class="su-h1">Game UI you<br>already know<br><span class="su-accent">how to write.</span></h1>
      <p class="su-tagline">Browser-grade <strong>HTML, CSS and JS</strong> — plus Solid-style
        <strong>TSX</strong> — driving real <strong>Bevy</strong> interfaces. Fine-grained
        reactivity, hot reload, no new mental model.</p>
      <div class="su-cta">
        <a class="su-btn su-btn-primary" href="docs/">READ THE DOCS</a>
        <a class="su-btn su-btn-ghost" href="examples/">SEE EXAMPLES</a>
      </div>
      <div class="su-stack"><span>RUST</span><i>+</i><span>BEVY_UI</span><i>+</i><span>WASM</span><i>+</i><span>NATIVE</span></div>
    </div>
    <div class="su-hero-panel">
      <div class="su-panel-label"><span class="su-accent">DETAIL A</span><span>counter.tsx</span><span class="su-panel-label-end">SCALE 1:1</span></div>
      <div class="su-frame">
        <pre class="su-code"><span class="k">function</span> <span class="fn">Counter</span>() {
  <span class="k">const</span> [count, setCount] = <span class="fn">createSignal</span>(<span class="n">0</span>);
  <span class="k">return</span> (
    &lt;<span class="tag">button</span> <span class="attr">onClick</span>={() =&gt; <span class="fn">setCount</span>(count() + <span class="n">1</span>)}&gt;
      clicked {count()} times
    &lt;/<span class="tag">button</span>&gt;
  );
}</pre>
        <div class="su-live">
          <div class="su-tickline" id="su-live-label"><span></span>LIVE — BOOTING RUNTIME…</div>
          <div class="su-live-stage">
            <iframe id="su-counter-frame" class="su-counter-frame" src="examples/counter/embed.html"
                    title="Live counter example" loading="lazy"></iframe>
            <div class="su-live-overlay" id="su-live-overlay"></div>
          </div>
          <button class="su-btn su-btn-reset" id="su-reset" type="button">RESET</button>
        </div>
      </div>
    </div>
  </div>

  <div class="su-section-head">
    <h2>Principle of operation</h2><div class="su-dash"></div><span>4 CALLOUTS</span>
  </div>
  <div class="su-features">
    <div class="su-feature"><div class="su-num">1</div><h3>Web stack</h3>
      <p>Author interfaces in plain HTML, CSS and JavaScript, rendered natively through bevy_ui.</p>
      <div class="su-feature-tag">DOM SURFACE</div></div>
    <div class="su-feature"><div class="su-num">2</div><h3>Solid-style TSX</h3>
      <p>Fine-grained reactive components driven by signals — no virtual DOM diffing.</p>
      <div class="su-feature-tag">SUPERSOLID</div></div>
    <div class="su-feature"><div class="su-num">3</div><h3>Hot reload</h3>
      <p>Edit a .tsx file and watch the running game update, signal state intact.</p>
      <div class="su-feature-tag">NATIVE BUILDS</div></div>
    <div class="su-feature"><div class="su-num">4</div><h3>Familiar APIs</h3>
      <p>A browser-like DOM and CSS surface, so web knowledge transfers directly.</p>
      <div class="su-feature-tag">ZERO RELEARN</div></div>
  </div>

  <div class="su-note">
    <div class="su-note-rail">!</div>
    <div class="su-note-body">
      <div class="su-note-title">Early build — not final</div>
      <p>superui is in very early development and largely AI-generated. APIs are subject to
         change without notice. Working demos are in the <a href="examples/">examples</a>.</p>
    </div>
  </div>
</div>
```

- [ ] **Step 2: Rewrite `website/theme/css/landing.css`**

Home-only styling. **Every selector in this file must be scoped under
`body:has(.superui-landing)`** — not just the ones that look risky.

This is not stylistic caution; it is closing a bug class that already bit once. `book.toml`
lists `additional-css = ["theme/css/site.css", "theme/css/landing.css"]`, so landing.css
loads after site.css on *every* page, not only the home page. Any unscoped selector in
landing.css therefore wins equal-specificity ties against site.css site-wide. That is
exactly how the old `.su-eyebrow` hero-badge rule hijacked the docs page's section eyebrow
after Task 4 introduced the same class name — the docs eyebrow rendered as a teal pill
instead of a flat muted label, on every docs page.

Two consequences for this step:

- Delete the old `.su-eyebrow`, `.su-lhead`, `.su-lnav`, `.su-lfoot`, `.su-gh`, `.su-chip`,
  `.su-dot`, `.su-code-card` and `.su-tl*` rules outright. They are dead markup from the
  pre-restyle home page and the bespoke header that Task 2 removed. Do not merely stop
  using them.
- Scope every surviving rule. `.su-landing-main { … }` becomes
  `body:has(.superui-landing) .su-landing-main { … }`, and so on for the whole file.

Rules to write, with their exact intent:

| Selector | Declarations |
|---|---|
| `.su-landing-main` | `position:relative; z-index:4; max-width:var(--su-app-max); margin:0 auto; padding:clamp(24px,3.5vw,42px) var(--su-edge) 40px` |
| `.su-hero` | `display:grid; grid-template-columns:repeat(auto-fit,minmax(340px,1fr)); gap:clamp(28px,4vw,52px); align-items:start` |
| `.su-tickline` | `display:flex; align-items:center; gap:10px; font-family:var(--font-mono); font-size:10.5px; letter-spacing:.2em; color:var(--su-teal)`; its `> span` is `width:26px; height:1px; background:var(--su-teal)` |
| `.su-h1` | `font-family:var(--font-display); font-weight:700; font-size:clamp(32px,4.4vw,46px); line-height:1.04; text-transform:uppercase; margin:20px 0 0; color:var(--su-heading)` |
| `.su-h1 .su-accent` | `color:var(--su-teal)` |
| `.su-tagline` | `font-size:17px; line-height:1.6; color:var(--su-body); max-width:470px; margin:20px 0 0`; `strong` → `color:var(--su-text); font-weight:600` |
| `.su-cta` | `display:flex; gap:12px; flex-wrap:wrap; margin-top:28px` |
| `.su-btn` | `font-family:var(--font-display); font-weight:600; font-size:15px; letter-spacing:.1em; text-transform:uppercase; padding:14px 24px; border:1px solid transparent; cursor:pointer; text-decoration:none; display:inline-block` |
| `.su-btn-primary` | `color:#0d2b40; background:var(--su-teal)`; hover `background:var(--su-teal-bright)` |
| `.su-btn-ghost` | `color:var(--su-text); background:transparent; border-color:var(--su-line)`; hover `border-color:var(--su-teal); color:var(--su-teal)` |
| `.su-stack` | `display:flex; align-items:center; gap:12px; flex-wrap:wrap; margin-top:30px; padding-top:16px; border-top:1px solid var(--su-line-soft); font-family:var(--font-mono); font-size:10.5px; letter-spacing:.14em; color:var(--su-muted)`; `i` → `color:var(--su-teal); font-style:normal` |
| `.su-panel-label` | `display:flex; gap:10px; flex-wrap:wrap; padding-bottom:9px; font-family:var(--font-mono); font-size:10px; letter-spacing:.18em; color:var(--su-muted)`; `.su-panel-label-end` → `margin-left:auto` |
| `.su-frame` | `position:relative; border:1px solid var(--su-line); background:rgba(7,26,40,.72)` |
| `.su-frame::before` / `::after` | 11px corner brackets, `border-top`+`border-left` teal 2px at `top:-1px;left:-1px`, and `border-bottom`+`border-right` at `bottom:-1px;right:-1px` |
| `.su-code` | `margin:0; padding:20px; font-family:var(--font-mono); font-size:13px; line-height:1.72; color:#b6cfdf; overflow:auto` |
| `.su-code .k` | `color:#8fb8ff` — keywords |
| `.su-code .fn` | `color:var(--su-teal)` — function names |
| `.su-code .tag` | `color:#9de8b4` — JSX tags |
| `.su-code .attr` | `color:var(--su-teal)` — JSX attributes |
| `.su-code .n` | `color:var(--su-amber)` — numeric literals |
| `.su-live` | `border-top:1px dashed rgba(220,233,242,.24); padding:15px 20px 18px` |
| `.su-live-stage` | `position:relative; height:150px; overflow:hidden; background:var(--su-bg-deep); border:1px solid var(--su-line-soft)` |
| `.su-counter-frame` | `width:100%; height:100%; border:0; display:block; background:var(--su-bg-deep)` |
| `.su-live-overlay` | `position:absolute; inset:0; display:grid; place-items:center; background:var(--su-bg-deep); transition:opacity .3s`; `.su-hidden` → `opacity:0; pointer-events:none` |
| `.su-btn-reset` | `margin-top:10px; font-family:var(--font-mono); font-size:12px; letter-spacing:.08em; color:var(--su-body); background:transparent; border:1px solid var(--su-line); padding:11px 14px` |
| `.su-section-head` | `display:flex; align-items:center; gap:14px; margin-top:56px`; `h2` → `font-family:var(--font-display); font-weight:600; font-size:clamp(18px,2.2vw,23px); letter-spacing:.12em; text-transform:uppercase; margin:0; color:var(--su-heading)`; trailing `span` → mono 10px `color:var(--su-dim)` |
| `.su-dash` | `flex:1; height:1px; background:repeating-linear-gradient(90deg,rgba(220,233,242,.42) 0 7px,transparent 7px 14px); animation:bp-march 6s linear infinite` |
| `.su-features` | `display:grid; grid-template-columns:repeat(auto-fit,minmax(240px,1fr)); gap:20px; margin-top:30px` |
| `.su-feature` | `position:relative; border:1px solid var(--su-line-soft); background:var(--su-panel); padding:24px 20px 20px`; hover `border-color:rgba(52,230,214,.5); background:rgba(52,230,214,.05)` |
| `.su-num` | `position:absolute; top:-15px; left:18px; width:30px; height:30px; border-radius:50%; display:grid; place-items:center; background:var(--su-bg); border:1px solid var(--su-teal); color:var(--su-teal); font-family:var(--font-mono); font-weight:600; font-size:13px` — deliberately round; the Task 1b reset lists only mdBook's own selectors, so no `!important` is needed |
| `.su-feature h3` | `font-family:var(--font-display); font-weight:600; font-size:18px; letter-spacing:.06em; text-transform:uppercase; margin:12px 0 8px; color:var(--su-heading)` |
| `.su-feature p` | `margin:0; font-size:14.5px; line-height:1.55; color:var(--su-body)` |
| `.su-feature-tag` | `margin-top:14px; padding-top:11px; border-top:1px dashed rgba(220,233,242,.2); font-family:var(--font-mono); font-size:9.5px; letter-spacing:.16em; color:var(--su-dim)` |
| `.su-note` | `display:flex; align-items:stretch; margin-top:32px; border:1px solid var(--su-orange); background:rgba(255,138,107,.07)` |
| `.su-note-rail` | `width:44px; flex:0 0 44px; display:grid; place-items:center; border-right:1px solid rgba(255,138,107,.5); font-family:var(--font-mono); font-weight:600; font-size:15px; color:var(--su-orange)` |
| `.su-note-title` | `font-family:var(--font-display); font-weight:600; font-size:14px; letter-spacing:.14em; text-transform:uppercase; color:var(--su-orange)` |
| `.su-note-body p` | `margin:8px 0 0; font-size:14.5px; line-height:1.55; color:#c2d4e0; max-width:780px` |
| `.su-note-body a` | `color:var(--su-orange)` |

Then, because mdBook's `chrome.css` rule `.content a:link` recolours every content link,
restate the button colours at matching specificity (landing.css loads after chrome.css, so
an equal-specificity rule wins):

```css
body:has(.superui-landing) .su-btn-primary { color: #0d2b40; }
body:has(.superui-landing) .su-btn-ghost { color: var(--su-text); }
body:has(.superui-landing) .su-note-body a { color: var(--su-orange); }
```

- [ ] **Step 3: Match the embed background in `examples/counter/web-embed.html`**

Change `background: #0b1220` to `background: #071a28` in the inline `<style>`, so the live
preview does not show a different navy than the panel around it.

- [ ] **Step 4: Build and verify**

```bash
cd /home/tim/bevy_superui/.claude/worktrees/website-blueprint-restyle
export CARGO_TARGET_DIR=/home/tim/bevy_superui/target
export MDBOOK=/tmp/claude-1000/-home-tim-bevy-superui/6dc0b566-e8ae-4579-bf7d-88828f5447e5/scratchpad/mdbook
(cd website && "$MDBOOK" build && "$MDBOOK" serve -p 3000)
```

Open `/`. Confirm: shared header with HOME lit and no sidebar toggle; two-column hero;
corner brackets on the code frame; four numbered callout cards with circular badges; orange
early-build note; title-block footer. Confirm no `//` appears anywhere on the page.

Note the live counter iframe will 404 unless demos have been built locally
(`bash tools/build-demos.sh counter`, which needs the wasm toolchain). A 404 here is
expected and is **not** a failure of this task — verify the frame, label and RESET button
are laid out correctly regardless.

---

## Task 6: Examples gallery cards

**Files:**
- Modify: `tools/mdbook-gallery/src/gallery.rs` (the `render` function and its test)
- Modify: `website/theme/css/site.css` (append the gallery rules)

**Interfaces:**
- Consumes: tokens from Task 1.
- Produces: card markup `a.card > div.card-plate + div.card-body`, where `.card-body`
  contains `.card-title`, `.card-desc` and optionally `.badges > .badge`.

- [ ] **Step 1: Write the failing test in `tools/mdbook-gallery/src/gallery.rs`**

Replace the body of `renders_categories_cards_badges_as_fragment` with this, keeping the
existing `ex()` helper and the second test untouched:

```rust
    #[test]
    fn renders_categories_cards_badges_as_fragment() {
        let examples = vec![
            ex("todomvc", "Apps", &[]),
            ex("todomvc_supersolid", "Apps", &[]),
            ex("horde", "Stress tests", &["Playable game"]),
        ];
        let out = render(&examples);
        // Category headers, first-seen order, each with a rule and an item count.
        assert!(out.contains("<h2>Apps</h2>"));
        assert!(out.contains("<h2>Stress tests</h2>"));
        assert!(out.find("Apps").unwrap() < out.find("Stress tests").unwrap());
        assert!(out.contains(r#"<span class="cat-count">2 ITEMS</span>"#));
        assert!(out.contains(r#"<span class="cat-count">1 ITEM</span>"#));
        // Card links are relative to the /examples/ page.
        assert!(out.contains(r#"href="todomvc/""#));
        assert!(out.contains(r#"href="todomvc_supersolid/""#));
        // Each card opens with a drawing plate carrying the slug.
        assert!(out.contains(r#"<div class="card-plate"><span class="plate-slug">todomvc</span></div>"#));
        // Title/description use plain divs, NOT <h3>/<p> (which mdBook would turn
        // into anchored headings, nesting an <a> inside the card <a> and breaking it).
        assert!(out.contains(r#"<div class="card-title">todomvc title</div>"#));
        assert!(out.contains(r#"<div class="card-desc">"#));
        assert!(!out.contains("<h3"));
        assert!(!out.contains("<p>"));
        // Badge chip.
        assert!(out.contains(r#"<span class="badge">Playable game</span>"#));
        // A fragment — no document shell.
        assert!(!out.contains("<html"));
        assert!(!out.contains("<style"));
    }
```

- [ ] **Step 2: Run it to confirm it fails**

```bash
cd /home/tim/bevy_superui/.claude/worktrees/website-blueprint-restyle
export CARGO_TARGET_DIR=/home/tim/bevy_superui/target
cargo test -p mdbook-gallery renders_categories_cards_badges_as_fragment
```

Expected: FAIL on the `cat-count` assertion (the current markup has no count element).

- [ ] **Step 3: Rewrite `render` in `tools/mdbook-gallery/src/gallery.rs`**

```rust
pub fn render(examples: &[Example]) -> String {
    // Category order = first appearance in the manifest.
    let mut categories: Vec<&str> = Vec::new();
    for e in examples {
        if !categories.iter().any(|c| *c == e.category) {
            categories.push(&e.category);
        }
    }

    let mut out = String::new();
    for cat in categories {
        let items: Vec<&Example> = examples.iter().filter(|e| e.category == cat).collect();
        let count = items.len();
        let noun = if count == 1 { "ITEM" } else { "ITEMS" };
        out.push_str(&format!(
            "<section class=\"gallery-cat\"><div class=\"cat-head\"><h2>{cat}</h2>\
             <span class=\"cat-rule\"></span><span class=\"cat-count\">{count} {noun}</span></div>\
             <div class=\"cards\">"
        ));
        for e in items {
            let badges: String = e
                .tags
                .iter()
                .map(|t| format!("<span class=\"badge\">{t}</span>"))
                .collect();
            let badges_html = if badges.is_empty() {
                String::new()
            } else {
                format!("<div class=\"badges\">{badges}</div>")
            };
            // Use div (not <h3>/<p>) inside the card anchor: mdBook post-processes
            // headings to inject a `<a class="header">` link, and a nested <a> inside
            // this card <a> is invalid HTML — the browser splits the card apart. Plain
            // divs are immune across mdBook versions.
            out.push_str(&format!(
                "<a class=\"card\" href=\"{slug}/\">\
                 <div class=\"card-plate\"><span class=\"plate-slug\">{slug}</span></div>\
                 <div class=\"card-body\">\
                 <div class=\"card-title\">{title}</div>\
                 <div class=\"card-desc\">{desc}</div>{badges_html}</div></a>",
                slug = e.slug,
                title = e.title,
                desc = e.description,
            ));
        }
        out.push_str("</div></section>");
    }
    out
}
```

- [ ] **Step 4: Run the tests to confirm they pass**

```bash
cd /home/tim/bevy_superui/.claude/worktrees/website-blueprint-restyle
export CARGO_TARGET_DIR=/home/tim/bevy_superui/target
cargo test -p mdbook-gallery
```

Expected: both tests PASS.

- [ ] **Step 5: Append the gallery rules to `website/theme/css/site.css`**

```css
/* ============ examples gallery ============ */
.cat-head { display: flex; align-items: center; gap: 14px; margin: 38px 0 22px; }
.cat-head h2 {
  font-family: var(--font-display); font-weight: 600; font-size: 15px;
  letter-spacing: 0.18em; text-transform: uppercase; color: var(--su-heading);
  border: none; margin: 0;
}
.cat-head .cat-rule {
  flex: 1; height: 1px;
  background: repeating-linear-gradient(90deg, rgba(220,233,242,0.4) 0 7px, transparent 7px 14px);
}
.cat-head .cat-count {
  font-family: var(--font-mono); font-size: 10px; letter-spacing: 0.16em; color: var(--su-dim);
}
.cards { display: grid; grid-template-columns: repeat(auto-fill, minmax(292px, 1fr)); gap: 20px; }
.card {
  position: relative; display: block; text-decoration: none; color: inherit;
  border: 1px solid var(--su-line-soft); background: var(--su-panel);
}
.card:hover { border-color: rgba(52, 230, 214, 0.55); background: rgba(52, 230, 214, 0.06); }
.card::before, .card::after { content: ""; position: absolute; width: 11px; height: 11px; }
.card::before { top: -1px; left: -1px;
  border-top: 2px solid var(--su-teal); border-left: 2px solid var(--su-teal); }
.card::after { bottom: -1px; right: -1px;
  border-bottom: 2px solid var(--su-teal); border-right: 2px solid var(--su-teal); }

/* drawing plate: grid ground + crosshair reticle + the package slug */
.card-plate {
  position: relative; height: 118px; overflow: hidden;
  border-bottom: 1px solid var(--su-line-soft); background: rgba(7, 26, 40, 0.6);
  display: grid; place-items: center;
  background-image:
    linear-gradient(rgba(220,233,242,0.06) 1px, transparent 1px),
    linear-gradient(90deg, rgba(220,233,242,0.06) 1px, transparent 1px);
  background-size: 22px 22px;
}
.card-plate::before {
  content: ""; width: 56px; height: 56px; border: 1px solid rgba(52, 230, 214, 0.5);
}
.card-plate::after {
  content: ""; position: absolute; width: 4px; height: 4px; background: var(--su-teal);
}
.plate-slug {
  position: absolute; bottom: 10px; right: 12px;
  font-family: var(--font-mono); font-size: 10px; letter-spacing: 0.1em; color: var(--su-muted);
}
.card-body { padding: 16px 18px 18px; }
.card-title {
  font-family: var(--font-display); font-weight: 600; font-size: 18px;
  letter-spacing: 0.06em; text-transform: uppercase; color: var(--su-heading); margin-bottom: 8px;
}
.card-desc { font-size: 14px; line-height: 1.5; color: var(--su-body); }
.badges { margin-top: 14px; display: flex; gap: 7px; flex-wrap: wrap; }
.badge {
  font-family: var(--font-mono); font-size: 10px; letter-spacing: 0.08em; padding: 3px 8px;
  border: 1px solid rgba(52, 230, 214, 0.35); color: var(--su-teal);
}
```

- [ ] **Step 6: Build and verify the gallery**

```bash
(cd website && "$MDBOOK" build && "$MDBOOK" serve -p 3000)
```

Open `/examples/index.html`. Confirm: two category rows with dashed rules and `4 ITEMS` /
`2 ITEMS` counts; square cards with teal corner brackets top-left and bottom-right; a
gridded plate with a crosshair and the slug bottom-right; no numbered chip on the category
headings.

---

## Task 7: Demo pages

**Files:**
- Create: `website/src/assets/demo.css`
- Modify: `tools/gallery/host.html.tmpl` (rewrite)
- Modify: `xtask/src/host.rs` (two placeholders + test)
- Modify: `xtask/src/manifest.rs` (make three fields live)
- Modify: `tools/gallery/vendor/highlight.css` (retint)

**Interfaces:**
- Consumes: `blueprint.css`, `blueprint.js`, and the `[data-su-header]` / `[data-su-footer]`
  placeholder contract from Task 2.
- Produces: template placeholders `{{CATEGORY}}` and `{{DESCRIPTION}}` alongside the
  existing `{{TITLE}}`, `{{SLUG}}`, `{{WASM_JS}}`, `{{SOURCES_JSON}}`.

- [ ] **Step 1: Write the failing test in `xtask/src/host.rs`**

Extend the existing `renders_canvas_wasm_and_tsx_source` test with assertions for the new
spec column. Keep every existing assertion.

```rust
        let out = render(&ex, &sources);
        assert!(out.contains(r#"id="superui-canvas""#));
        assert!(out.contains("import init from './game_menu.js'"));
        assert!(out.contains("assets/ui/game_menu/app.tsx"));
        assert!(out.contains("cargo run -p game_menu"));
        assert!(!out.contains("{{"), "no unsubstituted template tokens");
        // Site back-nav (relative to /examples/<slug>/).
        assert!(out.contains(r#"href="../""#), "links back to the examples gallery");
        // Shared chrome: same stylesheet and script the book uses.
        assert!(out.contains("../../assets/blueprint.css"), "shares the book's stylesheet");
        assert!(out.contains("../../assets/blueprint.js"), "shares the book's chrome script");
        assert!(out.contains(r#"path_to_root = "../../""#), "chrome can resolve site links");
        assert!(out.contains("data-su-header"), "header placeholder present");
        assert!(out.contains("data-su-footer"), "footer placeholder present");
        // Spec column is filled from the manifest.
        assert!(out.contains("Apps"), "category shown in the breadcrumb/spec column");
        assert!(out.contains("menu"), "description shown in the spec column");
```

- [ ] **Step 2: Run it to confirm it fails**

```bash
cd /home/tim/bevy_superui/.claude/worktrees/website-blueprint-restyle
export CARGO_TARGET_DIR=/home/tim/bevy_superui/target
cargo test -p xtask renders_canvas_wasm_and_tsx_source
```

Expected: FAIL on the `blueprint.css` assertion.

- [ ] **Step 3: Make the manifest fields live in `xtask/src/manifest.rs`**

`description`, `category` and `tags` are about to be read by `host::render`, so remove their
`#[allow(dead_code)]` attributes and the stale comment above them. The struct becomes:

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct Example {
    pub slug: String,
    // Read from the manifest by the CI workflow (jq), not by xtask itself.
    #[allow(dead_code)]
    pub package: String,
    pub title: String,
    pub description: String,
    pub category: String,
    #[serde(default)]
    pub tags: Vec<String>,
}
```

- [ ] **Step 4: Substitute the new placeholders in `xtask/src/host.rs`**

```rust
/// Render the host page for one example: title/slug/category/description substitution,
/// the wasm glue filename, the tag badges, and the embedded authored-source list the
/// code viewer reads.
pub fn render(ex: &Example, sources: &[SourceFile]) -> String {
    let sources_json = serde_json::to_string(sources).expect("sources serialize");
    let badges: String = ex
        .tags
        .iter()
        .map(|t| format!("<span class=\"badge\">{t}</span>"))
        .collect();
    TEMPLATE
        .replace("{{TITLE}}", &ex.title)
        .replace("{{SLUG}}", &ex.slug)
        .replace("{{CATEGORY}}", &ex.category)
        .replace("{{DESCRIPTION}}", &ex.description)
        .replace("{{BADGES}}", &badges)
        .replace("{{WASM_JS}}", &format!("{}.js", ex.slug))
        .replace("{{SOURCES_JSON}}", &sources_json)
}
```

- [ ] **Step 5: Rewrite `tools/gallery/host.html.tmpl`**

```html
<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<base href="./">
<title>{{TITLE}} — superui</title>
<!-- The shared chrome. These two files are copied verbatim (unhashed) from
     website/src/assets/, so this relative path is stable in the built site. -->
<link rel="stylesheet" href="../../assets/blueprint.css">
<link rel="stylesheet" href="../../assets/demo.css">
<link rel="stylesheet" href="../vendor/highlight.css">
<script>var path_to_root = "../../";</script>
<script defer src="../../assets/blueprint.js"></script>
</head>
<body>
  <header class="su-bar" data-su-header></header>

  <main class="demo-main">
    <div class="demo-crumbs">
      <a href="../">← EXAMPLES</a><span>/</span><span>{{CATEGORY}}</span><span>/</span>
      <span class="demo-crumb-now">{{TITLE}}</span>
    </div>

    <div class="demo-grid" id="demo-grid">
      <div class="demo-stage">
        <div class="demo-stage-bar">
          <div class="demo-tabs">
            <button class="demo-tab is-active" data-view="app" type="button">VIEW</button>
            <button class="demo-tab" data-view="code" type="button">PARTS</button>
          </div>
          <span class="demo-stage-meta">WASM · {{SLUG}}</span>
          <button class="demo-expand" id="demo-expand" type="button">⤢ FULL SHEET</button>
        </div>

        <div class="demo-frame" id="app-pane">
          <span class="demo-frame-label demo-frame-label--tl">VIEWPORT — APP MOUNTS HERE</span>
          <span class="demo-frame-label demo-frame-label--br">BEVY · WGPU</span>
          <canvas id="superui-canvas"></canvas>
          <div id="loader" class="demo-loader">
            <div class="demo-radar"><i></i><b></b></div>
            <div class="demo-loader-title">Plotting drawing</div>
            <div class="demo-loader-text">Loading {{TITLE}} — this is a large WebAssembly binary, please wait…</div>
            <div class="demo-loader-bar"><span></span></div>
          </div>
        </div>

        <div class="demo-parts hidden" id="code-pane">
          <div class="demo-parts-list">
            <div class="demo-parts-head">PARTS LIST</div>
            <div id="tabs"></div>
          </div>
          <div class="demo-parts-code">
            <div class="demo-parts-path"><span id="active-path"></span></div>
            <pre id="code-scroll"><code id="code-el"></code></pre>
          </div>
        </div>
      </div>

      <aside class="demo-spec">
        <div class="demo-spec-card">
          <div class="demo-spec-eyebrow">{{CATEGORY}}</div>
          <h1>{{TITLE}}</h1>
          <div class="demo-spec-rule"></div>
          <p>{{DESCRIPTION}}</p>
          <div class="badges">{{BADGES}}</div>
        </div>

        <div class="demo-warn">
          <div class="demo-warn-rail">!</div>
          <div class="demo-warn-body">
            <div class="demo-warn-title">AS-BUILT — WASM</div>
            <p>This is a prebuilt binary. Live UI hot reload is <strong>native only</strong> —
               clone the repo and run it locally to edit the interface while the game runs.</p>
          </div>
        </div>

        <div class="demo-run">
          <div class="demo-run-head">HOW TO RUN IT LOCALLY</div>
          <pre><span class="c"># clone &amp; run</span>
git clone https://github.com/strowk/bevy_superui
cargo run -p <span class="s">{{SLUG}}</span>
<span class="c"># edit the UI while it runs:</span>
cargo run -p <span class="s">{{SLUG}}</span> --features hmr</pre>
        </div>

        <a class="su-source demo-source" href="https://github.com/strowk/bevy_superui"
           target="_blank" rel="noopener">VIEW SOURCE ↗</a>
      </aside>
    </div>
  </main>

  <div data-su-footer></div>

  <script>window.__SOURCES__ = {{SOURCES_JSON}};</script>
  <script src="../vendor/highlight.min.js"></script>
  <script>
    const sources = window.__SOURCES__ || [];
    const tabsEl = document.getElementById('tabs');
    const codeEl = document.getElementById('code-el');
    const pathEl = document.getElementById('active-path');
    const cache = {};
    const LANG_COLOR = { typescript: '#34e6d6', xml: '#ff8a6b', css: '#8fb8ff', javascript: '#ffc46b' };
    const LANG_LABEL = { typescript: 'TSX', xml: 'HTML', css: 'CSS', javascript: 'JS' };

    async function show(i) {
      const src = sources[i];
      [...tabsEl.children].forEach((t, j) => t.classList.toggle('is-active', i === j));
      if (pathEl) pathEl.textContent = src.path;
      if (cache[i] == null) {
        try { cache[i] = await (await fetch(src.path)).text(); }
        catch (e) { cache[i] = '// failed to load ' + src.path; }
      }
      delete codeEl.dataset.highlighted;
      codeEl.textContent = cache[i];
      codeEl.className = 'language-' + src.lang;
      if (window.hljs) hljs.highlightElement(codeEl);
    }

    sources.forEach((src, i) => {
      const b = document.createElement('button');
      b.className = 'demo-part' + (i === 0 ? ' is-active' : '');
      b.type = 'button';
      const color = LANG_COLOR[src.lang] || '#7fa3bd';
      b.innerHTML = '<span class="demo-part-badge" style="color:' + color +
        ';border-color:' + color + '">' + (LANG_LABEL[src.lang] || '·') + '</span>' +
        '<span>' + src.name + '</span>';
      b.onclick = () => show(i);
      tabsEl.appendChild(b);
    });
    if (sources.length) show(0);

    // VIEW / PARTS — works at every width (the old switch was mobile-only).
    const appPane = document.getElementById('app-pane');
    const codePane = document.getElementById('code-pane');
    document.querySelectorAll('.demo-tab').forEach(t => {
      t.onclick = () => {
        document.querySelectorAll('.demo-tab').forEach(x => x.classList.remove('is-active'));
        t.classList.add('is-active');
        const app = t.dataset.view === 'app';
        appPane.classList.toggle('hidden', !app);
        codePane.classList.toggle('hidden', app);
      };
    });

    const expand = document.getElementById('demo-expand');
    const grid = document.getElementById('demo-grid');
    expand.onclick = () => {
      const full = grid.classList.toggle('is-full');
      expand.textContent = full ? '⤡ FIT SHEET' : '⤢ FULL SHEET';
    };
  </script>

  <script type="module">
    import init from './{{WASM_JS}}';
    const loader = document.getElementById('loader');
    init().catch((e) => {
      // winit throws to yield control back to the browser event loop; ignore that one.
      if (!(e instanceof Error) || !String(e.message).includes('Using exceptions for control flow')) {
        console.error(e);
      }
    }).finally(() => { if (loader) loader.remove(); });
  </script>
</body>
</html>
```

- [ ] **Step 6: Run the xtask tests to confirm they pass**

```bash
cd /home/tim/bevy_superui/.claude/worktrees/website-blueprint-restyle
export CARGO_TARGET_DIR=/home/tim/bevy_superui/target
cargo test -p xtask
```

Expected: PASS. If `no unsubstituted template tokens` fails, a `{{…}}` token in the template
has no matching `.replace()` in `host.rs`.

- [ ] **Step 7: Create `website/src/assets/demo.css`**

Rules to write:

| Selector | Declarations |
|---|---|
| `.hidden` | `display:none !important` |
| `.demo-main` | `position:relative; z-index:4; max-width:1620px; margin:0 auto; padding:clamp(22px,3vw,36px) var(--su-edge) 60px` |
| `.demo-crumbs` | `display:flex; align-items:center; gap:11px; flex-wrap:wrap; font-family:var(--font-mono); font-size:11px; letter-spacing:.12em; color:var(--su-muted)`; `a` → `color:var(--su-teal); text-decoration:none`; `.demo-crumb-now` → `color:var(--su-heading)` |
| `.demo-grid` | `display:grid; grid-template-columns:minmax(0,1.62fr) minmax(268px,1fr); gap:clamp(16px,2.4vw,24px); margin-top:18px; align-items:start` |
| `.demo-grid.is-full` | `display:flex; flex-direction:column` |
| `@media (max-width:900px)` | `.demo-grid { display:flex; flex-direction:column }` |
| `.demo-stage-bar` | `display:flex; align-items:center; gap:9px; flex-wrap:wrap; padding-bottom:9px` |
| `.demo-tabs` | `display:flex` |
| `.demo-tab` | `font-family:var(--font-mono); font-size:10.5px; letter-spacing:.16em; cursor:pointer; padding:7px 15px; border:1px solid var(--su-line); background:transparent; color:var(--su-body); margin-right:-1px` |
| `.demo-tab.is-active` | `border-color:var(--su-teal); background:rgba(52,230,214,.12); color:var(--su-teal)` |
| `.demo-stage-meta` | `margin-left:auto; font-family:var(--font-mono); font-size:9.5px; letter-spacing:.18em; color:var(--su-dim)` |
| `.demo-expand` | same as `.demo-tab` but `font-size:9.5px; letter-spacing:.14em; padding:7px 11px; margin-right:0` |
| `.demo-frame` | `position:relative; border:1px solid var(--su-line); background:rgba(7,26,40,.85); overflow:hidden; min-height:390px; height:520px`; plus a `background-image` grid at `26px 26px` using `rgba(220,233,242,0.055)` lines |
| `.demo-frame::before` / `::after` | 13px teal corner brackets at the two diagonal corners (`top:9px;left:9px` with `border-top`+`border-left`; `bottom:9px;right:9px` with `border-bottom`+`border-right`), 1px `var(--su-teal)`, matching `.card`'s treatment |
| `.demo-grid.is-full .demo-frame` | `min-height:540px; height:calc(100vh - 250px)` |
| `.demo-frame-label` | `position:absolute; z-index:2; font-family:var(--font-mono); font-size:9.5px; letter-spacing:.2em`; `--tl` → `top:14px; left:32px; color:var(--su-teal)`; `--br` → `bottom:14px; right:32px; color:var(--su-dim)` |
| `#superui-canvas` | `width:100%; height:100%; display:block` |
| `.demo-loader` | `position:absolute; inset:0; display:flex; flex-direction:column; align-items:center; justify-content:center; gap:12px; text-align:center; padding:0 24px; background:rgba(7,26,40,.9)` |
| `.demo-radar` | `position:relative; width:66px; height:66px; border:1px solid var(--su-line-soft)`; `i` → `position:absolute; top:50%; left:50%; width:50%; height:1px; background:var(--su-teal); transform-origin:0 50%; animation:bp-plot 2.4s linear infinite`; `b` → `position:absolute; top:50%; left:50%; width:5px; height:5px; margin:-2.5px 0 0 -2.5px; background:var(--su-teal)` |
| `.demo-loader-title` | `font-family:var(--font-display); font-weight:600; font-size:15px; letter-spacing:.2em; text-transform:uppercase; color:var(--su-teal)` |
| `.demo-loader-text` | `font-family:var(--font-mono); font-size:12.5px; line-height:1.6; color:var(--su-muted); max-width:410px` |
| `.demo-loader-bar` | `position:relative; width:240px; height:2px; background:rgba(220,233,242,.16); overflow:hidden`; `span` → `position:absolute; top:0; width:30%; height:100%; background:var(--su-teal); animation:bp-sweep 1.6s ease-in-out infinite` |
| `.demo-parts` | `display:grid; grid-template-columns:214px minmax(0,1fr); border:1px solid var(--su-line); background:rgba(7,26,40,.85); min-height:390px` |
| `.demo-parts-list` | `border-right:1px solid var(--su-line-soft); padding:12px 8px; overflow:auto; background:var(--su-panel)` |
| `.demo-parts-head` | `font-family:var(--font-mono); font-size:9px; letter-spacing:.2em; color:var(--su-dim); padding:2px 8px 10px` |
| `.demo-part` | `display:flex; align-items:center; gap:8px; width:100%; text-align:left; font-family:var(--font-mono); font-size:11.5px; padding:6px 8px; cursor:pointer; background:transparent; border:none; border-left:2px solid transparent; color:var(--su-body)` |
| `.demo-part.is-active` | `color:var(--su-heading); background:rgba(52,230,214,.12); border-left-color:var(--su-teal)` |
| `.demo-part-badge` | `display:inline-grid; place-items:center; min-width:32px; font-size:8.5px; letter-spacing:.06em; padding:2px 4px; border:1px solid currentColor` |
| `.demo-parts-path` | `display:flex; padding:9px 14px; border-bottom:1px solid var(--su-line-soft); font-family:var(--font-mono); font-size:11px; letter-spacing:.08em; color:var(--su-muted)` |
| `#code-scroll` | `margin:0; overflow:auto; max-height:520px` |
| `.demo-spec` | `display:flex; flex-direction:column; gap:14px; min-width:0` |
| `.demo-spec-card` | `border:1px solid var(--su-line-soft); background:var(--su-panel); padding:20px` |
| `.demo-spec-eyebrow` | `font-family:var(--font-mono); font-size:10px; letter-spacing:.18em; color:var(--su-dim)` |
| `.demo-spec-card h1` | `font-family:var(--font-display); font-weight:700; font-size:clamp(23px,2.7vw,30px); letter-spacing:.03em; text-transform:uppercase; margin:11px 0 0; color:var(--su-heading)` |
| `.demo-spec-rule` | `margin-top:11px; height:2px; background:linear-gradient(90deg,var(--su-teal) 0 54px,rgba(220,233,242,.18) 54px)` |
| `.demo-spec-card p` | `font-size:15px; line-height:1.6; color:var(--su-body); margin:14px 0 0` |
| `.demo-warn` | `display:flex; align-items:stretch; border:1px solid rgba(255,138,107,.5); background:rgba(255,138,107,.06)` |
| `.demo-warn-rail` | `width:40px; flex:0 0 40px; display:grid; place-items:center; border-right:1px solid rgba(255,138,107,.4); font-family:var(--font-mono); font-weight:600; font-size:14px; color:var(--su-orange)` |
| `.demo-warn-body` | `padding:14px 16px`; `.demo-warn-title` → `font-family:var(--font-mono); font-size:10px; letter-spacing:.18em; color:var(--su-orange)`; `p` → `margin:7px 0 0; font-size:13.5px; line-height:1.55; color:#c2d4e0`; `strong` → `color:#ffb59f` |
| `.demo-run` | `border:1px solid var(--su-line); background:rgba(7,26,40,.7)` |
| `.demo-run-head` | `padding:9px 13px; border-bottom:1px solid var(--su-line-soft); font-family:var(--font-mono); font-size:10px; letter-spacing:.16em; color:var(--su-muted)` |
| `.demo-run pre` | `margin:0; padding:14px 16px; font-family:var(--font-mono); font-size:12px; line-height:1.74; color:#b6cfdf; white-space:pre-wrap; word-break:break-word`; `.c` → `color:var(--su-dim)`; `.s` → `color:#9de8b4` |
| `.demo-source` | `justify-content:center; padding:13px; font-family:var(--font-display); font-weight:600; font-size:14px; letter-spacing:.14em` |
| `.badges` / `.badge` | same as the gallery rules in Task 6 — duplicated here because demo pages do not load `site.css` |

- [ ] **Step 8: Retint `tools/gallery/vendor/highlight.css`**

The file is minified GitHub Dark. Replace only the colour values, leaving the selectors and
the leading `pre code.hljs{…}` rule intact:

| Old | New | Token |
|---|---|---|
| `#c9d1d9` | `#b6cfdf` | base text |
| `#0d1117` | `transparent` | `.hljs` background — the panel behind it supplies the colour |
| `#ff7b72` | `#8fb8ff` | keywords |
| `#d2a8ff` | `#34e6d6` | titles / function names |
| `#79c0ff` | `#ffc46b` | numbers, literals, attributes |
| `#a5d6ff` | `#9de8b4` | strings |
| `#ffa657` | `#34e6d6` | built-ins |
| `#8b949e` | `#5c7f99` | comments |
| `#7ee787` | `#9de8b4` | tag names |
| `#1f6feb` | `#34e6d6` | sections |
| `#f2cc60` | `#ffc46b` | bullets |

- [ ] **Step 9: Generate a demo page and verify it without building wasm**

Building real wasm needs the exact `wasm-bindgen` CLI version and takes a long time. Render
the template against a staged directory instead — this exercises the real `xtask` code path:

```bash
cd /home/tim/bevy_superui/.claude/worktrees/website-blueprint-restyle
export CARGO_TARGET_DIR=/home/tim/bevy_superui/target
export MDBOOK=/tmp/claude-1000/-home-tim-bevy-superui/6dc0b566-e8ae-4579-bf7d-88828f5447e5/scratchpad/mdbook

# build the book first so website/book/assets/ exists for the demo page to link against
(cd website && "$MDBOOK" build)

# stage a fake demo dir where the host page expects its sources
mkdir -p website/book/examples/game_menu/assets/ui/game_menu
cp examples/game_menu/assets/ui/game_menu/*.tsx website/book/examples/game_menu/assets/ui/game_menu/ 2>/dev/null || true
cargo run -q -p xtask -- host-page --slug game_menu --out website/book/examples/game_menu

# serve the built book and open the demo page
(cd website/book && python3 -m http.server 3001)
```

Open `http://localhost:3001/examples/game_menu/`. Confirm: the header is identical to the
book's (logo, HOME/DOCS/EXAMPLES with **EXAMPLES lit**, SOURCE button, no sidebar toggle,
no search); breadcrumb reads `← EXAMPLES / Apps / Game Menu`; the drawing frame shows corner
brackets, the grid ground, both corner labels and the rotating radar loader; PARTS lists the
`.tsx` files with coloured badges and shows highlighted code; `⤢ FULL SHEET` stacks the
layout; the spec column shows the description, badges, the orange native-only warning, the
run block and VIEW SOURCE; the title-block footer is present.

The canvas stays empty and the console shows a 404 for `game_menu.js` — expected, since no
wasm was built. Everything else must render.

- [ ] **Step 10: Clean up the staged directory**

```bash
rm -rf website/book/examples/game_menu
```

---

## Task 8: Cross-page verification

No new code. This is the gate that the user's actual requirement is met.

**Files:** none.

- [ ] **Step 1: Full clean build and the Rust test suite**

```bash
cd /home/tim/bevy_superui/.claude/worktrees/website-blueprint-restyle
export CARGO_TARGET_DIR=/home/tim/bevy_superui/target
export MDBOOK=/tmp/claude-1000/-home-tim-bevy-superui/6dc0b566-e8ae-4579-bf7d-88828f5447e5/scratchpad/mdbook
rm -rf website/book
(cd website && "$MDBOOK" build)
cargo test -p mdbook-gallery -p xtask
```

Expected: build succeeds with no preprocessor errors; all tests pass.

- [ ] **Step 2: Confirm no stale theme remnants survive**

```bash
# no cyber-theme fonts
! grep -rq 'Chakra Petch\|Space Grotesk\|JetBrains Mono' website/ tools/ && echo "OK fonts"
# no cyber-theme ground colour
! grep -rq '#080b12' website/ tools/ && echo "OK palette"
# no "//" label idiom in the shipped chrome
! grep -q '//' website/src/index.md && echo "OK no slash labels on home"
```

If the last check trips on a URL (`https://`), inspect the hit rather than assuming failure.

- [ ] **Step 3: Header identity check — the headline requirement**

Serve at 1440px and screenshot the header region of `/`, `/docs/getting-started.html` and
`/examples/index.html`. The three crops must be identical except for which tab is lit and
the presence of the sidebar toggle and search on the two book pages. Specifically verify the
logo's x-position is the same on all three.

Then on `/docs/getting-started.html`, toggle the sidebar and confirm the header does not
shift.

- [ ] **Step 4: Responsive sweep**

At 360px, 768px, 1440px and 2560px, on each of `/`, `/docs/getting-started.html`,
`/examples/index.html` and the staged demo page: no horizontal scroll, no content hidden
behind the fixed header, header exactly 64px tall and never wrapped.

- [ ] **Step 5: Report status**

Summarise what was verified and, honestly, what was not — in particular that the live
counter iframe and the demo canvas were not exercised against real wasm, and that CI is the
first place the full pipeline runs end to end.

---

## Self-Review

**Spec coverage.** Every spec section maps to a task: tokens/background → 1; header, footer,
`blueprint.js`, the `//` removals → 2; D3 fixed header, D2 home sidebar, responsive ladder,
gutter/`--su-app-max` → 3; D5 docs motif, sidebar `CONTENTS`, eyebrow → 4; home page and the
kept live counter → 5; gallery cards → 6; D4 demo layout, template placeholders, omitted
search, highlight retint → 7; verification → 8. The version chip's removal is covered by
Task 2 rebuilding the header from scratch (it is never re-added) and its content reappears
in the footer's `REV` cell.

**Placeholder scan.** No TBD/TODO. Two deliberate deviations from the skill's defaults,
both stated rather than hidden: no commit steps (the user asked for an uncommitted working
tree), and the decorative CSS in Tasks 5 and 7 is specified as exact selector→declaration
tables rather than transcribed stylesheets. Every value in those tables is concrete; nothing
says "style appropriately".

**Type consistency.** `render(&Example, &[SourceFile]) -> String` keeps its signature in
Task 7. The template placeholder set is `{{TITLE}}`, `{{SLUG}}`, `{{CATEGORY}}`,
`{{DESCRIPTION}}`, `{{BADGES}}`, `{{WASM_JS}}`, `{{SOURCES_JSON}}` — the same seven in the
template (Step 5) and in `host.rs` (Step 4), which the existing `no unsubstituted template
tokens` assertion enforces. Card markup is `a.card > .card-plate + .card-body` in both the
Rust generator (Task 6 Step 3) and the CSS (Task 6 Step 5). Header classes `.su-bar`,
`.su-bar-inner`, `.su-brand`, `.su-tabs`, `.su-tab.is-active`, `.su-source` are identical in
`blueprint.js` (Task 2 Step 1), `blueprint.css` (Task 2 Step 2) and the demo template
(Task 7 Step 5). Home ids `su-counter-frame` / `su-live-label` / `su-live-overlay` /
`su-reset` match between `initLandingCounter()` and `index.md`.

**One issue found and fixed inline:** Task 3's original rule set both
`.menu-bar { position: fixed !important }` and `.page-wrapper, .menu-bar { position:
relative }`. The `!important` wins so behaviour was correct, but the contradiction would
mislead a reader — Step 1 now instructs dropping `.menu-bar` from the second rule.
