# Website redesign — cyber/terminal theme + live counter — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restyle the mdBook site (`website/`) into the cyber/terminal redesign and embed a live, restartable compiled `counter` WebAssembly example on the landing page.

**Architecture:** Pure mdBook extension points — no template fork. Google-Fonts `<link>` via `theme/head.hbs`; theme in `theme/css/site.css` (global) + `theme/css/landing.css` (landing); one `theme/js/site.js` (via `additional-js`) injects the animated background layers, reskins the menu-bar into the redesign header, and wires the landing counter iframe. The `counter` example is built to wasm by the existing gallery pipeline (`examples/gallery.json` matrix) and embedded in an `<iframe>`; Reset reloads the iframe.

**Tech Stack:** mdBook 0.4.x (theme `navy`), Handlebars head partial, vanilla CSS/JS, Bevy→wasm via `wasm-bindgen`, Google Fonts.

## Global Constraints

- **Design reference (ground truth):** `target/redesign_website_design/` — `body.html` (reconstructed DOM, all inline styles), `design-notes.md` (tokens), `keyframes.css`, and screenshots `screenshot.jpeg` (landing), `redesign_docs.jpeg`, `redesign_examples.jpeg`. Match these.
- **Fonts:** Chakra Petch (display/headings/logo/CTA), JetBrains Mono (code/labels/nav/chips/footer), Space Grotesk (body). Delivered via Google Fonts CDN.
- **Palette (exact):** teal `#34e6d6` / bright `#4ff0e0` / deep `#2bd0c0` / light `#7ff3e9`; amber `#ffb454` / light `#ffce8a`; text body `#cdd8e6`, heading `#f2f8f7`, muted `#8a97a8` / `#5f7085`; bg `#06080d`→`#080b12`; panels `rgba(9,13,20,.85)` / `rgba(13,19,28,.7)`; teal borders `rgba(52,230,214,.16)`→`.55`; amber border `rgba(255,180,84,.4)`. Syntax: purple `#c58fff`, cyan-fn `#7ff3e9`, blue-tag `#5fd7ff`, amber-num `#ffb454`.
- **Motion:** all animation must be disabled under `@media (prefers-reduced-motion: reduce)` (static gradient + grid remain).
- **Single dark theme:** no theme toggle (hide mdBook's).
- **Keep working:** mdBook search, sidebar toggle, and print must still function on doc pages.
- **Site base URL:** `/bevy_superui/` (`book.toml` `site-url`). Landing lives at site root; the counter iframe uses the relative `src="examples/counter/embed.html"`.
- **Do not use git worktrees** (huge `target/`). Work on branch `website-redesign-cyber-theme`.
- Commit after every task.

---

## File structure

| File | New/Modify | Responsibility |
|---|---|---|
| `examples/gallery.json` | Modify | Add the `counter` gallery entry. |
| `tools/build-demos.sh` | Modify | Build `counter`; drop `embed.html` into its output. |
| `examples/counter/web-embed.html` | Create | Canvas-only landing-embed host page (source; copied to output as `embed.html`). |
| `.github/workflows/deploy-pages.yml` | Modify | Copy `embed.html` into the counter artifact. |
| `website/theme/head.hbs` | Create | Google Fonts `<link>`s. |
| `website/book.toml` | Modify | Add `landing.css` to `additional-css`; add `additional-js`. |
| `website/theme/js/site.js` | Create | Background layers; menu-bar→header; landing counter wiring. |
| `website/theme/css/site.css` | Modify (expand) | Global theme: vars, bg layers + keyframes + reduced-motion, header, sidebar, content, code, callouts, prev/next, gallery cards, footer. |
| `website/theme/css/landing.css` | Create | Landing-only styles. |
| `website/src/index.md` | Modify (rewrite) | Redesign landing markup. |

**Verification convention (all tasks):** the executing session serves the built book and screenshots each affected screen with Playwright, comparing against the reference JPEGs in `target/redesign_website_design/`. To serve locally: `bash tools/build-demos.sh counter` (once), then `mdbook serve website -p 3000` (or `mdbook build website` + a static server on `website/book`). Subagents that lack a browser run the build + `grep` checks in their steps and report; the orchestrator does the visual diff during review.

---

### Task 1: Counter example — gallery entry, wasm build, embed page, CI

**Files:**
- Modify: `examples/gallery.json`
- Modify: `tools/build-demos.sh:29-42,73-88`
- Create: `examples/counter/web-embed.html`
- Modify: `.github/workflows/deploy-pages.yml:97-98`

**Interfaces:**
- Produces: built demo dir `website/src/examples/counter/` containing `counter.js`, `counter_bg.wasm`, `index.html` (gallery page), `embed.html` (canvas-only), `assets/`. The landing iframe (Task 7) consumes `examples/counter/embed.html`, which `postMessage`s `'superui:ready'` to its parent after `init()` settles.

- [ ] **Step 1: Add the counter gallery entry**

Edit `examples/gallery.json` — add as the FIRST element of `"examples"` (it's the smallest intro example):

```json
    { "slug": "counter", "package": "counter", "category": "Apps",
      "title": "Counter",
      "description": "The smallest Supersolid app — a single reactive button, in Solid-style .tsx." },
```

- [ ] **Step 2: Create the canvas-only embed host page**

Create `examples/counter/web-embed.html`:

```html
<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<base href="./">
<title>counter — live embed</title>
<style>
  html, body { margin: 0; height: 100%; background: #0b1220; overflow: hidden; }
  #superui-canvas { width: 100%; height: 100%; display: block; }
</style>
</head>
<body>
  <canvas id="superui-canvas"></canvas>
  <script type="module">
    import init from './counter.js';
    init().catch((e) => {
      // winit throws to yield to the browser event loop; ignore that one.
      if (!(e instanceof Error) || !String(e.message).includes('Using exceptions for control flow')) {
        console.error(e);
      }
    }).finally(() => {
      // Tell the landing page the runtime has booted.
      try { window.parent.postMessage('superui:ready', '*'); } catch (_) {}
    });
  </script>
</body>
</html>
```

- [ ] **Step 3: Teach build-demos.sh to build counter + drop embed.html**

In `tools/build-demos.sh`, add `counter` to the `BUILD_ARGS` map and the default slug list, and copy the embed page after the host page is generated.

Add to the `declare -A BUILD_ARGS=(` block:

```bash
  [counter]=""
```

Change the default slug list line:

```bash
  slugs=(counter todomvc todomvc_supersolid game_menu citadel horde)
```

In the per-slug loop, immediately after the `cp -r "examples/$slug/assets" "$out/assets"` line, add:

```bash
  # The landing page embeds counter via a canvas-only host page (no code viewer).
  if [ "$slug" = "counter" ]; then
    cp "examples/counter/web-embed.html" "$out/embed.html"
  fi
```

- [ ] **Step 4: Build the counter demo**

Run: `bash tools/build-demos.sh counter`
Expected: ends with `-> website/src/examples/counter`. Then:

Run: `ls website/src/examples/counter`
Expected output includes: `assets  counter.js  counter_bg.wasm  embed.html  index.html`

- [ ] **Step 5: Verify the embed actually runs the counter**

Serve and load the embed page in a browser (orchestrator uses Playwright; manual fallback below):

```bash
mdbook build website
# serve website/book on :3000 with any static server, then open
# http://localhost:3000/examples/counter/embed.html
```
Expected: a teal "clicked 0 times" button renders on a dark canvas; clicking increments it; the parent receives a `superui:ready` message (visible in Task 7). No uncaught console errors other than the ignored winit control-flow throw.

- [ ] **Step 6: Wire embed.html into the deploy workflow**

In `.github/workflows/deploy-pages.yml`, the per-slug `build` job generates the standard host page and copies assets. After the `Copy app assets` step (line ~97-98), add a step so the counter artifact also carries `embed.html`:

```yaml
      - name: Add landing embed page (counter only)
        if: ${{ matrix.slug == 'counter' }}
        run: cp "examples/counter/web-embed.html" "stage/${{ matrix.slug }}/embed.html"
```

(The `discover` job already picks `counter` up from `gallery.json`, so CI builds it automatically.)

- [ ] **Step 7: Commit**

```bash
git add examples/gallery.json tools/build-demos.sh examples/counter/web-embed.html .github/workflows/deploy-pages.yml
git commit -m "feat(website): build counter demo + canvas-only landing embed page"
```

---

### Task 2: Theme scaffolding — fonts, config, animated background

**Files:**
- Create: `website/theme/head.hbs`
- Modify: `website/book.toml:9`
- Create: `website/theme/js/site.js`
- Modify: `website/theme/css/site.css` (prepend theme vars + background layers)

**Interfaces:**
- Produces: CSS custom properties (`--su-teal`, `--su-amber`, `--su-bg`, `--su-panel`, `--su-border`, `--su-text`, `--su-heading`, `--su-muted`, `--font-display`, `--font-mono`, `--font-body`) available site-wide; three fixed `<div class="su-bg su-bg--aurora|--grid|--scan">` layers injected on every page by `site.js`; the `initSuperui()` idempotent entry point in `site.js` that later tasks extend.

- [ ] **Step 1: Add the Google Fonts head partial**

Create `website/theme/head.hbs`:

```html
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link href="https://fonts.googleapis.com/css2?family=Chakra+Petch:wght@500;600;700&family=JetBrains+Mono:wght@400;500;700&family=Space+Grotesk:wght@400;500;600&display=swap" rel="stylesheet">
```

- [ ] **Step 2: Register landing.css + site.js in book.toml**

In `website/book.toml`, replace the `additional-css` line and add `additional-js` under `[output.html]`:

```toml
additional-css = ["theme/css/site.css", "theme/css/landing.css"]
additional-js = ["theme/js/site.js"]
```

- [ ] **Step 3: Create the landing.css placeholder**

Create `website/theme/css/landing.css` with a header comment so the build finds it (filled in Task 6):

```css
/* Landing-page-only styles (index.md). See Task 6. */
```

- [ ] **Step 4: Prepend theme tokens + background layers to site.css**

At the TOP of `website/theme/css/site.css`, before the existing content, insert:

```css
/* ============ superui redesign theme ============ */
:root, .navy, .light, .rust, .coal, .ayu {
  --su-teal: #34e6d6;
  --su-teal-bright: #4ff0e0;
  --su-teal-deep: #2bd0c0;
  --su-teal-light: #7ff3e9;
  --su-amber: #ffb454;
  --su-amber-light: #ffce8a;
  --su-bg: #080b12;
  --su-panel: rgba(13, 19, 28, 0.7);
  --su-panel-solid: #0d131c;
  --su-border: rgba(52, 230, 214, 0.16);
  --su-border-strong: rgba(52, 230, 214, 0.35);
  --su-text: #cdd8e6;
  --su-heading: #f2f8f7;
  --su-muted: #8a97a8;
  --su-muted-dim: #5f7085;
  --font-display: "Chakra Petch", system-ui, sans-serif;
  --font-mono: "JetBrains Mono", ui-monospace, "Cascadia Code", Consolas, monospace;
  --font-body: "Space Grotesk", system-ui, sans-serif;

  /* Override mdBook navy variables so built-in chrome matches. */
  --bg: var(--su-bg);
  --fg: var(--su-text);
  --sidebar-bg: #0b0f17;
  --sidebar-fg: var(--su-text);
  --sidebar-active: var(--su-teal);
  --links: var(--su-teal-light);
}

html, body { background: var(--su-bg); color: var(--su-text); font-family: var(--font-body); }

/* Layered fixed background (divs injected by site.js). */
.su-bg { position: fixed; inset: 0; pointer-events: none; }
.su-bg--aurora {
  inset: -20%; z-index: 0; filter: blur(30px); opacity: 0.8;
  background:
    radial-gradient(600px 500px at 30% 40%, rgba(52,230,214,0.14), transparent 60%),
    radial-gradient(560px 480px at 70% 60%, rgba(255,180,84,0.09), transparent 60%);
  animation: su-aurora 24s ease-in-out infinite;
}
.su-bg--grid {
  z-index: 0; opacity: 0.6;
  background-image:
    linear-gradient(rgba(52,230,214,0.05) 1px, transparent 1px),
    linear-gradient(90deg, rgba(52,230,214,0.05) 1px, transparent 1px);
  background-size: 40px 40px;
  animation: su-drift 18s linear infinite;
}
.su-bg--scan {
  z-index: 0; opacity: 0.35; mix-blend-mode: overlay;
  background: repeating-linear-gradient(transparent 0 2px, rgba(0,0,0,0.35) 2px 3px);
}
/* Base page gradient behind the layers. */
body::before {
  content: ""; position: fixed; inset: 0; z-index: -1; pointer-events: none;
  background:
    radial-gradient(1200px 600px at 80% -10%, rgba(52,230,214,0.10), transparent 60%),
    radial-gradient(900px 500px at 0% 110%, rgba(255,180,84,0.06), transparent 55%),
    linear-gradient(#06080d, #080b12);
}
/* mdBook content must sit above the background layers. */
.page-wrapper, #menu-bar, .menu-bar { position: relative; z-index: 1; }

@keyframes su-aurora {
  0%,100% { transform: translate3d(0,0,0) scale(1); }
  50% { transform: translate3d(5%,4%,0) scale(1.12); }
}
@keyframes su-drift { 0% { background-position: 0 0; } 100% { background-position: 40px 40px; } }
@keyframes su-pulse {
  0%,100% { opacity: 1; box-shadow: 0 0 0 0 rgba(52,230,214,0.55); }
  50% { opacity: 0.55; box-shadow: 0 0 0 4px rgba(52,230,214,0); }
}
@keyframes su-flicker { 0%,100%,48%,52% { opacity: 0.9; } 50% { opacity: 0.6; } }
@keyframes su-glow {
  0%,100% { box-shadow: 0 24px 60px rgba(0,0,0,0.5), inset 0 0 30px rgba(52,230,214,0.05); }
  50% { box-shadow: 0 24px 70px rgba(0,0,0,0.5), inset 0 0 42px rgba(52,230,214,0.10), 0 0 26px rgba(52,230,214,0.18); }
}

@media (prefers-reduced-motion: reduce) {
  .su-bg--aurora, .su-bg--grid { animation: none; }
  * { animation-duration: 0s !important; animation-iteration-count: 1 !important; }
}
```

- [ ] **Step 5: Create site.js with the idempotent bootstrap + background layers**

Create `website/theme/js/site.js`:

```js
// superui redesign — injected on every mdBook page (additional-js).
(function () {
  function injectBackground() {
    if (document.querySelector('.su-bg')) return; // idempotent
    for (const kind of ['aurora', 'grid', 'scan']) {
      const el = document.createElement('div');
      el.className = 'su-bg su-bg--' + kind;
      document.body.insertBefore(el, document.body.firstChild);
    }
  }

  function initSuperui() {
    injectBackground();
    // Task 3 adds: enhanceHeader();
    // Task 7 adds: initLandingCounter();
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', initSuperui);
  } else {
    initSuperui();
  }
  window.__superuiInit = initSuperui;
})();
```

- [ ] **Step 6: Build and verify**

Run: `mdbook build website`
Expected: builds with no error.

Run: `grep -c "su-bg--aurora" website/book/theme/js/site.js`
Expected: `1`

Run: `grep -c "Chakra+Petch" website/book/index.html`
Expected: `1` (head.hbs was injected).

Orchestrator: serve `website/book`, open any doc page, confirm the animated grid/aurora background appears and text uses the new fonts.

- [ ] **Step 7: Commit**

```bash
git add website/theme/head.hbs website/book.toml website/theme/js/site.js website/theme/css/site.css website/theme/css/landing.css
git commit -m "feat(website): fonts, theme tokens, animated background layers"
```

---

### Task 3: Site chrome — menu-bar header + footer

**Files:**
- Modify: `website/theme/js/site.js` (add `enhanceHeader()`)
- Modify: `website/theme/css/site.css` (append header + footer styles)

**Interfaces:**
- Consumes: `initSuperui()` from Task 2.
- Produces: reskinned `.menu-bar` carrying the SU badge + "SUPERUI / BEVY GAME UI" wordmark, a pulsing "v0.1 · EARLY BUILD" chip, and a GitHub link; a restyled bottom footer bar. These are chrome shared by all non-landing pages.

- [ ] **Step 1: Add enhanceHeader() to site.js**

In `website/theme/js/site.js`, replace the `// Task 3 adds: enhanceHeader();` line with a call `enhanceHeader();` and add this function inside the IIFE (above `initSuperui`):

```js
  function enhanceHeader() {
    const bar = document.getElementById('menu-bar');
    if (!bar || bar.querySelector('.su-brand')) return; // idempotent

    // mdBook sets a global `var path_to_root` (e.g. "../") on every page.
    const root = (typeof path_to_root === 'string') ? path_to_root : '';

    // Brand (badge + wordmark) linking to the site root, inserted after left-buttons.
    const brand = document.createElement('a');
    brand.className = 'su-brand';
    brand.href = root + 'index.html';
    brand.innerHTML =
      '<span class="su-badge">SU</span>' +
      '<span class="su-word"><b>SUPERUI</b><small>BEVY GAME UI</small></span>';
    const left = bar.querySelector('.left-buttons') || bar.firstElementChild;
    left.parentNode.insertBefore(brand, left.nextSibling);

    // Right cluster: version chip + GitHub text link (default git icon hidden via CSS).
    const right = bar.querySelector('.right-buttons') || bar;
    const chip = document.createElement('span');
    chip.className = 'su-chip';
    chip.innerHTML = '<span class="su-dot"></span>v0.1 · EARLY BUILD';
    right.insertBefore(chip, right.firstChild);

    const gh = document.createElement('a');
    gh.className = 'su-gh';
    gh.href = 'https://github.com/strowk/bevy_superui';
    gh.target = '_blank';
    gh.rel = 'noopener';
    gh.textContent = 'GITHUB ↗';
    right.appendChild(gh);
  }
```

- [ ] **Step 2: Append header + footer CSS to site.css**

Append to `website/theme/css/site.css`:

```css
/* ============ header (reskinned mdBook menu-bar) ============ */
#menu-bar, .menu-bar {
  background: rgba(8, 11, 18, 0.72);
  backdrop-filter: blur(8px);
  border-bottom: 1px solid rgba(52, 230, 214, 0.18);
}
.menu-bar .icon-button { color: var(--su-muted); }
.menu-bar .icon-button:hover { color: var(--su-teal-light); }
/* single dark theme: hide the theme picker */
#theme-toggle, .menu-bar #theme-toggle, #theme-list { display: none !important; }
/* our brand replaces the book title; our GITHUB link replaces the default git icon */
#menu-bar .menu-title { display: none; }
#git-repository-button { display: none !important; }

.su-brand { display: inline-flex; align-items: center; gap: 12px; text-decoration: none;
  margin: 0 8px 0 6px; }
.su-badge { width: 30px; height: 30px; display: grid; place-items: center;
  font-family: var(--font-mono); font-weight: 700; font-size: 13px; color: var(--su-teal-light);
  background: linear-gradient(135deg, rgba(52,230,214,0.22), rgba(52,230,214,0.04));
  border: 1px solid rgba(52,230,214,0.55);
  clip-path: polygon(22% 0, 100% 0, 100% 78%, 78% 100%, 0 100%, 0 22%);
  box-shadow: 0 0 18px rgba(52,230,214,0.35), inset 0 0 12px rgba(52,230,214,0.15); }
.su-word { line-height: 1; display: inline-flex; flex-direction: column; }
.su-word b { font-family: var(--font-display); font-weight: 700; font-size: 17px;
  letter-spacing: 0.16em; color: var(--su-heading); }
.su-word small { font-family: var(--font-mono); font-size: 8px; letter-spacing: 0.34em;
  color: var(--su-muted-dim); margin-top: 3px; }

.su-chip { display: inline-flex; align-items: center; gap: 8px; font-family: var(--font-mono);
  font-size: 11px; color: var(--su-muted); white-space: nowrap; margin-right: 4px; }
.su-dot { width: 8px; height: 8px; border-radius: 50%; background: var(--su-amber);
  box-shadow: 0 0 8px var(--su-amber); animation: su-pulse 2.4s ease-in-out infinite; }
.su-gh { font-family: var(--font-mono); font-size: 12px; letter-spacing: 0.06em;
  color: var(--su-text); border: 1px solid rgba(205,216,230,0.22); padding: 6px 12px;
  border-radius: 6px; text-decoration: none; white-space: nowrap; }
.su-gh:hover { border-color: rgba(52,230,214,0.5); color: var(--su-teal-light); }

/* ============ footer ============ */
.su-footer, #content .su-footer { border-top: 1px solid rgba(52,230,214,0.14);
  padding: 22px 26px; display: flex; align-items: center; gap: 14px;
  font-family: var(--font-mono); font-size: 11px; color: var(--su-muted-dim); }
.su-footer .su-f-brand { color: var(--su-teal); }
.su-footer .su-f-right { margin-left: auto; }
```

- [ ] **Step 3: Build and verify**

Run: `mdbook build website`
Expected: no error.

Run: `grep -c "su-brand" website/book/theme/js/site.js`
Expected: `1`

Orchestrator: serve, open a doc page — the menu-bar shows the SU badge + SUPERUI wordmark on the left and the amber-pulsing "v0.1 · EARLY BUILD" chip + "GITHUB ↗" on the right; theme toggle is gone; search + sidebar toggle still open/close. Compare against `redesign_docs.jpeg` header.

- [ ] **Step 4: Commit**

```bash
git add website/theme/js/site.js website/theme/css/site.css
git commit -m "feat(website): reskin menu-bar into redesign header + footer styles"
```

---

### Task 4: Docs pages — sidebar, content, code, callouts, prev/next

**Files:**
- Modify: `website/theme/css/site.css` (append docs styles)

**Interfaces:**
- Consumes: theme tokens from Task 2.
- Produces: doc-page styling only (CSS); no new JS or markup.

- [ ] **Step 1: Append docs-page CSS**

Append to `website/theme/css/site.css`:

```css
/* ============ sidebar → "MODULE INDEX" ============ */
#sidebar, .sidebar { background: #0b0f17; border-right: 1px solid rgba(52,230,214,0.12); }
.sidebar .sidebar-scrollbox { padding: 14px 12px; }
.chapter li.part-title {
  font-family: var(--font-mono); font-size: 11px; letter-spacing: 0.14em;
  text-transform: uppercase; color: var(--su-teal); margin: 18px 0 8px; padding: 0 6px;
}
.chapter li a { font-family: var(--font-body); font-size: 14px; color: var(--su-muted);
  border-radius: 6px; border-left: 2px solid transparent; padding: 7px 10px; }
.chapter li a:hover { color: var(--su-text); background: rgba(52,230,214,0.06); }
.chapter li a.active { color: var(--su-teal-light); background: rgba(52,230,214,0.10);
  border-left-color: var(--su-teal); }

/* ============ content ============ */
#content main { color: var(--su-text); }
#content h1, #content h2, #content h3, #content h4 {
  font-family: var(--font-display); color: var(--su-heading); font-weight: 700; }
#content h1 { font-size: clamp(34px, 5vw, 48px); letter-spacing: -0.01em; }
#content h2 { color: var(--su-heading); border: none; margin-top: 2em; }
#content a { color: var(--su-teal-light); }
#content p, #content li { color: var(--su-text); }

/* code blocks */
#content pre {
  background: rgba(9,13,20,0.85); border: 1px solid rgba(52,230,214,0.16);
  border-radius: 10px; }
#content pre > code { font-family: var(--font-mono); font-size: 13px; line-height: 1.6; }
#content code:not(pre code) {
  font-family: var(--font-mono); color: var(--su-teal-light);
  background: rgba(52,230,214,0.08); border-radius: 4px; padding: 1px 5px; }
/* mdBook's hover clipboard button = our "COPY" control */
.buttons .clip-button, pre .buttons { color: var(--su-muted); }
.buttons .clip-button:hover { color: var(--su-teal-light); }

/* blockquote → TIP callout */
#content blockquote {
  border: 1px solid rgba(52,230,214,0.2); border-left: 3px solid var(--su-teal);
  background: rgba(52,230,214,0.06); color: var(--su-text);
  border-radius: 8px; padding: 14px 18px; }

/* prev/next */
.nav-chapters { color: var(--su-muted); font-family: var(--font-mono); }
.nav-chapters:hover { color: var(--su-teal-light); }
.mobile-nav-chapters { color: var(--su-teal-light); }
```

- [ ] **Step 2: Build and verify**

Run: `mdbook build website`
Expected: no error.

Orchestrator: serve, open `docs/getting-started.html`. Compare to `redesign_docs.jpeg`: teal group eyebrows in the sidebar, active item with teal left-border; Chakra headings; dark teal-bordered code blocks with a working hover COPY button; blockquotes render as the teal-left-border callout; prev/next restyled. Confirm the sidebar toggle + search still work.

- [ ] **Step 3: Commit**

```bash
git add website/theme/css/site.css
git commit -m "feat(website): restyle docs sidebar, content, code, callouts, prev/next"
```

---

### Task 5: Examples gallery cards

**Files:**
- Modify: `website/theme/css/site.css` (append gallery styles)

**Interfaces:**
- Consumes: theme tokens (Task 2), the existing `.gallery-cat` / `.cards` / `.card` markup emitted by `mdbook-gallery` (see current `site.css:47-59`).
- Produces: gallery-card styling only.

- [ ] **Step 1: Replace the existing gallery-card block**

In `website/theme/css/site.css`, find the current `--- examples gallery cards ---` block (the `.gallery-cat`, `.cards`, `.card`, `.badge` rules) and replace it with:

```css
/* ============ examples gallery ============ */
.gallery-cat h2 {
  font-family: var(--font-mono); font-size: 12px; letter-spacing: 0.16em;
  text-transform: uppercase; color: var(--su-muted); border: none; margin: 34px 0 14px; }
.cards { display: grid; grid-template-columns: repeat(auto-fill, minmax(260px, 1fr)); gap: 16px; }
.card {
  position: relative; display: block; text-decoration: none; color: inherit;
  background: var(--su-panel); border: 1px solid rgba(52,230,214,0.16);
  border-radius: 10px; padding: 22px 20px; overflow: hidden;
  transition: border-color .15s, transform .15s, box-shadow .15s; }
.card:hover { border-color: rgba(52,230,214,0.5); transform: translateY(-2px);
  box-shadow: 0 14px 34px rgba(0,0,0,0.4); }
/* corner brackets */
.card::before, .card::after {
  content: ""; position: absolute; width: 14px; height: 14px; pointer-events: none; }
.card::before { top: -1px; left: -1px;
  border-top: 2px solid var(--su-teal); border-left: 2px solid var(--su-teal); }
.card::after { bottom: -1px; right: -1px;
  border-bottom: 2px solid var(--su-teal); border-right: 2px solid var(--su-teal); }
.card .card-title { margin: 0 0 8px; font-family: var(--font-display); font-weight: 600;
  font-size: 19px; color: var(--su-teal-light); }
.card .card-desc { margin: 0; color: var(--su-muted); font-size: 14px; line-height: 1.55; }
.badges { margin-top: 12px; display: flex; gap: 6px; flex-wrap: wrap; }
.badge { font-family: var(--font-mono); font-size: 11px; padding: 2px 8px; border-radius: 999px;
  background: rgba(52,230,214,0.10); border: 1px solid rgba(52,230,214,0.3); color: var(--su-teal-light); }
```

- [ ] **Step 2: Build and verify**

Run: `mdbook build website`
Expected: no error.

Orchestrator: serve, open `examples/` (the gallery index). Compare to `redesign_examples.jpeg`: category eyebrows, panel cards with teal corner brackets, teal titles, pill badges, hover lift. Confirm the new "Counter" card appears (from Task 1's gallery entry).

- [ ] **Step 3: Commit**

```bash
git add website/theme/css/site.css
git commit -m "feat(website): restyle examples gallery cards"
```

---

### Task 6: Landing page — markup + landing.css

**Files:**
- Modify: `website/src/index.md` (rewrite)
- Modify: `website/theme/css/landing.css` (fill in)

**Interfaces:**
- Consumes: theme tokens (Task 2); the `.superui-landing` chrome-hiding rules already in `site.css:12-28` (keep them; the marker class stays `superui-landing`).
- Produces: landing DOM including `#su-counter-frame` (the iframe), `#su-live-label` (the "// LIVE PREVIEW" eyebrow), and `#su-reset` (the Reset button) — all consumed by Task 7.

- [ ] **Step 1: Rewrite index.md**

Replace the entire contents of `website/src/index.md` with:

```html
<div class="superui-landing"></div>

<header class="su-lhead">
  <a class="su-brand" href="index.html">
    <span class="su-badge">SU</span>
    <span class="su-word"><b>SUPERUI</b><small>BEVY GAME UI</small></span>
  </a>
  <nav class="su-lnav">
    <a class="su-nav-active" href="index.html">HOME</a>
    <a href="docs/">DOCS</a>
    <a href="examples/">EXAMPLES</a>
  </nav>
  <div class="su-lhead-right">
    <span class="su-chip"><span class="su-dot"></span>v0.1 · EARLY BUILD</span>
    <a class="su-gh" href="https://github.com/strowk/bevy_superui" target="_blank" rel="noopener">GITHUB ↗</a>
  </div>
</header>

<main class="su-landing-main">
  <div class="su-hero">
    <div class="su-hero-text">
      <div class="su-eyebrow"><span class="su-eyebrow-dot"></span>SYSTEM ONLINE // UI RUNTIME</div>
      <h1 class="su-h1">Game UI<br>that speaks<br><span class="su-accent">your stack.</span></h1>
      <p class="su-tagline">Browser-like <strong>HTML / CSS / JS</strong> and Solid-style
        <strong>TSX</strong> for <strong>Bevy</strong> — fine-grained reactivity with live hot reload.</p>
      <div class="su-cta">
        <a class="su-btn su-btn-primary" href="docs/">READ THE DOCS →</a>
        <a class="su-btn su-btn-ghost" href="examples/">SEE EXAMPLES</a>
      </div>
    </div>

    <div class="su-card su-code-card">
      <div class="su-card-bar">
        <span class="su-tl su-tl-r"></span><span class="su-tl su-tl-a"></span><span class="su-tl su-tl-t"></span>
        <span class="su-card-name">counter.tsx</span>
        <span class="su-card-tag">CODE SAMPLE</span>
      </div>
      <pre class="su-code"><span class="k">function</span> <span class="fn">Counter</span>() {
  <span class="k">const</span> [count, setCount] = <span class="fn">createSignal</span>(<span class="n">0</span>);
  <span class="k">return</span> (
    &lt;<span class="tag">button</span> <span class="attr">onClick</span>={() =&gt; <span class="fn">setCount</span>(count() + <span class="n">1</span>)}&gt;
      clicked {count()} times
    &lt;/<span class="tag">button</span>&gt;
  );
}</pre>
      <div class="su-live">
        <div class="su-live-label" id="su-live-label">// LIVE PREVIEW · booting runtime…</div>
        <div class="su-live-stage">
          <iframe id="su-counter-frame" class="su-counter-frame" src="examples/counter/embed.html"
                  title="Live counter example" loading="lazy"></iframe>
          <div class="su-live-overlay" id="su-live-overlay"></div>
        </div>
        <button class="su-btn su-btn-reset" id="su-reset" type="button">reset</button>
      </div>
    </div>
  </div>

  <div class="su-features">
    <div class="su-feature"><div class="su-f-tag">01 // DOM</div><h3>Web stack</h3>
      <p>Author UI in plain HTML, CSS and JavaScript, running natively on bevy_ui.</p></div>
    <div class="su-feature"><div class="su-f-tag">02 // TSX</div><h3>Solid-style TSX</h3>
      <p>Fine-grained reactive components via the supersolid framework.</p></div>
    <div class="su-feature"><div class="su-f-tag">03 // HMR</div><h3>Hot reload</h3>
      <p>Edit .tsx and see changes live — with signal state preserved.</p></div>
    <div class="su-feature"><div class="su-f-tag">04 // API</div><h3>Familiar APIs</h3>
      <p>A browser-like DOM/CSS surface. Reuse the web knowledge you already have.</p></div>
  </div>

  <div class="su-banner">
    <span class="su-banner-chip">⚠ EARLY BUILD</span>
    <p>superui is in very early development and largely AI-generated; APIs are in flux.
       Explore the working <a href="examples/">examples</a>.</p>
  </div>
</main>

<footer class="su-footer su-lfoot">
  <span class="su-f-brand">SUPERUI</span><span>// build a12f · 2026</span>
  <span class="su-f-right">MIT / APACHE-2.0 · MADE FOR BEVY</span>
</footer>
```

- [ ] **Step 2: Fill in landing.css**

Replace the contents of `website/theme/css/landing.css` with:

```css
/* Landing-page-only styles (index.md). */
.su-landing-main { position: relative; z-index: 5; max-width: 1160px;
  margin: 0 auto; padding: clamp(32px, 6vw, 56px) clamp(18px, 5vw, 26px) 90px; }

/* landing header/footer (chrome hidden; we render our own) */
.su-lhead { position: relative; z-index: 6; display: flex; align-items: center;
  gap: 16px 20px; flex-wrap: wrap; padding: 14px 26px;
  border-bottom: 1px solid rgba(52,230,214,0.18); background: rgba(8,11,18,0.72);
  backdrop-filter: blur(8px); }
.su-lnav { display: flex; gap: 6px; margin-left: 14px; }
.su-lnav a { font-family: var(--font-mono); font-size: 12px; letter-spacing: 0.1em;
  color: var(--su-muted); padding: 8px 14px; border-radius: 6px; text-decoration: none; }
.su-lnav a:hover { color: var(--su-text); }
.su-lnav a.su-nav-active { color: #04120f;
  background: linear-gradient(135deg, var(--su-teal-bright), var(--su-teal-deep));
  box-shadow: 0 0 16px rgba(52,230,214,0.4); }
.su-lhead-right { margin-left: auto; display: flex; align-items: center; gap: 16px; }
.su-lfoot { max-width: 1240px; margin: 0 auto; }

/* hero */
.su-hero { display: grid; grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
  gap: clamp(28px, 5vw, 44px); align-items: center; }
.su-eyebrow { display: inline-flex; align-items: center; gap: 10px; font-family: var(--font-mono);
  font-size: 11px; letter-spacing: 0.22em; color: var(--su-teal);
  border: 1px solid var(--su-border-strong); border-radius: 999px; padding: 6px 14px;
  background: rgba(52,230,214,0.06); }
.su-eyebrow-dot { width: 6px; height: 6px; border-radius: 50%; background: var(--su-teal);
  box-shadow: 0 0 8px var(--su-teal); }
.su-h1 { font-family: var(--font-display); font-weight: 700;
  font-size: clamp(40px, 8.5vw, 66px); line-height: 1.02; letter-spacing: -0.01em;
  margin: 22px 0 0; color: var(--su-heading); text-shadow: 0 0 34px rgba(52,230,214,0.25); }
.su-h1 .su-accent { color: var(--su-teal); }
.su-tagline { font-size: 18px; line-height: 1.6; color: #93a2b3; max-width: 460px; margin: 22px 0 0; }
.su-tagline strong { color: var(--su-text); }
.su-cta { display: flex; gap: 12px; flex-wrap: wrap; margin-top: 30px; }
.su-btn { font-family: var(--font-display); font-weight: 600; letter-spacing: 0.06em;
  font-size: 14px; padding: 14px 24px; border-radius: 8px; text-decoration: none;
  cursor: pointer; border: 1px solid transparent; display: inline-block; }
.su-btn-primary { color: #04120f;
  background: linear-gradient(135deg, var(--su-teal-bright), var(--su-teal-deep));
  box-shadow: 0 0 26px rgba(52,230,214,0.4); }
.su-btn-ghost { color: var(--su-text); background: transparent; border-color: var(--su-border-strong); }

/* code card */
.su-card { position: relative; background: rgba(9,13,20,0.85);
  border: 1px solid rgba(52,230,214,0.25); border-radius: 12px; overflow: hidden; }
.su-code-card { animation: su-glow 6s ease-in-out infinite; }
.su-card-bar { display: flex; align-items: center; gap: 8px; padding: 11px 14px;
  border-bottom: 1px solid rgba(52,230,214,0.16); background: rgba(52,230,214,0.05); }
.su-tl { width: 10px; height: 10px; border-radius: 50%; }
.su-tl-r { background: #ff5f57; } .su-tl-a { background: var(--su-amber); } .su-tl-t { background: var(--su-teal); }
.su-card-name { margin-left: 8px; font-family: var(--font-mono); font-size: 12px; color: var(--su-muted); }
.su-card-tag { margin-left: auto; font-family: var(--font-mono); font-size: 10px;
  letter-spacing: 0.06em; color: var(--su-muted-dim); }
.su-code { margin: 0; padding: 18px 18px 6px; font-family: var(--font-mono); font-size: 13px;
  line-height: 1.7; color: #93a2b3; overflow: auto; }
.su-code .k { color: #c58fff; } .su-code .fn { color: var(--su-teal-light); }
.su-code .tag { color: #5fd7ff; } .su-code .attr { color: var(--su-teal); } .su-code .n { color: var(--su-amber); }

/* live preview */
.su-live { padding: 6px 18px 20px; border-top: 1px dashed rgba(52,230,214,0.14); margin-top: 8px; }
.su-live-label { font-family: var(--font-mono); font-size: 10px; letter-spacing: 0.24em;
  color: var(--su-muted-dim); margin: 12px 0; }
.su-live-stage { position: relative; height: 140px; border-radius: 8px; overflow: hidden;
  background: #0b1220; border: 1px solid rgba(52,230,214,0.16); }
.su-counter-frame { width: 100%; height: 100%; border: 0; display: block; background: #0b1220; }
.su-live-overlay { position: absolute; inset: 0; display: grid; place-items: center;
  background: #0b1220; color: var(--su-muted); font-family: var(--font-mono); font-size: 12px;
  transition: opacity .3s; }
.su-live-overlay::after { content: "booting runtime…"; }
.su-live-overlay.su-hidden { opacity: 0; pointer-events: none; }
.su-btn-reset { margin-top: 10px; font-family: var(--font-mono); font-size: 12px;
  color: var(--su-muted); background: transparent; border: 1px solid rgba(205,216,230,0.18);
  padding: 10px 14px; border-radius: 7px; }
.su-btn-reset:hover { border-color: rgba(52,230,214,0.4); color: var(--su-teal-light); }

/* features */
.su-features { display: grid; grid-template-columns: repeat(auto-fit, minmax(230px, 1fr));
  gap: 16px; margin-top: 64px; }
.su-feature { position: relative; background: var(--su-panel);
  border: 1px solid rgba(52,230,214,0.16); border-radius: 10px; padding: 22px 20px;
  transition: border-color .15s, transform .15s; }
.su-feature:hover { border-color: rgba(52,230,214,0.45); transform: translateY(-2px); }
.su-f-tag { font-family: var(--font-mono); font-size: 11px; letter-spacing: 0.14em; color: var(--su-teal); }
.su-feature h3 { font-family: var(--font-display); font-weight: 600; font-size: 19px;
  margin: 12px 0 8px; color: #eaf4f2; }
.su-feature p { margin: 0; font-size: 14px; line-height: 1.55; color: var(--su-muted); }

/* early-build banner */
.su-banner { display: flex; align-items: center; gap: 16px; margin-top: 34px; padding: 16px 20px;
  border: 1px solid rgba(255,180,84,0.4); border-radius: 10px;
  background: linear-gradient(90deg, rgba(255,180,84,0.10), rgba(255,180,84,0.02)); }
.su-banner-chip { font-family: var(--font-mono); font-size: 11px; letter-spacing: 0.16em;
  color: var(--su-amber); border: 1px solid rgba(255,180,84,0.5); padding: 6px 10px;
  border-radius: 5px; white-space: nowrap; animation: su-flicker 5s ease infinite; }
.su-banner p { margin: 0; font-size: 14px; line-height: 1.55; color: #d8c9a8; }
.su-banner a { color: var(--su-amber-light); }

@media (max-width: 640px) { .su-lnav { order: 3; width: 100%; } }
```

- [ ] **Step 3: Build and verify**

Run: `mdbook build website`
Expected: no error.

Run: `grep -c "su-counter-frame" website/book/index.html`
Expected: `1`

Orchestrator: serve, open the site root. Compare to `screenshot.jpeg`: header, hero with teal "your stack.", code card with colored syntax + traffic lights, the live-preview region showing the counter iframe (loading overlay visible), features grid, amber early-build banner, footer. mdBook sidebar/menu-bar must be hidden on this page.

- [ ] **Step 4: Commit**

```bash
git add website/src/index.md website/theme/css/landing.css
git commit -m "feat(website): redesign landing page markup + styles"
```

---

### Task 7: Live counter wiring — loading label + Reset

**Files:**
- Modify: `website/theme/js/site.js` (add `initLandingCounter()`)

**Interfaces:**
- Consumes: `#su-counter-frame`, `#su-live-label`, `#su-live-overlay`, `#su-reset` from Task 6; the `'superui:ready'` `postMessage` from `embed.html` (Task 1).
- Produces: none (terminal behavior).

- [ ] **Step 1: Add initLandingCounter() to site.js**

In `website/theme/js/site.js`, replace the `// Task 7 adds: initLandingCounter();` line with `initLandingCounter();` and add this function inside the IIFE:

```js
  function initLandingCounter() {
    const frame = document.getElementById('su-counter-frame');
    if (!frame) return; // not the landing page
    const label = document.getElementById('su-live-label');
    const overlay = document.getElementById('su-live-overlay');
    const reset = document.getElementById('su-reset');

    function arming() {
      if (label) label.textContent = '// LIVE PREVIEW · booting runtime…';
      if (overlay) overlay.classList.remove('su-hidden');
    }
    function ready() {
      if (label) label.textContent = '// LIVE PREVIEW';
      if (overlay) overlay.classList.add('su-hidden');
    }

    window.addEventListener('message', (e) => {
      if (e.source === frame.contentWindow && e.data === 'superui:ready') ready();
    });
    // Fallback: if the message is missed, reveal after the frame's load event + a grace delay.
    frame.addEventListener('load', () => setTimeout(ready, 4000));

    if (reset) reset.addEventListener('click', () => {
      arming();
      frame.contentWindow.location.reload();
    });

    arming();
  }
```

- [ ] **Step 2: Build and verify**

Run: `mdbook build website`
Expected: no error.

Run: `grep -c "superui:ready" website/book/theme/js/site.js`
Expected: `1`

- [ ] **Step 3: End-to-end check (orchestrator, Playwright)**

Serve the built book (must include the Task-1 counter build). On the landing page:
1. The live label reads `// LIVE PREVIEW · booting runtime…` and the overlay covers the frame.
2. Within a few seconds the counter renders, the label flips to `// LIVE PREVIEW`, and the overlay fades.
3. Clicking the teal counter button increments its count.
4. Clicking `reset` re-shows the booting label/overlay, the app reloads and returns to "clicked 0 times". In devtools Network, `counter_bg.wasm` is served from cache (no full re-download).

- [ ] **Step 4: Commit**

```bash
git add website/theme/js/site.js
git commit -m "feat(website): wire live counter iframe loading state + reset"
```

---

### Task 8: Full-site visual pass + reduced-motion + responsive

**Files:**
- Modify: any of `website/theme/css/site.css`, `website/theme/css/landing.css`, `website/theme/js/site.js` (fixes only)

**Interfaces:**
- Consumes: everything from Tasks 1–7.
- Produces: final polished site.

- [ ] **Step 1: Full build**

Run: `bash tools/build-demos.sh counter && mdbook build website`
Expected: both succeed; `website/book/examples/counter/embed.html` exists.

Run: `ls website/book/examples/counter/embed.html`
Expected: the path prints (no error).

- [ ] **Step 2: Visual regression (orchestrator, Playwright at 1280px)**

Screenshot and compare each screen to its reference; note and fix any material deviation in the relevant CSS file:
- Landing → `target/redesign_website_design/screenshot.jpeg`
- A docs page → `redesign_docs.jpeg`
- Examples gallery → `redesign_examples.jpeg`

- [ ] **Step 3: Reduced-motion check**

In the browser, emulate `prefers-reduced-motion: reduce` and reload each page.
Expected: no aurora/grid/pulse/flicker/glow animation; static gradient + grid still present; layout unchanged.

- [ ] **Step 4: Responsive check**

Resize to 480px width.
Expected: landing hero + code card stack vertically; header wraps; docs sidebar collapses via mdBook's existing responsive behavior; nothing overflows horizontally.

- [ ] **Step 5: Functional regression**

On a docs page confirm: search opens and returns results, sidebar toggle works, print (`window.print` preview) is usable. On the landing, confirm the CTAs link to `docs/` and `examples/`.

- [ ] **Step 6: Commit any fixes**

```bash
git add -A website/
git commit -m "polish(website): visual pass, reduced-motion + responsive fixes"
```

---

## Self-review notes

- **Spec coverage:** delivery mechanism (T2), global theme + bg (T2), menu-bar/header + footer (T3), docs pages (T4), gallery (T5), landing (T6), live counter + Reset (T7), counter gallery entry + build + embed + deploy (T1), verification (T8). All spec sections mapped.
- **Interfaces:** `site.js` grows one function per task (`injectBackground`→T2, `enhanceHeader`→T3, `initLandingCounter`→T7), all funnelled through the idempotent `initSuperui()`. Landing DOM ids (`su-counter-frame`, `su-live-label`, `su-live-overlay`, `su-reset`) are defined in T6 and consumed in T7. `embed.html` posts `superui:ready` (T1) consumed in T7.
- **Deploy:** CI `discover` job auto-includes `counter` from `gallery.json` (T1 step 1); the extra copy step (T1 step 6) supplies `embed.html`.
