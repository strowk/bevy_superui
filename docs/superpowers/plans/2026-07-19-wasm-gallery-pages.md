# WASM Example Gallery on GitHub Pages — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a GitHub Actions pipeline that compiles superui example apps to WebAssembly and publishes them as a public GitHub Pages gallery, each demo showing the running app beside its source code, starting with TodoMVC.

**Architecture:** A manifest (`examples/gallery.json`) drives a matrix build. Each example compiles to wasm (`cargo` → `wasm-bindgen` → `wasm-opt`), then a Rust `xtask` crate generates its host page (live app + tabbed code viewer) and the gallery landing page. A three-job workflow (discover → build → assemble-and-deploy) publishes `dist/` to Pages. The code viewer `fetch()`es the same asset files the wasm app loads, so displayed code can never drift from running code.

**Tech Stack:** Rust (Bevy 0.17.3, wasm32-unknown-unknown), wasm-bindgen 0.2.126, binaryen (`wasm-opt`), GitHub Actions (`actions/upload-pages-artifact`, `actions/deploy-pages`), vendored highlight.js.

## Global Constraints

- Bevy version: `0.17` (locked `0.17.3`). Never bump as part of this work.
- wasm-bindgen crate: `0.2.126`. The `wasm-bindgen-cli` version **must match exactly**; CI derives it at runtime because `Cargo.lock` is gitignored.
- Render backend: **WebGL2** (`webgl2` bevy feature on wasm only).
- All published URLs are **relative** (Pages serves under `/<repo>/`). Host pages set `<base href="./">`.
- A published example's **slug is a permanent URL contract** — renaming breaks external links.
- Rust edition `2021`, workspace `resolver = "2"`.
- **No CDN / no runtime external dependencies** — the syntax highlighter is vendored and committed.
- Hot reload is native-only by design; never attempt to enable it on wasm.

---

## File Structure

- `examples/gallery.json` — manifest (the one edit point to add an example).
- `xtask/` — Rust helper crate: `Cargo.toml`, `src/main.rs` (CLI), `src/manifest.rs`, `src/sources.rs`, `src/host.rs`, `src/gallery.rs`.
- `tools/gallery/host.html.tmpl` — host page template (app + code viewer).
- `tools/gallery/gallery.html.tmpl` — gallery landing template.
- `tools/gallery/vendor/highlight.min.js`, `tools/gallery/vendor/highlight.css` — vendored highlighter.
- `.github/workflows/deploy-pages.yml` — the pipeline.
- `examples/todomvc/Cargo.toml`, `examples/todomvc/src/main.rs` — target-split + wasm canvas config.
- `Cargo.toml` (root) — add `xtask` to members.
- `.gitignore` — ignore `/dist`.
- `README.md` — hand-written examples table + Pages setup note.

---

## Task 1: Make the todomvc example wasm-buildable

Split bevy features by target (native keeps `file_watcher`; wasm gets `webgl2`) and add a wasm-only canvas config so the app binds to the host page's `<canvas>`.

**Files:**
- Modify: `examples/todomvc/Cargo.toml`
- Modify: `examples/todomvc/src/main.rs`

**Interfaces:**
- Produces: a `wasm32-unknown-unknown` binary at `target/wasm32-unknown-unknown/release/todomvc.wasm` whose Bevy window binds to CSS selector `#superui-canvas`. Later tasks (host template, workflow) depend on that selector name and that wasm path/name.

- [ ] **Step 1: Split the bevy dependency by target in `examples/todomvc/Cargo.toml`**

Replace the current single bevy dependency line:
```toml
# Full Bevy (windowing + rendering) — this is the runnable app, not a headless lib.
# `file_watcher` is required for the AssetPlugin hot-reload seam to do anything
# (design §6); `watch_for_changes_override` is inert without it.
bevy = { version = "0.17", features = ["file_watcher"] }
```
with a base dependency plus target-specific feature additions (Cargo **unions** features across target tables):
```toml
# Full Bevy (windowing + rendering) — this is the runnable app, not a headless lib.
bevy = { version = "0.17" }
```
Then add these two blocks. Put the native one next to the existing `[dependencies]`, and add the wasm feature to the EXISTING `[target.'cfg(target_arch = "wasm32")'.dependencies]` block (which already carries `getrandom`):
```toml
# Native only: file watching drives hot reload (design §6). The `notify` crate
# behind `file_watcher` does not build on wasm, which is why hot reload is
# native-only.
[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
bevy = { version = "0.17", features = ["file_watcher"] }
```
And extend the existing wasm target block so it reads:
```toml
# Boa (pulled transitively via superui) needs the JS getrandom backend on wasm.
# `webgl2` selects the broadly-compatible WebGL2 wgpu backend for the web build.
[target.'cfg(target_arch = "wasm32")'.dependencies]
getrandom = { version = "0.3", features = ["wasm_js"] }
bevy = { version = "0.17", features = ["webgl2"] }
```

- [ ] **Step 2: Add the wasm-only canvas config in `examples/todomvc/src/main.rs`**

Replace the plugin-adding block:
```rust
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(AssetPlugin {
        // Enable native hot reload (design §6). Inert on wasm.
        watch_for_changes_override: Some(true),
        ..default()
    }))
    .add_plugins(SuperUiPlugin);
```
with:
```rust
    let mut app = App::new();
    let default_plugins = DefaultPlugins.set(AssetPlugin {
        // Enable native hot reload (design §6). Inert on wasm.
        watch_for_changes_override: Some(true),
        ..default()
    });
    // On the web the app renders into the host page's <canvas id="superui-canvas">
    // and resizes to its container. Inert/native uses the default OS window.
    #[cfg(target_arch = "wasm32")]
    let default_plugins = default_plugins.set(WindowPlugin {
        primary_window: Some(Window {
            canvas: Some("#superui-canvas".into()),
            fit_canvas_to_parent: true,
            ..default()
        }),
        ..default()
    });
    app.add_plugins(default_plugins).add_plugins(SuperUiPlugin);
```

- [ ] **Step 3: Verify the native build/tests still pass**

Run: `cargo test -p todomvc`
Expected: PASS (the target-split and the `#[cfg(wasm32)]` block are inert on native; existing tests behave exactly as before).

- [ ] **Step 4: Add the wasm target and verify the wasm build compiles**

Run:
```bash
rustup target add wasm32-unknown-unknown
cargo build -p todomvc --release --target wasm32-unknown-unknown
```
Expected: PASS, producing `target/wasm32-unknown-unknown/release/todomvc.wasm`. (First build is slow — Bevy is large.) If it fails on `file_watcher`/`notify`, the target-split in Step 1 is wrong.

- [ ] **Step 5: Commit**

```bash
git add examples/todomvc/Cargo.toml examples/todomvc/src/main.rs
git commit -m "feat(todomvc): target-split bevy features + wasm canvas config"
```

---

## Task 2: xtask crate — manifest model + source enumeration

Scaffold the `xtask` crate and implement the two pure data pieces: loading the manifest and enumerating an example's source files in display order.

**Files:**
- Modify: `Cargo.toml` (root) — add `xtask` to `members`.
- Create: `xtask/Cargo.toml`
- Create: `xtask/src/main.rs` (module wiring + stub CLI; filled in Tasks 3–4)
- Create: `xtask/src/manifest.rs`
- Create: `xtask/src/sources.rs`

**Interfaces:**
- Produces:
  - `manifest::Example { slug: String, package: String, title: String, description: String }` (derives `serde::Deserialize`, `Clone`).
  - `manifest::load(path: &Path) -> Result<Vec<Example>, Box<dyn Error>>` — reads `{ "examples": [...] }`.
  - `sources::SourceFile { name: String, path: String, lang: String }` (derives `serde::Serialize`; sort key `order` is `#[serde(skip)]`).
  - `sources::enumerate(example_base: &Path, slug: &str) -> io::Result<Vec<SourceFile>>` — lists files under `<example_base>/<slug>/assets/ui/<slug>/`, ordered HTML→CSS→JS→other(alpha), with `path = "assets/ui/<slug>/<name>"`.

- [ ] **Step 1: Add `xtask` to the workspace members in root `Cargo.toml`**

Change:
```toml
members = ["crates/*", "examples/*"]
```
to:
```toml
members = ["crates/*", "examples/*", "xtask"]
```

- [ ] **Step 2: Create `xtask/Cargo.toml`**

```toml
[package]
name = "xtask"
edition = "2021"
version = "0.1.0"
license.workspace = true
publish = false

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

- [ ] **Step 3: Create `xtask/src/manifest.rs`**

```rust
use serde::Deserialize;
use std::error::Error;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct Example {
    pub slug: String,
    pub package: String,
    pub title: String,
    pub description: String,
}

#[derive(Debug, Deserialize)]
struct Manifest {
    examples: Vec<Example>,
}

/// Load `{ "examples": [ { slug, package, title, description }, ... ] }`.
pub fn load(path: &Path) -> Result<Vec<Example>, Box<dyn Error>> {
    let text = std::fs::read_to_string(path)?;
    let manifest: Manifest = serde_json::from_str(&text)?;
    Ok(manifest.examples)
}
```

- [ ] **Step 4: Create `xtask/src/sources.rs` with the failing test first**

```rust
use serde::Serialize;
use std::io;
use std::path::Path;

#[derive(Debug, Serialize, PartialEq)]
pub struct SourceFile {
    pub name: String,
    pub path: String,
    pub lang: String,
    #[serde(skip)]
    pub order: u8,
}

/// List the app's authored source files under `<base>/<slug>/assets/ui/<slug>/`,
/// ordered HTML → CSS → JS → everything-else(alphabetical). `path` is the
/// fetch path relative to the host page (which sets `<base href="./">`).
pub fn enumerate(example_base: &Path, slug: &str) -> io::Result<Vec<SourceFile>> {
    let dir = example_base
        .join(slug)
        .join("assets")
        .join("ui")
        .join(slug);
    let mut files = Vec::new();
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let ext = Path::new(&name)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        let (lang, order) = match ext.as_str() {
            "html" | "htm" => ("xml", 0u8), // highlight.js uses "xml" for HTML
            "css" => ("css", 1),
            "js" | "mjs" => ("javascript", 2),
            "json" => ("json", 3),
            _ => ("plaintext", 4),
        };
        files.push(SourceFile {
            name: name.clone(),
            path: format!("assets/ui/{slug}/{name}"),
            lang: lang.to_string(),
            order,
        });
    }
    files.sort_by(|a, b| a.order.cmp(&b.order).then_with(|| a.name.cmp(&b.name)));
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enumerates_in_display_order_with_fetch_paths() {
        // Build a temp fixture: assets/ui/demo/{app.js,index.html,style.css,notes.txt}
        let base = std::env::temp_dir().join("xtask_sources_test");
        let ui = base.join("demo").join("assets").join("ui").join("demo");
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&ui).unwrap();
        for f in ["app.js", "index.html", "style.css", "notes.txt"] {
            std::fs::write(ui.join(f), b"x").unwrap();
        }

        let out = enumerate(&base, "demo").unwrap();
        let names: Vec<_> = out.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["index.html", "style.css", "app.js", "notes.txt"]);
        assert_eq!(out[0].lang, "xml");
        assert_eq!(out[0].path, "assets/ui/demo/index.html");
        assert_eq!(out[2].lang, "javascript");

        std::fs::remove_dir_all(&base).unwrap();
    }
}
```

- [ ] **Step 5: Create `xtask/src/main.rs` wiring the modules with a stub CLI**

```rust
mod gallery;
mod host;
mod manifest;
mod sources;

fn main() {
    eprintln!("xtask: subcommands wired in later tasks (host-page, gallery-index)");
    std::process::exit(2);
}
```
Also create empty-but-compiling `xtask/src/host.rs` and `xtask/src/gallery.rs` so `main.rs` compiles now:
```rust
// xtask/src/host.rs — filled in Task 3
```
```rust
// xtask/src/gallery.rs — filled in Task 4
```

- [ ] **Step 6: Run the test to verify it passes**

Run: `cargo test -p xtask`
Expected: PASS (`enumerates_in_display_order_with_fetch_paths`). If `main.rs` warns about unused modules, that is fine.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml xtask/
git commit -m "feat(xtask): crate scaffold + manifest model + source enumeration"
```

---

## Task 3: xtask `host-page` subcommand + host template + vendored highlighter

Generate a per-example host page (live app pane + tabbed code viewer) from a template, and vendor the highlighter it references.

**Files:**
- Create: `tools/gallery/host.html.tmpl`
- Create: `tools/gallery/vendor/highlight.min.js` (downloaded, committed)
- Create: `tools/gallery/vendor/highlight.css` (downloaded, committed)
- Modify: `xtask/src/host.rs`
- Modify: `xtask/src/main.rs` (implement `host-page` subcommand + flag parsing)

**Interfaces:**
- Consumes: `manifest::Example`, `sources::SourceFile`, `sources::enumerate`.
- Produces:
  - `host::render(ex: &manifest::Example, sources: &[sources::SourceFile]) -> String`.
  - CLI `xtask host-page --slug <slug> --out <dir>` → writes `<dir>/index.html`. Reads manifest at `examples/gallery.json`, enumerates from base `examples`.

- [ ] **Step 1: Vendor the highlighter (download + commit exact files)**

Run (pins highlight.js 11.9.0; any 11.x is fine but pin one):
```bash
mkdir -p tools/gallery/vendor
curl -Lo tools/gallery/vendor/highlight.min.js https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.9.0/highlight.min.js
curl -Lo tools/gallery/vendor/highlight.css   https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.9.0/styles/github-dark.min.css
```
Expected: two non-empty files. These are committed so the site has no runtime CDN dependency. (The `highlight.min.js` core bundle includes xml/css/javascript/json grammars.)

- [ ] **Step 2: Create `tools/gallery/host.html.tmpl`**

```html
<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<base href="./">
<title>{{TITLE}} — superui demo</title>
<link rel="stylesheet" href="../vendor/highlight.css">
<style>
  :root { --bg:#1e1e28; --panel:#262633; --fg:#e6e6ef; --muted:#9a9ab0; }
  * { box-sizing:border-box; }
  html,body { margin:0; height:100%; background:var(--bg); color:var(--fg);
              font-family:system-ui, sans-serif; }
  #banner { padding:8px 12px; background:#2f2a4a; font-size:13px; border-bottom:1px solid #3a3a52; }
  #banner code { background:#00000040; padding:1px 5px; border-radius:4px; }
  #view-tabs { display:none; background:var(--panel); }
  #layout { display:flex; height:calc(100% - 37px); }
  #app-pane { flex:1 1 55%; position:relative; min-width:0; border-right:1px solid #3a3a52; }
  #superui-canvas { width:100%; height:100%; display:block; }
  #loader { position:absolute; inset:0; display:flex; align-items:center;
            justify-content:center; padding:0 24px; text-align:center; color:var(--muted); font-size:14px; }
  #code-pane { flex:1 1 45%; display:flex; flex-direction:column; min-width:0; }
  #tabs { display:flex; gap:2px; background:var(--panel); padding:6px 6px 0; flex-wrap:wrap; }
  .tab { padding:6px 12px; cursor:pointer; color:var(--muted); border:none; background:transparent;
         font-size:13px; border-radius:6px 6px 0 0; }
  .tab.active { color:var(--fg); background:var(--bg); }
  #code-scroll { flex:1; overflow:auto; margin:0; }
  #code-scroll code { display:block; padding:12px 16px; font-size:13px; line-height:1.5;
                      font-family:ui-monospace, "Cascadia Code", Consolas, monospace; }
  @media (max-width:800px) {
    #layout { flex-direction:column; }
    #app-pane, #code-pane { flex:1 1 auto; border:none; }
    #view-tabs { display:flex; }
    .hidden { display:none !important; }
  }
</style>
</head>
<body>
  <div id="banner">
    ▶ Static WebAssembly build of <strong>{{TITLE}}</strong>. Live hot-reload of HTML/CSS/JS is
    <strong>native-only</strong> — <code>git clone … &amp;&amp; cargo run -p {{SLUG}}</code> to try it.
  </div>
  <div id="view-tabs">
    <button class="tab active" data-view="app">Demo</button>
    <button class="tab" data-view="code">Code</button>
  </div>
  <div id="layout">
    <div id="app-pane">
      <canvas id="superui-canvas"></canvas>
      <div id="loader">Loading {{TITLE}} — this is a large WebAssembly binary, please wait…</div>
    </div>
    <div id="code-pane">
      <div id="tabs"></div>
      <pre id="code-scroll"><code id="code-el"></code></pre>
    </div>
  </div>

  <script>window.__SOURCES__ = {{SOURCES_JSON}};</script>
  <script src="../vendor/highlight.min.js"></script>
  <script>
    const sources = window.__SOURCES__ || [];
    const tabsEl = document.getElementById('tabs');
    const codeEl = document.getElementById('code-el');
    const cache = {};
    async function show(i) {
      const src = sources[i];
      [...tabsEl.children].forEach((t, j) => t.classList.toggle('active', i === j));
      if (cache[i] == null) {
        try { cache[i] = await (await fetch(src.path)).text(); }
        catch (e) { cache[i] = '// failed to load ' + src.path; }
      }
      codeEl.textContent = cache[i];
      codeEl.className = 'language-' + src.lang;
      if (window.hljs) hljs.highlightElement(codeEl);
    }
    sources.forEach((src, i) => {
      const b = document.createElement('button');
      b.className = 'tab' + (i === 0 ? ' active' : '');
      b.textContent = src.name;
      b.onclick = () => show(i);
      tabsEl.appendChild(b);
    });
    if (sources.length) show(0);

    const appPane = document.getElementById('app-pane');
    const codePane = document.getElementById('code-pane');
    document.querySelectorAll('#view-tabs .tab').forEach(t => {
      t.onclick = () => {
        document.querySelectorAll('#view-tabs .tab').forEach(x => x.classList.remove('active'));
        t.classList.add('active');
        const app = t.dataset.view === 'app';
        appPane.classList.toggle('hidden', !app);
        codePane.classList.toggle('hidden', app);
      };
    });
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

- [ ] **Step 3: Implement `xtask/src/host.rs` with a failing test first**

```rust
use crate::manifest::Example;
use crate::sources::SourceFile;

const TEMPLATE: &str = include_str!("../../tools/gallery/host.html.tmpl");

/// Render the host page for one example: substitutes title/description/slug,
/// the wasm glue filename, and the embedded source-file list the code viewer reads.
pub fn render(ex: &Example, sources: &[SourceFile]) -> String {
    let sources_json = serde_json::to_string(sources).expect("sources serialize");
    TEMPLATE
        .replace("{{TITLE}}", &ex.title)
        .replace("{{DESCRIPTION}}", &ex.description)
        .replace("{{SLUG}}", &ex.slug)
        .replace("{{WASM_JS}}", &format!("{}.js", ex.slug))
        .replace("{{SOURCES_JSON}}", &sources_json)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_canvas_wasm_and_sources() {
        let ex = Example {
            slug: "todomvc".into(),
            package: "todomvc".into(),
            title: "TodoMVC".into(),
            description: "classic".into(),
        };
        let sources = vec![SourceFile {
            name: "index.html".into(),
            path: "assets/ui/todomvc/index.html".into(),
            lang: "xml".into(),
            order: 0,
        }];
        let out = render(&ex, &sources);
        assert!(out.contains(r#"id="superui-canvas""#));
        assert!(out.contains("import init from './todomvc.js'"));
        assert!(out.contains("assets/ui/todomvc/index.html"));
        assert!(out.contains("cargo run -p todomvc"));
        assert!(!out.contains("{{"), "no unsubstituted template tokens");
    }
}
```

- [ ] **Step 4: Implement the `host-page` subcommand + flag parser in `xtask/src/main.rs`**

Replace the stub `main` with:
```rust
mod gallery;
mod host;
mod manifest;
mod sources;

use std::path::Path;

const MANIFEST: &str = "examples/gallery.json";
const EXAMPLE_BASE: &str = "examples";

fn main() {
    if let Err(e) = run() {
        eprintln!("xtask error: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("host-page") => host_page(&args[2..]),
        Some("gallery-index") => gallery_index(&args[2..]),
        other => Err(format!(
            "usage: xtask <host-page --slug S --out DIR | gallery-index --out FILE> (got {other:?})"
        )
        .into()),
    }
}

/// Minimal `--flag value` parser.
fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned())
}

fn host_page(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let slug = flag(args, "--slug").ok_or("host-page requires --slug")?;
    let out_dir = flag(args, "--out").ok_or("host-page requires --out")?;
    let examples = manifest::load(Path::new(MANIFEST))?;
    let ex = examples
        .iter()
        .find(|e| e.slug == slug)
        .ok_or_else(|| format!("slug '{slug}' not found in {MANIFEST}"))?;
    let srcs = sources::enumerate(Path::new(EXAMPLE_BASE), &slug)?;
    let html = host::render(ex, &srcs);
    std::fs::create_dir_all(&out_dir)?;
    std::fs::write(Path::new(&out_dir).join("index.html"), html)?;
    println!("wrote {out_dir}/index.html ({} source files)", srcs.len());
    Ok(())
}

fn gallery_index(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let out = flag(args, "--out").ok_or("gallery-index requires --out")?;
    let examples = manifest::load(Path::new(MANIFEST))?;
    let html = gallery::render(&examples);
    if let Some(parent) = Path::new(&out).parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&out, html)?;
    println!("wrote {out} ({} examples)", examples.len());
    Ok(())
}
```
(Note: `gallery::render` is stubbed until Task 4 — add a temporary `pub fn render(_: &[crate::manifest::Example]) -> String { String::new() }` to `gallery.rs` now so this compiles; Task 4 replaces it.)

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p xtask`
Expected: PASS (`renders_canvas_wasm_and_sources` + the Task 2 test).

- [ ] **Step 6: Commit**

```bash
git add tools/gallery/host.html.tmpl tools/gallery/vendor/ xtask/src/host.rs xtask/src/main.rs xtask/src/gallery.rs
git commit -m "feat(xtask): host-page subcommand, host template, vendored highlighter"
```

---

## Task 4: xtask `gallery-index` subcommand + gallery template + manifest

Generate the root landing page from the manifest, and create the manifest itself.

**Files:**
- Create: `examples/gallery.json`
- Create: `tools/gallery/gallery.html.tmpl`
- Modify: `xtask/src/gallery.rs` (replace the Task 3 stub)

**Interfaces:**
- Consumes: `manifest::Example`.
- Produces: `gallery::render(examples: &[manifest::Example]) -> String`; CLI `xtask gallery-index --out <file>` (wired in Task 3).

- [ ] **Step 1: Create `examples/gallery.json`**

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

- [ ] **Step 2: Create `tools/gallery/gallery.html.tmpl`**

```html
<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<base href="./">
<title>superui — examples gallery</title>
<style>
  :root { --bg:#1e1e28; --card:#262633; --fg:#e6e6ef; --muted:#9a9ab0; --accent:#7c6cff; }
  * { box-sizing:border-box; }
  body { margin:0; background:var(--bg); color:var(--fg);
         font-family:system-ui, sans-serif; padding:32px; }
  header { max-width:900px; margin:0 auto 24px; }
  h1 { margin:0 0 4px; }
  header p { color:var(--muted); margin:0; }
  main { max-width:900px; margin:0 auto; display:grid;
         grid-template-columns:repeat(auto-fill, minmax(260px, 1fr)); gap:16px; }
  .card { display:block; text-decoration:none; color:inherit; background:var(--card);
          border:1px solid #3a3a52; border-radius:10px; padding:18px; transition:border-color .15s; }
  .card:hover { border-color:var(--accent); }
  .card h2 { margin:0 0 6px; font-size:18px; }
  .card p { margin:0; color:var(--muted); font-size:14px; }
</style>
</head>
<body>
  <header>
    <h1>superui examples</h1>
    <p>Browser-like HTML/CSS/JS apps running in Bevy, compiled to WebAssembly.</p>
  </header>
  <main>{{CARDS}}</main>
</body>
</html>
```

- [ ] **Step 3: Replace `xtask/src/gallery.rs` with the real implementation + failing test**

```rust
use crate::manifest::Example;

const TEMPLATE: &str = include_str!("../../tools/gallery/gallery.html.tmpl");

/// Render the gallery landing page: one card per example linking to `./<slug>/`.
pub fn render(examples: &[Example]) -> String {
    let cards: String = examples
        .iter()
        .map(|e| {
            format!(
                "<a class=\"card\" href=\"./{slug}/\"><h2>{title}</h2><p>{desc}</p></a>\n",
                slug = e.slug,
                title = e.title,
                desc = e.description
            )
        })
        .collect();
    TEMPLATE.replace("{{CARDS}}", &cards)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_card_linking_to_slug() {
        let examples = vec![Example {
            slug: "todomvc".into(),
            package: "todomvc".into(),
            title: "TodoMVC".into(),
            description: "classic".into(),
        }];
        let out = render(&examples);
        assert!(out.contains(r#"href="./todomvc/""#));
        assert!(out.contains("<h2>TodoMVC</h2>"));
        assert!(!out.contains("{{CARDS}}"));
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p xtask`
Expected: PASS (all three tests).

- [ ] **Step 5: Verify both subcommands end-to-end against real files**

Run:
```bash
cargo run -p xtask -- gallery-index --out dist/index.html
cargo run -p xtask -- host-page --slug todomvc --out dist/todomvc
```
Expected: prints `wrote dist/index.html (1 examples)` and `wrote dist/todomvc/index.html (3 source files)`. Open `dist/todomvc/index.html` and confirm the three real source names (`index.html`, `style.css`, `app.js`) appear in `window.__SOURCES__`.

- [ ] **Step 6: Commit**

```bash
git add examples/gallery.json tools/gallery/gallery.html.tmpl xtask/src/gallery.rs
git commit -m "feat(xtask): gallery-index subcommand, template, and manifest"
```

---

## Task 5: Deploy workflow + local end-to-end verification

Add the three-job Pages pipeline and prove the assembled site actually runs (this is where the §1 asset-base-path risk is verified empirically).

**Files:**
- Create: `.github/workflows/deploy-pages.yml`
- Modify: `.gitignore` (add `/dist`)

**Interfaces:**
- Consumes: everything above (manifest, xtask subcommands, wasm build, vendored highlighter).
- Produces: a deployed Pages site at `https://<user>.github.io/<repo>/` with `todomvc/` reachable.

- [ ] **Step 1: Add `/dist` to `.gitignore`**

Append:
```
# Assembled Pages output (built by CI / local dry-run)
/dist
```

- [ ] **Step 2: Create `.github/workflows/deploy-pages.yml`**

```yaml
name: Deploy Pages

on:
  push:
    branches: [main]
  workflow_dispatch:

permissions:
  contents: read
  pages: write
  id-token: write

concurrency:
  group: pages
  cancel-in-progress: false

jobs:
  discover:
    runs-on: ubuntu-latest
    outputs:
      matrix: ${{ steps.set.outputs.matrix }}
    steps:
      - uses: actions/checkout@v4
      - id: set
        run: |
          matrix=$(jq -c '{include: .examples | map({slug, package})}' examples/gallery.json)
          echo "matrix=$matrix" >> "$GITHUB_OUTPUT"

  build:
    needs: discover
    runs-on: ubuntu-latest
    strategy:
      matrix: ${{ fromJSON(needs.discover.outputs.matrix) }}
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust + wasm target
        run: |
          rustup toolchain install stable --profile minimal
          rustup target add wasm32-unknown-unknown

      - name: Resolve dependencies and derive wasm-bindgen version
        run: |
          cargo metadata --format-version=1 --quiet > metadata.json
          WB=$(jq -r '.packages[] | select(.name=="wasm-bindgen") | .version' metadata.json | sort -u | head -1)
          echo "Using wasm-bindgen $WB"
          echo "WB_VERSION=$WB" >> "$GITHUB_ENV"

      - name: Install wasm-bindgen-cli (exact match) and binaryen
        uses: taiki-e/install-action@v2
        with:
          tool: wasm-bindgen-cli@${{ env.WB_VERSION }},binaryen

      - uses: Swatinem/rust-cache@v2
        with:
          key: wasm-${{ matrix.slug }}

      - name: Build wasm (release, WebGL2)
        run: cargo build -p ${{ matrix.package }} --release --target wasm32-unknown-unknown

      - name: wasm-bindgen
        run: |
          wasm-bindgen --no-typescript --target web \
            --out-dir "stage/${{ matrix.slug }}" --out-name "${{ matrix.slug }}" \
            "target/wasm32-unknown-unknown/release/${{ matrix.package }}.wasm"

      - name: wasm-opt
        run: |
          wasm-opt -Oz -o "stage/${{ matrix.slug }}/${{ matrix.slug }}_bg.wasm" \
            "stage/${{ matrix.slug }}/${{ matrix.slug }}_bg.wasm"

      - name: Generate host page
        run: cargo run -p xtask -- host-page --slug ${{ matrix.slug }} --out "stage/${{ matrix.slug }}"

      - name: Copy app assets
        run: cp -r "examples/${{ matrix.slug }}/assets" "stage/${{ matrix.slug }}/assets"

      - uses: actions/upload-artifact@v4
        with:
          name: example-${{ matrix.slug }}
          path: stage/${{ matrix.slug }}

  assemble-and-deploy:
    needs: [discover, build]
    runs-on: ubuntu-latest
    environment:
      name: github-pages
      url: ${{ steps.deploy.outputs.page_url }}
    steps:
      - uses: actions/checkout@v4

      - name: Download example artifacts
        uses: actions/download-artifact@v4
        with:
          path: downloaded
          pattern: example-*

      - name: Assemble dist/
        run: |
          mkdir -p dist
          # Each artifact is downloaded to downloaded/example-<slug>/ ; move to dist/<slug>/
          for d in downloaded/example-*; do
            slug="${d#downloaded/example-}"
            mv "$d" "dist/$slug"
          done
          cp -r tools/gallery/vendor dist/vendor

      - name: Generate gallery index
        run: cargo run -p xtask -- gallery-index --out dist/index.html

      - uses: actions/upload-pages-artifact@v3
        with:
          path: dist

      - id: deploy
        uses: actions/deploy-pages@v4
```

- [ ] **Step 3: Local end-to-end dry-run (verifies the §1 asset-base-path risk)**

Install the tools locally if missing, then assemble and serve:
```bash
cargo install wasm-bindgen-cli --version 0.2.126   # must match the crate version
# binaryen/wasm-opt: choco install binaryen  (optional locally; skip the wasm-opt line if unavailable)

cargo build -p todomvc --release --target wasm32-unknown-unknown
wasm-bindgen --no-typescript --target web --out-dir dist/todomvc --out-name todomvc \
  target/wasm32-unknown-unknown/release/todomvc.wasm
cargo run -p xtask -- host-page --slug todomvc --out dist/todomvc
cp -r examples/todomvc/assets dist/todomvc/assets
cp -r tools/gallery/vendor dist/vendor
cargo run -p xtask -- gallery-index --out dist/index.html

python -m http.server -d dist 8080
```
Open `http://localhost:8080/todomvc/` and confirm ALL of:
- the TodoMVC app renders in the left/app pane (proves the wasm build + `#superui-canvas` binding work);
- **assets load from the subdirectory** — the app shows the "todos" heading and input, i.e. `assets/ui/todomvc/*` fetched correctly under `/todomvc/` (this is the §1 risk — if the app is blank but the canvas exists, the asset base path is wrong; check the browser network tab for 404s on `assets/ui/todomvc/...`);
- the code panel shows three tabs (`index.html`, `style.css`, `app.js`) with syntax highlighting, and switching tabs works;
- `http://localhost:8080/` shows the gallery with a TodoMVC card linking to `./todomvc/`;
- narrowing the window below 800px reveals the Demo/Code tabs and they toggle the panes.

Record the outcome. If assets 404, the fix is in the host template `<base>` / fetch paths or `main.rs` `AssetPlugin` file path — do not proceed until the app renders.

- [ ] **Step 4: Commit**

```bash
git add .github/workflows/deploy-pages.yml .gitignore
git commit -m "ci: build examples to wasm and deploy gallery to GitHub Pages"
```

---

## Task 6: README examples table + Pages setup docs

Document the live gallery and the (user-owned) one-time Pages configuration.

**Files:**
- Modify: `README.md`

**Interfaces:**
- Consumes: the stable `/<slug>/` URL contract.

- [ ] **Step 1: Add an examples table + deployment section to `README.md`**

Append (replace `<USER>`/`<REPO>` with the real repo once created — leave a clear placeholder note if not yet known):
```markdown
## Live examples

Each example is compiled to WebAssembly and published on GitHub Pages, showing the
running app beside its HTML/CSS/JS source.

| Example | Live demo | Description |
| --- | --- | --- |
| TodoMVC | [Open](https://<USER>.github.io/<REPO>/todomvc/) | The classic TodoMVC in plain HTML/CSS/JS on superui |

> ▶ These are static wasm builds. **Hot reload of HTML/CSS/JS is native-only** —
> `git clone` and `cargo run -p todomvc` to edit the app live.

## Deploying the gallery (maintainers)

The gallery is built and published by `.github/workflows/deploy-pages.yml` on every
push to `main` (or via **Run workflow**). One-time setup:

1. Repo **Settings → Pages → Source → GitHub Actions**.
2. Push to `main`; the workflow builds each example listed in `examples/gallery.json`.

To add an example: create the crate under `examples/<slug>/`, then append one object
to `examples/gallery.json`. The slug becomes its permanent URL — don't rename a
published slug (it breaks external links).
```

- [ ] **Step 2: Commit**

```bash
git add README.md
git commit -m "docs: live examples table + Pages deployment instructions"
```

---

## Self-Review

**Spec coverage:**
- §1 site shape → Task 3 (host page), Task 4 (gallery index), Task 5 (assembly places `vendor/`, `<slug>/`, assets). ✔
- §1 asset base-path risk → Task 5 Step 3 verifies empirically. ✔
- §2 manifest → Task 4 Step 1. ✔
- §3 code viewer (split, per-file tabs, vendored highlighter, auto source enumeration) → Task 2 (enumerate), Task 3 (template + vendor). ✔
- §4 xtask (host-page, gallery-index, tests) → Tasks 2–4. ✔
- §5 workflow (discover/build/assemble-deploy, wasm-bindgen version match, rust-cache, permissions, concurrency) → Task 5. ✔
- §6 footguns (file_watcher split, wasm-bindgen match, canvas config, size/loader) → Task 1 (split + canvas), Task 5 (version derive), Task 3 template (loader). ✔
- §7 native-only banner → Task 3 template + Task 6 README. ✔
- §8 routing + hand-written README table → Task 6. ✔
- §9 first deliverable = green todomvc deploy → Tasks 1–5. ✔

**Placeholder scan:** No TBD/TODO; all code blocks are complete. The only intentional placeholders are `<USER>`/`<REPO>` in README (unknowable until the repo exists) — flagged as such in Task 6.

**Type consistency:** `Example` fields (slug/package/title/description), `SourceFile` fields (name/path/lang/order), `manifest::load`, `sources::enumerate`, `host::render`, `gallery::render` are used identically across Tasks 2–4. The `#superui-canvas` selector matches between Task 1 (`main.rs`) and Task 3 (template). The wasm artifact name `<package>.wasm` and glue `<slug>.js` are consistent between Task 1, Task 3 template (`{{WASM_JS}}` = `<slug>.js`), and Task 5 (`--out-name <slug>`). Note: `package` and `slug` are both `todomvc` here; the workflow builds by `package` and names output by `slug`.
