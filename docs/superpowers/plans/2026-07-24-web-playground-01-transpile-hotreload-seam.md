# Web Playground Part 1: In-Browser Transpile + Hot-Reload Seam — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make a wasm playground build of a supersolid demo re-transpile edited `.tsx`/`.css` in the browser and hot-swap the running UI (signal state preserved), returning diagnostics + JS errors to the page.

**Architecture:** Add a `transpiler` feature that lets `oxc` (via `supersolid`) into a wasm build; a new wasm-capable crate `superui_playground_web` that takes edited source from JS, transpiles/parses it, and drives the *existing* `detect_hot_reload`/`apply_hot_reload` seam by overwriting the mounted asset and firing `AssetEvent::Modified`. All new logic lives in native-testable functions/systems; wasm-bindgen exports are thin wrappers.

**Tech Stack:** Rust, Bevy 0.19, `oxc` 0.140 (via `supersolid`), the vendored `superui_flair_*` 0.8 CSS engine, `wasm-bindgen`, Boa (JS engine).

**Spec:** `docs/superpowers/specs/2026-07-24-web-playground-01-transpile-hotreload-seam-design.md`

## Global Constraints

- **Model 2 is already implemented** on `main`: `SuperUiRoot { html }`, `SuperUiSubresources { css: Option<Handle<StyleSheet>>, js: Handle<JsSource> }`, two-phase `mount_when_ready`, `detect_hot_reload` sourcing js/css ids from `SuperUiSubresources`. Do not reshape these; extend them.
- **Preserve today's oxc placement.** Native builds link `oxc` (unchanged); normal wasm gallery builds must stay oxc-free. Only a `transpiler`-feature wasm build links oxc.
- **Bevy 0.19 non-send API:** `world.insert_non_send(x)`, `world.remove_non_send::<T>()`, `world.contains_non_send::<T>()`, `world.non_send_mut::<T>()`. Not the `*_resource` spellings.
- **`get_mut` may not emit `AssetEvent::Modified` in this Bevy version.** After every `Assets::<T>::get_mut`, also `world.write_message(AssetEvent::Modified { id })` explicitly (the existing tests do this — see `crates/superui/tests/integration.rs:67`).
- **flair CSS-string parser:** `superui_css::parser::InlineCssStyleSheetParser` (a `#[derive(SystemParam)]`) with `load_stylesheet(&self, &str) -> Result<StyleSheet, CssStyleLoaderError>`. `StyleSheet` is `superui_css::style::StyleSheet`.
- **`oxc` version stays `0.140`** (workspace `Cargo.toml`); do not bump it.
- **No worktrees** (`target/` is huge — per `CLAUDE.md`). Land on `main`.

## Review Focus

- **Broken `.tsx` on Run:** `apply_source("app.tsx", garbage)` must return `{ok:false, diagnostics:[…]}` and must NOT panic or poison the queue — the running UI keeps working. (Test in Task 3.)
- **Malformed `.css` on Run:** a CSS parse error must push a diagnostic and leave the current stylesheet in place, not blank the UI or panic. (Test in Task 5.)
- **Edit for a file that isn't the mounted entry/subresource** (e.g. an unknown path): `apply_source` must no-op gracefully (drop or diagnostic), never overwrite the wrong asset. (Test in Task 4.)
- **Run before the UI has mounted** (assets still loading): a queued edit that arrives before `SuperUiSubresources` exists must be dropped or deferred without panicking. (Test in Task 4.)
- **`get_mut` not emitting `Modified`:** the drain system must fire the event explicitly so the reload actually happens; a test must fail if the explicit fire is removed. (Test in Task 4.)

---

## File Structure

- `crates/superui/Cargo.toml` — add wasm-optional `supersolid` dep + `transpiler` feature (Task 1).
- `crates/superui/src/assets.rs` — widen `TsxLoader` cfg gate (Task 1).
- `crates/superui/src/lib.rs` — widen the `TsxLoader` re-export cfg gate (Task 1).
- `crates/superui/src/mount.rs` — widen `TsxLoader` registration cfg gate; extend `live_source()` (Task 1).
- `crates/superui_bridge/src/runtime.rs` — add `errors` accumulation + `take_errors()` (Task 2).
- `crates/superui_bridge/tests/errors.rs` — new test (Task 2).
- `crates/superui_playground_web/Cargo.toml` — new crate (Task 3).
- `crates/superui_playground_web/src/lib.rs` — `Edit`, queue, diagnostics sink, `apply_source_inner`/`poll_diagnostics_inner`, `PlaygroundBridgePlugin`, `drain_playground_edits`, wasm-bindgen exports (Tasks 3–8).
- `crates/superui_playground_web/tests/support/mod.rs` — native harness (Task 4).
- `crates/superui_playground_web/tests/js_edit.rs`, `css_edit.rs`, `html_edit.rs`, `diagnostics.rs` — integration tests (Tasks 4–7).
- `examples/counter/Cargo.toml`, `examples/counter/src/main.rs` — `playground` feature wiring (Task 8).
- `examples/counter/web-playground.html` — proof harness page (Task 8).

---

## Task 1: `transpiler` feature — let oxc into a wasm build

**Files:**
- Modify: `crates/superui/Cargo.toml`
- Modify: `crates/superui/src/assets.rs` (the two `#[cfg(not(target_arch = "wasm32"))]` on `TsxLoader`)
- Modify: `crates/superui/src/lib.rs:9-10` (the `TsxLoader` re-export gate)
- Modify: `crates/superui/src/mount.rs:65-67` (`live_source`) and `:161-162` (registration gate)

**Interfaces:**
- Produces: cargo feature `superui/transpiler`; `live_source()` returns `true` on `wasm32 + transpiler`; `TsxLoader` compiled & registered whenever `any(not(wasm32), feature = "transpiler")`.

- [ ] **Step 1: Widen the `TsxLoader` cfg in `assets.rs`.** Replace both `#[cfg(not(target_arch = "wasm32"))]` attributes on `pub struct TsxLoader;` and `impl AssetLoader for TsxLoader` with:

```rust
#[cfg(any(not(target_arch = "wasm32"), feature = "transpiler"))]
```

- [ ] **Step 2: Widen the re-export in `lib.rs`.** Change:

```rust
#[cfg(not(target_arch = "wasm32"))]
pub use assets::TsxLoader;
```
to
```rust
#[cfg(any(not(target_arch = "wasm32"), feature = "transpiler"))]
pub use assets::TsxLoader;
```

- [ ] **Step 3: Widen the registration in `mount.rs`.** Change the two lines at `mount.rs:161-162`:

```rust
#[cfg(not(target_arch = "wasm32"))]
app.register_asset_loader(crate::assets::TsxLoader);
```
to
```rust
#[cfg(any(not(target_arch = "wasm32"), feature = "transpiler"))]
app.register_asset_loader(crate::assets::TsxLoader);
```

- [ ] **Step 4: Extend `live_source()` in `mount.rs`.** Replace the body:

```rust
pub(crate) fn live_source() -> bool {
    cfg!(all(not(target_arch = "wasm32"), feature = "hmr"))
        || cfg!(all(target_arch = "wasm32", feature = "transpiler"))
}
```

- [ ] **Step 5: Declare the feature + wasm dep in `crates/superui/Cargo.toml`.** Under `[features]` add `transpiler = ["dep:supersolid"]`. Add a wasm-target optional dep (the native `supersolid` dep stays as-is, unconditional):

```toml
[target.'cfg(target_arch = "wasm32")'.dependencies]
supersolid = { path = "../supersolid", optional = true }
```

Note: if `supersolid` is currently only a `[target.'cfg(not(target_arch = "wasm32"))'.dependencies]` entry, keep that entry unchanged and add the wasm one above it. `dep:supersolid` in the feature refers to whichever target-scoped dep is active.

- [ ] **Step 6: Verify native is unchanged and still builds.**

Run: `cargo build -p superui`
Expected: builds; no behavior change (TsxLoader still present on native).

- [ ] **Step 7: Verify the wasm gate — oxc stays out without the feature, comes in with it.**

Run: `cargo build -p superui --target wasm32-unknown-unknown`
Expected: builds; `supersolid`/`oxc` NOT in the dependency graph.

Run: `cargo tree -p superui --target wasm32-unknown-unknown -i oxc`
Expected: `oxc` not found (error "package ID specification `oxc` did not match any packages") — confirms oxc absent.

Run: `cargo build -p superui --target wasm32-unknown-unknown --features transpiler`
Expected: builds; oxc compiled in.

Run: `cargo tree -p superui --target wasm32-unknown-unknown --features transpiler -i oxc`
Expected: `oxc v0.140.0` appears — confirms oxc present only with the feature.

- [ ] **Step 8: Run the existing seam unit tests (unchanged behavior).**

Run: `cargo test -p superui --lib model2_tests`
Expected: PASS (resolve_script/collect_refs still green — the `live` param path is unaffected).

- [ ] **Step 9: Commit.**

```bash
git add crates/superui/Cargo.toml crates/superui/src/assets.rs crates/superui/src/lib.rs crates/superui/src/mount.rs
git commit -m "feat(superui): transpiler feature to compile oxc into wasm playground builds"
```

---

## Task 2: `UiRuntime` error sink + `take_errors()`

**Files:**
- Modify: `crates/superui_bridge/src/runtime.rs` (struct `UiRuntime` ~line 74; `run_script` ~line 205 where it already `warn!`s the error; add method)
- Test: `crates/superui_bridge/tests/errors.rs` (new)

**Interfaces:**
- Consumes: `UiRuntime::new(dom, root, stylesheet, hmr) `, `run_script(&mut self, &str)` (existing).
- Produces: `UiRuntime::take_errors(&mut self) -> Vec<String>` — returns and clears JS eval errors captured since the last call.

- [ ] **Step 1: Write the failing test** in `crates/superui_bridge/tests/errors.rs`:

```rust
//! `UiRuntime` captures uncaught JS eval errors and hands them out via take_errors.
use std::cell::RefCell;
use std::rc::Rc;

use bevy::prelude::*;
use superui_bridge::UiRuntime;

mod support;
use support::{mount, test_app};

#[test]
fn run_script_error_is_captured_and_drained() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document("<div id='root'></div>")));
    let mut app = test_app();
    let _root = mount(&mut app, dom);

    // A throwing top-level statement.
    app.world_mut()
        .non_send_mut::<UiRuntime>()
        .run_script("throw new Error('boom');");

    let errs = app.world_mut().non_send_mut::<UiRuntime>().take_errors();
    assert!(errs.iter().any(|e| e.contains("boom")), "error captured: {errs:?}");

    // Draining clears them.
    let again = app.world_mut().non_send_mut::<UiRuntime>().take_errors();
    assert!(again.is_empty(), "second drain is empty: {again:?}");

    // A clean script leaves no errors.
    app.world_mut()
        .non_send_mut::<UiRuntime>()
        .run_script("var x = 1 + 1;");
    assert!(app.world_mut().non_send_mut::<UiRuntime>().take_errors().is_empty());
}
```

- [ ] **Step 2: Run it to verify it fails.**

Run: `cargo test -p superui_bridge --test errors`
Expected: FAIL — `take_errors` does not exist.

- [ ] **Step 3: Add the `errors` field.** In `crates/superui_bridge/src/runtime.rs`, add to `struct UiRuntime` a field `errors: Vec<String>,` and initialize it to `Vec::new()` in `UiRuntime::new`.

- [ ] **Step 4: Capture at the existing catch site.** In `run_script`, where it currently does `warn!("superui: JS error: {e}")` (~line 210), also push the message:

```rust
warn!("superui: JS error: {e}");
self.errors.push(format!("{e}"));
```

- [ ] **Step 5: Add the accessor.** Add to `impl UiRuntime`:

```rust
/// Return and clear JS eval errors captured since the last call.
pub fn take_errors(&mut self) -> Vec<String> {
    core::mem::take(&mut self.errors)
}
```

- [ ] **Step 6: Run the test to verify it passes.**

Run: `cargo test -p superui_bridge --test errors`
Expected: PASS.

- [ ] **Step 7: Confirm no regressions in the bridge crate.**

Run: `cargo test -p superui_bridge`
Expected: PASS.

- [ ] **Step 8: Commit.**

```bash
git add crates/superui_bridge/src/runtime.rs crates/superui_bridge/tests/errors.rs
git commit -m "feat(superui_bridge): capture JS eval errors via UiRuntime::take_errors"
```

---

## Task 3: `superui_playground_web` crate + `apply_source_inner` (TSX transpile + enqueue)

**Files:**
- Create: `crates/superui_playground_web/Cargo.toml`
- Create: `crates/superui_playground_web/src/lib.rs`

**Interfaces:**
- Consumes: `supersolid::transpile(&str, &TranspileOptions) -> { code: String, diagnostics: Vec<Diagnostic> }` (see `crates/supersolid/src/lib.rs` for the exact `TranspileOptions` / `Diagnostic` shapes).
- Produces:
  - `enum Edit { Js(String), Css(String), Html(String) }`
  - `pub fn apply_source_inner(path: &str, src: &str) -> String` — returns JSON `{"ok":bool,"diagnostics":[{"severity":"…","message":"…"}]}`, enqueues one `Edit`.
  - `pub fn drain_queue() -> Vec<Edit>` (test helper; drains the thread-local queue).
  - `pub fn push_diag(msg: String)` / `pub fn poll_diagnostics_inner() -> String` (diagnostics sink; JSON array `[{"message":"…"}]`).

- [ ] **Step 1: Create the crate manifest** `crates/superui_playground_web/Cargo.toml`:

```toml
[package]
name = "superui_playground_web"
edition.workspace = true
version.workspace = true
license.workspace = true
publish = false

[dependencies]
superui = { path = "../superui", features = ["transpiler", "hmr"] }
superui_bridge = { path = "../superui_bridge" }
superui_css = { path = "../superui_css" }
supersolid = { path = "../supersolid" }
bevy = { workspace = true, default-features = false, features = ["std", "bevy_ui", "bevy_text"] }
serde_json = "1"

[target.'cfg(target_arch = "wasm32")'.dependencies]
wasm-bindgen = "0.2"

[dev-dependencies]
# native integration harness mirrors crates/superui/tests/support
superui_html = { path = "../superui_html" }
superui_dom = { path = "../superui_dom" }
```

(The workspace `members = ["crates/*", …]` auto-includes this crate; no workspace edit needed. Match the exact `bevy` feature list `crates/superui/Cargo.toml` uses if this list under-includes; the test harness needs `UiPlugin`, added in the harness, not here.)

- [ ] **Step 2: Write the failing test** at the bottom of `crates/superui_playground_web/src/lib.rs`:

```rust
#[cfg(test)]
mod apply_source_tests {
    use super::*;

    #[test]
    fn tsx_transpiles_and_enqueues_js() {
        let _ = drain_queue(); // isolate
        let out = apply_source_inner("app.tsx", "const n: number = 1; const a = <div>{n}</div>;");
        assert!(out.contains("\"ok\":true"), "ok result: {out}");
        let edits = drain_queue();
        match edits.as_slice() {
            [Edit::Js(code)] => {
                assert!(code.contains("$ss.el(\"div\")"), "JSX lowered: {code}");
                assert!(!code.contains(": number"), "types stripped: {code}");
            }
            other => panic!("expected one Edit::Js, got {other:?}"),
        }
    }

    #[test]
    fn broken_tsx_returns_not_ok_without_panicking() {
        let _ = drain_queue();
        let out = apply_source_inner("app.tsx", "const a = <div>{  ;");
        // Never panics; reports the problem. (supersolid degrades gracefully, so
        // it may still enqueue partial JS — the contract is only: no panic + a report.)
        assert!(out.contains("\"diagnostics\""), "diagnostics present: {out}");
    }

    #[test]
    fn css_and_html_enqueue_raw() {
        let _ = drain_queue();
        apply_source_inner("style.css", "div { color: red }");
        apply_source_inner("index.html", "<html></html>");
        let edits = drain_queue();
        assert!(matches!(edits[0], Edit::Css(_)));
        assert!(matches!(edits[1], Edit::Html(_)));
    }
}
```

- [ ] **Step 3: Run it to verify it fails.**

Run: `cargo test -p superui_playground_web --lib`
Expected: FAIL — crate/functions do not exist.

- [ ] **Step 4: Implement the queue, sink, and `apply_source_inner`** in `crates/superui_playground_web/src/lib.rs` (top of file):

```rust
//! Browser "watcher" for the superui web playground: takes edited source from JS,
//! transpiles/parses it, and drives the existing superui hot-reload seam. All logic
//! is native-testable; the wasm-bindgen exports are thin wrappers (see bottom).

use std::cell::RefCell;

#[derive(Debug)]
pub enum Edit {
    Js(String),
    Css(String),
    Html(String),
}

thread_local! {
    static QUEUE: RefCell<Vec<Edit>> = const { RefCell::new(Vec::new()) };
    static DIAGS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// Drain the pending edits (used by the Bevy drain system and by tests).
pub fn drain_queue() -> Vec<Edit> {
    QUEUE.with(|q| core::mem::take(&mut *q.borrow_mut()))
}

/// Record a runtime/parse diagnostic for the console to poll.
pub fn push_diag(msg: String) {
    DIAGS.with(|d| d.borrow_mut().push(msg));
}

/// JSON array of diagnostics recorded since the last poll.
pub fn poll_diagnostics_inner() -> String {
    let msgs = DIAGS.with(|d| core::mem::take(&mut *d.borrow_mut()));
    let arr: Vec<_> = msgs.into_iter().map(|m| serde_json::json!({ "message": m })).collect();
    serde_json::Value::Array(arr).to_string()
}

/// Classify an edited file, transpile `.tsx`/`.ts` synchronously, enqueue one Edit,
/// and return `{ok, diagnostics}` JSON. CSS/HTML parse errors surface later via poll.
pub fn apply_source_inner(path: &str, src: &str) -> String {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".tsx") || lower.ends_with(".ts") {
        let tsx = !lower.ends_with(".ts");
        let opts = supersolid::TranspileOptions {
            tsx,
            module_id: Some(path.to_string()),
            ..Default::default()
        };
        let result = supersolid::transpile(src, &opts);
        let diags: Vec<_> = result
            .diagnostics
            .iter()
            .map(|d| serde_json::json!({ "severity": format!("{:?}", d.severity), "message": d.message }))
            .collect();
        let ok = diags.is_empty();
        QUEUE.with(|q| q.borrow_mut().push(Edit::Js(result.code)));
        serde_json::json!({ "ok": ok, "diagnostics": diags }).to_string()
    } else if lower.ends_with(".css") {
        QUEUE.with(|q| q.borrow_mut().push(Edit::Css(src.to_string())));
        serde_json::json!({ "ok": true, "diagnostics": [] }).to_string()
    } else if lower.ends_with(".html") {
        QUEUE.with(|q| q.borrow_mut().push(Edit::Html(src.to_string())));
        serde_json::json!({ "ok": true, "diagnostics": [] }).to_string()
    } else {
        serde_json::json!({ "ok": false, "diagnostics": [{ "severity": "Error", "message": format!("unsupported file: {path}") }] }).to_string()
    }
}
```

Note: verify the real field names against `crates/supersolid/src/lib.rs` — `TranspileOptions { tsx, module_id, .. }`, and `Diagnostic { severity, message }`. Adjust the mapping if a field differs. (The `TsxLoader` in `crates/superui/src/assets.rs:86-96` shows the exact usage to copy.)

- [ ] **Step 5: Run the test to verify it passes.**

Run: `cargo test -p superui_playground_web --lib`
Expected: PASS.

- [ ] **Step 6: Commit.**

```bash
git add crates/superui_playground_web/Cargo.toml crates/superui_playground_web/src/lib.rs
git commit -m "feat(superui_playground_web): apply_source_inner transpiles TSX and queues edits"
```

---

## Task 4: `drain_playground_edits` + `PlaygroundBridgePlugin` — JS branch

**Files:**
- Modify: `crates/superui_playground_web/src/lib.rs` (add plugin + system)
- Create: `crates/superui_playground_web/tests/support/mod.rs`
- Create: `crates/superui_playground_web/tests/js_edit.rs`

**Interfaces:**
- Consumes: `superui::{SuperUiPlugin, SuperUiRoot, JsSource}`; `superui_bridge::UiRuntime`; `drain_queue()`; the crate's `Edit`.
- Produces: `pub struct PlaygroundBridgePlugin;` (adds `drain_playground_edits` to `Update`, ordered `before` superui's `detect_hot_reload`). The mounted-subresource handles are read via the `pub(crate)`… — because `SuperUiSubresources` is `pub(crate)` in superui, the system instead reads the js handle from the `AssetServer` path it re-derives; see Step 4.

- [ ] **Step 1: Create the native harness** `crates/superui_playground_web/tests/support/mod.rs` (mirrors `crates/superui/tests/support/mod.rs`, adds the playground plugin + a watch override so HMR rehydration is active):

```rust
#![allow(dead_code)]
use bevy::asset::io::memory::{Dir, MemoryAssetReader};
use bevy::asset::io::{AssetSourceBuilder, AssetSourceId};
use bevy::input::InputPlugin;
use bevy::input_focus::{InputFocus, InputFocusVisible};
use bevy::picking::{InteractionPlugin, PickingPlugin};
use bevy::image::TextureAtlasPlugin;
use bevy::prelude::*;
use bevy::text::TextPlugin;
use bevy::ui::UiPlugin;
use std::sync::LazyLock;
use superui::{HtmlSource, SuperUiPlugin, SuperUiRoot};
use superui_playground_web::PlaygroundBridgePlugin;

pub static ASSETS: LazyLock<Dir> = LazyLock::new(|| Dir::new("assets".into()));

pub fn put(name: &str, bytes: &[u8]) {
    ASSETS.insert_asset(name.as_ref(), bytes.to_vec());
}

pub fn app() -> App {
    let mut app = App::new();
    app.register_asset_source(
        AssetSourceId::Default,
        AssetSourceBuilder::new(move || Box::new(MemoryAssetReader { root: ASSETS.clone() })),
    );
    // watch override ON so hmr_active() is true → state-preserving rehydration,
    // exactly as a real playground build configures itself.
    let asset_plugin = AssetPlugin { watch_for_changes_override: Some(true), ..default() };
    app.add_plugins((
        bevy::time::TimePlugin,
        bevy::app::TaskPoolPlugin::default(),
        asset_plugin,
        WindowPlugin::default(),
        bevy::image::ImagePlugin::default(),
        TextureAtlasPlugin,
        TextPlugin,
        (InputPlugin, PickingPlugin, InteractionPlugin, UiPlugin),
        SuperUiPlugin,
        PlaygroundBridgePlugin,
    ));
    app.init_resource::<InputFocus>().init_resource::<InputFocusVisible>();
    app.finish();
    app
}

pub fn entry_doc(body: &str, css: &str, js: &str) -> String {
    format!(
        "<html><head><link rel=\"stylesheet\" href=\"{css}\">\
         <script src=\"{js}\"></script></head><body>{body}</body></html>"
    )
}

pub fn spawn_root(app: &mut App, entry: &str, body: &str, css: &str, js: &str) -> Entity {
    put(entry, entry_doc(body, css, js).as_bytes());
    let server = app.world().resource::<AssetServer>().clone();
    let root = SuperUiRoot { html: server.load::<HtmlSource>(entry.to_string()) };
    app.world_mut().spawn((Node::default(), root)).id()
}

pub fn tick(app: &mut App, n: usize) {
    for _ in 0..n { app.update(); }
}

/// Reconciled `Text` of the first `<span>`'s text-child entity (counter label).
pub fn label_text(app: &mut App) -> String {
    use superui_css::prelude::TypeName;
    let mut spans = app.world_mut().query::<(&TypeName, &Children)>();
    let child = spans
        .iter(app.world())
        .find(|(t, _)| t.0 == "span")
        .and_then(|(_, c)| c.iter().next().copied());
    match child {
        Some(e) => app.world().get::<Text>(e).map(|t| t.0.clone()).unwrap_or_default(),
        None => String::new(),
    }
}
```

Note: `label_text` mirrors `current_label_text` in `crates/superui_bridge/tests/supersolid_render.rs:115`; if the reconciled text lives on a differently-shaped entity, copy that helper's exact traversal.

- [ ] **Step 2: Write the failing test** `crates/superui_playground_web/tests/js_edit.rs`:

```rust
//! A queued JS edit drives the seam to a state-preserving re-exec.
mod support;
use support::*;

use superui_bridge::UiRuntime;
use superui_playground_web::apply_source_inner;

// A hand-written $ss counter module (what the transpiler emits), tagged for HMR.
const COUNTER_JS: &str = r#"
    function Counter() {
        var c = createSignal(0);
        globalThis.__c = c;
        var wrap = $ss.el("div");
        var label = $ss.el("span");
        $ss.insert(label, function () { return c[0](); });
        $ss.child(wrap, label);
        return wrap;
    }
    $ss.hot("counter.js#Counter", Counter);
    render(function () { return $ss.cmp(Counter, {}); },
           document.getElementById("root"));
"#;

#[test]
fn js_edit_reexecs_and_preserves_signal() {
    put("counter.js", COUNTER_JS.as_bytes());
    put("c.css", b"");
    let mut app = app();
    let _root = spawn_root(&mut app, "pg_js.html", "<div id='root'></div>", "c.css", "counter.js");
    tick(&mut app, 32);
    assert_eq!(label_text(&mut app), "0", "initial reconcile");

    // Bump the signal to 5.
    app.world_mut().non_send_mut::<UiRuntime>().run_script("globalThis.__c[1](5);");
    tick(&mut app, 2);
    assert_eq!(label_text(&mut app), "5");

    // Edit the SAME module through the bridge (cosmetic change: a comment).
    let edited = format!("{COUNTER_JS}\n// touched");
    apply_source_inner("counter.js", &edited);
    tick(&mut app, 4);

    assert_eq!(label_text(&mut app), "5", "re-exec via the queue rehydrates the signal");
}

#[test]
fn unknown_path_and_premmount_edit_do_not_panic() {
    let _ = superui_playground_web::drain_queue();
    // Unknown extension: reported not-ok, enqueues nothing harmful.
    let out = apply_source_inner("notes.txt", "hi");
    assert!(out.contains("\"ok\":false"));
    // A JS edit enqueued before any UI mounts must drain without panicking.
    apply_source_inner("counter.js", "var x=1;");
    let mut app = app();
    tick(&mut app, 3); // no SuperUiRoot spawned; drain sees no subresources
}
```

- [ ] **Step 3: Run it to verify it fails.**

Run: `cargo test -p superui_playground_web --test js_edit`
Expected: FAIL — `PlaygroundBridgePlugin` / behavior not implemented.

- [ ] **Step 4: Implement the plugin + JS branch** in `crates/superui_playground_web/src/lib.rs`. Because `SuperUiSubresources` is `pub(crate)` in superui, expose the mounted js/css/html handles from superui for the bridge. Add to `crates/superui/src/mount.rs` a tiny public accessor component OR a public query helper. Simplest: make `SuperUiSubresources` and its fields `pub` (widen visibility) and re-export it:

  - In `crates/superui/src/mount.rs`: change `pub(crate) struct SuperUiSubresources` → `pub struct SuperUiSubresources`, fields already `pub`.
  - In `crates/superui/src/lib.rs`: add `pub use mount::SuperUiSubresources;`.

  Then the system:

```rust
use bevy::prelude::*;
use bevy::asset::AssetEvent;
use superui::{HtmlSource, JsSource, SuperUiRoot, SuperUiSubresources};

pub struct PlaygroundBridgePlugin;

impl Plugin for PlaygroundBridgePlugin {
    fn build(&self, app: &mut App) {
        // Ordered before superui's detect_hot_reload so the Modified event we emit
        // is seen the same frame. detect_hot_reload is public from superui? If not,
        // order before it by name via superui's exported system, else rely on
        // Update ordering + the explicit event (which survives to next frame anyway).
        app.add_systems(Update, drain_playground_edits);
    }
}

fn drain_playground_edits(world: &mut World) {
    let edits = crate::drain_queue();
    if edits.is_empty() {
        return;
    }
    // Resolve mounted handles; if not mounted yet, drop the edits (Review Focus).
    let handles = {
        let mut q = world.query::<(&SuperUiRoot, &SuperUiSubresources)>();
        q.iter(world).next().map(|(root, sub)| (root.html.clone(), sub.js.clone(), sub.css.clone()))
    };
    let Some((html_h, js_h, css_h_opt)) = handles else { return };

    for edit in edits {
        match edit {
            crate::Edit::Js(code) => {
                if let Some(mut a) = world.resource_mut::<Assets<JsSource>>().get_mut(&js_h) {
                    a.0 = code;
                }
                world.write_message(AssetEvent::Modified { id: js_h.id() });
            }
            crate::Edit::Html(text) => {
                if let Some(mut a) = world.resource_mut::<Assets<HtmlSource>>().get_mut(&html_h) {
                    a.0 = text;
                }
                world.write_message(AssetEvent::Modified { id: html_h.id() });
            }
            crate::Edit::Css(_text) => { /* Task 5 */ }
        }
    }
}
```

  If `detect_hot_reload` is not `pub` from superui (check `crates/superui/src/lib.rs`), the explicit `write_message` still delivers on the next frame's `detect_hot_reload` read, so ordering is a latency nicety, not a correctness requirement — the test ticks several frames and still passes.

- [ ] **Step 5: Run the test to verify it passes.**

Run: `cargo test -p superui_playground_web --test js_edit`
Expected: PASS (both tests).

- [ ] **Step 6: Confirm superui still builds/tests after widening `SuperUiSubresources` visibility.**

Run: `cargo test -p superui`
Expected: PASS.

- [ ] **Step 7: Commit.**

```bash
git add crates/superui/src/mount.rs crates/superui/src/lib.rs crates/superui_playground_web/
git commit -m "feat(superui_playground_web): drain JS edits into the hot-reload seam"
```

---

## Task 5: CSS branch — live restyle via `InlineCssStyleSheetParser`

**Files:**
- Modify: `crates/superui_playground_web/src/lib.rs` (`drain_playground_edits` Css arm)
- Create: `crates/superui_playground_web/tests/css_edit.rs`

**Interfaces:**
- Consumes: `superui_css::parser::InlineCssStyleSheetParser` (`SystemParam`) `.load_stylesheet(&str) -> Result<StyleSheet, _>`; `superui_css::style::StyleSheet`; `push_diag`.

- [ ] **Step 1: Write the failing test** `crates/superui_playground_web/tests/css_edit.rs`:

```rust
//! A queued CSS edit reparses and restyles without losing signal state.
mod support;
use support::*;

use superui_bridge::UiRuntime;
use superui_css::style::StyleSheet;
use superui_playground_web::apply_source_inner;

const COUNTER_JS: &str = r#"
    function Counter() {
        var c = createSignal(7);
        globalThis.__c = c;
        var wrap = $ss.el("div");
        var label = $ss.el("span");
        $ss.insert(label, function () { return c[0](); });
        $ss.child(wrap, label);
        return wrap;
    }
    $ss.hot("counter.js#Counter", Counter);
    render(function () { return $ss.cmp(Counter, {}); }, document.getElementById("root"));
"#;

#[test]
fn css_edit_restyles_and_preserves_state() {
    put("cnt.js", COUNTER_JS.as_bytes());
    put("s.css", b"span { color: red }");
    let mut app = app();
    let _root = spawn_root(&mut app, "pg_css.html", "<div id='root'></div>", "s.css", "cnt.js");
    tick(&mut app, 32);
    assert_eq!(label_text(&mut app), "7");

    let sheets_before = app.world().resource::<Assets<StyleSheet>>().len();
    apply_source_inner("s.css", "span { color: blue }");
    tick(&mut app, 4);

    // Signal state survives a restyle; the stylesheet asset was replaced in place.
    assert_eq!(label_text(&mut app), "7", "restyle preserves state");
    assert_eq!(app.world().resource::<Assets<StyleSheet>>().len(), sheets_before);
}

#[test]
fn malformed_css_reports_and_keeps_running() {
    let _ = superui_playground_web::drain_queue();
    put("m.js", COUNTER_JS.as_bytes());
    put("m.css", b"span { color: red }");
    let mut app = app();
    let _root = spawn_root(&mut app, "pg_cssbad.html", "<div id='root'></div>", "m.css", "m.js");
    tick(&mut app, 32);

    apply_source_inner("m.css", "span { color: ; @@@ }");
    tick(&mut app, 4);

    // No panic; a diagnostic was recorded; UI still shows the counter.
    let diags = superui_playground_web::poll_diagnostics_inner();
    assert!(diags.contains("message"), "a parse diagnostic was recorded: {diags}");
    assert_eq!(label_text(&mut app), "7");
}
```

- [ ] **Step 2: Run it to verify it fails.**

Run: `cargo test -p superui_playground_web --test css_edit`
Expected: FAIL — the Css arm is a no-op.

- [ ] **Step 3: Implement the Css arm.** `drain_playground_edits` is an exclusive system; use a `SystemState` to borrow the `SystemParam`. Replace the `Edit::Css(_text) => {}` arm and add the param plumbing:

```rust
use bevy::ecs::system::SystemState;
use superui_css::parser::InlineCssStyleSheetParser;
use superui_css::style::StyleSheet;

// inside drain_playground_edits, for Edit::Css(text):
crate::Edit::Css(text) => {
    let Some(css_h) = css_h_opt.clone() else {
        crate::push_diag("edit targets CSS but the document declares no stylesheet".into());
        continue;
    };
    // Parse in a scoped block so the SystemParam's borrow of `world` is released
    // (load_stylesheet returns an OWNED StyleSheet) before we mutably borrow Assets.
    let parsed = {
        let mut state: SystemState<InlineCssStyleSheetParser> = SystemState::new(world);
        let parser = state.get(world);
        parser.load_stylesheet(&text)
    };
    match parsed {
        Ok(sheet) => {
            if let Some(mut slot) = world.resource_mut::<Assets<StyleSheet>>().get_mut(&css_h) {
                *slot = sheet;
            }
            world.write_message(AssetEvent::Modified { id: css_h.id() });
        }
        Err(e) => crate::push_diag(format!("CSS parse error: {e}")),
    }
}
```

  The scoped block is load-bearing: `InlineCssStyleSheetParser` borrows `world` (it reads flair's registries), so its borrow must end before `world.resource_mut::<Assets<StyleSheet>>()`. `load_stylesheet` returns an owned `StyleSheet` value (see `crates/superui_flair_css_parser/src/loader.rs:243`), so `parsed` outlives the block cleanly.

- [ ] **Step 4: Run the test to verify it passes.**

Run: `cargo test -p superui_playground_web --test css_edit`
Expected: PASS.

- [ ] **Step 5: Commit.**

```bash
git add crates/superui_playground_web/src/lib.rs crates/superui_playground_web/tests/css_edit.rs
git commit -m "feat(superui_playground_web): live CSS restyle via InlineCssStyleSheetParser"
```

---

## Task 6: HTML branch — full remount

**Files:**
- Modify: `crates/superui_playground_web/src/lib.rs` (Html arm already written in Task 4 Step 4; verify it triggers remount)
- Create: `crates/superui_playground_web/tests/html_edit.rs`

**Interfaces:**
- Consumes: the Task 4 Html arm (overwrite `Assets<HtmlSource>` + emit `Modified`).

- [ ] **Step 1: Write the failing test** `crates/superui_playground_web/tests/html_edit.rs`:

```rust
//! An index.html edit remounts and re-discovers subresources (state reset is expected).
mod support;
use support::*;

use superui_playground_web::apply_source_inner;

const JS_A: &str = r#"
    var h = document.getElementById('root');
    var s = document.createElement('span'); s.textContent = 'A'; h.appendChild(s);
"#;

#[test]
fn html_edit_remounts_with_new_manifest() {
    put("a.js", JS_A.as_bytes());
    put("h.css", b"");
    let mut app = app();
    // The entry references a.js; body has the #root mount point.
    let _root = spawn_root(&mut app, "pg_html.html", "<div id='root'></div>", "h.css", "a.js");
    tick(&mut app, 32);
    assert_eq!(label_text(&mut app), "A", "initial script ran");

    // Edit the entry HTML to a new document (still referencing a.js + h.css).
    let new_doc = entry_doc("<div id='root'></div><p id='mark'>x</p>", "h.css", "a.js");
    apply_source_inner("pg_html.html", &new_doc);
    tick(&mut app, 32);

    // Remounted: the new <p id=mark> exists (proves the manifest was re-read).
    use superui_css::prelude::TypeName;
    let mut q = app.world_mut().query::<&TypeName>();
    let has_p = q.iter(app.world()).any(|t| t.0 == "p");
    assert!(has_p, "HTML remount re-parsed the new document");
}
```

  Note: `spawn_root`'s `entry` arg is the HTML asset path; `apply_source_inner("pg_html.html", …)` targets that same path, so `path.ends_with(".html")` routes to the Html arm and overwrites `SuperUiRoot.html`'s asset.

- [ ] **Step 2: Run it to verify it fails or passes.**

Run: `cargo test -p superui_playground_web --test html_edit`
Expected: If Task 4's Html arm is correct, this may already PASS. If it FAILS, inspect whether the Html arm overwrote the right handle and emitted `Modified`.

- [ ] **Step 3: Fix if needed.** Ensure the Html arm overwrites `SuperUiRoot.html`'s asset (`html_h`) and emits `AssetEvent::Modified { id: html_h.id() }`, so superui's `apply_hot_reload` html branch tears down + `mount_when_ready` rebuilds. No new code if Task 4 is correct.

- [ ] **Step 4: Run to verify pass.**

Run: `cargo test -p superui_playground_web --test html_edit`
Expected: PASS.

- [ ] **Step 5: Commit.**

```bash
git add crates/superui_playground_web/tests/html_edit.rs crates/superui_playground_web/src/lib.rs
git commit -m "test(superui_playground_web): HTML edit remounts via the seam"
```

---

## Task 7: `poll_diagnostics` — surface runtime JS errors each frame

**Files:**
- Modify: `crates/superui_playground_web/src/lib.rs` (add a system that drains `UiRuntime::take_errors` into the sink; register it in the plugin)
- Create: `crates/superui_playground_web/tests/diagnostics.rs`

**Interfaces:**
- Consumes: `UiRuntime::take_errors()` (Task 2); `push_diag`/`poll_diagnostics_inner` (Task 3).

- [ ] **Step 1: Write the failing test** `crates/superui_playground_web/tests/diagnostics.rs`:

```rust
//! Runtime JS errors from an edit reach the diagnostics sink for the console.
mod support;
use support::*;

use superui_playground_web::{apply_source_inner, poll_diagnostics_inner};

const GOOD_JS: &str = r#"
    var s = document.createElement('span'); s.textContent = '1';
    document.getElementById('root').appendChild(s);
"#;

#[test]
fn runtime_error_from_edit_reaches_poll() {
    put("d.js", GOOD_JS.as_bytes());
    put("d.css", b"");
    let mut app = app();
    let _root = spawn_root(&mut app, "pg_diag.html", "<div id='root'></div>", "d.css", "d.js");
    tick(&mut app, 32);
    let _ = poll_diagnostics_inner(); // clear any startup noise

    // Edit to a throwing script; the seam re-execs it, UiRuntime captures the throw.
    apply_source_inner("d.js", "throw new Error('kaboom');");
    tick(&mut app, 4);

    let diags = poll_diagnostics_inner();
    assert!(diags.contains("kaboom"), "runtime error surfaced: {diags}");
}
```

- [ ] **Step 2: Run it to verify it fails.**

Run: `cargo test -p superui_playground_web --test diagnostics`
Expected: FAIL — errors are captured on `UiRuntime` but never moved to the sink.

- [ ] **Step 3: Add the drain-errors system** in `crates/superui_playground_web/src/lib.rs` and register it in `PlaygroundBridgePlugin::build` after `drain_playground_edits`:

```rust
fn drain_runtime_errors(rt: Option<bevy::prelude::NonSendMut<superui_bridge::UiRuntime>>) {
    if let Some(mut rt) = rt {
        for e in rt.take_errors() {
            crate::push_diag(e);
        }
    }
}
```
```rust
// in PlaygroundBridgePlugin::build:
app.add_systems(Update, (drain_playground_edits, drain_runtime_errors).chain());
```

- [ ] **Step 4: Run to verify pass.**

Run: `cargo test -p superui_playground_web --test diagnostics`
Expected: PASS.

- [ ] **Step 5: Whole-crate test sweep.**

Run: `cargo test -p superui_playground_web`
Expected: PASS (lib + js_edit + css_edit + html_edit + diagnostics).

- [ ] **Step 6: Commit.**

```bash
git add crates/superui_playground_web/src/lib.rs crates/superui_playground_web/tests/diagnostics.rs
git commit -m "feat(superui_playground_web): drain runtime JS errors into poll_diagnostics"
```

---

## Task 8: wasm-bindgen exports + counter playground build + proof harness

**Files:**
- Modify: `crates/superui_playground_web/src/lib.rs` (wasm-bindgen exports)
- Modify: `examples/counter/Cargo.toml` (`playground` feature + dep)
- Modify: `examples/counter/src/main.rs` (add plugin + watch override under the feature)
- Create: `examples/counter/web-playground.html` (proof harness)

**Interfaces:**
- Produces: JS globals `apply_source(path, src) -> string`, `poll_diagnostics() -> string` on the counter wasm module.

- [ ] **Step 1: Add the wasm-bindgen exports** at the bottom of `crates/superui_playground_web/src/lib.rs`:

```rust
#[cfg(target_arch = "wasm32")]
mod wasm_exports {
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen]
    pub fn apply_source(path: &str, src: &str) -> String {
        super::apply_source_inner(path, src)
    }

    #[wasm_bindgen]
    pub fn poll_diagnostics() -> String {
        super::poll_diagnostics_inner()
    }
}
```

- [ ] **Step 2: Verify the crate still builds for wasm.**

Run: `cargo build -p superui_playground_web --target wasm32-unknown-unknown`
Expected: builds (wasm-bindgen exports compile).

- [ ] **Step 3: Add the `playground` feature to `examples/counter/Cargo.toml`.**

```toml
[features]
# Web playground: transpile TSX in-browser + live edit via the bridge crate.
# NOTE: deliberately does NOT pull bevy/file_watcher (no watcher on wasm); the
# app sets watch_for_changes_override itself (see main.rs).
playground = ["superui/transpiler", "superui/hmr", "dep:superui_playground_web"]

[dependencies]
superui_playground_web = { path = "../../crates/superui_playground_web", optional = true }
```

- [ ] **Step 4: Wire the feature in `examples/counter/src/main.rs`.** The file already builds its `AssetPlugin` in `web_asset_plugin` and sets it on `DefaultPlugins`. Extend that helper to add the watch override under the feature, and restructure `main` to a mutable `App` so the plugin can be added conditionally.

Extend `web_asset_plugin` (append after the existing wasm arm):

```rust
fn web_asset_plugin(plugin: AssetPlugin) -> AssetPlugin {
    #[cfg(target_arch = "wasm32")]
    let plugin = AssetPlugin { meta_check: bevy::asset::AssetMetaCheck::Never, ..plugin };
    // Playground builds drive edits through the bridge and need the HMR gate active,
    // which requires watching = true. No file watcher runs on wasm; the bridge fires
    // AssetEvent::Modified itself.
    #[cfg(feature = "playground")]
    let plugin = AssetPlugin { watch_for_changes_override: Some(true), ..plugin };
    plugin
}
```

Rewrite `main` from the builder chain to a mutable `App`:

```rust
fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(web_asset_plugin(default())).set(WindowPlugin {
        primary_window: Some(web_window(Window::default())),
        ..default()
    }));
    app.add_plugins(SuperUiPlugin);
    #[cfg(feature = "playground")]
    app.add_plugins(superui_playground_web::PlaygroundBridgePlugin);
    app.add_systems(Startup, setup);
    app.run();
}
```

  On wasm+playground, `live_source()` is now true, so mount loads `app.tsx` live (in-browser transpile) — no generated JS needed.

- [ ] **Step 5: Build the counter playground wasm.**

Run: `cargo build -p counter --release --target wasm32-unknown-unknown --features playground`
Expected: builds; links oxc + the bridge.

- [ ] **Step 6: Create the proof harness** `examples/counter/web-playground.html`. A minimal page that loads the wasm-bindgen glue, shows two textareas (`app.tsx`, `style.css`) prefilled from `assets/ui/counter/`, a Run button, and a `<pre>` for diagnostics. Skeleton:

```html
<!doctype html><html><head><meta charset="utf-8"><title>superui playground proof</title></head>
<body>
  <canvas id="superui-canvas" width="480" height="320"></canvas>
  <div><textarea id="tsx" rows="16" cols="60"></textarea><textarea id="css" rows="16" cols="40"></textarea></div>
  <button id="run">Run</button>
  <pre id="diag"></pre>
  <script type="module">
    import init, { apply_source, poll_diagnostics } from "./counter.js";
    await init();
    const tsx = document.getElementById("tsx");
    const css = document.getElementById("css");
    tsx.value = await (await fetch("assets/ui/counter/app.tsx")).text();
    css.value = await (await fetch("assets/ui/counter/style.css")).text();
    document.getElementById("run").onclick = () => {
      const d = document.getElementById("diag");
      d.textContent = apply_source("app.tsx", tsx.value) + "\n" + apply_source("style.css", css.value);
    };
    setInterval(() => {
      const p = poll_diagnostics();
      if (p && p !== "[]") document.getElementById("diag").textContent += "\n" + p;
    }, 500);
  </script>
</body></html>
```

- [ ] **Step 7: Stage + wasm-bindgen + serve (manual).**

```bash
wasm-bindgen --no-typescript --target web \
  --out-dir /tmp/pg --out-name counter \
  target/wasm32-unknown-unknown/release/counter.wasm
cp examples/counter/web-playground.html /tmp/pg/index.html
cp -r examples/counter/assets /tmp/pg/assets
python -m http.server -d /tmp/pg 8973
```

- [ ] **Step 8: Manual wasm smoke (REQUIRED — green tests do not prove a windowed/wasm launch).**

Open `http://127.0.0.1:8973/`. Verify:
1. The counter demo auto-runs (mounts from live-transpiled `app.tsx`).
2. Click the counter a few times (count > 0).
3. Edit `app.tsx` (e.g. change the button label), click **Run** → UI updates, **count preserved**.
4. Edit `style.css` (e.g. change a color), click **Run** → restyle applied, count preserved.
5. Break the `.tsx` (type `<div`), Run → the `<pre>` shows a diagnostic, no crash.

- [ ] **Step 9: Commit.**

```bash
git add crates/superui_playground_web/src/lib.rs examples/counter/Cargo.toml examples/counter/src/main.rs examples/counter/web-playground.html
git commit -m "feat(counter): playground wasm build with in-browser transpile + live edit harness"
```

---

## Final verification

- [ ] **Workspace build + tests.**

Run: `cargo build --workspace` then `cargo test -p superui -p superui_bridge -p superui_playground_web`
Expected: PASS. (Re-run flaky `tsx_loader` tests per the known-flake note if they trip.)

- [ ] **Wasm oxc-placement guard (no regression to gallery demos).**

Run: `cargo tree -p counter --target wasm32-unknown-unknown -i oxc`
Expected: oxc ABSENT (normal counter wasm stays oxc-free).

Run: `cargo tree -p counter --target wasm32-unknown-unknown --features playground -i oxc`
Expected: `oxc v0.140.0` present (only the playground build links it).
