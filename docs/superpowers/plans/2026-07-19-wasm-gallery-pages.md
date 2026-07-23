# WASM Example Gallery on GitHub Pages — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a GitHub Actions pipeline that compiles five superui example apps to WebAssembly and publishes them as a public GitHub Pages gallery, each demo showing the running app beside its authored source, grouped into Apps and Stress-test categories.

**Architecture:** A manifest (`examples/gallery.json`) drives a matrix build. Each example compiles to wasm (`cargo` → `wasm-bindgen` → `wasm-opt`), then a Rust `xtask` crate generates its host page (live app + tabbed code viewer) and the category-grouped gallery landing page. A three-job workflow (discover → build → assemble-and-deploy) publishes `dist/` to Pages. The code viewer shows **authored** source (`app.tsx`/CSS/HTML, or `app.js` for the plain example) and hides generated/tooling files.

**Tech Stack:** Rust (Bevy 0.17.3, wasm32-unknown-unknown), wasm-bindgen 0.2.126, binaryen (`wasm-opt`), GitHub Actions (`actions/upload-pages-artifact`, `actions/deploy-pages`), vendored highlight.js.

## Global Constraints

- Bevy version: `0.17` (locked `0.17.3`). Never bump.
- wasm-bindgen crate: `0.2.126`. `wasm-bindgen-cli` must match **exactly**; CI derives it at runtime (`Cargo.lock` is gitignored).
- Render backend: **WebGL2** (`webgl2` bevy feature on wasm only).
- All published URLs are **relative** (Pages serves under `/<repo>/`). Host pages set `<base href="./">`.
- A published example's **slug is a permanent URL contract** — renaming breaks external links.
- Rust edition `2021`, workspace `resolver = "2"`.
- **No CDN / no runtime external dependencies** — the highlighter is vendored and committed.
- The five example slugs/packages are: `todomvc`, `todomvc_supersolid`, `game_menu` (Apps); `citadel`, `horde` (Stress tests). `horde` carries a `["Playable game"]` tag.
- Code viewer shows **authored** source only; `app.generated.js`, `*.d.ts`, `tsconfig.json`, and dotfiles are hidden.
- Hot reload is native-only by design; never attempt to enable it on wasm.

---

## File Structure

- `examples/gallery.json` — manifest (slug/package/category/title/description, optional tags/build_args).
- `xtask/` — `Cargo.toml`, `src/main.rs` (CLI), `src/manifest.rs`, `src/sources.rs`, `src/host.rs`, `src/gallery.rs`.
- `tools/gallery/host.html.tmpl`, `tools/gallery/gallery.html.tmpl` — templates.
- `tools/gallery/vendor/highlight.min.js`, `tools/gallery/vendor/highlight.css` — vendored highlighter.
- `.github/workflows/deploy-pages.yml` — the pipeline.
- `examples/todomvc/Cargo.toml` — target-split (only todomvc still needs it).
- `examples/{todomvc,todomvc_supersolid,game_menu,citadel,horde}/src/main.rs` — `web_window` helper + wasm canvas wiring; the four TSX examples also gain the `webgl2` wasm feature in their Cargo.toml.
- `Cargo.toml` (root) — add `xtask` to members.
- `.gitignore` — ignore `/dist`.
- `README.md` — hand-written examples table (grouped by category) + Pages setup note.

---

## Task 1: Make all five examples wasm-buildable with a host canvas

Give every example a wasm `webgl2` feature (todomvc via a target-split that also preserves native `file_watcher`; the four TSX examples by extending their existing wasm target block) and route each primary window through a `web_window` helper that binds the wasm canvas.

**Files:**
- Modify: `examples/todomvc/Cargo.toml`
- Modify: `examples/{todomvc_supersolid,game_menu,citadel,horde}/Cargo.toml`
- Modify: `examples/{todomvc,todomvc_supersolid,game_menu,citadel,horde}/src/main.rs`

**Interfaces:**
- Produces: five `wasm32-unknown-unknown` binaries whose Bevy window binds to CSS selector `#superui-canvas`. Later tasks depend on that selector name and on each artifact being named `<slug>.wasm` (package name == slug for all five).

- [ ] **Step 1: todomvc — target-split bevy features in `examples/todomvc/Cargo.toml`**

Replace:
```toml
# Full Bevy (windowing + rendering) — this is the runnable app, not a headless lib.
# `file_watcher` is required for the AssetPlugin hot-reload seam to do anything
# (design §6); `watch_for_changes_override` is inert without it.
bevy = { version = "0.17", features = ["file_watcher"] }
```
with:
```toml
# Full Bevy (windowing + rendering) — this is the runnable app, not a headless lib.
bevy = { version = "0.17" }
```
Add a native target block (near `[dependencies]`):
```toml
# Native only: file watching drives hot reload (design §6). The `notify` crate
# behind `file_watcher` does not build on wasm — this is why hot reload is native-only.
[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
bevy = { version = "0.17", features = ["file_watcher"] }
```
Extend the EXISTING wasm target block (which already has `getrandom`) to add `webgl2`:
```toml
# Boa (via superui) needs the JS getrandom backend on wasm; `webgl2` selects the
# broadly-compatible WebGL2 wgpu backend for the web build.
[target.'cfg(target_arch = "wasm32")'.dependencies]
getrandom = { version = "0.3", features = ["wasm_js"] }
bevy = { version = "0.17", features = ["webgl2"] }
```

- [ ] **Step 2: Add `webgl2` to the four TSX examples' wasm target block**

In EACH of `examples/todomvc_supersolid/Cargo.toml`, `examples/game_menu/Cargo.toml`, `examples/citadel/Cargo.toml`, `examples/horde/Cargo.toml`, find the existing block:
```toml
[target.'cfg(target_arch = "wasm32")'.dependencies]
getrandom = { version = "0.3", features = ["wasm_js"] }
```
and add the bevy webgl2 line so it reads:
```toml
[target.'cfg(target_arch = "wasm32")'.dependencies]
getrandom = { version = "0.3", features = ["wasm_js"] }
bevy = { version = "0.17", features = ["webgl2"] }
```
(These four already keep `file_watcher` native-only, so no split is needed.)

- [ ] **Step 3: Add the `web_window` helper + canvas wiring to each `main.rs`**

The helper is identical in every example. Add it as a free function in each `main.rs` (e.g. just above `fn main`):
```rust
/// On the web, bind the primary window to the host page's `<canvas id="superui-canvas">`
/// and size it to that element. Identity on native (default OS window).
fn web_window(window: bevy::window::Window) -> bevy::window::Window {
    #[cfg(target_arch = "wasm32")]
    let window = bevy::window::Window {
        canvas: Some("#superui-canvas".into()),
        fit_canvas_to_parent: true,
        ..window
    };
    window
}
```
Then route each example's primary window through it:

**`todomvc` and `horde`** (they set `AssetPlugin` inline in `add_plugins(...)`): append a `.set(WindowPlugin { .. })` to the `DefaultPlugins` builder. For `todomvc`:
```rust
    app.add_plugins(DefaultPlugins.set(AssetPlugin {
        // Enable native hot reload (design §6). Inert on wasm.
        watch_for_changes_override: Some(true),
        ..default()
    }).set(WindowPlugin {
        primary_window: Some(web_window(Window::default())),
        ..default()
    }))
    .add_plugins(SuperUiPlugin);
```
For `horde` the `AssetPlugin` block differs but the change is the same — append `.set(WindowPlugin { primary_window: Some(web_window(Window::default())), ..default() })` to its `DefaultPlugins.set(AssetPlugin { .. })`:
```rust
    app.add_plugins(DefaultPlugins.set(AssetPlugin {
        watch_for_changes_override: Some(cfg!(not(target_arch = "wasm32"))),
        ..default()
    }).set(WindowPlugin {
        primary_window: Some(web_window(Window::default())),
        ..default()
    }))
    .init_state::<GameState>()
```

**`game_menu` and `todomvc_supersolid`** (they build `let asset_plugin = AssetPlugin { .. };` then `DefaultPlugins.set(asset_plugin)`): append the same `.set(WindowPlugin { .. })`. For `game_menu`:
```rust
    app.add_plugins(DefaultPlugins.set(asset_plugin).set(WindowPlugin {
        primary_window: Some(web_window(Window::default())),
        ..default()
    }))
        .add_plugins(SuperUiPlugin)
        .add_systems(Startup, setup);
```
For `todomvc_supersolid`:
```rust
    app.add_plugins(DefaultPlugins.set(asset_plugin).set(WindowPlugin {
        primary_window: Some(web_window(Window::default())),
        ..default()
    }))
        .add_plugins(SuperUiPlugin);
```

**`citadel`** (it already sets a custom `WindowPlugin` with title/resolution): wrap its existing `Window` in `web_window(...)`:
```rust
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(web_window(Window {
                title: "Citadel".into(),
                resolution: (1600u32, 900u32).into(),
                ..default()
            })),
            ..default()
        }))
```
Ensure `Window` and `WindowPlugin` are in scope. All five use `use bevy::prelude::*;` (which re-exports both). If any file does not, add `use bevy::window::{Window, WindowPlugin};`.

- [ ] **Step 4: Verify all five compile on native**

Run: `cargo build -p todomvc -p todomvc_supersolid -p game_menu -p citadel -p horde`
Expected: PASS. (`web_window` is identity on native; adding a default `WindowPlugin` where none existed is equivalent to Bevy's default.)

- [ ] **Step 5: Verify todomvc's native tests still pass**

Run: `cargo test -p todomvc`
Expected: PASS (unchanged behavior on native).

- [ ] **Step 6: Verify the wasm build compiles (representative: plain + TSX)**

Run:
```bash
rustup target add wasm32-unknown-unknown
cargo build -p todomvc --release --target wasm32-unknown-unknown
cargo build -p todomvc_supersolid --release --target wasm32-unknown-unknown
```
Expected: PASS, producing `target/wasm32-unknown-unknown/release/{todomvc,todomvc_supersolid}.wasm`. (First builds are slow.) `citadel`/`horde`/`game_menu` are exercised by CI in Task 5; if `citadel`/`horde` fail on `bevy_dev_tools`, the remedy is `"build_args": "--no-default-features"` in the manifest (Task 4) — note it and continue.

- [ ] **Step 7: Commit**

```bash
git add examples/*/Cargo.toml examples/*/src/main.rs
git commit -m "feat(examples): make all five examples wasm-buildable with host canvas"
```

---

## Task 2: xtask crate — manifest model + authored-source enumeration

Scaffold `xtask` and implement manifest loading and the TSX-aware source enumeration that keeps authored files and hides generated/tooling files.

**Files:**
- Modify: `Cargo.toml` (root) — add `xtask` to `members`.
- Create: `xtask/Cargo.toml`, `xtask/src/main.rs` (stub CLI), `xtask/src/manifest.rs`, `xtask/src/sources.rs`, `xtask/src/host.rs` (empty), `xtask/src/gallery.rs` (empty).

**Interfaces:**
- Produces:
  - `manifest::Example { slug, package, title, description, category: String, tags: Vec<String> }` (`serde::Deserialize`, `Clone`; `tags` defaults to `[]` via `#[serde(default)]`). Extra manifest fields like `build_args` are ignored by xtask.
  - `manifest::load(path: &Path) -> Result<Vec<Example>, Box<dyn Error>>`.
  - `sources::SourceFile { name, path, lang }` (`serde::Serialize`; sort key `order` is `#[serde(skip)]`).
  - `sources::enumerate(example_base: &Path, slug: &str) -> io::Result<Vec<SourceFile>>` — keeps authored source under `<base>/<slug>/assets/ui/<slug>/`, ordered tsx/jsx → html → css → ts → js.

- [ ] **Step 1: Add `xtask` to workspace members in root `Cargo.toml`**

Change `members = ["crates/*", "examples/*"]` to `members = ["crates/*", "examples/*", "xtask"]`.

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
    pub category: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct Manifest {
    examples: Vec<Example>,
}

/// Load `{ "examples": [ { slug, package, category, title, description, tags? }, ... ] }`.
/// Unknown fields (e.g. `build_args`, used only by the workflow) are ignored.
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

/// Decide whether a filename is authored source worth showing, and if so its
/// highlight.js language + display order. Hides generated/tooling/dotfiles.
fn classify(name: &str) -> Option<(&'static str, u8)> {
    if name.starts_with('.') {
        return None; // .gitkeep, .gitignore, …
    }
    if name.ends_with(".generated.js") || name.ends_with(".d.ts") || name == "tsconfig.json" {
        return None; // transpiler output + TS tooling
    }
    let ext = Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    match ext.as_str() {
        "tsx" => Some(("typescript", 0)),
        "jsx" => Some(("javascript", 0)),
        "html" | "htm" => Some(("xml", 1)), // highlight.js uses "xml" for HTML
        "css" => Some(("css", 2)),
        "ts" => Some(("typescript", 3)),
        "js" | "mjs" => Some(("javascript", 4)),
        _ => None,
    }
}

/// List authored source files under `<base>/<slug>/assets/ui/<slug>/`, ordered
/// tsx/jsx → html → css → ts → js (ties alphabetical). `path` is the fetch path
/// relative to the host page (which sets `<base href="./">`).
pub fn enumerate(example_base: &Path, slug: &str) -> io::Result<Vec<SourceFile>> {
    let dir = example_base.join(slug).join("assets").join("ui").join(slug);
    let mut files = Vec::new();
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if let Some((lang, order)) = classify(&name) {
            files.push(SourceFile {
                path: format!("assets/ui/{slug}/{name}"),
                name,
                lang: lang.to_string(),
                order,
            });
        }
    }
    files.sort_by(|a, b| a.order.cmp(&b.order).then_with(|| a.name.cmp(&b.name)));
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_authored_source_and_hides_generated_tooling() {
        // Simulate a TSX example directory.
        let base = std::env::temp_dir().join("xtask_sources_tsx_test");
        let ui = base.join("demo").join("assets").join("ui").join("demo");
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&ui).unwrap();
        for f in [
            "app.tsx",
            "app.generated.js", // generated -> hidden
            "index.html",
            "theme.css",
            "supersolid-shim.d.ts", // tooling -> hidden
            "tsconfig.json",        // tooling -> hidden
            ".gitkeep",             // dotfile -> hidden
        ] {
            std::fs::write(ui.join(f), b"x").unwrap();
        }

        let out = enumerate(&base, "demo").unwrap();
        let names: Vec<_> = out.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["app.tsx", "index.html", "theme.css"]);
        assert_eq!(out[0].lang, "typescript");
        assert_eq!(out[0].path, "assets/ui/demo/app.tsx");
        assert_eq!(out[1].lang, "xml");

        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn plain_example_shows_html_css_js() {
        let base = std::env::temp_dir().join("xtask_sources_plain_test");
        let ui = base.join("plain").join("assets").join("ui").join("plain");
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&ui).unwrap();
        for f in ["app.js", "index.html", "style.css"] {
            std::fs::write(ui.join(f), b"x").unwrap();
        }
        let out = enumerate(&base, "plain").unwrap();
        let names: Vec<_> = out.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["index.html", "style.css", "app.js"]);
        std::fs::remove_dir_all(&base).unwrap();
    }
}
```

- [ ] **Step 5: Create `xtask/src/main.rs` (stub) + empty `host.rs`/`gallery.rs`**

`xtask/src/main.rs`:
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
`xtask/src/host.rs`: `// filled in Task 3`
`xtask/src/gallery.rs`: `// filled in Task 4`

- [ ] **Step 6: Run tests**

Run: `cargo test -p xtask`
Expected: PASS (both enumeration tests).

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml xtask/
git commit -m "feat(xtask): scaffold + manifest model + authored-source enumeration"
```

---

## Task 3: xtask `host-page` subcommand + host template + vendored highlighter

Generate a per-example host page (live app pane + tabbed code viewer) and vendor the highlighter it references.

**Files:**
- Create: `tools/gallery/host.html.tmpl`, `tools/gallery/vendor/highlight.min.js`, `tools/gallery/vendor/highlight.css`
- Modify: `xtask/src/host.rs`, `xtask/src/main.rs`

**Interfaces:**
- Consumes: `manifest::Example`, `sources::{SourceFile, enumerate}`.
- Produces: `host::render(ex: &Example, sources: &[SourceFile]) -> String`; CLI `xtask host-page --slug <slug> --out <dir>` → writes `<dir>/index.html` (manifest at `examples/gallery.json`, example base `examples`).

- [ ] **Step 1: Vendor the highlighter (download + commit)**

Run:
```bash
mkdir -p tools/gallery/vendor
curl -Lo tools/gallery/vendor/highlight.min.js https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.9.0/highlight.min.js
curl -Lo tools/gallery/vendor/highlight.css   https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.9.0/styles/github-dark.min.css
```
Expected: two non-empty files. The core `highlight.min.js` bundle includes `xml`, `css`, `javascript`, `typescript`, `json`. Committed so the site has no runtime CDN dependency.

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
  #loader { position:absolute; inset:0; display:flex; align-items:center; justify-content:center;
            padding:0 24px; text-align:center; color:var(--muted); font-size:14px; }
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
    ▶ Static WebAssembly build of <strong>{{TITLE}}</strong>. Live hot-reload of the UI is
    <strong>native-only</strong> — <code>git clone … &amp;&amp; cargo run -p {{SLUG}}</code>
    (add <code>--features hmr</code> for the supersolid TSX examples) to edit it live.
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

/// Render the host page for one example: title/slug substitution, the wasm glue
/// filename, and the embedded authored-source list the code viewer reads.
pub fn render(ex: &Example, sources: &[SourceFile]) -> String {
    let sources_json = serde_json::to_string(sources).expect("sources serialize");
    TEMPLATE
        .replace("{{TITLE}}", &ex.title)
        .replace("{{SLUG}}", &ex.slug)
        .replace("{{WASM_JS}}", &format!("{}.js", ex.slug))
        .replace("{{SOURCES_JSON}}", &sources_json)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_canvas_wasm_and_tsx_source() {
        let ex = Example {
            slug: "game_menu".into(),
            package: "game_menu".into(),
            title: "Game Menu".into(),
            description: "menu".into(),
            category: "Apps".into(),
            tags: vec![],
        };
        let sources = vec![SourceFile {
            name: "app.tsx".into(),
            path: "assets/ui/game_menu/app.tsx".into(),
            lang: "typescript".into(),
            order: 0,
        }];
        let out = render(&ex, &sources);
        assert!(out.contains(r#"id="superui-canvas""#));
        assert!(out.contains("import init from './game_menu.js'"));
        assert!(out.contains("assets/ui/game_menu/app.tsx"));
        assert!(out.contains("cargo run -p game_menu"));
        assert!(!out.contains("{{"), "no unsubstituted template tokens");
    }
}
```

- [ ] **Step 4: Implement the CLI in `xtask/src/main.rs`**

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
Add a temporary stub to `xtask/src/gallery.rs` so this compiles (Task 4 replaces it):
```rust
pub fn render(_examples: &[crate::manifest::Example]) -> String {
    String::new()
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p xtask`
Expected: PASS (`renders_canvas_wasm_and_tsx_source` + the Task 2 tests).

- [ ] **Step 6: Commit**

```bash
git add tools/gallery/host.html.tmpl tools/gallery/vendor/ xtask/src/host.rs xtask/src/main.rs xtask/src/gallery.rs
git commit -m "feat(xtask): host-page subcommand, host template, vendored highlighter"
```

---

## Task 4: xtask `gallery-index` (category sections + tag badges) + template + manifest

Generate the root landing page grouped by category with tag badges, and create the five-example manifest.

**Files:**
- Create: `examples/gallery.json`, `tools/gallery/gallery.html.tmpl`
- Modify: `xtask/src/gallery.rs`

**Interfaces:**
- Consumes: `manifest::Example` (incl. `category`, `tags`).
- Produces: `gallery::render(examples: &[Example]) -> String`.

- [ ] **Step 1: Create `examples/gallery.json`**

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
  body { margin:0; background:var(--bg); color:var(--fg); font-family:system-ui, sans-serif; padding:32px; }
  header, section { max-width:960px; margin:0 auto; }
  header { margin-bottom:24px; }
  h1 { margin:0 0 4px; }
  header p { color:var(--muted); margin:0; }
  h2.cat { margin:28px auto 12px; font-size:15px; text-transform:uppercase; letter-spacing:.08em;
           color:var(--muted); border-bottom:1px solid #3a3a52; padding-bottom:6px; }
  .cards { display:grid; grid-template-columns:repeat(auto-fill, minmax(260px, 1fr)); gap:16px; }
  .card { display:block; text-decoration:none; color:inherit; background:var(--card);
          border:1px solid #3a3a52; border-radius:10px; padding:18px; transition:border-color .15s; }
  .card:hover { border-color:var(--accent); }
  .card h3 { margin:0 0 6px; font-size:18px; }
  .card p { margin:0; color:var(--muted); font-size:14px; }
  .badges { margin-top:10px; display:flex; gap:6px; flex-wrap:wrap; }
  .badge { font-size:11px; padding:2px 8px; border-radius:999px; background:#3a3358; color:#d8d2ff; }
</style>
</head>
<body>
  <header>
    <h1>superui examples</h1>
    <p>Browser-like HTML/CSS/JS &amp; Solid-style TSX apps running in Bevy, compiled to WebAssembly.
       Stress tests are deliberately heavy and may run slowly in-browser.</p>
  </header>
  {{SECTIONS}}
</body>
</html>
```

- [ ] **Step 3: Replace `xtask/src/gallery.rs` with the real implementation + failing test**

```rust
use crate::manifest::Example;

const TEMPLATE: &str = include_str!("../../tools/gallery/gallery.html.tmpl");

/// Render the gallery: one `<section>` per category (first-seen order), each card
/// linking to `./<slug>/` and rendering its tags as badge chips.
pub fn render(examples: &[Example]) -> String {
    // Category order = first appearance in the manifest.
    let mut categories: Vec<&str> = Vec::new();
    for e in examples {
        if !categories.iter().any(|c| *c == e.category) {
            categories.push(&e.category);
        }
    }

    let mut sections = String::new();
    for cat in categories {
        sections.push_str(&format!("<section><h2 class=\"cat\">{cat}</h2><div class=\"cards\">"));
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
            sections.push_str(&format!(
                "<a class=\"card\" href=\"./{slug}/\"><h3>{title}</h3><p>{desc}</p>{badges_html}</a>",
                slug = e.slug,
                title = e.title,
                desc = e.description,
            ));
        }
        sections.push_str("</div></section>");
    }
    TEMPLATE.replace("{{SECTIONS}}", &sections)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ex(slug: &str, category: &str, tags: &[&str]) -> Example {
        Example {
            slug: slug.into(),
            package: slug.into(),
            title: slug.into(),
            description: "d".into(),
            category: category.into(),
            tags: tags.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn groups_by_category_and_renders_badges() {
        let examples = vec![
            ex("todomvc", "Apps", &[]),
            ex("todomvc_supersolid", "Apps", &[]),
            ex("horde", "Stress tests", &["Playable game"]),
        ];
        let out = render(&examples);
        assert!(out.contains(r#"<h2 class="cat">Apps</h2>"#));
        assert!(out.contains(r#"<h2 class="cat">Stress tests</h2>"#));
        assert!(out.contains(r#"href="./todomvc/""#));
        assert!(out.contains(r#"href="./todomvc_supersolid/""#));
        assert!(out.contains(r#"<span class="badge">Playable game</span>"#));
        // Apps section precedes Stress tests (first-seen order).
        assert!(out.find("Apps").unwrap() < out.find("Stress tests").unwrap());
        assert!(!out.contains("{{SECTIONS}}"));
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p xtask`
Expected: PASS (all four tests).

- [ ] **Step 5: Verify both subcommands end-to-end against real files**

Run:
```bash
cargo run -p xtask -- gallery-index --out dist/index.html
cargo run -p xtask -- host-page --slug todomvc --out dist/todomvc
cargo run -p xtask -- host-page --slug horde --out dist/horde
```
Expected: `wrote dist/index.html (5 examples)`, `dist/todomvc/index.html (3 source files)`, and `dist/horde/index.html (4 source files)`. Open `dist/horde/index.html` and confirm `window.__SOURCES__` lists `app.tsx` first (typescript) and does NOT include `app.generated.js`. Open `dist/index.html` and confirm two sections (Apps, Stress tests) and the "Playable game" badge on the horde card.

- [ ] **Step 6: Commit**

```bash
git add examples/gallery.json tools/gallery/gallery.html.tmpl xtask/src/gallery.rs
git commit -m "feat(xtask): gallery-index with category sections + tag badges; add manifest"
```

---

## Task 5: Deploy workflow + local end-to-end verification

Add the three-job Pages pipeline (matrix over all five examples) and prove the assembled site actually runs (verifies the §1 asset-base-path risk).

**Files:**
- Create: `.github/workflows/deploy-pages.yml`
- Modify: `.gitignore` (add `/dist`)

**Interfaces:**
- Consumes: everything above.
- Produces: a deployed Pages site at `https://<user>.github.io/<repo>/` with all five `<slug>/` reachable.

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
          matrix=$(jq -c '{include: .examples | map({slug, package, build_args: (.build_args // "")})}' examples/gallery.json)
          echo "matrix=$matrix" >> "$GITHUB_OUTPUT"

  build:
    needs: discover
    runs-on: ubuntu-latest
    strategy:
      fail-fast: false
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
        run: cargo build -p ${{ matrix.package }} --release --target wasm32-unknown-unknown ${{ matrix.build_args }}

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

      - name: Copy app assets (after build so app.generated.js exists)
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

Install tools locally if missing, then assemble a plain + a TSX example and serve:
```bash
cargo install wasm-bindgen-cli --version 0.2.126   # must match the crate version
# binaryen/wasm-opt optional locally; skip the wasm-opt line if unavailable.

for slug in todomvc todomvc_supersolid; do
  cargo build -p "$slug" --release --target wasm32-unknown-unknown
  wasm-bindgen --no-typescript --target web --out-dir "dist/$slug" --out-name "$slug" \
    "target/wasm32-unknown-unknown/release/$slug.wasm"
  cargo run -p xtask -- host-page --slug "$slug" --out "dist/$slug"
  cp -r "examples/$slug/assets" "dist/$slug/assets"
done
cp -r tools/gallery/vendor dist/vendor
cargo run -p xtask -- gallery-index --out dist/index.html

python -m http.server -d dist 8080
```
Open `http://localhost:8080/todomvc_supersolid/` and confirm ALL of:
- the TodoMVC app renders in the app pane (wasm build + `#superui-canvas` binding work);
- **assets load from the subdirectory** — the app shows its UI, i.e. `assets/ui/todomvc_supersolid/*` (including `app.generated.js`) fetched correctly under `/todomvc_supersolid/` (§1 risk; if the app is blank, check the browser network tab for 404s on `assets/ui/...`);
- the code panel leads with `app.tsx` (typescript highlighting), shows CSS + `index.html`, and does NOT show `app.generated.js`; tab switching works;
- `http://localhost:8080/` shows two sections (Apps, Stress tests) and the horde card's "Playable game" badge;
- narrowing below 800px reveals Demo/Code tabs that toggle the panes.

Also open `http://localhost:8080/todomvc/` and confirm the plain example renders and shows `index.html`/`style.css`/`app.js`. Record the outcome; if assets 404, fix the host template `<base>`/paths before proceeding.

- [ ] **Step 4: Commit**

```bash
git add .github/workflows/deploy-pages.yml .gitignore
git commit -m "ci: build five examples to wasm and deploy gallery to GitHub Pages"
```

---

## Task 6: README examples table + Pages setup docs

Document the live gallery (grouped by category) and the one-time Pages configuration.

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Add an examples table + deployment section to `README.md`**

Append (replace `<USER>`/`<REPO>` with `strowk`/`bevy_superui` once the Pages URL is confirmed; leave a note if not yet live):
```markdown
## Live examples

Each example is compiled to WebAssembly and published on GitHub Pages, showing the
running app beside its authored source (TSX where applicable).

**Apps**

| Example | Live demo | Description |
| --- | --- | --- |
| TodoMVC (HTML/CSS/JS) | [Open](https://<USER>.github.io/<REPO>/todomvc/) | Classic TodoMVC in plain HTML/CSS/JS |
| TodoMVC (Supersolid TSX) | [Open](https://<USER>.github.io/<REPO>/todomvc_supersolid/) | The same app authored in Solid-style .tsx |
| Game Menu | [Open](https://<USER>.github.io/<REPO>/game_menu/) | Multi-screen sci-fi game menu in supersolid TSX |

**Stress tests** (deliberately heavy — may run slowly in-browser)

| Example | Live demo | Description |
| --- | --- | --- |
| Citadel | [Open](https://<USER>.github.io/<REPO>/citadel/) | Economy sim UI — reactive-node stress test |
| Horde | [Open](https://<USER>.github.io/<REPO>/horde/) | Survivors-like **playable game** + reactive-HUD stress test |

> ▶ These are static wasm builds. **Hot reload of the UI is native-only** —
> `git clone` and `cargo run -p <example>` (add `--features hmr` for the supersolid
> TSX examples) to edit HTML/CSS/TSX live.

## Deploying the gallery (maintainers)

The gallery is built and published by `.github/workflows/deploy-pages.yml` on every
push to `main` (or via **Run workflow**). One-time setup: repo **Settings → Pages →
Source → GitHub Actions**.

To add an example: create the crate under `examples/<slug>/` (wasm-buildable, with a
`web_window` canvas hook), then append one object to `examples/gallery.json`. The slug
becomes its permanent URL — don't rename a published slug.
```

- [ ] **Step 2: Commit**

```bash
git add README.md
git commit -m "docs: live examples table + Pages deployment instructions"
```

---

## Self-Review

**Spec coverage:**
- §0 inventory (5 examples, 2 TodoMVCs, horde=game) → Task 4 manifest, Task 1 wasm-readiness, Task 6 table. ✔
- §1 site shape + asset-base-path risk → Tasks 3–5; risk verified in Task 5 Step 3. ✔
- §2 manifest (category, tags, build_args) → Task 4 Step 1; build_args honored in Task 5 workflow. ✔
- §3 code viewer (authored vs generated, tsx first, vendored highlighter) → Task 2 (classify/enumerate), Task 3 (template + vendor). ✔
- §4 xtask (host-page, gallery-index w/ sections+badges, tests) → Tasks 2–4. ✔
- §5 workflow (discover/build/assemble, matrix over 5, build_args, wasm-bindgen match, rust-cache, permissions, concurrency) → Task 5. ✔
- §6 footguns (todomvc split, webgl2 on all, web_window canvas incl. citadel merge, wasm-bindgen match, loader) → Task 1 + Task 3 template + Task 5. ✔
- §7 native-only banner → Task 3 template + Task 6. ✔
- §8 routing + grouped README table → Task 6. ✔
- §9 five-example green deploy → Tasks 1–5. ✔

**Placeholder scan:** No TBD/TODO; all code blocks complete. Only intentional placeholders are `<USER>`/`<REPO>` in README (flagged) and the acknowledged fallback note about `--no-default-features` for stress tests (a documented remedy, not an unfinished step).

**Type consistency:** `Example { slug, package, title, description, category, tags }` and `SourceFile { name, path, lang, order }` are used identically across Tasks 2–4 (host test and gallery test both construct the full `Example`). `classify`/`enumerate`/`host::render`/`gallery::render`/`manifest::load` signatures match their call sites. The `#superui-canvas` selector matches between Task 1 (`web_window`) and Task 3 (template). Artifact naming is consistent: package==slug for all five, wasm at `<package>.wasm`, glue `<slug>.js`, `{{WASM_JS}}` = `<slug>.js`, `--out-name <slug>`.
