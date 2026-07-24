# Supersolid Example Boilerplate Reduction — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make authoring a superui app a web-like, single-entry experience — one `index.html` that declares its own stylesheet and script — and delete the per-example `.tsx`/`.js` split boilerplate.

**Architecture:** `index.html` becomes an HTML-as-manifest entry point. A two-phase mount discovers its `<link>`/`<script>` subresources; the `.tsx`→generated-`.js` decision lives in one framework seam (`superui_paths::generated_js` + a `live` cfg). `SuperUiRoot` collapses to `{ html }`; a `from_asset_dir` builder + a one-line `build.rs` helper replace the hand-wired handles, `USE_LIVE_TSX`, and path bookkeeping.

**Tech Stack:** Rust, Bevy 0.17 (asset system, ECS, `bevy_ui`), oxc (via `supersolid`), flair CSS (`bevy_flair_style`), Boa (via `superui_bridge`).

## Global Constraints

- **Generated artifact path:** `<ui-dir>/.superui/build/<stem>.js`, always a forward-slash asset path. Never derive an asset path from a `PathBuf` via `Display`/`to_string_lossy` (Windows yields backslashes and breaks Bevy loading). `superui_paths` is the single source of this convention.
- **Entry filename convention:** `index.html` (`superui_paths::ENTRY_HTML`).
- **The seam (`live`):** `cfg!(all(not(target_arch = "wasm32"), feature = "hmr"))`. `live` → author `.tsx`; else generated `.js`. Expressed in exactly one place.
- **`oxc` must never enter the wasm binary.** `superui` may depend on `supersolid` only under `cfg(not(target_arch = "wasm32"))` (already the case). The new `superui_paths` crate has **zero** dependencies so both `superui` (all targets) and `supersolid` (host) can share it.
- **Single stylesheet:** flair applies one `Handle<StyleSheet>` per node. Discover the first `<link rel="stylesheet">`; `warn!` (never silent) on extras. `UiRuntime::new`'s signature does **not** change.
- **Graceful degradation:** transpile diagnostics are `warn!`/`cargo:warning`, never hard failures.
- **Workflow:** land directly on `main` (project uses no PRs). Do **not** use worktrees (huge `target/`). The current branch `website-redesign-cyber-theme` is unrelated — switch to `main` before implementing.
- **Spec:** `docs/superpowers/specs/2026-07-24-supersolid-example-boilerplate-design.md`.

---

### Task 1: `superui_paths` crate (the path convention)

**Files:**
- Create: `crates/superui_paths/Cargo.toml`
- Create: `crates/superui_paths/src/lib.rs`

**Interfaces:**
- Produces:
  - `pub const GENERATED_DIR: &str` (= `".superui/build"`)
  - `pub const ENTRY_HTML: &str` (= `"index.html"`)
  - `pub fn generated_js(src: &str) -> String`
  - `pub fn parent_dir(path: &str) -> &str`
  - `pub fn join_asset(dir: &str, rel: &str) -> String`

- [ ] **Step 1: Create the crate manifest**

`crates/superui_paths/Cargo.toml`:

```toml
[package]
name = "superui_paths"
edition.workspace = true
version.workspace = true
license.workspace = true

[dependencies]
```

(The workspace `members = ["crates/*", ...]` picks this up automatically.)

- [ ] **Step 2: Write the failing tests**

`crates/superui_paths/src/lib.rs`:

```rust
//! The forward-slash asset-path convention shared by `superui` (runtime resolution)
//! and `supersolid` (build-time output). Zero dependencies so both — including the
//! wasm build of `superui` — can depend on it without pulling in `oxc`.

/// Subfolder (relative to a UI directory) for build-time generated artifacts.
pub const GENERATED_DIR: &str = ".superui/build";

/// Conventional entry-document filename resolved by `from_asset_dir`.
pub const ENTRY_HTML: &str = "index.html";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_js_maps_source_to_build_dir() {
        assert_eq!(generated_js("ui/counter/app.tsx"), "ui/counter/.superui/build/app.js");
        assert_eq!(generated_js("ui/counter/app.ts"), "ui/counter/.superui/build/app.js");
        assert_eq!(generated_js("app.tsx"), ".superui/build/app.js");
    }

    #[test]
    fn parent_dir_takes_everything_before_the_last_slash() {
        assert_eq!(parent_dir("ui/counter/index.html"), "ui/counter");
        assert_eq!(parent_dir("index.html"), "");
    }

    #[test]
    fn join_asset_resolves_relative_and_root_paths() {
        assert_eq!(join_asset("ui/counter", "style.css"), "ui/counter/style.css");
        assert_eq!(join_asset("ui/counter", "./app.tsx"), "ui/counter/app.tsx");
        assert_eq!(join_asset("ui/counter", "/shared/x.css"), "shared/x.css");
        assert_eq!(join_asset("", "app.js"), "app.js");
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p superui_paths`
Expected: FAIL — `cannot find function generated_js` (and `parent_dir`, `join_asset`).

- [ ] **Step 4: Implement the functions**

Add above the `#[cfg(test)]` module in `crates/superui_paths/src/lib.rs`:

```rust
/// `ui/counter/app.tsx` → `ui/counter/.superui/build/app.js`.
pub fn generated_js(src: &str) -> String {
    let dir = parent_dir(src);
    let file = src.rsplit('/').next().unwrap_or(src);
    let stem = file.rsplit_once('.').map(|(s, _)| s).unwrap_or(file);
    if dir.is_empty() {
        format!("{GENERATED_DIR}/{stem}.js")
    } else {
        format!("{dir}/{GENERATED_DIR}/{stem}.js")
    }
}

/// Everything before the final `/`, or `""` when there is none.
pub fn parent_dir(path: &str) -> &str {
    match path.rfind('/') {
        Some(i) => &path[..i],
        None => "",
    }
}

/// Resolve `rel` (an HTML `href`/`src`) against `dir`. A leading `/` is treated as
/// asset-root-relative; a leading `./` is stripped.
pub fn join_asset(dir: &str, rel: &str) -> String {
    let rel = rel.strip_prefix("./").unwrap_or(rel);
    if let Some(abs) = rel.strip_prefix('/') {
        abs.to_string()
    } else if dir.is_empty() {
        rel.to_string()
    } else {
        format!("{dir}/{rel}")
    }
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p superui_paths`
Expected: PASS (3 tests).

- [ ] **Step 6: Commit**

```bash
git add crates/superui_paths
git commit -m "feat(superui_paths): shared asset-path convention crate"
```

---

### Task 2: `supersolid::build::transpile_dir` (one-line build.rs helper)

**Files:**
- Modify: `crates/supersolid/Cargo.toml` (add `superui_paths` dep)
- Modify: `crates/supersolid/src/lib.rs` (add `pub mod build;`)
- Create: `crates/supersolid/src/build.rs`

**Interfaces:**
- Consumes: `superui_paths::{generated_js, parent_dir}`; existing `crate::transpile_file(&Path, &Path) -> std::io::Result<TranspileResult>`.
- Produces: `pub fn transpile_dir(ui_dir: &str)` — for build.rs use.

- [ ] **Step 1: Add the dependency**

In `crates/supersolid/Cargo.toml`, under `[dependencies]`:

```toml
[dependencies]
oxc.workspace = true
superui_paths = { path = "../superui_paths" }
```

- [ ] **Step 2: Declare the module**

In `crates/supersolid/src/lib.rs`, add near the top with the other module/`pub use` lines:

```rust
pub mod build;
```

- [ ] **Step 3: Write the failing test + module skeleton**

Create `crates/supersolid/src/build.rs`:

```rust
//! Build-script helper: pre-transpile the `.tsx`/`.ts` entries in a UI directory
//! to `<dir>/.superui/build/<stem>.js` for wasm / no-HMR native builds. Runs on
//! the HOST, so `oxc` never enters the wasm binary.

use std::path::Path;

/// Transpile every top-level `.tsx`/`.ts` file in `ui_dir` to its generated `.js`
/// under `<ui_dir>/.superui/build/`. Intended to be the whole body of a `build.rs`.
///
/// Skips work entirely when `CARGO_FEATURE_HMR` is set: those builds load the live
/// `.tsx` through the transpiling asset loader, so the artifact is unused.
pub fn transpile_dir(ui_dir: &str) {
    transpile_dir_impl(ui_dir, std::env::var_os("CARGO_FEATURE_HMR").is_some());
}

fn transpile_dir_impl(ui_dir: &str, skip: bool) {
    if skip {
        return;
    }
    let entries = match std::fs::read_dir(ui_dir) {
        Ok(e) => e,
        Err(e) => {
            println!("cargo:warning=supersolid: cannot read {ui_dir}: {e}");
            return;
        }
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !(name.ends_with(".tsx") || name.ends_with(".ts")) {
            continue;
        }
        let src = format!("{ui_dir}/{name}");
        let out = superui_paths::generated_js(&src);
        let _ = std::fs::create_dir_all(superui_paths::parent_dir(&out));
        println!("cargo:rerun-if-changed={src}");
        match crate::transpile_file(Path::new(&src), Path::new(&out)) {
            Ok(result) => {
                for d in &result.diagnostics {
                    println!("cargo:warning=supersolid: {}", d.message);
                }
            }
            Err(e) => println!("cargo:warning=supersolid: could not transpile {src}: {e}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::transpile_dir_impl;
    use std::path::PathBuf;

    // Isolated temp UI dir under the target dir. No Date/rand (unavailable here in
    // some sandboxes anyway) — key the name off the test name.
    fn temp_ui_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("superui_build_test_{tag}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn writes_generated_js_for_each_tsx() {
        let dir = temp_ui_dir("writes");
        std::fs::write(dir.join("app.tsx"), "const a = <div class=\"x\"/>;").unwrap();
        let dir_str = dir.to_string_lossy().replace('\\', "/");

        transpile_dir_impl(&dir_str, false);

        let out = dir.join(".superui").join("build").join("app.js");
        assert!(out.exists(), "expected generated {out:?}");
        let js = std::fs::read_to_string(out).unwrap();
        assert!(js.contains("$ss.el(\"div\")"), "JSX must be lowered:\n{js}");
    }

    #[test]
    fn skip_flag_writes_nothing() {
        let dir = temp_ui_dir("skip");
        std::fs::write(dir.join("app.tsx"), "const a = 1;").unwrap();
        let dir_str = dir.to_string_lossy().replace('\\', "/");

        transpile_dir_impl(&dir_str, true);

        assert!(!dir.join(".superui").exists(), "skip=true must not transpile");
    }
}
```

- [ ] **Step 4: Run the tests to verify they fail, then pass**

Run: `cargo test -p supersolid build::`
Expected: after Step 3 the module compiles and both tests PASS (the implementation is already in place — this task's test *is* the spec for the helper). If `read_dir`/`transpile_file` wiring is wrong, fix until green.

- [ ] **Step 5: Commit**

```bash
git add crates/supersolid/Cargo.toml crates/supersolid/src/lib.rs crates/supersolid/src/build.rs
git commit -m "feat(supersolid): transpile_dir build helper writing to .superui/build"
```

---

### Task 3: `superui` core — manifest entry, two-phase mount, hot-reload remount

**Files:**
- Modify: `crates/superui/Cargo.toml` (add `superui_paths` dep)
- Modify: `crates/superui/src/mount.rs` (reshape `SuperUiRoot`, builders, `SuperUiSubresources`, seam, two-phase `mount_when_ready`, discovery)
- Modify: `crates/superui/src/hot_reload.rs` (source ids from `SuperUiSubresources`; `html` branch → full remount)
- Modify: `crates/superui/tests/support/mod.rs` (Model-2 `spawn_root`)
- Modify: `crates/superui/tests/integration.rs` (adjust 4 `spawn_root` call sites)

**Interfaces:**
- Consumes: `superui_paths::{ENTRY_HTML, generated_js, join_asset, parent_dir}`; `UiRuntime::new(Rc<RefCell<Dom>>, Entity, Handle<StyleSheet>, bool)`, `UiRuntime::{run_script, bound_non_root_entities, root, dirty}`; `superui_html::parse_document(&str) -> superui_dom::Dom`; `superui_dom::Dom::{document, tag, get_attribute, children}`.
- Produces:
  - `pub struct SuperUiRoot { pub html: Handle<HtmlSource> }` with `#[require(Node)]`
  - `SuperUiRoot::from_asset_dir(dir: &str, assets: &AssetServer) -> impl Bundle`
  - `SuperUiRoot::from_asset_dir_with(dir: &str, node: Node, assets: &AssetServer) -> impl Bundle`
  - `pub(crate) struct SuperUiSubresources { pub css: Option<Handle<StyleSheet>>, pub js: Handle<JsSource> }`
  - `pub(crate) fn live_source() -> bool`, `fn resolve_script`, `fn collect_refs`, `fn discover_subresources`

- [ ] **Step 1: Add the dependency**

In `crates/superui/Cargo.toml`, under the main `[dependencies]` (target-agnostic — needed on wasm too):

```toml
superui_paths = { path = "../superui_paths" }
```

- [ ] **Step 2: Write the failing unit tests for the seam + head-walk**

At the bottom of `crates/superui/src/mount.rs`, add a new test module (leave the existing `hmr_gate_tests` untouched):

```rust
#[cfg(test)]
mod model2_tests {
    use super::{collect_refs, resolve_script};

    #[test]
    fn resolve_script_applies_the_tsx_js_seam() {
        // Non-live: .tsx/.ts map to the generated build artifact.
        assert_eq!(resolve_script("ui/counter", "app.tsx", false), "ui/counter/.superui/build/app.js");
        assert_eq!(resolve_script("ui/counter", "app.ts", false), "ui/counter/.superui/build/app.js");
        // Live: load the author source as-is.
        assert_eq!(resolve_script("ui/counter", "app.tsx", true), "ui/counter/app.tsx");
        // Plain .js passes through regardless.
        assert_eq!(resolve_script("ui/counter", "app.js", false), "ui/counter/app.js");
    }

    #[test]
    fn collect_refs_finds_first_link_and_script_resolved() {
        let dom = superui_html::parse_document(
            r#"<html><head>
                 <link rel="stylesheet" href="style.css">
                 <script type="module" src="app.tsx"></script>
               </head><body><div id="root"></div></body></html>"#,
        );
        let (css, js) = collect_refs(&dom, "ui/counter", false);
        assert_eq!(css.as_deref(), Some("ui/counter/style.css"));
        assert_eq!(js.as_deref(), Some("ui/counter/.superui/build/app.js"));
    }

    #[test]
    fn collect_refs_live_keeps_tsx_source() {
        let dom = superui_html::parse_document(
            r#"<html><head><script src="app.tsx"></script></head><body></body></html>"#,
        );
        let (_css, js) = collect_refs(&dom, "ui/x", true);
        assert_eq!(js.as_deref(), Some("ui/x/app.tsx"));
    }
}
```

- [ ] **Step 3: Run to verify it fails (compile error)**

Run: `cargo test -p superui model2_tests`
Expected: FAIL to compile — `cannot find function collect_refs` / `resolve_script` in the new test module. (This is the RED state; the crate only compiles again once Steps 4, 6, 7, 8, 9 are all done — the `SuperUiRoot` reshape is a breaking change that cascades through mount, hot_reload, and the test harness at once.)

- [ ] **Step 4: Reshape `SuperUiRoot`, add builders, the seam, and discovery helpers**

In `crates/superui/src/mount.rs`, replace the `SuperUiRoot` struct definition (currently the `#[derive(Component, Default)] pub struct SuperUiRoot { html, css, js }` block) with:

```rust
/// Marks an authored-UI mount point. The single authored asset is the entry HTML;
/// stylesheets and the script are discovered from its `<head>` at mount. The root
/// entity is the bevy_ui parent the DOM `<body>` reconciles under, so it needs a
/// `Node` — `#[require(Node)]` inserts a default one when a spawn omits it.
#[derive(Component, Default)]
#[require(Node)]
pub struct SuperUiRoot {
    pub html: Handle<HtmlSource>,
}

impl SuperUiRoot {
    /// Spawn an authored UI from `<dir>/index.html` (an asset-root-relative dir).
    /// Bundles a full-viewport `Node`.
    pub fn from_asset_dir(dir: &str, assets: &AssetServer) -> impl Bundle {
        Self::from_asset_dir_with(dir, fill_node(), assets)
    }

    /// Like [`SuperUiRoot::from_asset_dir`] but with a caller-supplied root `Node`.
    pub fn from_asset_dir_with(dir: &str, node: Node, assets: &AssetServer) -> impl Bundle {
        let path = format!("{dir}/{}", superui_paths::ENTRY_HTML);
        (node, SuperUiRoot { html: assets.load::<HtmlSource>(path) })
    }
}

fn fill_node() -> Node {
    Node { width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }
}

/// The subresources discovered from the entry HTML's `<head>` (inserted in mount
/// Phase 1; consumed in Phase 2 and by hot reload).
#[derive(Component)]
pub(crate) struct SuperUiSubresources {
    /// First `<link rel=stylesheet>`, or `None` when the document declares none.
    pub css: Option<Handle<StyleSheet>>,
    /// The `<script src>` resolved through the tsx/js seam.
    pub js: Handle<JsSource>,
}

/// Live authoring: native + the `hmr` feature. Every other build loads generated JS.
pub(crate) fn live_source() -> bool {
    cfg!(all(not(target_arch = "wasm32"), feature = "hmr"))
}

/// Map a `<script src>` to the asset path to load, applying the tsx/js seam.
fn resolve_script(dir: &str, src: &str, live: bool) -> String {
    let path = superui_paths::join_asset(dir, src);
    let is_tsx = path.ends_with(".tsx") || path.ends_with(".ts");
    if is_tsx && !live {
        superui_paths::generated_js(&path)
    } else {
        path
    }
}

/// Depth-first (document order) walk collecting the first stylesheet `href` and the
/// first `<script src>` (already resolved through the seam). Warns on extras.
fn collect_refs(dom: &superui_dom::Dom, dir: &str, live: bool) -> (Option<String>, Option<String>) {
    fn walk(
        dom: &superui_dom::Dom,
        node: superui_dom::NodeId,
        dir: &str,
        live: bool,
        css: &mut Option<String>,
        js: &mut Option<String>,
    ) {
        match dom.tag(node) {
            Some("link") => {
                let is_sheet = dom
                    .get_attribute(node, "rel")
                    .map(|r| r.eq_ignore_ascii_case("stylesheet"))
                    .unwrap_or(false);
                if is_sheet {
                    if let Some(href) = dom.get_attribute(node, "href") {
                        if css.is_none() {
                            *css = Some(superui_paths::join_asset(dir, href));
                        } else {
                            warn!(
                                "superui: only one stylesheet is supported for now; using \
                                 the first <link rel=stylesheet> and ignoring \"{href}\""
                            );
                        }
                    }
                }
            }
            Some("script") => {
                if let Some(src) = dom.get_attribute(node, "src") {
                    if js.is_none() {
                        *js = Some(resolve_script(dir, src, live));
                    } else {
                        warn!("superui: multiple <script src>; ignoring {src}");
                    }
                }
            }
            _ => {}
        }
        for &child in dom.children(node) {
            walk(dom, child, dir, live, css, js);
        }
    }

    let mut css = None;
    let mut js = None;
    walk(dom, dom.document(), dir, live, &mut css, &mut js);
    (css, js)
}

/// Turn discovered ref paths into loaded handles (kicks off the async loads).
fn discover_subresources(
    dom: &superui_dom::Dom,
    dir: &str,
    live: bool,
    assets: &AssetServer,
) -> (Option<Handle<StyleSheet>>, Handle<JsSource>) {
    let (css_path, js_path) = collect_refs(dom, dir, live);
    let css = css_path.map(|p| assets.load::<StyleSheet>(p));
    let js = match js_path {
        Some(p) => assets.load::<JsSource>(p),
        None => {
            warn!("superui: entry HTML declares no <script src>; the UI will be inert");
            Handle::default()
        }
    };
    (css, js)
}
```

Add `NodeId` to the `superui_dom` use path if needed: the code above references `superui_dom::NodeId` and `superui_dom::Dom` by full path, so no new `use` is required. Confirm `superui_dom` is a dependency (it is).

- [ ] **Step 5: Checkpoint — do NOT run tests yet**

The crate still won't compile: `mount_when_ready` and `hot_reload` reference the now-removed `css`/`js` fields, and the harness `spawn_root` still builds the old struct. Continue straight to Steps 6–9; the first green run of `cargo test -p superui` is Step 10. (Optional sanity check now: `cargo build -p superui` will fail with errors pointing at exactly the sites Steps 6–9 fix.)

- [ ] **Step 6: Rewrite `mount_when_ready` as the two-phase loader**

In `crates/superui/src/mount.rs`, replace the entire body of `pub fn mount_when_ready(world: &mut World)` (from its opening `{` to its closing `}`) with:

```rust
    // Guard: one mounted UI at a time.
    if world.contains_non_send::<UiRuntime>() {
        return;
    }

    // The single SuperUiRoot entity + its entry-HTML handle.
    let (entity, html_handle) = {
        let mut q = world.query::<(Entity, &SuperUiRoot)>();
        let Ok((entity, root)) = q.single(world) else {
            return;
        };
        (entity, root.html.clone())
    };

    // The entry HTML must be loaded before we can read the manifest.
    if !matches!(
        world.resource::<AssetServer>().load_state(html_handle.id()),
        LoadState::Loaded
    ) {
        return;
    }

    // Phase 1: discover the declared subresources and kick off their loads.
    if world.get::<SuperUiSubresources>(entity).is_none() {
        let html_src = match world.resource::<Assets<HtmlSource>>().get(&html_handle) {
            Some(h) => h.0.clone(),
            None => return,
        };
        let dir = html_handle
            .path()
            .map(|p| superui_paths::parent_dir(&p.to_string()).to_string())
            .unwrap_or_default();
        let dom = superui_html::parse_document(&html_src);
        let assets = world.resource::<AssetServer>().clone();
        let (css, js) = discover_subresources(&dom, &dir, live_source(), &assets);
        world.entity_mut(entity).insert(SuperUiSubresources { css, js });
        return;
    }

    // Phase 2: wait for the subresources, then build the runtime.
    let (css_handle, js_handle) = {
        let sub = world.get::<SuperUiSubresources>(entity).unwrap();
        (sub.css.clone(), sub.js.clone())
    };
    {
        let server = world.resource::<AssetServer>();
        let css_ok = match &css_handle {
            Some(h) => matches!(server.load_state(h.id()), LoadState::Loaded),
            None => true,
        };
        let js_ok = matches!(server.load_state(js_handle.id()), LoadState::Loaded);
        if !(css_ok && js_ok) {
            return;
        }
    }
    let html_src = match world.resource::<Assets<HtmlSource>>().get(&html_handle) {
        Some(h) => h.0.clone(),
        None => return,
    };
    let js_src = match world.resource::<Assets<JsSource>>().get(&js_handle) {
        Some(j) => j.0.clone(),
        None => return,
    };

    // HMR gate: feature + asset watcher. Warn once if the feature is on but nothing
    // is watching, then stay off.
    let watching = world.resource::<AssetServer>().watching_for_changes();
    let hmr = hmr_active(watching);
    #[cfg(feature = "hmr")]
    if !watching {
        bevy::log::warn!(
            "superui: `hmr` feature is enabled but the AssetServer is not watching for \
             changes; state-preserving hot reload is OFF. Enable `bevy/file_watcher` to \
             activate it."
        );
    }

    let dom = Rc::new(RefCell::new(superui_html::parse_document(&html_src)));
    let mut rt = UiRuntime::new(dom, entity, css_handle.unwrap_or_default(), hmr);
    rt.run_script(&js_src);
    world.insert_non_send_resource(rt);
```

- [ ] **Step 7: Refactor `hot_reload.rs` (ids from subresources; html → remount)**

In `crates/superui/src/hot_reload.rs`:

Replace the imports block at the top (the `use std::cell::RefCell; use std::rc::Rc;` and `use crate::mount::SuperUiRoot;` lines) with:

```rust
use bevy::ecs::message::MessageReader;
use bevy::prelude::*;
use superui_bridge::UiRuntime;
use superui_css::style::StyleSheet;

use crate::assets::{HtmlSource, JsSource};
use crate::mount::{SuperUiRoot, SuperUiSubresources};
```

Replace the whole `pub fn detect_hot_reload(...)` function with:

```rust
/// Compare `AssetEvent::Modified` ids against the mounted root's handles and set
/// flags. `html` comes from `SuperUiRoot`; `js`/`css` from the discovered
/// `SuperUiSubresources` (absent until Phase 1 runs).
pub fn detect_hot_reload(
    mut html_events: MessageReader<AssetEvent<HtmlSource>>,
    mut js_events: MessageReader<AssetEvent<JsSource>>,
    mut css_events: MessageReader<AssetEvent<StyleSheet>>,
    root: Query<(&SuperUiRoot, Option<&SuperUiSubresources>)>,
    mut flags: ResMut<HotReloadFlags>,
) {
    let Ok((root, sub)) = root.single() else {
        return;
    };
    for e in html_events.read() {
        if let AssetEvent::Modified { id } = e {
            if *id == root.html.id() {
                flags.html = true;
            }
        }
    }
    for e in js_events.read() {
        if let AssetEvent::Modified { id } = e {
            if sub.map(|s| *id == s.js.id()).unwrap_or(false) {
                flags.js = true;
            }
        }
    }
    for e in css_events.read() {
        if let AssetEvent::Modified { id } = e {
            if sub.and_then(|s| s.css.as_ref()).map(|h| *id == h.id()).unwrap_or(false) {
                flags.css = true;
            }
        }
    }
}
```

Replace the whole `pub fn apply_hot_reload(world: &mut World)` function with:

```rust
/// Consume `HotReloadFlags`. HTML change → full remount (state lost): tear down the
/// runtime + `SuperUiSubresources` so `mount_when_ready` re-reads the manifest and
/// re-discovers subresources. JS/CSS change → mutate the live runtime in place.
///
/// Runs inside the chained bridge set gated by `runtime_exists`; removing the
/// runtime here makes the remaining chained systems skip (their inherited
/// `runtime_exists` condition re-evaluates to false), so the teardown is safe.
pub fn apply_hot_reload(world: &mut World) {
    let (html_changed, js_changed, _css_changed) = {
        let mut flags = world.resource_mut::<HotReloadFlags>();
        let v = (flags.html, flags.js, flags.css);
        flags.html = false;
        flags.js = false;
        flags.css = false;
        v
    };
    if !(html_changed || js_changed || _css_changed) {
        return;
    }

    // Root entity + discovered subresources (nothing to do before Phase 1 ran).
    let (entity, js_handle) = {
        let mut q = world.query::<(Entity, &SuperUiSubresources)>();
        match q.iter(world).next() {
            Some((e, sub)) => (e, sub.js.clone()),
            None => return,
        }
    };

    if html_changed {
        // Full remount: despawn the reconciled subtree, drop the runtime + marker.
        if let Some(rt) = world.remove_non_send_resource::<UiRuntime>() {
            for e in rt.bound_non_root_entities() {
                if let Ok(ec) = world.get_entity_mut(e) {
                    ec.despawn();
                }
            }
        }
        world.entity_mut(entity).remove::<SuperUiSubresources>();
        return; // mount_when_ready rebuilds next frame
    }

    // JS/CSS change: keep state, re-exec / restyle against the current DOM.
    let Some(mut rt) = world.remove_non_send_resource::<UiRuntime>() else {
        return;
    };
    if js_changed {
        if let Some(js) = world
            .resource::<Assets<JsSource>>()
            .get(&js_handle)
            .map(|j| j.0.clone())
        {
            rt.run_script(&js);
        }
    }
    rt.dirty = true; // CSS-only change still needs a reconcile pass.
    world.insert_non_send_resource(rt);
}
```

- [ ] **Step 8: Update the test harness `spawn_root` to Model 2**

In `crates/superui/tests/support/mod.rs`:

Change the import line `use superui::{HtmlSource, JsSource, SuperUiPlugin, SuperUiRoot};` to:

```rust
use superui::{HtmlSource, SuperUiPlugin, SuperUiRoot};
```

Remove the now-unused `use superui_css::style::StyleSheet;` line.

Replace the whole `pub fn spawn_root(...)` with three items — a doc builder, the
auto-named `spawn_root`, and a fixed-entry variant for reload tests:

```rust
/// Build a Model-2 entry document: a `<head>` that links `css` and scripts `js`
/// (both in-memory asset paths at the root), with `body` as the document body.
pub fn entry_doc(body: &str, css: &str, js: &str) -> String {
    format!(
        "<html><head><link rel=\"stylesheet\" href=\"{css}\">\
         <script src=\"{js}\"></script></head><body>{body}</body></html>"
    )
}

/// Spawn a Model-2 `SuperUiRoot` from a freshly synthesized, uniquely-named entry.
pub fn spawn_root(app: &mut App, body: &str, css: &str, js: &str) -> Entity {
    static SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    spawn_root_entry(app, &format!("index_{n}.html"), body, css, js)
}

/// Like [`spawn_root`] but with a caller-chosen entry asset path, so a test can
/// re-`put(entry, entry_doc(...))` later to exercise remount-on-html-change.
pub fn spawn_root_entry(app: &mut App, entry: &str, body: &str, css: &str, js: &str) -> Entity {
    put(entry, entry_doc(body, css, js).as_bytes());
    let server = app.world().resource::<AssetServer>().clone();
    let root = SuperUiRoot { html: server.load::<HtmlSource>(entry.to_string()) };
    app.world_mut().spawn((Node::default(), root)).id()
}
```

- [ ] **Step 9: Update the 4 `spawn_root` call sites in `integration.rs`**

Each site currently does `put("<name>.html", BODY)` then `spawn_root(&mut app, "<name>.html", "<name>.css", "<name>.js")`. The css/js `put(...)` lines stay unchanged throughout.

**Three non-reload tests** (`t8`, `t9`, `cap`) — delete the `put("<name>.html", …)` line and pass the body bytes as the first `spawn_root` arg:

- `put("t8.html", b"<ul id='list'></ul>");` → delete; and
  `spawn_root(&mut app, "t8.html", "t8.css", "t8.js")` → `spawn_root(&mut app, "<ul id='list'></ul>", "t8.css", "t8.js")`
- `put("t9.html", b"<div id='host'></div>");` → delete; and
  `spawn_root(&mut app, "t9.html", "t9.css", "t9.js")` → `spawn_root(&mut app, "<div id='host'></div>", "t9.css", "t9.js")`
- `put("cap.html", b"<button id='add'>Add</button><ul id='list'></ul>");` → delete; and
  `spawn_root(&mut app, "cap.html", …)` → `spawn_root(&mut app, "<button id='add'>Add</button><ul id='list'></ul>", "cap.css", "cap.js")`

**The reload test** `html_hot_reload_despawns_old_entities_no_leak` — this is the **remount coverage** (spec's "index.html remount" test). It must keep a valid manifest across the mutation, so route it through the fixed-entry helper:

- Delete `put("leak.html", b"<ul id='h'></ul>");`.
- Change `let _root = spawn_root(&mut app, "leak.html", "leak.css", "leak.js");` to:
  ```rust
  let _root = spawn_root_entry(&mut app, "leak.html", "<ul id='h'></ul>", "leak.css", "leak.js");
  ```
- The in-place HTML mutation currently sets `h.0 = "<ul id='h'></ul>".to_string();` — replace the entry document, not a bare body, so the re-parsed manifest still declares its subresources:
  ```rust
  h.0 = entry_doc("<ul id='h'></ul>", "leak.css", "leak.js");
  ```
- Leave the JS in-place mutation and the `write_message(AssetEvent::Modified { id: html_handle.id() })` untouched — they still trigger the reload; under Model 2 that fires the full remount (teardown → `mount_when_ready` re-discovers → rebuild → re-run the mutated `leak.js`).
- Bump the final `tick(&mut app, 8);` to `tick(&mut app, 12);` (remount adds ~2 frames).

Ensure `spawn_root_entry` and `entry_doc` are in scope the same way `spawn_root`/`put`/`tick` already are (the file's existing `support` glob import covers them).

- [ ] **Step 10: Run the full superui test suite**

Run: `cargo test -p superui`
Expected: PASS — the model2 unit tests, the existing `hmr_gate_tests`, and all `integration.rs` tests (DOM structure, hot-reload leak, capture). If an integration test times out waiting to mount, increase its `tick(&mut app, N)` count by a few frames (two-phase load adds one frame).

- [ ] **Step 11: Commit**

```bash
git add crates/superui/Cargo.toml crates/superui/src/mount.rs crates/superui/src/hot_reload.rs crates/superui/tests
git commit -m "feat(superui): HTML-as-manifest entry, two-phase mount, remount-on-html-change"
```

---

### Task 4: Migrate the `counter` example (reference)

**Files:**
- Modify: `examples/counter/assets/ui/counter/index.html`
- Modify: `examples/counter/src/main.rs`
- Modify: `examples/counter/build.rs`
- Modify: `examples/counter/.gitignore`
- Delete (on disk): `examples/counter/assets/ui/counter/app.generated.js`

**Interfaces:**
- Consumes: `SuperUiRoot::from_asset_dir`, `supersolid::build::transpile_dir` (Tasks 3, 2).

- [ ] **Step 1: Rewrite the entry HTML as a manifest**

Replace the entire contents of `examples/counter/assets/ui/counter/index.html`:

```html
<html>
  <head>
    <link rel="stylesheet" href="style.css">
    <script type="module" src="app.tsx"></script>
  </head>
  <body>
    <div id="root"></div>
  </body>
</html>
```

- [ ] **Step 2: Slim `main.rs`**

Replace the entire contents of `examples/counter/src/main.rs`:

```rust
//! The smallest Supersolid app: a single button that counts its own clicks,
//! authored in Solid-style `.tsx` and mounted from a web-like `index.html`.
//!
//! - `cargo run -p counter --features hmr` — native, live `.tsx` via the
//!   transpiling asset loader, state-preserving hot reload.
//! - `cargo run -p counter` — native, loads the pre-transpiled
//!   `.superui/build/app.js` (build.rs output); no HMR.
//! - `cargo build -p counter --target wasm32-unknown-unknown` — web build.

use bevy::prelude::*;
use superui::prelude::{SuperUiPlugin, SuperUiRoot};

/// On the web, bind the primary window to the host page's canvas. Identity on native.
fn web_window(window: bevy::window::Window) -> bevy::window::Window {
    #[cfg(target_arch = "wasm32")]
    let window = bevy::window::Window {
        canvas: Some("#superui-canvas".into()),
        fit_canvas_to_parent: true,
        ..window
    };
    window
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(web_window(Window::default())),
            ..default()
        }))
        .add_plugins(SuperUiPlugin)
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    commands.spawn(Camera2d);
    commands.spawn(SuperUiRoot::from_asset_dir("ui/counter", &assets));
}
```

(The `AssetPlugin` watch override and `USE_LIVE_TSX` are gone: the `hmr` feature pulls `bevy/file_watcher`, which enables watching by default; superui no longer touches the global watcher.)

- [ ] **Step 3: One-line the build script**

Replace the entire contents of `examples/counter/build.rs`:

```rust
//! Pre-transpile the counter's `.tsx` to `.superui/build/*.js` for wasm / no-HMR
//! native builds. Build scripts run on the HOST, so `oxc` never enters the wasm
//! binary. Skips itself under `--features hmr` (that build loads live `.tsx`).
fn main() {
    supersolid::build::transpile_dir("assets/ui/counter");
}
```

- [ ] **Step 4: Update `.gitignore`**

Replace the entire contents of `examples/counter/.gitignore`:

```gitignore
# Build-time generated JS (wasm / no-HMR native path).
**/.superui/
superui_modules/
```

- [ ] **Step 5: Remove the stale generated file**

```bash
rm -f examples/counter/assets/ui/counter/app.generated.js
```

- [ ] **Step 6: Build both native paths + verify a real window**

Run: `cargo build -p counter`
Expected: builds; `examples/counter/assets/ui/counter/.superui/build/app.js` now exists (build.rs output).

Run: `cargo build -p counter --features hmr`
Expected: builds (transpile is skipped by the `CARGO_FEATURE_HMR` early-out).

Then verify the app actually boots (green tests do NOT prove this — they run on big-stack worker threads; the windowed app parses `render.js` on the ~8MB main stack configured in `.cargo/config.toml`):

Run: `cargo run -p counter`
Expected: a window opens showing the "clicked 0 times" button; clicking increments it.

- [ ] **Step 7: Commit**

```bash
git add examples/counter
git commit -m "feat(counter): migrate to HTML-as-manifest + from_asset_dir; .superui/build output"
```

---

### Task 5: Migrate the remaining five examples

**Files (per example `E` with ui-dir `D` and stylesheet `S`):**
- Modify: `examples/E/assets/ui/D/index.html`
- Modify: the file that spawns `SuperUiRoot` (see table)
- Modify: `examples/E/build.rs` (supersolid examples only)
- Modify: `examples/E/.gitignore`
- Delete (on disk): `examples/E/assets/ui/D/app.generated.js` (supersolid examples only)

**Per-example table:**

| Example (`E`) | ui-dir (`D`) | stylesheet (`S`) | script `src` | spawn file | has build.rs |
|---|---|---|---|---|---|
| `game_menu` | `game_menu` | `style.css` | `app.tsx` | `src/main.rs` | yes |
| `todomvc_supersolid` | `todomvc_supersolid` | `style.css` | `app.tsx` | `src/main.rs` | yes |
| `citadel` | `citadel` | `theme.css` | `app.tsx` | `src/ui/supersolid/mod.rs` | yes |
| `horde` | `horde` | `theme.css` | `app.tsx` | `src/ui/supersolid/mod.rs` | yes |
| `todomvc` (classic) | `todomvc` | `style.css` | `app.js` | `src/main.rs` | no |

**Interfaces:** Consumes `SuperUiRoot::{from_asset_dir, from_asset_dir_with}`, `supersolid::build::transpile_dir`.

- [ ] **Step 1: Rewrite each `index.html` as a manifest**

For each example, wrap the existing body fragment in a document that declares its stylesheet and script. The five supersolid examples currently contain `<div id="root"></div>`; `todomvc` contains its `<div id="app">…</div>` fragment (preserve it verbatim as the body).

Template (substitute `S` and the script `src` from the table):

```html
<html>
  <head>
    <link rel="stylesheet" href="S">
    <script type="module" src="app.tsx"></script>
  </head>
  <body>
    <!-- existing body fragment for this example -->
  </body>
</html>
```

For `todomvc`, use `href="style.css"` and `src="app.js"`, and put the existing `<div id="app">…</div>` block (from `examples/todomvc/assets/ui/todomvc/index.html`) inside `<body>`.

- [ ] **Step 2: Convert each spawn site**

In each spawn file, delete the `const USE_LIVE_TSX …` line, the `let js: Handle<JsSource> = if USE_LIVE_TSX { … } else { … };` selection, and the unused `use superui_css::style::StyleSheet;` / `Handle<JsSource>` imports. Replace the `commands.spawn((Node { … }, SuperUiRoot { html, css, js }))` tuple with a builder call:

- If the example's existing root `Node` was full-fill (`width: 100%`, `height: 100%`), use:
  ```rust
  commands.spawn(SuperUiRoot::from_asset_dir("ui/D", &assets));
  ```
- Otherwise preserve the exact layout with the `_with` variant:
  ```rust
  commands.spawn(SuperUiRoot::from_asset_dir_with("ui/D", Node { /* the existing literal */ }, &assets));
  ```

Check each `Node { … }` literal before choosing; `game_menu`, `citadel`, and `horde` set an explicit `Node` — keep it via `from_asset_dir_with` unless it is exactly the full-fill node. `todomvc_supersolid` and `todomvc` use `Node::default()`/fill — use `from_asset_dir`.

- [ ] **Step 3: Remove the watch coupling**

In `game_menu`, `todomvc_supersolid`, `citadel`, and `horde`, find `watch_for_changes_override: Some(USE_LIVE_TSX)` (in `main.rs`; for `citadel`/`horde` it is in their top-level `src/main.rs`, not the `supersolid/mod.rs`). Remove that override — if the `AssetPlugin` was customized *only* for the watch flag, drop the `.set(AssetPlugin { … })` entirely so `DefaultPlugins` uses its default `AssetPlugin`. Leave any `WindowPlugin` `.set(...)` intact.

Leave `todomvc`'s `watch_for_changes_override: Some(true)` **as-is** — that is the classic app's deliberate choice to watch its own assets, not the feature-coupling we are removing.

- [ ] **Step 4: One-line each supersolid `build.rs`**

For `game_menu`, `todomvc_supersolid`, `citadel`, `horde`, replace `build.rs` with (substitute `D`):

```rust
//! Pre-transpile this example's `.tsx` to `.superui/build/*.js` for wasm / no-HMR
//! native builds. Runs on the HOST; skips under `--features hmr`.
fn main() {
    supersolid::build::transpile_dir("assets/ui/D");
}
```

`todomvc` has no `build.rs` — leave it that way.

- [ ] **Step 5: Update each `.gitignore` and delete stale artifacts**

For every migrated example, ensure `.gitignore` ignores `**/.superui/` and no longer lists `app.generated.js`. For the supersolid examples, delete the stale file:

```bash
rm -f examples/game_menu/assets/ui/game_menu/app.generated.js
rm -f examples/todomvc_supersolid/assets/ui/todomvc_supersolid/app.generated.js
rm -f examples/citadel/assets/ui/citadel/app.generated.js
rm -f examples/horde/assets/ui/horde/app.generated.js
```

- [ ] **Step 6: Build every example + the whole workspace**

Run each, expecting a clean build:

```bash
cargo build -p game_menu
cargo build -p todomvc_supersolid
cargo build -p citadel
cargo build -p horde
cargo build -p todomvc
cargo build --workspace
```

Expected: all green. Then spot-check one supersolid app and the classic app in real windows:

```bash
cargo run -p todomvc_supersolid
cargo run -p todomvc
```

Expected: each opens and is interactive (supersolid TodoMVC adds/toggles items; classic TodoMVC likewise).

- [ ] **Step 7: Commit**

```bash
git add examples/game_menu examples/todomvc_supersolid examples/citadel examples/horde examples/todomvc
git commit -m "feat(examples): migrate game_menu/todomvc_supersolid/citadel/horde/todomvc to manifest entry"
```

---

### Task 6: Final verification + spec/plan commit

**Files:**
- (No source changes) — commit the design + plan docs.

- [ ] **Step 1: Full workspace test + build**

Run: `cargo test --workspace`
Expected: green.

Run: `cargo build --workspace`
Expected: green.

- [ ] **Step 2: Confirm the wasm path compiles for one example**

Run: `cargo build -p counter --target wasm32-unknown-unknown`
Expected: builds (loads the generated `.superui/build/app.js`; `oxc`/`TsxLoader` are absent on wasm). If the target isn't installed, note it and skip rather than adding a toolchain.

- [ ] **Step 3: Commit the spec and plan**

```bash
git add docs/superpowers/specs/2026-07-24-supersolid-example-boilerplate-design.md docs/superpowers/plans/2026-07-24-supersolid-example-boilerplate.md
git commit -m "docs: supersolid example boilerplate reduction spec + plan"
```

---

## Notes for the implementer

- **Two-phase timing:** mount now takes one extra frame (Phase 1 discovers, Phase 2 builds). Headless integration tests that `tick(&mut app, N)` may need `N + 2`.
- **`Handle::path()`** returns `Option<&AssetPath>`; `AssetPath` is `Display`. `parent_dir(&p.to_string())` yields the asset dir with forward slashes — the correct, Windows-safe form.
- **Removing the runtime mid-chain is intentional** in `apply_hot_reload`'s remount path; the chained bridge set's `run_if(runtime_exists)` makes later systems skip that frame, and `mount_when_ready` rebuilds next frame.
- **Do not** reintroduce a `css: Vec` through `UiRuntime` — flair is one-sheet-per-node; multi-`<link>` is future work and out of scope.
