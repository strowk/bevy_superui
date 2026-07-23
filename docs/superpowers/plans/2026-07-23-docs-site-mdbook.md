# superui Official Site (mdBook) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the superui official website — a chrome-light landing page, a documentation section, and the examples gallery — as a single mdBook project deployed to GitHub Pages.

**Architecture:** One mdBook project under `website/` produces the entire site: `/` landing (`src/index.md`), `/docs/…` chapters, `/examples/` gallery index (`src/examples/README.md`, populated by a custom mdBook preprocessor over `examples/gallery.json`). CI builds the book, then overlays the existing CI-built wasm demo folders under `dist/examples/<slug>/`. `mdbook serve website` gives one live-reload dev loop for landing + docs + gallery.

**Tech Stack:** mdBook 0.4.x (docs engine + dev server), a Rust preprocessor binary `mdbook-gallery` (serde/serde_json only), the existing `xtask` (retains `host-page` only), GitHub Pages deploy.

## Global Constraints

- **Zero Node.js / npm** — Rust-native toolchain only.
- **New crate deps limited to `serde` + `serde_json`** (match the lean `xtask` style; do NOT pull the full `mdbook` crate as a library — hand-roll the preprocessor JSON protocol).
- **Rust edition 2021**, workspace version `0.1.0`, license `MIT OR Apache-2.0`.
- **mdBook 0.4.x** (install via `cargo install mdbook` locally; `taiki-e/install-action` in CI).
- **GitHub Pages project base path is `/bevy_superui/`** — rely on relative links + mdBook's `site-url` so nothing hardcodes it in page bodies.
- **Palette:** gallery dark family — `--bg:#1e1e28`, `--panel/card:#262633`, `--fg:#e6e6ef`, `--muted:#9a9ab0`, `--accent:#7c6cff`, border `#3a3a52`, badge bg `#3a3358` / fg `#d8d2ff`.
- **Gallery marker token:** `<!-- superui:gallery -->` (an HTML comment, so it is inert if the preprocessor does not run and never collides with mdBook's `{{#…}}` link preprocessor).
- **Do not disturb the in-progress gallery-deploy repair.** If assemble/path details there change, rebase Task 7 onto them.

---

## File Structure

**Create:**
- `website/book.toml` — mdBook config (theme, site-url, preprocessor).
- `website/src/SUMMARY.md` — table of contents.
- `website/src/index.md` — landing page (chrome-light hero, raw HTML).
- `website/src/docs/README.md` — Introduction (→ `/docs/`).
- `website/src/docs/getting-started.md` — Getting Started.
- `website/src/docs/reference/{css,html,js-dom,keyed}.md` — moved from existing docs.
- `website/src/examples/README.md` — gallery index page holding the marker.
- `website/theme/css/site.css` — palette override + landing chrome-hide + card styles.
- `tools/mdbook-gallery/Cargo.toml` — preprocessor crate.
- `tools/mdbook-gallery/src/main.rs` — CLI entry (`supports` check + stdin/stdout).
- `tools/mdbook-gallery/src/gallery.rs` — `Example`, manifest load, fragment renderer.
- `tools/mdbook-gallery/src/preprocess.rs` — marker expansion over the book JSON.

**Modify:**
- `Cargo.toml` — add `tools/mdbook-gallery` to workspace members.
- `xtask/src/main.rs` — remove `gallery-index` subcommand + `mod gallery`.
- `tools/gallery/host.html.tmpl` — add site back-nav links.
- `xtask/src/host.rs` — extend test for nav links.
- `.github/workflows/deploy-pages.yml` — mdBook build + overlay demos under `examples/`.
- `README.md` — demo links → `/examples/<slug>/`; add a "Documentation site" section.

**Delete:**
- `xtask/src/gallery.rs` (logic moves to the preprocessor as a fragment renderer).
- `tools/gallery/gallery.html.tmpl` (standalone gallery shell retired).

**Move (git mv):**
- `docs/support/css.md` → `website/src/docs/reference/css.md`
- `docs/support/html.md` → `website/src/docs/reference/html.md`
- `docs/support/js-dom.md` → `website/src/docs/reference/js-dom.md`
- `docs/superui/keyed.md` → `website/src/docs/reference/keyed.md`

---

## Task 1: Scaffold the mdBook site

**Files:**
- Create: `website/book.toml`, `website/src/SUMMARY.md`, `website/src/index.md`, `website/src/docs/README.md`, `website/src/docs/getting-started.md`, `website/src/examples/README.md`, `website/theme/css/site.css` (empty placeholder)

**Interfaces:**
- Produces: the `website/` mdBook project that later tasks fill in. SUMMARY paths (`index.md`, `docs/README.md`, `docs/getting-started.md`, `docs/reference/*.md`, `examples/README.md`) are the contract Task 2 (reference pages) and Task 4 (preprocessor) rely on.

Note: the `[preprocessor.gallery]` config is intentionally NOT added yet (its crate does not exist until Task 4). The marker in `examples/README.md` renders as an inert HTML comment until then.

- [ ] **Step 1: Create `website/book.toml`**

```toml
[book]
title = "superui"
authors = ["Timur Sultanaev"]
description = "Browser-like HTML/CSS/JS + Solid-style TSX game UI for Bevy."
src = "src"
language = "en"

[output.html]
default-theme = "navy"
preferred-dark-theme = "navy"
site-url = "/bevy_superui/"
git-repository-url = "https://github.com/strowk/bevy_superui"
additional-css = ["theme/css/site.css"]
no-section-label = true
```

- [ ] **Step 2: Create `website/src/SUMMARY.md`**

```markdown
# Summary

[Home](index.md)

# Guide

- [Introduction](docs/README.md)
- [Getting Started](docs/getting-started.md)

# Reference

- [CSS](docs/reference/css.md)
- [HTML](docs/reference/html.md)
- [JS / DOM](docs/reference/js-dom.md)
- [Keyed lists & performance](docs/reference/keyed.md)

# Examples

- [Gallery](examples/README.md)
```

- [ ] **Step 3: Create placeholder reference pages so Step 6 builds**

The four `docs/reference/*.md` files are populated by Task 2 (git mv). For this task to build, create them as one-line stubs now; Task 2 overwrites them via move.

Create `website/src/docs/reference/css.md`, `html.md`, `js-dom.md`, `keyed.md`, each containing only:

```markdown
# (placeholder — populated in Task 2)
```

- [ ] **Step 4: Create landing/intro/getting-started/examples pages (minimal, real content — hero & CSS come in Task 3)**

`website/src/index.md`:

```markdown
<div class="superui-landing"></div>

# superui

Browser-like HTML/CSS/JS and Solid-style TSX game UI for Bevy, with hot reload.

(Landing hero is styled in Task 3.)
```

`website/src/docs/README.md`:

```markdown
# Introduction

superui is a Bevy plugin that provides a browser-like environment for running
HTML/CSS/JS applications — and Solid-style `.tsx` components via the supersolid
framework — as game UI. It is built on top of `bevy_ui` (inheriting some of its
limitations for now) and a modified `bevy_flair` for CSS support.

The goal is the best possible developer experience for writing game UI in Bevy:
rapid iteration (hot reload) and compatibility with existing web-development
knowledge.

## Status

This is in very early development. Some working examples already run — see the
[gallery](../examples/). The code is largely AI-generated and not yet fully
reviewed; APIs are expected to be in flux, though the surface deliberately
mirrors familiar web APIs. Use at your own risk.
```

`website/src/docs/getting-started.md`:

```markdown
# Getting Started

> superui is not yet published to crates.io. Add it as a path or git dependency.

## Add the dependency

```toml
[dependencies]
superui = { git = "https://github.com/strowk/bevy_superui" }
superui_css = { git = "https://github.com/strowk/bevy_superui" }
bevy = "0.17"
```

## Mount a UI

Author your UI under `assets/ui/hello/` as `index.html`, `style.css`, and
`app.js`, then mount it on a `SuperUiRoot`:

```rust
use bevy::prelude::*;
use superui::prelude::{SuperUiPlugin, SuperUiRoot};
use superui::JsSource;
use superui_css::style::StyleSheet;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(SuperUiPlugin)
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    commands.spawn(Camera2d);

    // The SuperUiRoot node must fill the window so percentage/inset children
    // resolve against the full viewport.
    commands.spawn((
        Node { width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
        SuperUiRoot {
            html: assets.load("ui/hello/index.html"),
            css: assets.load::<StyleSheet>("ui/hello/style.css"),
            js: assets.load::<JsSource>("ui/hello/app.js"),
        },
    ));
}
```

For Solid-style `.tsx` authoring, hot reload, and web (wasm) builds, see the
[examples](../examples/) — each shows its full authored source.
```

`website/src/examples/README.md`:

```markdown
# Examples

Each example is compiled to WebAssembly and published here, showing the running
app beside its authored source. Heavier stress-test examples may run slowly
in-browser.

<!-- superui:gallery -->
```

- [ ] **Step 5: Create empty theme CSS placeholder**

Create `website/theme/css/site.css` with a single comment line (filled in Task 3):

```css
/* superui site theme — populated in Task 3 */
```

- [ ] **Step 6: Build the book to verify it is well-formed**

Run: `mdbook build website`
Expected: exits 0; creates `website/book/index.html`, `website/book/docs/index.html`, `website/book/docs/getting-started.html`, `website/book/examples/index.html`. (If `mdbook` is missing: `cargo install mdbook` first.)

- [ ] **Step 7: Ignore the build output**

Add `website/book/` to `.gitignore` (append the line if the file exists; create it otherwise).

- [ ] **Step 8: Commit**

```bash
git add website .gitignore
git commit -m "feat(site): scaffold mdBook website project"
```

---

## Task 2: Move existing reference docs into the book

**Files:**
- Move: `docs/support/css.md` → `website/src/docs/reference/css.md`; `docs/support/html.md` → `website/src/docs/reference/html.md`; `docs/support/js-dom.md` → `website/src/docs/reference/js-dom.md`; `docs/superui/keyed.md` → `website/src/docs/reference/keyed.md`
- Modify (if needed): the moved files' internal relative links

**Interfaces:**
- Consumes: SUMMARY.md reference entries from Task 1.
- Produces: populated Reference section.

- [ ] **Step 1: Move the four files (overwriting Task 1 placeholders)**

```bash
git rm website/src/docs/reference/css.md website/src/docs/reference/html.md website/src/docs/reference/js-dom.md website/src/docs/reference/keyed.md
git mv docs/support/css.md   website/src/docs/reference/css.md
git mv docs/support/html.md  website/src/docs/reference/html.md
git mv docs/support/js-dom.md website/src/docs/reference/js-dom.md
git mv docs/superui/keyed.md website/src/docs/reference/keyed.md
```

- [ ] **Step 2: Fix any cross-file links inside the moved docs**

Search the moved files for relative markdown links to other docs or repo paths:

Run: `grep -rnE "\]\((\.\./|\./|docs/)" website/src/docs/reference/`
For each hit, rewrite it to resolve within the book (sibling reference pages are `./name.md`; the examples gallery is `../../examples/`). If there are no hits, do nothing.

- [ ] **Step 3: Ensure each moved page has a top-level `# Heading`**

Run: `head -n 3 website/src/docs/reference/*.md`
Expected: each file starts with a single `# Title`. If any lacks one, add an appropriate `# Title` as the first line (mdBook uses it for the sidebar entry title alongside SUMMARY).

- [ ] **Step 4: Build to verify links resolve**

Run: `mdbook build website 2>&1 | tee /tmp/mdbook.log; grep -i "warn\|error" /tmp/mdbook.log || echo "clean"`
Expected: `clean` (no broken-link warnings), exit 0.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "docs(site): move CSS/HTML/JS-DOM/keyed reference into the book"
```

---

## Task 3: Landing hero + site theme CSS

**Files:**
- Modify: `website/src/index.md` (full hero)
- Modify: `website/theme/css/site.css` (palette override + landing chrome-hide)

**Interfaces:**
- Consumes: the `.superui-landing` marker div from Task 1's `index.md`.
- Produces: `body:has(.superui-landing)` chrome-hidden landing; palette variables used site-wide. Card styles (`.cards/.card/.badge`) are added in Task 4, not here.

This is a visual task: the exact mdBook default-theme selector/variable names can vary slightly by version, so the last step is a manual `mdbook serve` check with a tweak allowance.

- [ ] **Step 1: Write the full landing `website/src/index.md`**

mdBook does not process markdown inside a raw-HTML block, so the code snippet is a normal fenced block separated by blank lines from the HTML sections.

```markdown
<div class="superui-landing"></div>

<section class="hero">
  <h1>superui</h1>
  <p class="tagline">Browser-like HTML/CSS/JS &amp; Solid-style TSX game UI for
  Bevy — with hot reload.</p>
  <div class="cta">
    <a class="btn btn-primary" href="docs/">Read the docs</a>
    <a class="btn" href="examples/">See examples</a>
    <a class="btn" href="https://github.com/strowk/bevy_superui">GitHub</a>
  </div>
</section>

Write reactive UI the way you already know:

```jsx
function Counter() {
  const [count, setCount] = createSignal(0);
  return (
    <button onClick={() => setCount(count() + 1)}>
      clicked {count()} times
    </button>
  );
}
```

<section class="features">
  <div class="feature"><h3>Web stack</h3><p>Author UI in plain HTML, CSS and
  JavaScript, running on <code>bevy_ui</code>.</p></div>
  <div class="feature"><h3>Solid-style TSX</h3><p>Fine-grained reactive
  components via the supersolid framework.</p></div>
  <div class="feature"><h3>Hot reload</h3><p>Edit <code>.tsx</code> and see
  changes live, with state preserved.</p></div>
  <div class="feature"><h3>Familiar APIs</h3><p>A browser-like DOM/CSS surface —
  reuse your web knowledge.</p></div>
</section>

<section class="status-note">
  <strong>Early stage:</strong> superui is in very early development and largely
  AI-generated; APIs are in flux. Explore the working
  <a href="examples/">examples</a>.
</section>
```

- [ ] **Step 2: Write `website/theme/css/site.css`**

```css
/* --- superui palette (overrides mdBook 'navy' variables) --- */
:root, .navy {
  --bg: #1e1e28;
  --fg: #e6e6ef;
  --sidebar-bg: #262633;
  --sidebar-fg: #e6e6ef;
  --sidebar-active: #7c6cff;
  --links: #a99bff;
  --sidebar-non-existant: #6a6a80;
}

/* --- chrome-light landing: hide sidebar + top bar on the landing only --- */
body:has(.superui-landing) #sidebar,
body:has(.superui-landing) .sidebar,
body:has(.superui-landing) #menu-bar,
body:has(.superui-landing) .menu-bar,
body:has(.superui-landing) .sidebar-resize-handle {
  display: none !important;
}
body:has(.superui-landing) .page-wrapper {
  margin-left: 0 !important;
  left: 0 !important;
}
body:has(.superui-landing) #content main {
  max-width: none;
  margin: 0;
  padding: 0;
}

/* --- landing hero --- */
.superui-landing { display: none; }
.hero { text-align: center; padding: 72px 24px 32px; }
.hero h1 { font-size: 56px; margin: 0 0 12px; }
.hero .tagline { color: var(--muted, #9a9ab0); font-size: 20px; max-width: 640px; margin: 0 auto 24px; }
.cta { display: flex; gap: 12px; justify-content: center; flex-wrap: wrap; }
.btn { display: inline-block; text-decoration: none; padding: 10px 18px; border-radius: 8px;
       border: 1px solid #3a3a52; color: var(--fg); font-size: 15px; }
.btn-primary { background: #7c6cff; border-color: #7c6cff; color: #fff; }
.features { display: grid; grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
            gap: 16px; max-width: 900px; margin: 40px auto; padding: 0 24px; }
.feature { background: #262633; border: 1px solid #3a3a52; border-radius: 10px; padding: 18px; }
.feature h3 { margin: 0 0 6px; }
.feature p { margin: 0; color: #9a9ab0; }
.status-note { max-width: 720px; margin: 0 auto 64px; padding: 14px 18px; text-align: center;
               background: #2f2a4a; border: 1px solid #3a3a52; border-radius: 8px; color: #d8d2ff; }
```

- [ ] **Step 3: Serve and visually verify the landing**

Run: `mdbook serve website` and open `http://localhost:3000/`.
Expected: the landing shows the hero, feature cards, and status note **with no sidebar and no top menu bar**; other pages (`/docs/`, `/examples/`) still have the sidebar. Confirm the dark palette matches the gallery tone.

If the sidebar/menu still shows on the landing, inspect the rendered page in the browser devtools, find the actual sidebar/menu-bar element ids/classes for the installed mdBook version, and adjust the selectors in Step 2 accordingly. Re-verify.

- [ ] **Step 4: Commit**

```bash
git add website/src/index.md website/theme/css/site.css
git commit -m "feat(site): chrome-light landing hero + palette theme"
```

---

## Task 4: Gallery preprocessor (`mdbook-gallery`)

**Files:**
- Create: `tools/mdbook-gallery/Cargo.toml`, `tools/mdbook-gallery/src/main.rs`, `tools/mdbook-gallery/src/gallery.rs`, `tools/mdbook-gallery/src/preprocess.rs`
- Modify: `Cargo.toml` (workspace members), `website/book.toml` (register preprocessor), `website/theme/css/site.css` (append card styles)

**Interfaces:**
- Consumes: `examples/gallery.json` (`{ "examples": [ { slug, title, description, category, tags? }, … ] }`); the `<!-- superui:gallery -->` marker in `examples/README.md`.
- Produces:
  - `gallery::render(&[Example]) -> String` — the card-grid **fragment** (no `<html>`/`<style>` shell).
  - `gallery::load(&Path) -> Result<Vec<Example>, _>` and `gallery::manifest_path(&serde_json::Value) -> PathBuf`.
  - `preprocess::expand(&mut serde_json::Value, &str)` — replaces the marker in every chapter's `content`.
  - Binary `mdbook-gallery`: `mdbook-gallery supports <renderer>` exits 0; otherwise reads `[context, book]` JSON on stdin and writes the mutated `book` JSON on stdout.

- [ ] **Step 1: Create `tools/mdbook-gallery/Cargo.toml`**

```toml
[package]
name = "mdbook-gallery"
edition = "2021"
version = "0.1.0"
license.workspace = true
publish = false

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

- [ ] **Step 2: Add the crate to the workspace**

Modify `Cargo.toml` members from:

```toml
members = ["crates/*", "examples/*", "xtask"]
```

to:

```toml
members = ["crates/*", "examples/*", "xtask", "tools/mdbook-gallery"]
```

- [ ] **Step 3: Write the failing tests for the fragment renderer + expansion**

Create `tools/mdbook-gallery/src/gallery.rs` with the struct and a test module first (implementation stubs return empty so the test fails meaningfully). Start with just enough to compile:

```rust
use serde::Deserialize;
use serde_json::Value;
use std::path::{Path, PathBuf};

pub const MARKER: &str = "<!-- superui:gallery -->";

#[derive(Debug, Clone, Deserialize)]
pub struct Example {
    pub slug: String,
    pub title: String,
    pub description: String,
    pub category: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Deserialize)]
struct Manifest {
    examples: Vec<Example>,
}

pub fn manifest_path(ctx: &Value) -> PathBuf {
    let root = ctx.get("root").and_then(Value::as_str).unwrap_or(".");
    let rel = ctx
        .get("config")
        .and_then(|c| c.get("preprocessor"))
        .and_then(|p| p.get("gallery"))
        .and_then(|g| g.get("manifest"))
        .and_then(Value::as_str)
        .unwrap_or("../examples/gallery.json");
    Path::new(root).join(rel)
}

pub fn load(path: &Path) -> Result<Vec<Example>, Box<dyn std::error::Error>> {
    let text = std::fs::read_to_string(path)?;
    let m: Manifest = serde_json::from_str(&text)?;
    Ok(m.examples)
}

pub fn render(_examples: &[Example]) -> String {
    String::new() // stub — implemented in Step 5
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ex(slug: &str, category: &str, tags: &[&str]) -> Example {
        Example {
            slug: slug.into(),
            title: format!("{slug} title"),
            description: "d".into(),
            category: category.into(),
            tags: tags.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn renders_categories_cards_badges_as_fragment() {
        let examples = vec![
            ex("todomvc", "Apps", &[]),
            ex("todomvc_supersolid", "Apps", &[]),
            ex("horde", "Stress tests", &["Playable game"]),
        ];
        let out = render(&examples);
        // Category headers, first-seen order.
        assert!(out.contains("<h2>Apps</h2>"));
        assert!(out.contains("<h2>Stress tests</h2>"));
        assert!(out.find("Apps").unwrap() < out.find("Stress tests").unwrap());
        // Card links are relative to the /examples/ page.
        assert!(out.contains(r#"href="todomvc/""#));
        assert!(out.contains(r#"href="todomvc_supersolid/""#));
        // Badge chip.
        assert!(out.contains(r#"<span class="badge">Playable game</span>"#));
        // A fragment — no document shell.
        assert!(!out.contains("<html"));
        assert!(!out.contains("<style"));
    }

    #[test]
    fn manifest_path_defaults_relative_to_root() {
        let ctx = serde_json::json!({ "root": "/repo/website" });
        assert_eq!(
            manifest_path(&ctx),
            PathBuf::from("/repo/website/../examples/gallery.json")
        );
    }
}
```

- [ ] **Step 4: Run the tests to verify the renderer test fails**

Run: `cargo test -p mdbook-gallery`
Expected: `renders_categories_cards_badges_as_fragment` FAILS (stub returns empty); `manifest_path_defaults_relative_to_root` PASSES.

- [ ] **Step 5: Implement `render` (replace the stub)**

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
        out.push_str(&format!(
            "<section class=\"gallery-cat\"><h2>{cat}</h2><div class=\"cards\">"
        ));
        for e in examples.iter().filter(|e| e.category == cat) {
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
            out.push_str(&format!(
                "<a class=\"card\" href=\"{slug}/\"><h3>{title}</h3><p>{desc}</p>{badges_html}</a>",
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

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p mdbook-gallery`
Expected: both tests PASS.

- [ ] **Step 7: Write `tools/mdbook-gallery/src/preprocess.rs` with a failing test**

```rust
use crate::gallery::MARKER;
use serde_json::Value;

/// Replace the gallery marker with `fragment` in every chapter's `content`.
pub fn expand(book: &mut Value, fragment: &str) {
    walk(book, fragment);
}

fn walk(v: &mut Value, fragment: &str) {
    match v {
        Value::Object(map) => {
            if let Some(Value::String(content)) = map.get_mut("content") {
                if content.contains(MARKER) {
                    *content = content.replace(MARKER, fragment);
                }
            }
            for (_k, child) in map.iter_mut() {
                walk(child, fragment);
            }
        }
        Value::Array(arr) => {
            for child in arr.iter_mut() {
                walk(child, fragment);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_marker_in_nested_chapters() {
        let mut book = serde_json::json!({
            "items": [
                { "Chapter": {
                    "content": format!("intro {} tail", MARKER),
                    "sub_items": [
                        { "Chapter": { "content": format!("child {}", MARKER), "sub_items": [] } }
                    ]
                }}
            ]
        });
        expand(&mut book, "<GRID>");
        let s = serde_json::to_string(&book).unwrap();
        assert!(!s.contains(MARKER), "marker should be gone");
        assert_eq!(s.matches("<GRID>").count(), 2, "both chapters expanded");
    }
}
```

- [ ] **Step 8: Run the preprocess test**

Run: `cargo test -p mdbook-gallery`
Expected: all tests PASS (the `expand` implementation is already written above).

- [ ] **Step 9: Write `tools/mdbook-gallery/src/main.rs`**

```rust
use std::io::{self, Read};

mod gallery;
mod preprocess;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    // mdBook calls `<cmd> supports <renderer>` first; we support all renderers.
    if args.get(1).map(String::as_str) == Some("supports") {
        std::process::exit(0);
    }
    if let Err(e) = run() {
        eprintln!("mdbook-gallery error: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;

    // stdin is the JSON array `[context, book]`.
    let mut parsed: serde_json::Value = serde_json::from_str(&input)?;
    let arr = parsed.as_array_mut().ok_or("expected [context, book]")?;
    let ctx = arr.first().cloned().unwrap_or_default();

    let path = gallery::manifest_path(&ctx);
    let examples = gallery::load(&path)?;
    let fragment = gallery::render(&examples);

    let book = arr.get_mut(1).ok_or("missing book element")?;
    preprocess::expand(book, &fragment);

    serde_json::to_writer(io::stdout(), book)?;
    Ok(())
}
```

- [ ] **Step 10: Verify the whole crate builds and tests pass**

Run: `cargo test -p mdbook-gallery && cargo build -p mdbook-gallery`
Expected: tests PASS, build succeeds.

- [ ] **Step 11: Register the preprocessor in `website/book.toml`**

Append to `website/book.toml`:

```toml
[preprocessor.gallery]
command = "cargo run -q -p mdbook-gallery --"
manifest = "../examples/gallery.json"
```

(`cargo run -q` keeps stdout clean — cargo's own output goes to stderr — so the JSON contract holds, and there is no separate install step for local `mdbook serve`.)

- [ ] **Step 12: Append card styles to `website/theme/css/site.css`**

```css
/* --- examples gallery cards --- */
.gallery-cat h2 { margin: 28px 0 12px; font-size: 15px; text-transform: uppercase;
                  letter-spacing: .08em; color: #9a9ab0; border-bottom: 1px solid #3a3a52;
                  padding-bottom: 6px; }
.cards { display: grid; grid-template-columns: repeat(auto-fill, minmax(260px, 1fr)); gap: 16px; }
.card { display: block; text-decoration: none; color: inherit; background: #262633;
        border: 1px solid #3a3a52; border-radius: 10px; padding: 18px; transition: border-color .15s; }
.card:hover { border-color: #7c6cff; }
.card h3 { margin: 0 0 6px; font-size: 18px; }
.card p { margin: 0; color: #9a9ab0; font-size: 14px; }
.badges { margin-top: 10px; display: flex; gap: 6px; flex-wrap: wrap; }
.badge { font-size: 11px; padding: 2px 8px; border-radius: 999px; background: #3a3358; color: #d8d2ff; }
```

- [ ] **Step 13: End-to-end build — the marker expands into cards**

Run:
```bash
mdbook build website && grep -q 'class="card"' website/book/examples/index.html && echo "GALLERY OK"
```
Expected: prints `GALLERY OK` (the preprocessor ran and injected cards), and `grep -c "superui:gallery" website/book/examples/index.html` returns 0.

- [ ] **Step 14: Commit**

```bash
git add tools/mdbook-gallery Cargo.toml Cargo.lock website/book.toml website/theme/css/site.css
git commit -m "feat(site): mdbook-gallery preprocessor renders gallery from manifest"
```

---

## Task 5: Retire the old xtask gallery generator

**Files:**
- Delete: `xtask/src/gallery.rs`, `tools/gallery/gallery.html.tmpl`
- Modify: `xtask/src/main.rs` (drop `gallery-index` subcommand + `mod gallery`)

**Interfaces:**
- Consumes: nothing new.
- Produces: `xtask` now exposes only `host-page`. The gallery-rendering responsibility lives entirely in `mdbook-gallery` (Task 4).

- [ ] **Step 1: Delete the retired files**

```bash
git rm xtask/src/gallery.rs tools/gallery/gallery.html.tmpl
```

- [ ] **Step 2: Remove gallery wiring from `xtask/src/main.rs`**

Delete the `mod gallery;` line (top of file). Delete the `Some("gallery-index") => gallery_index(&args[2..]),` match arm. Delete the entire `fn gallery_index(...) { … }`. Update the usage string in the fallthrough arm to:

```rust
        other => Err(format!(
            "usage: xtask host-page --slug S --out DIR (got {other:?})"
        )
        .into()),
```

- [ ] **Step 3: Verify xtask builds and its remaining tests pass**

Run: `cargo test -p xtask`
Expected: compiles with no reference to `gallery`; `host.rs` and `sources.rs` tests PASS.

- [ ] **Step 4: Confirm nothing else references the removed items**

Run: `grep -rn "gallery-index\|gallery.html.tmpl\|mod gallery\|xtask/src/gallery" . --include=*.rs --include=*.yml --include=*.toml`
Expected: no matches in `xtask/`, `.github/`, or `Cargo` files. (Matches inside `docs/` specs/plans are fine.) If the CI workflow still references `gallery-index`, leave it — Task 7 rewrites the workflow.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "refactor(xtask): drop gallery-index; gallery now lives in mdbook-gallery"
```

---

## Task 6: Site back-nav on demo host pages

**Files:**
- Modify: `tools/gallery/host.html.tmpl` (add a nav row + minimal CSS)
- Modify: `xtask/src/host.rs` (extend the render test)

**Interfaces:**
- Consumes: existing `host::render` template substitution.
- Produces: every generated demo page links back to Home / Docs / Examples via relative paths that work under `/examples/<slug>/`.

- [ ] **Step 1: Add the failing assertion to `xtask/src/host.rs`**

In the `renders_canvas_wasm_and_tsx_source` test, add after the existing asserts:

```rust
        // Site back-nav (relative to /examples/<slug>/).
        assert!(out.contains(r#"href="../""#), "links back to the examples gallery");
        assert!(out.contains(r#"href="../../docs/""#), "links to docs");
        assert!(out.contains("Examples"));
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p xtask host::`
Expected: FAIL (template has no such nav yet).

- [ ] **Step 3: Add the nav to `tools/gallery/host.html.tmpl`**

Immediately after the opening `<body>` tag (before `<div id="banner">`), insert:

```html
  <nav id="site-nav">
    <a href="../../">Home</a>
    <a href="../../docs/">Docs</a>
    <a href="../">Examples</a>
    <a href="https://github.com/strowk/bevy_superui">GitHub</a>
  </nav>
```

Add to the `<style>` block (near the `#banner` rules):

```css
  #site-nav { display:flex; gap:16px; padding:8px 12px; background:#262633;
              border-bottom:1px solid #3a3a52; font-size:13px; }
  #site-nav a { color:#a99bff; text-decoration:none; }
  #site-nav a:hover { text-decoration:underline; }
```

Note: the layout height math uses `calc(100% - 37px)` for `#layout`. Adding the nav bar adds ~33px; update `#layout { height:calc(100% - 70px); }` so the demo canvas still fills without overflow. Verify visually is deferred to CI, but keep the arithmetic consistent.

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p xtask host::`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add tools/gallery/host.html.tmpl xtask/src/host.rs
git commit -m "feat(site): add Home/Docs/Examples back-nav to demo host pages"
```

---

## Task 7: CI deploy + README

**Files:**
- Modify: `.github/workflows/deploy-pages.yml` (assemble-and-deploy job)
- Modify: `README.md` (demo links → `/examples/`; add a "Documentation site" section)

**Interfaces:**
- Consumes: per-example build artifacts `example-<slug>` (unchanged from the `build` job), the `website/` mdBook project, and the `mdbook-gallery` preprocessor.
- Produces: `dist/` = mdBook output (landing + docs + gallery index) with demo folders overlaid under `dist/examples/<slug>/`.

- [ ] **Step 1: Rewrite the `assemble-and-deploy` job in `.github/workflows/deploy-pages.yml`**

Replace the whole `assemble-and-deploy:` job (from `assemble-and-deploy:` through the final `uses: actions/deploy-pages@v4`) with:

```yaml
  assemble-and-deploy:
    needs: [discover, build]
    runs-on: ubuntu-latest
    environment:
      name: github-pages
      url: ${{ steps.deploy.outputs.page_url }}
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        run: rustup toolchain install stable --profile minimal

      - name: Install mdBook
        uses: taiki-e/install-action@v2
        with:
          tool: mdbook

      - name: Build the site (landing + docs + gallery index)
        run: mdbook build website --dest-dir "$GITHUB_WORKSPACE/dist"

      - name: Download example artifacts
        uses: actions/download-artifact@v4
        with:
          path: downloaded
          pattern: example-*

      - name: Overlay demos under dist/examples/
        run: |
          mkdir -p dist/examples
          cp -r tools/gallery/vendor dist/examples/vendor
          for d in downloaded/example-*; do
            slug="${d#downloaded/example-}"
            rm -rf "dist/examples/$slug"
            mv "$d" "dist/examples/$slug"
          done

      - uses: actions/upload-pages-artifact@v3
        with:
          path: dist

      - id: deploy
        uses: actions/deploy-pages@v4
```

Key changes from the old job: `mdbook build` replaces the `xtask gallery-index` step; vendor + demos now go under `dist/examples/` instead of `dist/` root; the build runs **before** the overlay (mdBook cleans its dest dir).

- [ ] **Step 2: Validate the workflow YAML parses**

Run: `python -c "import yaml,sys; yaml.safe_load(open('.github/workflows/deploy-pages.yml')); print('yaml ok')"`
Expected: `yaml ok`. (If `python` is unavailable, use any YAML linter; the file must parse.)

- [ ] **Step 3: Update README demo links to `/examples/<slug>/`**

In `README.md`, change each live-demo link under the "Live examples" tables from `https://strowk.github.io/bevy_superui/<slug>/` to `https://strowk.github.io/bevy_superui/examples/<slug>/`:

Run to find them: `grep -n "github.io/bevy_superui/" README.md`
Rewrite each URL to insert `examples/` before the slug (todomvc, todomvc_supersolid, game_menu, citadel, horde).

- [ ] **Step 4: Add a "Documentation site" section to `README.md`**

Insert after the "Live examples" intro (before the tables, or as a new top-level section — place it where it reads naturally):

```markdown
## Documentation site

The full site (landing, docs, and the examples gallery) is a single mdBook
project under `website/`, deployed to GitHub Pages by the `Deploy Pages`
workflow.

Run it locally:

```bash
cargo install mdbook        # once
mdbook serve website        # live-reload at http://localhost:3000
```

The gallery index is generated from `examples/gallery.json` by the
`mdbook-gallery` preprocessor (built automatically via `cargo run` during the
mdBook build). Per-example wasm demos are built only in CI; the
`/examples/<slug>/` links 404 under local `mdbook serve`, which is expected.
```

- [ ] **Step 5: Verify the site still builds end-to-end after doc edits**

Run: `mdbook build website && grep -q 'class="card"' website/book/examples/index.html && echo OK`
Expected: `OK`.

- [ ] **Step 6: Commit**

```bash
git add .github/workflows/deploy-pages.yml README.md
git commit -m "ci(site): build mdBook site + overlay demos under /examples/; update README"
```

---

## Task 8: Push, verify deployment, verify the live site

**Files:** none (deploy + verification only).

**Interfaces:**
- Consumes: everything from Tasks 1–7 on `main`; the `Deploy Pages` workflow (`.github/workflows/deploy-pages.yml`), which triggers on push to `main`.
- Produces: a confirmed-live site at `https://strowk.github.io/bevy_superui/`.

This task is procedural (no TDD). It runs only after Tasks 1–7 are committed and the working tree is clean. **Prerequisite:** the Playwright MCP server must be connected (its `mcp__playwright__browser_*` tools available) for the visual verification steps.

> Heads-up on the in-progress gallery-deploy repair: before pushing, confirm the local `main` is rebased on the latest remote `main` so this deploy does not race or clobber that work. If a `Deploy Pages` run is already in flight, let it finish first.

- [ ] **Step 1: Confirm a clean tree and sync with remote main**

Run:
```bash
git status --porcelain && git fetch origin && git log --oneline origin/main..HEAD
```
Expected: no uncommitted changes (empty `--porcelain` output); the log shows exactly the Task 1–7 commits ahead of `origin/main`. If `origin/main` has new commits, `git rebase origin/main` and re-run Task 7's `mdbook build` verification before continuing.

- [ ] **Step 2: Push to main**

Run: `git push origin main`
Expected: push succeeds; `git rev-parse HEAD` matches `git rev-parse origin/main`.

- [ ] **Step 3: Watch the Deploy Pages run to completion via `gh`**

Run:
```bash
sleep 10
RUN_ID=$(gh run list --workflow "Deploy Pages" --branch main --limit 1 --json databaseId --jq '.[0].databaseId')
echo "watching run $RUN_ID"
gh run watch "$RUN_ID" --exit-status --interval 20
```
Expected: `gh run watch` streams job progress and exits 0 when the run **succeeds**. If it exits non-zero, the deploy failed — do NOT proceed to visual verification; instead:
```bash
gh run view "$RUN_ID" --log-failed
```
Read the failing step, fix the underlying cause (likely in `website/` or `deploy-pages.yml`), commit, and restart from Step 2. This is a debugging loop, not a step to skip.

- [ ] **Step 4: Confirm the deployed page URL and that it serves 200**

Run:
```bash
gh run view "$RUN_ID" --json url --jq .url
curl -sI https://strowk.github.io/bevy_superui/ | head -n 1
curl -sI https://strowk.github.io/bevy_superui/examples/ | head -n 1
```
Expected: both `curl` calls return `HTTP/2 200`. (GitHub Pages CDN can lag the workflow by up to a minute; if you see 404, wait 30s and retry the `curl` before assuming failure.)

- [ ] **Step 5: Verify the live landing page with Playwright MCP**

Use `mcp__playwright__browser_navigate` to open `https://strowk.github.io/bevy_superui/`, then `mcp__playwright__browser_snapshot` to capture the accessibility tree, then `mcp__playwright__browser_take_screenshot` (full page) for a visual record.

Assert from the snapshot:
- The hero heading `superui` and the tagline text are present.
- The CTA links **Read the docs**, **See examples**, **GitHub** are present.
- The four feature cards (Web stack / Solid-style TSX / Hot reload / Familiar APIs) are present.
- The mdBook **sidebar and top menu bar are NOT visible** (chrome-light landing).

If the sidebar/menu-bar IS visible on the live page, the `body:has(.superui-landing)` CSS did not take effect — treat as a failure, revisit Task 3's selectors against the deployed HTML, commit, and restart from Step 2.

- [ ] **Step 6: Verify the live docs page with Playwright MCP**

Navigate to `https://strowk.github.io/bevy_superui/docs/` (`mcp__playwright__browser_navigate`) and `mcp__playwright__browser_snapshot`.

Assert:
- The **Introduction** heading/content renders.
- The mdBook **sidebar IS visible** here (shows Guide / Reference / Examples entries) — confirming chrome is hidden only on the landing.
- Clicking a Reference sidebar entry (e.g. CSS) via `mcp__playwright__browser_click` navigates to that page and it renders.

- [ ] **Step 7: Verify the live gallery with Playwright MCP**

Navigate to `https://strowk.github.io/bevy_superui/examples/` and `mcp__playwright__browser_snapshot` + `mcp__playwright__browser_take_screenshot`.

Assert:
- Category sections (**Apps**, **Stress tests**) render.
- Example cards for `todomvc`, `todomvc_supersolid`, `game_menu`, `citadel`, `horde` are present, with the `Playable game` badge on Horde.
- No literal `<!-- superui:gallery -->` text is visible (the preprocessor ran).
- A card's link points under `/bevy_superui/examples/<slug>/` (inspect an anchor via `mcp__playwright__browser_evaluate` returning `document.querySelector('a.card').getAttribute('href')`).

- [ ] **Step 8: Spot-check one demo host page loads (shell only)**

Navigate to `https://strowk.github.io/bevy_superui/examples/todomvc/`. The wasm binary is large; use `mcp__playwright__browser_navigate` then `mcp__playwright__browser_snapshot`.

Assert (shell, not full app run):
- The back-nav row (Home / Docs / Examples / GitHub) is present.
- The `superui-canvas` element and the loading banner are present.
- The **Code** view lists the authored source files (tabs).

(Do not block on the wasm app fully initializing — it can be slow on the CDN and is not the subject of this verification.)

- [ ] **Step 9: Close the browser and report**

Call `mcp__playwright__browser_close`. Summarize the verification: deployed run id + URL, and a PASS/observations line for landing, docs, gallery, and the demo shell, attaching the screenshots taken.

---

## Self-Review

**Spec coverage:**
- Single mdBook driver, book output = dist/ → Tasks 1, 7. ✓
- URL layout (`/`, `/docs/`, `/examples/`, `/examples/<slug>/`) → SUMMARY (Task 1) + CI overlay (Task 7). ✓
- Chrome-light landing via marker + CSS `:has()` → Task 3 (chosen over an `index.hbs` override — same effect, less version-sensitivity; noted as a deliberate refinement of the spec's "theme override" intent). ✓
- Docs: Introduction + Getting Started (new) + Reference (moved) → Tasks 1, 2. ✓
- Gallery via preprocessor over gallery.json, emitting a fragment → Task 4. ✓
- Demo pages unchanged mechanism, overlaid after build → Tasks 6, 7. ✓
- Theming/palette consistency + `site-url` base path → Tasks 1, 3. ✓
- xtask reduced to host-page; gallery.rs + gallery.html.tmpl retired → Task 5. ✓
- Preprocessor unit tests (fragment render, marker expand) + xtask tests green → Tasks 4, 5, 6. ✓
- CI: install mdbook, build, overlay, README links → Task 7. ✓
- Deploy to main + verify deployment (`gh`) + verify live site (Playwright MCP) → Task 8. ✓

**Deviations from spec (deliberate):**
- Landing chrome-hide uses a `.superui-landing` marker + `body:has()` CSS instead of a `theme/index.hbs` override. Same outcome, avoids editing a version-sensitive Handlebars template. Marker token is `<!-- superui:gallery -->` for the gallery (HTML comment, collision-proof) rather than `{{#gallery}}`.
- Preprocessor uses `command = "cargo run -q -p mdbook-gallery --"` (no install step for local dev) rather than a PATH-installed binary.

**Placeholder scan:** No `TBD`/`TODO`/"handle edge cases"/"similar to Task N" — every code and content step is concrete. (The Task 1 reference stubs are explicitly transient and overwritten by Task 2's `git mv`.)

**Type/name consistency:** `gallery::render`, `gallery::load`, `gallery::manifest_path`, `preprocess::expand`, `MARKER` used consistently across Tasks 4–5. Marker string `<!-- superui:gallery -->` identical in `examples/README.md` (Task 1) and the preprocessor constant (Task 4). Demo back-nav relative paths (`../`, `../../docs/`) consistent between Task 6 template and its test.
