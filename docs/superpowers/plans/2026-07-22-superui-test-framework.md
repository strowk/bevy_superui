# superui-test Framework Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Playwright-shaped E2E testing tool for superui UIs: TypeScript `.spec.ts` files, transpiled through superui's own oxc→Boa pipeline and run inside Boa, drive a real in-process Bevy+superui `App` by simulating input, assert on the live arena DOM, and match pixel screenshots — with a headless CLI and an egui UI mode.

**Architecture:** A new library crate `superui_test_engine` hosts a mode-agnostic engine: it transpiles a spec, installs a `$sstest` host ABI (backed by host-held Boa promises) plus a thin JS prelude that defines `test`/`page`/`locator`/`expect`, then runs a driver loop that pumps `app.update()` and resolves the promises as commands settle. A `superui-test` binary wraps the engine with config, discovery, a headless reporter, and an egui UI mode. Screenshots use Bevy's offscreen `RenderTarget::Image` + `Screenshot::image` capture.

**Tech Stack:** Rust, Bevy 0.17.3, Boa 0.21.1 (`boa_engine`), oxc 0.140 (via the existing `supersolid` transpiler crate), `serde`/`serde_json` for the JS↔Rust command boundary, `image` crate for PNG baseline/diff, `bevy_egui` for UI mode.

## Global Constraints

- Bevy version: `0.17` (workspace uses `bevy = { version = "0.17" }`; render features pulled explicitly where needed). Copy verbatim.
- Boa: `boa_engine` workspace dependency, version `0.21.1`. Use `JsPromise::new_pending`, `ResolvingFunctions { resolve, reject }`, `context.run_jobs()`.
- Transpiler: depend on `supersolid = { path = "../supersolid" }`; call `supersolid::transpile(src, &TranspileOptions)` → `TranspileResult { code, diagnostics, style_imports }`. For specs use `TranspileOptions { tsx: false, runtime_specifiers: [..., "superui/test"], module_id }`. Native-only (`oxc` must never enter wasm); this whole crate is native-only.
- Windows: the `superui-test` binary inherits `/STACK:8388608` from `.cargo/config.toml` automatically — do not remove or duplicate it.
- Plugin set for headless (no-render) hosting, verbatim from `examples/horde/tests/support/mod.rs`: `bevy::time::TimePlugin`, `bevy::app::TaskPoolPlugin::default()`, `AssetPlugin::default()`, `WindowPlugin::default()`, `bevy::image::ImagePlugin::default()`, `TextureAtlasPlugin`, `TextPlugin`, `(InputPlugin, PickingPlugin, InteractionPlugin, UiPlugin)`, `bevy::state::app::StatesPlugin`; plus `.init_resource::<InputFocus>()`, `.init_resource::<InputFocusVisible>()`, `SuperUiPlugin`, then `app.finish()`.
- DOM access is always via the `UiRuntime` NonSend resource: `app.world().non_send_resource::<UiRuntime>().dom.borrow()`. Query with `Dom::query_selector[_all]`, read `text_content`, `get_attribute`, `classes`, `value`, `checked`.
- Input simulation: push `superui_bridge::PendingDomEvent::new(node, "click")` onto the `PendingDomEvents` resource, then tick. Reconcile settles when `UiRuntime.dirty == false` after an `app.update()`.
- Every crate/module added is native-only; gate the crate build so it is not a wasm target member issue (document, don't over-engineer).

---

## File structure

- `crates/superui_test_engine/Cargo.toml` — new library crate.
- `crates/superui_test_engine/src/lib.rs` — public API surface + re-exports.
- `crates/superui_test_engine/src/host.rs` — headless & render App hosting (build the `App`, mount a project's assets, mount/tick helpers).
- `crates/superui_test_engine/src/transpile.rs` — spec `.spec.ts` → JS via `supersolid`.
- `crates/superui_test_engine/src/abi.rs` — `TestState`, `$sstest` host functions, JS prelude, promise registry.
- `crates/superui_test_engine/src/locator.rs` — `LocatorSpec`, `resolve_locator`.
- `crates/superui_test_engine/src/command.rs` — `Command` enum (serde), `CommandOutcome`.
- `crates/superui_test_engine/src/driver.rs` — the frame-pump driver loop, command execution, auto-wait.
- `crates/superui_test_engine/src/matchers.rs` — expect matcher evaluation against the DOM.
- `crates/superui_test_engine/src/trace.rs` — `Step`, `Trace`, `TestResult`, `StepStatus`.
- `crates/superui_test_engine/src/render.rs` — offscreen render target + screenshot capture.
- `crates/superui_test_engine/src/snapshot.rs` — PNG baseline path, diff, `--update`.
- `crates/superui_test_engine/src/prelude.js` — the JS that defines `test`/`page`/`locator`/`expect` on top of `$sstest`.
- `crates/superui_test_engine/src/bin/superui_test.rs` — CLI binary (headless + `--ui`).
- `crates/superui_test_engine/src/config.rs` — `superui.test.toml` parsing + spec discovery.
- `crates/superui_test_engine/src/report.rs` — terminal + HTML/JSON reporter.
- `crates/superui_test_engine/src/ui_mode.rs` — egui UI mode.
- `examples/game_menu/superui.test.toml` + `examples/game_menu/tests/game_menu.spec.ts` — acceptance target.

---

## Task 1: Scaffold crate + headless host

**Files:**
- Create: `crates/superui_test_engine/Cargo.toml`
- Create: `crates/superui_test_engine/src/lib.rs`
- Create: `crates/superui_test_engine/src/host.rs`
- Test: `crates/superui_test_engine/tests/host_smoke.rs`
- Create fixture: `crates/superui_test_engine/tests/fixtures/basic/{index.html,style.css,app.tsx}`

**Interfaces:**
- Produces:
  - `pub struct HostProject { pub html: String, pub css: String, pub js_or_tsx: String, pub tsx: bool }`
  - `pub fn build_headless_app(project: &HostProject) -> bevy::app::App` — assembles the no-render App and registers the project's assets via `MemoryAssetReader`.
  - `pub fn mount(app: &mut App) -> bevy::prelude::Entity` — spawns `SuperUiRoot`, spins ≤256 ticks until `UiRuntime` exists.
  - `pub fn tick(app: &mut App, n: usize)` — `for _ in 0..n { app.update(); }`.

- [ ] **Step 1: Create the crate manifest**

`crates/superui_test_engine/Cargo.toml`:

```toml
[package]
name = "superui_test_engine"
version = "0.1.0"
edition = "2021"
publish = false

[dependencies]
bevy = { version = "0.17" }
superui = { path = "../superui" }
superui_bridge = { path = "../superui_bridge" }
superui_dom = { path = "../superui_dom" }
superui_css = { path = "../superui_css" }
supersolid = { path = "../supersolid" }
superui_js = { path = "../superui_js" }
boa_engine = { workspace = true }
boa_gc = { workspace = true }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
image = "0.25"
toml = "0.8"

[[bin]]
name = "superui_test"
path = "src/bin/superui_test.rs"
```

Note: confirm `boa_engine`/`boa_gc` are workspace deps (they are used by `superui_js`). If `boa_gc` is not a workspace dep, use `boa_gc = "0.21"`.

- [ ] **Step 2: Write the failing smoke test**

`crates/superui_test_engine/tests/host_smoke.rs`:

```rust
use superui_test_engine::host::{build_headless_app, mount, tick, HostProject};

fn fixture() -> HostProject {
    HostProject {
        html: include_str!("fixtures/basic/index.html").to_string(),
        css: include_str!("fixtures/basic/style.css").to_string(),
        js_or_tsx: include_str!("fixtures/basic/app.tsx").to_string(),
        tsx: true,
    }
}

#[test]
fn mounts_and_renders_fixture_dom() {
    let mut app = build_headless_app(&fixture());
    let _root = mount(&mut app);
    let rt = app.world().non_send_resource::<superui_bridge::UiRuntime>();
    let dom = rt.dom.borrow();
    let node = dom.query_selector(dom.document(), "#hello").expect("#hello exists");
    assert_eq!(dom.text_content(node), "Hello");
}
```

Create the fixture files. `index.html`: `<!doctype html><html><body></body></html>`. `style.css`: empty. `app.tsx`:

```tsx
import { render } from "supersolid";
render(() => <div id="hello">Hello</div>, document.body);
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test -p superui_test_engine --test host_smoke -- --nocapture`
Expected: FAIL to compile ("unresolved import `superui_test_engine::host`").

- [ ] **Step 4: Implement `lib.rs` and `host.rs`**

`crates/superui_test_engine/src/lib.rs`:

```rust
//! Playwright-shaped E2E test engine for superui UIs (native-only).
pub mod host;
```

`crates/superui_test_engine/src/host.rs` (model on `examples/horde/tests/support/mod.rs`):

```rust
use bevy::asset::io::memory::{Dir, MemoryAssetReader};
use bevy::asset::io::{AssetSource, AssetSourceId};
use bevy::asset::AssetPlugin;
use bevy::image::TextureAtlasPlugin;
use bevy::input::InputPlugin;
use bevy::input_focus::{InputFocus, InputFocusVisible};
use bevy::picking::{InteractionPlugin, PickingPlugin};
use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use bevy::text::TextPlugin;
use bevy::ui::UiPlugin;
use superui::prelude::{SuperUiPlugin, SuperUiRoot};
use superui::JsSource;
use superui_bridge::UiRuntime;
use superui_css::style::StyleSheet;

pub struct HostProject {
    pub html: String,
    pub css: String,
    pub js_or_tsx: String,
    pub tsx: bool,
}

pub fn build_headless_app(project: &HostProject) -> App {
    let dir = Dir::new("assets".into());
    dir.insert_asset("ui/index.html".as_ref(), project.html.as_bytes());
    dir.insert_asset("ui/style.css".as_ref(), project.css.as_bytes());
    let ui_js_path = if project.tsx { "ui/app.tsx" } else { "ui/app.js" };
    dir.insert_asset(ui_js_path.as_ref(), project.js_or_tsx.as_bytes());

    let mut app = App::new();
    app.register_asset_source(
        AssetSourceId::Default,
        AssetSource::build().with_reader(move || Box::new(MemoryAssetReader { root: dir.clone() })),
    );
    app.add_plugins((
        bevy::time::TimePlugin,
        bevy::app::TaskPoolPlugin::default(),
        AssetPlugin::default(),
        WindowPlugin::default(),
        bevy::image::ImagePlugin::default(),
        TextureAtlasPlugin,
        TextPlugin,
        (InputPlugin, PickingPlugin, InteractionPlugin, UiPlugin),
        StatesPlugin,
    ));
    app.init_resource::<InputFocus>().init_resource::<InputFocusVisible>();
    app.add_plugins(SuperUiPlugin);
    app.finish();

    // Store the js path so mount() can load the right handle type.
    app.insert_resource(HostAssetPaths { js: ui_js_path.to_string(), tsx: project.tsx });
    app
}

#[derive(Resource, Clone)]
struct HostAssetPaths { js: String, tsx: bool }

pub fn mount(app: &mut App) -> Entity {
    let paths = app.world().resource::<HostAssetPaths>().clone();
    let (html, css, js) = {
        let s = app.world().resource::<AssetServer>().clone();
        (
            s.load("ui/index.html"),
            s.load::<StyleSheet>("ui/style.css"),
            s.load::<JsSource>(paths.js.clone()),
        )
    };
    let root = app
        .world_mut()
        .spawn((Node::default(), SuperUiRoot { html, css, js }))
        .id();
    for _ in 0..256 {
        app.update();
        if app.world().contains_non_send::<UiRuntime>() {
            break;
        }
    }
    root
}

pub fn tick(app: &mut App, n: usize) {
    for _ in 0..n {
        app.update();
    }
}
```

Note: the `.tsx` asset is transpiled by superui's own `TsxLoader` at load time, so mounting exercises the real pipeline. `.js` projects use `JsLoader`.

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p superui_test_engine --test host_smoke`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/superui_test_engine
git commit -m "feat(test-engine): scaffold crate + headless superui host"
```

---

## Task 2: Transpile spec `.spec.ts` → JS

**Files:**
- Create: `crates/superui_test_engine/src/transpile.rs`
- Modify: `crates/superui_test_engine/src/lib.rs` (add `pub mod transpile;`)
- Test: `crates/superui_test_engine/src/transpile.rs` (unit tests)

**Interfaces:**
- Consumes: `supersolid::{transpile, TranspileOptions}`.
- Produces:
  - `pub fn transpile_spec(source: &str, module_id: &str) -> Result<String, String>` — returns JS code; strips `superui/test` imports; TS-only (no JSX lowering); returns `Err` only on a fatal transpile failure (diagnostics for stripped cross-module imports are non-fatal but logged by caller).

- [ ] **Step 1: Write the failing test**

Append to `crates/superui_test_engine/src/transpile.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::transpile_spec;

    #[test]
    fn strips_superui_test_import_and_keeps_body() {
        let src = r#"
            import { test, expect } from "superui/test";
            test("x", async ({ page }) => {
                const n: number = 1;
                await expect(page.locator(".a")).toHaveCount(n);
            });
        "#;
        let js = transpile_spec(src, "x.spec.ts").unwrap();
        assert!(!js.contains("import"), "imports must be stripped: {js}");
        assert!(js.contains("test("), "body preserved: {js}");
        assert!(!js.contains(": number"), "TS types stripped: {js}");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p superui_test_engine transpile`
Expected: FAIL ("cannot find function `transpile_spec`").

- [ ] **Step 3: Implement**

Top of `crates/superui_test_engine/src/transpile.rs`:

```rust
use supersolid::{transpile, TranspileOptions};

pub fn transpile_spec(source: &str, module_id: &str) -> Result<String, String> {
    let opts = TranspileOptions {
        runtime_specifiers: vec![
            "supersolid".into(),
            "solid-js".into(),
            "superui/test".into(),
        ],
        tsx: false,
        module_id: Some(module_id.to_string()),
    };
    let result = transpile(source, &opts);
    // Fatal only if codegen produced nothing from non-empty input.
    if result.code.trim().is_empty() && !source.trim().is_empty() {
        return Err(format!("transpile produced empty output for {module_id}"));
    }
    Ok(result.code)
}
```

Add `pub mod transpile;` to `lib.rs`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p superui_test_engine transpile`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/superui_test_engine/src
git commit -m "feat(test-engine): transpile .spec.ts via supersolid (TS-strip, superui/test stripped)"
```

---

## Task 3: `$sstest` ABI + host-held promises + JS prelude (async bridge core)

This is the highest-risk task. It proves the async model: a spec's `test()` registers, and an awaited no-op action resolves after a driver tick.

**Files:**
- Create: `crates/superui_test_engine/src/abi.rs`
- Create: `crates/superui_test_engine/src/command.rs`
- Create: `crates/superui_test_engine/src/prelude.js`
- Modify: `crates/superui_test_engine/src/lib.rs`
- Test: `crates/superui_test_engine/tests/async_bridge.rs`

**Interfaces:**
- Produces:
  - In `command.rs`:
    ```rust
    #[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
    #[serde(tag = "type", rename_all = "camelCase")]
    pub enum Command {
        Noop,
        // extended in later tasks: Click { locator }, Fill { .. }, Expect { .. }, Screenshot { .. }
    }
    #[derive(Clone, Debug)]
    pub struct Queued { pub id: u64, pub command: Command }
    ```
  - In `abi.rs`:
    - `pub struct RegisteredTest { pub name: String, pub func: boa_engine::object::builtins::JsFunction }`
    - `pub fn install(context: &mut boa_engine::Context)` — installs `$sstest` globals + evaluates `prelude.js`.
    - `pub fn take_registered_tests(context: &mut Context) -> Vec<RegisteredTest>`
    - `pub fn drain_queue(context: &mut Context) -> Vec<Queued>`
    - `pub fn resolve(context: &mut Context, id: u64, result_json: &str)` — resolves the pending promise for `id` with a parsed JSON value; on `{ "ok": false }` the prelude throws.
    - `pub fn run_test(context: &mut Context, test: &RegisteredTest) -> JsPromiseHandle` where `pub struct JsPromiseHandle(pub JsValue);` — invokes the test fn with a `page` arg and returns its promise.
    - `pub fn promise_settled(context: &mut Context, handle: &JsPromiseHandle) -> Option<Result<(), String>>` — inspects `JsPromise` state (`PromiseState::Fulfilled`/`Rejected`).

- [ ] **Step 1: Write `prelude.js`**

`crates/superui_test_engine/src/prelude.js` (thin JS over `$sstest`):

```js
// Thin Playwright-shaped surface over the $sstest host ABI.
globalThis.__superui_tests = [];

globalThis.test = function (name, fn) {
  $sstest.register(name, fn);
};

function makeLocator(steps) {
  return {
    steps: steps,
    locator(sel, opts) {
      const step = { sel: sel, hasText: opts && opts.hasText ? String(opts.hasText) : null };
      return makeLocator(steps.concat([step]));
    },
    nth(i) { const s = steps.slice(); s._nth = i; return makeLocator(s); },
    first() { return this.nth(0); },
    async click() { return enqueue({ type: "click", locator: serialize(this) }); },
    async fill(text) { return enqueue({ type: "fill", locator: serialize(this), text: String(text) }); },
    async press(key) { return enqueue({ type: "press", locator: serialize(this), key: String(key) }); },
    async hover() { return enqueue({ type: "hover", locator: serialize(this) }); },
  };
}
function serialize(loc) {
  return { steps: loc.steps.map(s => ({ sel: s.sel, hasText: s.hasText })), nth: loc.steps._nth ?? null };
}
function enqueue(cmd) {
  return $sstest.enqueue(JSON.stringify(cmd)).then(function (json) {
    const r = JSON.parse(json);
    if (r && r.ok === false) throw new Error(r.error || "assertion failed");
    return r.value;
  });
}

globalThis.page = {
  locator(sel, opts) { return makeLocator([]).locator(sel, opts); },
};

globalThis.expect = function (target) {
  const loc = target && target.steps !== undefined ? serialize(target) : null;
  const mk = (matcher, expected, opts) =>
    enqueue({ type: "expect", matcher: matcher, locator: loc, page: loc ? false : true,
              expected: expected === undefined ? null : expected, opts: opts || null });
  return {
    toBeVisible: () => mk("visible"),
    toHaveText: (t) => mk("text", String(t)),
    toHaveCount: (n) => mk("count", n),
    toHaveClass: (re) => mk("class", re instanceof RegExp ? re.source : String(re)),
    toHaveAttribute: (name, val) => mk("attribute", { name: name, value: val === undefined ? null : String(val) }),
    toHaveScreenshot: (name) => mk("screenshot", String(name)),
  };
};
```

- [ ] **Step 2: Write the failing test**

`crates/superui_test_engine/tests/async_bridge.rs`:

```rust
use boa_engine::Context;
use superui_test_engine::abi::{self, JsPromiseHandle};

#[test]
fn registers_tests_and_resolves_awaited_noop() {
    let mut ctx = Context::default();
    abi::install(&mut ctx);

    // A spec that awaits one enqueued no-op then finishes.
    ctx.eval(boa_engine::Source::from_bytes(
        br#"test("t", async () => { await $sstest.enqueue(JSON.stringify({type:"noop"})); });"#,
    )).unwrap();

    let tests = abi::take_registered_tests(&mut ctx);
    assert_eq!(tests.len(), 1);
    assert_eq!(tests[0].name, "t");

    let handle: JsPromiseHandle = abi::run_test(&mut ctx, &tests[0]);
    // Pump: the test body enqueues one noop, awaiting it.
    let _ = ctx.run_jobs();
    let q = abi::drain_queue(&mut ctx);
    assert_eq!(q.len(), 1);
    // Resolve it, then pump jobs so the await continuation + test completion run.
    abi::resolve(&mut ctx, q[0].id, r#"{"ok":true,"value":null}"#);
    let _ = ctx.run_jobs();
    assert!(matches!(abi::promise_settled(&mut ctx, &handle), Some(Ok(()))));
}
```

- [ ] **Step 3: Run to verify it fails**

Run: `cargo test -p superui_test_engine --test async_bridge`
Expected: FAIL (module `abi` not found).

- [ ] **Step 4: Implement `command.rs`**

```rust
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Command {
    Noop,
}

#[derive(Clone, Debug)]
pub struct Queued {
    pub id: u64,
    pub command: Command,
    pub raw: String,
}
```

Note: `raw` holds the original JSON so later tasks that add richer command variants can re-parse without changing this task. Later tasks extend the `Command` enum in place.

- [ ] **Step 5: Implement `abi.rs`**

```rust
use boa_engine::object::builtins::{JsFunction, JsPromise};
use boa_engine::property::Attribute;
use boa_engine::{
    js_string, Context, JsArgs, JsNativeError, JsObject, JsResult, JsValue, NativeFunction, Source,
};
use boa_engine::builtins::promise::{PromiseState, ResolvingFunctions};
use boa_gc::{Finalize, Trace};
use std::cell::RefCell;
use std::rc::Rc;

use crate::command::Queued;

pub struct RegisteredTest {
    pub name: String,
    pub func: JsFunction,
}

pub struct JsPromiseHandle(pub JsValue);

/// Realm-hosted state for the test ABI. Traced because it holds JS handles.
#[derive(Trace, Finalize, boa_engine::JsData, Default)]
struct TestState {
    #[unsafe_ignore_trace]
    next_id: u64,
    tests: Vec<TestEntry>,
    #[unsafe_ignore_trace]
    queue: Vec<Queued>,
    pending: Vec<PendingEntry>,
}

#[derive(Trace, Finalize)]
struct TestEntry {
    #[unsafe_ignore_trace]
    name: String,
    func: JsFunction,
}

#[derive(Trace, Finalize)]
struct PendingEntry {
    #[unsafe_ignore_trace]
    id: u64,
    resolvers: ResolvingFunctions,
}

fn with_state<R>(context: &mut Context, f: impl FnOnce(&mut TestState) -> R) -> R {
    let mut host = context.realm().host_defined_mut();
    let state = host.get_mut::<TestState>().expect("TestState installed");
    f(state)
}

pub fn install(context: &mut Context) {
    context.realm().host_defined_mut().insert(TestState::default());

    let obj = JsObject::with_object_proto(context.intrinsics());
    let register = NativeFunction::from_fn_ptr(js_register);
    let enqueue = NativeFunction::from_fn_ptr(js_enqueue);
    let reg_fn = register.to_js_function(context.realm());
    let enq_fn = enqueue.to_js_function(context.realm());
    obj.set(js_string!("register"), reg_fn, false, context).unwrap();
    obj.set(js_string!("enqueue"), enq_fn, false, context).unwrap();
    context
        .register_global_property(js_string!("$sstest"), obj, Attribute::all())
        .unwrap();

    context
        .eval(Source::from_bytes(include_str!("prelude.js")))
        .expect("prelude.js must evaluate");
}

fn js_register(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let name = args.get_or_undefined(0).to_string(context)?.to_std_string_escaped();
    let func = JsFunction::from_object(
        args.get_or_undefined(1)
            .as_object()
            .cloned()
            .ok_or_else(|| JsNativeError::typ().with_message("test(fn) requires a function"))?,
    )
    .map_err(|_| JsNativeError::typ().with_message("test(fn) requires a function"))?;
    with_state(context, |s| s.tests.push(TestEntry { name, func }));
    Ok(JsValue::undefined())
}

fn js_enqueue(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let raw = args.get_or_undefined(0).to_string(context)?.to_std_string_escaped();
    let command: crate::command::Command = serde_json::from_str(&raw)
        .map_err(|e| JsNativeError::typ().with_message(format!("bad command json: {e}")))?;
    let (promise, resolvers) = JsPromise::new_pending(context);
    let id = with_state(context, |s| {
        let id = s.next_id;
        s.next_id += 1;
        s.queue.push(Queued { id, command, raw });
        s.pending.push(PendingEntry { id, resolvers });
        id
    });
    let _ = id;
    Ok(promise.into())
}

pub fn take_registered_tests(context: &mut Context) -> Vec<RegisteredTest> {
    with_state(context, |s| {
        s.tests
            .drain(..)
            .map(|t| RegisteredTest { name: t.name, func: t.func })
            .collect()
    })
}

pub fn drain_queue(context: &mut Context) -> Vec<Queued> {
    with_state(context, |s| std::mem::take(&mut s.queue))
}

pub fn resolve(context: &mut Context, id: u64, result_json: &str) {
    let resolvers = with_state(context, |s| {
        if let Some(pos) = s.pending.iter().position(|p| p.id == id) {
            Some(s.pending.remove(pos).resolvers)
        } else {
            None
        }
    });
    if let Some(r) = resolvers {
        let val = JsValue::from(js_string!(result_json));
        // Prelude does JSON.parse on the resolved string.
        let _ = r.resolve.call(&JsValue::undefined(), &[val], context);
    }
}

pub fn run_test(context: &mut Context, test: &RegisteredTest) -> JsPromiseHandle {
    // Invoke with a `page`-bearing arg: `({ page })`. The prelude puts `page` on
    // globalThis, so passing the global object as the destructured arg works.
    let global = context.global_object();
    let arg = JsObject::with_object_proto(context.intrinsics());
    let page = global.get(js_string!("page"), context).unwrap();
    arg.set(js_string!("page"), page, false, context).unwrap();
    let ret = test
        .func
        .call(&JsValue::undefined(), &[arg.into()], context)
        .unwrap_or(JsValue::undefined());
    JsPromiseHandle(ret)
}

pub fn promise_settled(context: &mut Context, handle: &JsPromiseHandle) -> Option<Result<(), String>> {
    let obj = handle.0.as_object()?;
    let promise = JsPromise::from_object(obj.clone()).ok()?;
    match promise.state() {
        PromiseState::Pending => None,
        PromiseState::Fulfilled(_) => Some(Ok(())),
        PromiseState::Rejected(v) => {
            let msg = v.to_string(context).map(|s| s.to_std_string_escaped()).unwrap_or_default();
            Some(Err(msg))
        }
    }
}
```

Note on API details to confirm during implementation against `boa_engine-0.21.1` (all present in that source):
- `NativeFunction::to_js_function(realm)` exists; if not, use `FunctionObjectBuilder::new(context.realm(), NativeFunction::from_fn_ptr(f)).build()` (this exact builder is used in `crates/superui_api/src/document.rs`).
- `JsPromise::state()` returns `PromiseState` (`crates/superui_api/src/fetch.rs` and boa's `jspromise.rs`).
- `resolve.call(this, &[val], context)` — `ResolvingFunctions.resolve` is a `JsFunction`.
- `Rc`/`RefCell` imports may be unused; remove if the compiler warns.

Add `pub mod abi; pub mod command;` to `lib.rs`.

- [ ] **Step 6: Run to verify it passes**

Run: `cargo test -p superui_test_engine --test async_bridge -- --nocapture`
Expected: PASS. If it fails on a Boa API name, fix per the confirmation notes and re-run.

- [ ] **Step 7: Commit**

```bash
git add crates/superui_test_engine/src crates/superui_test_engine/tests
git commit -m "feat(test-engine): \$sstest ABI, host-held promises, JS prelude (async bridge core)"
```

---

## Task 4: Locator resolution

**Files:**
- Create: `crates/superui_test_engine/src/locator.rs`
- Modify: `crates/superui_test_engine/src/lib.rs`
- Test: unit tests in `locator.rs`

**Interfaces:**
- Produces:
  ```rust
  #[derive(serde::Deserialize, Clone, Debug, Default)]
  pub struct LocatorStep { pub sel: String, #[serde(default)] pub has_text: Option<String> }
  #[derive(serde::Deserialize, Clone, Debug, Default)]
  pub struct LocatorSpec { pub steps: Vec<LocatorStep>, #[serde(default)] pub nth: Option<usize> }
  pub fn resolve_locator(dom: &superui_dom::Dom, spec: &LocatorSpec) -> Vec<superui_dom::NodeId>;
  ```
  `serde(rename_all)` note: the prelude emits `hasText`; add `#[serde(rename = "hasText")]` on `has_text`.

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::{resolve_locator, LocatorSpec, LocatorStep};
    use superui_dom::Dom;

    fn dom() -> (Dom, ) {
        let mut d = Dom::new();
        let doc = d.document();
        let body = d.create_element("body");
        d.append_child(doc, body).unwrap();
        for (cls, txt) in [("tab", "MAIN"), ("tab", "SETTINGS")] {
            let el = d.create_element("div");
            d.set_attribute(el, "class", cls).unwrap();
            let t = d.create_text(txt);
            d.append_child(el, t).unwrap();
            d.append_child(body, el).unwrap();
        }
        (d,)
    }

    #[test]
    fn resolves_selector_with_has_text() {
        let (d,) = dom();
        let spec = LocatorSpec {
            steps: vec![LocatorStep { sel: ".tab".into(), has_text: Some("SETTINGS".into()) }],
            nth: None,
        };
        let got = resolve_locator(&d, &spec);
        assert_eq!(got.len(), 1);
        assert_eq!(d.text_content(got[0]), "SETTINGS");
    }

    #[test]
    fn resolves_nth() {
        let (d,) = dom();
        let spec = LocatorSpec {
            steps: vec![LocatorStep { sel: ".tab".into(), has_text: None }],
            nth: Some(0),
        };
        assert_eq!(resolve_locator(&d, &spec).len(), 1);
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p superui_test_engine locator`
Expected: FAIL (module not found).

- [ ] **Step 3: Implement**

```rust
use superui_dom::{Dom, NodeId};

#[derive(serde::Deserialize, Clone, Debug, Default)]
pub struct LocatorStep {
    pub sel: String,
    #[serde(rename = "hasText", default)]
    pub has_text: Option<String>,
}

#[derive(serde::Deserialize, Clone, Debug, Default)]
pub struct LocatorSpec {
    pub steps: Vec<LocatorStep>,
    #[serde(default)]
    pub nth: Option<usize>,
}

pub fn resolve_locator(dom: &Dom, spec: &LocatorSpec) -> Vec<NodeId> {
    // Start from a single virtual scope: the document.
    let mut scopes = vec![dom.document()];
    for step in &spec.steps {
        let mut next = Vec::new();
        for &scope in &scopes {
            for cand in dom.query_selector_all(scope, &step.sel) {
                if let Some(t) = &step.has_text {
                    if !dom.text_content(cand).contains(t.as_str()) {
                        continue;
                    }
                }
                if !next.contains(&cand) {
                    next.push(cand);
                }
            }
        }
        scopes = next;
    }
    match spec.nth {
        Some(i) => scopes.into_iter().nth(i).into_iter().collect(),
        None => scopes,
    }
}
```

Add `pub mod locator;` to `lib.rs`.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p superui_test_engine locator`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/superui_test_engine/src
git commit -m "feat(test-engine): locator spec resolution (selector + hasText + nth + chaining)"
```

---

## Task 5: Command model + driver loop + input actions

**Files:**
- Modify: `crates/superui_test_engine/src/command.rs` (extend `Command`)
- Create: `crates/superui_test_engine/src/driver.rs`
- Modify: `crates/superui_test_engine/src/host.rs` (add `install_abi_into_runtime`, `run_spec`)
- Modify: `crates/superui_test_engine/src/lib.rs`
- Test: `crates/superui_test_engine/tests/actions.rs`

**Interfaces:**
- Consumes: `abi::*`, `resolve_locator`, `LocatorSpec`, `UiRuntime`, `PendingDomEvents`, `PendingDomEvent`.
- Produces:
  - Extended `Command`:
    ```rust
    #[serde(tag = "type", rename_all = "camelCase")]
    pub enum Command {
        Noop,
        Click { locator: crate::locator::LocatorSpec },
        Fill { locator: crate::locator::LocatorSpec, text: String },
        Press { locator: crate::locator::LocatorSpec, key: String },
        Hover { locator: crate::locator::LocatorSpec },
        Expect { /* filled in Task 6 */ #[serde(flatten)] raw: serde_json::Value },
    }
    ```
  - `pub fn run_spec(app: &mut App, spec_js: &str) -> Vec<crate::trace::TestResult>` (Trace type stubbed in Task 7; for this task return a simpler `Vec<SpecTestResult>` and upgrade in Task 7).
  - For this task, define a minimal result: `pub struct SpecOutcome { pub name: String, pub passed: bool, pub error: Option<String> }` and `pub fn run_spec(app: &mut App, spec_js: &str) -> Vec<SpecOutcome>`.

- [ ] **Step 1: Write the failing test**

`crates/superui_test_engine/tests/actions.rs`:

```rust
use superui_test_engine::host::{build_headless_app, HostProject};
use superui_test_engine::driver::run_spec;
use superui_test_engine::transpile::transpile_spec;

fn project() -> HostProject {
    HostProject {
        html: "<!doctype html><html><body></body></html>".into(),
        css: String::new(),
        js_or_tsx: r#"
            import { createSignal, Show, render } from "supersolid";
            function App() {
                const [open, setOpen] = createSignal(false);
                return <div>
                    <div id="btn" onClick={() => setOpen(true)}>open</div>
                    <Show when={open()}><div id="panel">PANEL</div></Show>
                </div>;
            }
            render(App, document.body);
        "#.into(),
        tsx: true,
    }
}

#[test]
fn click_reveals_panel() {
    let mut app = build_headless_app(&project());
    let spec = r#"
        import { test, expect } from "superui/test";
        test("opens panel", async ({ page }) => {
            await page.locator("#btn").click();
            await expect(page.locator("#panel")).toHaveText("PANEL");
        });
    "#;
    let js = transpile_spec(spec, "t.spec.ts").unwrap();
    let results = run_spec(&mut app, &js);
    assert_eq!(results.len(), 1);
    assert!(results[0].passed, "error: {:?}", results[0].error);
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p superui_test_engine --test actions`
Expected: FAIL (module `driver` / `run_spec` missing).

- [ ] **Step 3: Extend `command.rs`**

Replace the enum with the extended version above. Add `use serde_json` where needed. Keep `Queued` unchanged.

- [ ] **Step 4: Implement `host` mount-with-abi helpers**

Add to `host.rs`:

```rust
use crate::abi;

/// After mount, install the $sstest ABI into the live runtime's Boa context.
pub fn install_abi(app: &mut App) {
    let mut rt = app.world_mut().remove_non_send_resource::<UiRuntime>().expect("mounted");
    abi::install(rt.engine.context_mut());
    app.world_mut().insert_non_send_resource(rt);
}
```

Confirm `UiRuntime.engine` is public and `BoaEngine::context_mut(&mut self) -> &mut boa_engine::Context` exists (reported present). If `engine` is private, add a `pub fn context_mut(&mut self) -> &mut Context` accessor to `UiRuntime` in `crates/superui_bridge/src/runtime.rs` and export it — a small, justified change; include it in this commit.

- [ ] **Step 5: Implement `driver.rs`**

```rust
use bevy::prelude::*;
use superui_bridge::{PendingDomEvent, PendingDomEvents, UiRuntime};
use superui_dom::NodeId;

use crate::abi::{self, JsPromiseHandle, RegisteredTest};
use crate::command::Command;
use crate::locator::{resolve_locator, LocatorSpec};

pub struct SpecOutcome {
    pub name: String,
    pub passed: bool,
    pub error: Option<String>,
}

const MAX_ITERS_PER_TEST: usize = 2000;
const SETTLE_TICKS: usize = 2;

pub fn run_spec(app: &mut App, spec_js: &str) -> Vec<SpecOutcome> {
    // Ensure UI mounted + ABI installed.
    let root = crate::host::mount(app);
    let _ = root;
    crate::host::install_abi(app);

    // Evaluate the spec to register tests.
    with_ctx(app, |ctx| {
        ctx.eval(boa_engine::Source::from_bytes(spec_js.as_bytes()))
            .map_err(|e| e.to_string())
    })
    .expect("spec eval");

    let tests = with_ctx(app, |ctx| abi::take_registered_tests(ctx));
    let mut out = Vec::new();
    for t in &tests {
        out.push(run_one(app, t));
    }
    out
}

fn run_one(app: &mut App, test: &RegisteredTest) -> SpecOutcome {
    let handle: JsPromiseHandle = with_ctx(app, |ctx| abi::run_test(ctx, test));
    // In-flight side-effecting commands awaiting settle: (id, remaining ticks).
    let mut inflight: Vec<(u64, usize)> = Vec::new();

    for _ in 0..MAX_ITERS_PER_TEST {
        // 1. Drain newly enqueued commands and start executing them.
        let queued = with_ctx(app, |ctx| abi::drain_queue(ctx));
        for q in queued {
            match &q.command {
                Command::Noop => {
                    with_ctx(app, |ctx| abi::resolve(ctx, q.id, r#"{"ok":true,"value":null}"#));
                }
                Command::Click { locator } => {
                    dispatch(app, locator, "click");
                    inflight.push((q.id, SETTLE_TICKS));
                }
                Command::Hover { locator } => {
                    dispatch(app, locator, "mouseover");
                    inflight.push((q.id, SETTLE_TICKS));
                }
                Command::Fill { locator, text } => {
                    fill(app, locator, text);
                    inflight.push((q.id, SETTLE_TICKS));
                }
                Command::Press { locator, key } => {
                    press(app, locator, key);
                    inflight.push((q.id, SETTLE_TICKS));
                }
                Command::Expect { .. } => {
                    // Implemented in Task 6; until then resolve ok to keep the loop moving.
                    with_ctx(app, |ctx| abi::resolve(ctx, q.id, r#"{"ok":true,"value":null}"#));
                }
            }
        }

        // 2. Tick Bevy (applies events, reconciles, runs Boa jobs so awaits resume).
        app.update();

        // 3. Resolve settled in-flight commands.
        let settled = !app.world().non_send_resource::<UiRuntime>().dirty;
        if settled {
            let ready: Vec<u64> = {
                inflight.iter_mut().for_each(|e| e.1 = e.1.saturating_sub(1));
                inflight.iter().filter(|e| e.1 == 0).map(|e| e.0).collect()
            };
            for id in ready {
                with_ctx(app, |ctx| abi::resolve(ctx, id, r#"{"ok":true,"value":null}"#));
            }
            inflight.retain(|e| e.1 > 0);
            // Pump the continuations enqueued by the resolves.
            with_ctx(app, |ctx| {
                let _ = ctx.run_jobs();
            });
        }

        // 4. Done?
        if inflight.is_empty() {
            if let Some(res) = with_ctx(app, |ctx| abi::promise_settled(ctx, &handle)) {
                return match res {
                    Ok(()) => SpecOutcome { name: test.name.clone(), passed: true, error: None },
                    Err(e) => SpecOutcome { name: test.name.clone(), passed: false, error: Some(e) },
                };
            }
        }
    }
    SpecOutcome {
        name: test.name.clone(),
        passed: false,
        error: Some("timed out".into()),
    }
}

fn with_ctx<R>(app: &mut App, f: impl FnOnce(&mut boa_engine::Context) -> R) -> R {
    let mut rt = app.world_mut().remove_non_send_resource::<UiRuntime>().expect("runtime");
    let r = f(rt.engine.context_mut());
    app.world_mut().insert_non_send_resource(rt);
    r
}

fn resolve_nodes(app: &App, spec: &LocatorSpec) -> Vec<NodeId> {
    let rt = app.world().non_send_resource::<UiRuntime>();
    let dom = rt.dom.borrow();
    resolve_locator(&dom, spec)
}

fn dispatch(app: &mut App, spec: &LocatorSpec, event: &str) {
    if let Some(&node) = resolve_nodes(app, spec).first() {
        app.world_mut()
            .resource_mut::<PendingDomEvents>()
            .0
            .push(PendingDomEvent::new(node, event));
    }
}

fn fill(app: &mut App, spec: &LocatorSpec, text: &str) {
    if let Some(&node) = resolve_nodes(app, spec).first() {
        {
            let rt = app.world().non_send_resource::<UiRuntime>();
            rt.dom.borrow_mut().set_value(node, text);
        }
        app.world_mut()
            .resource_mut::<PendingDomEvents>()
            .0
            .push(PendingDomEvent::new(node, "input"));
    }
}

fn press(app: &mut App, spec: &LocatorSpec, key: &str) {
    if let Some(&node) = resolve_nodes(app, spec).first() {
        // Phase-1: dispatch a keydown DOM event; text mutation for printable keys
        // is handled by the app's own handlers where wired.
        let _ = key;
        app.world_mut()
            .resource_mut::<PendingDomEvents>()
            .0
            .push(PendingDomEvent::new(node, "keydown"));
    }
}
```

Add `pub mod driver;` to `lib.rs`. Confirm `PendingDomEvent::new(node, "input")` and `set_value` signatures match (`crates/superui_dom/src/props.rs`, `crates/superui_bridge/src/events.rs`).

- [ ] **Step 6: Run to verify it passes**

Run: `cargo test -p superui_test_engine --test actions -- --nocapture`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/superui_test_engine
git commit -m "feat(test-engine): driver loop + click/fill/press/hover with settle-based resolution"
```

---

## Task 6: expect matchers with auto-wait

**Files:**
- Modify: `crates/superui_test_engine/src/command.rs` (replace `Expect` with a typed variant)
- Create: `crates/superui_test_engine/src/matchers.rs`
- Modify: `crates/superui_test_engine/src/driver.rs` (evaluate `Expect` with retry-until-timeout)
- Modify: `crates/superui_test_engine/src/lib.rs`
- Test: `crates/superui_test_engine/tests/matchers.rs`

**Interfaces:**
- Produces:
  - `command.rs`: replace `Expect { raw }` with
    ```rust
    Expect {
        matcher: String,          // "visible" | "text" | "count" | "class" | "attribute" | "screenshot"
        #[serde(default)] locator: Option<crate::locator::LocatorSpec>,
        #[serde(default)] expected: serde_json::Value,
        #[serde(default)] opts: serde_json::Value,
    }
    ```
  - `matchers.rs`: `pub fn evaluate(app: &App, cmd_matcher: &str, locator: &Option<LocatorSpec>, expected: &serde_json::Value) -> Result<(), String>` — returns `Ok(())` when the assertion currently holds, `Err(msg)` otherwise. (Screenshot is handled in Task 9, not here.)

- [ ] **Step 1: Write the failing test**

`crates/superui_test_engine/tests/matchers.rs`:

```rust
use superui_test_engine::host::{build_headless_app, HostProject};
use superui_test_engine::driver::run_spec;
use superui_test_engine::transpile::transpile_spec;

fn project() -> HostProject {
    HostProject {
        html: "<!doctype html><html><body></body></html>".into(),
        css: String::new(),
        js_or_tsx: r#"
            import { createSignal, For, render } from "supersolid";
            function App() {
                const [items, setItems] = createSignal(["a"]);
                return <div>
                    <div id="add" onClick={() => setItems([...items(), "b"])}>add</div>
                    <ul>{<For each={items()}>{(x) => <li class="item">{x}</li>}</For>}</ul>
                </div>;
            }
            render(App, document.body);
        "#.into(),
        tsx: true,
    }
}

#[test]
fn count_and_text_and_class_matchers_autowait() {
    let mut app = build_headless_app(&project());
    let spec = r#"
        import { test, expect } from "superui/test";
        test("adds item", async ({ page }) => {
            await expect(page.locator(".item")).toHaveCount(1);
            await page.locator("#add").click();
            await expect(page.locator(".item")).toHaveCount(2);
            await expect(page.locator(".item").nth(1)).toHaveText("b");
            await expect(page.locator(".item").first()).toHaveClass(/item/);
        });
    "#;
    let js = transpile_spec(spec, "t.spec.ts").unwrap();
    let results = run_spec(&mut app, &js);
    assert!(results[0].passed, "error: {:?}", results[0].error);
}

#[test]
fn failing_matcher_reports_error() {
    let mut app = build_headless_app(&project());
    let spec = r#"
        import { test, expect } from "superui/test";
        test("bad count", async ({ page }) => {
            await expect(page.locator(".item")).toHaveCount(99);
        });
    "#;
    let js = transpile_spec(spec, "t.spec.ts").unwrap();
    let results = run_spec(&mut app, &js);
    assert!(!results[0].passed);
    assert!(results[0].error.as_deref().unwrap_or("").contains("count"));
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p superui_test_engine --test matchers`
Expected: FAIL (matcher not implemented → the `toHaveCount(99)` currently resolves ok, so `failing_matcher_reports_error` fails; and the passing test may spuriously pass — both indicate work needed).

- [ ] **Step 3: Implement `matchers.rs`**

```rust
use bevy::prelude::*;
use superui_bridge::UiRuntime;
use crate::locator::{resolve_locator, LocatorSpec};

pub fn evaluate(
    app: &App,
    matcher: &str,
    locator: &Option<LocatorSpec>,
    expected: &serde_json::Value,
) -> Result<(), String> {
    let rt = app.world().non_send_resource::<UiRuntime>();
    let dom = rt.dom.borrow();
    let nodes = match locator {
        Some(spec) => resolve_locator(&dom, spec),
        None => vec![],
    };
    match matcher {
        "count" => {
            let want = expected.as_u64().unwrap_or(0) as usize;
            if nodes.len() == want { Ok(()) }
            else { Err(format!("expected count {want}, got {}", nodes.len())) }
        }
        "visible" => {
            if nodes.is_empty() { return Err("not visible: no match".into()); }
            // Visibility: attached + not inline display:none. (Layout-size check
            // via ComputedNode can be added once render host is used.)
            let node = nodes[0];
            let hidden = dom.get_attribute(node, "style")
                .map(|s| s.replace(' ', "").contains("display:none"))
                .unwrap_or(false);
            if hidden { Err("element has display:none".into()) } else { Ok(()) }
        }
        "text" => {
            let want = expected.as_str().unwrap_or("");
            let node = nodes.first().ok_or_else(|| "no match for text".to_string())?;
            let got = dom.text_content(*node);
            if got == want { Ok(()) } else { Err(format!("expected text {want:?}, got {got:?}")) }
        }
        "class" => {
            let pat = expected.as_str().unwrap_or("");
            let node = nodes.first().ok_or_else(|| "no match for class".to_string())?;
            let classes = dom.classes(*node).join(" ");
            // Phase-1: substring match of the regex source (no full regex engine).
            if classes.contains(pat.trim_matches(|c| c == '\\' || c == 'b')) { Ok(()) }
            else { Err(format!("expected class matching {pat:?} in {classes:?}")) }
        }
        "attribute" => {
            let name = expected.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let node = nodes.first().ok_or_else(|| "no match for attribute".to_string())?;
            let got = dom.get_attribute(*node, name).map(|s| s.to_string());
            match expected.get("value").and_then(|v| v.as_str()) {
                Some(want) if got.as_deref() == Some(want) => Ok(()),
                Some(want) => Err(format!("attribute {name}: expected {want:?}, got {got:?}")),
                None if got.is_some() => Ok(()),
                None => Err(format!("attribute {name} not present")),
            }
        }
        other => Err(format!("unknown matcher: {other}")),
    }
}
```

Note: the class matcher's regex-source handling is intentionally minimal (substring). Full JS-regex fidelity is out of scope; document this in the module doc-comment.

- [ ] **Step 4: Wire `Expect` into the driver**

In `driver.rs`, replace the `Command::Expect { .. }` arm in step 1 with an auto-wait poller. Add near the top:

```rust
const EXPECT_TIMEOUT_ITERS: usize = 120;
```

Add a second in-flight list for expects: `let mut expects: Vec<ExpectInFlight> = Vec::new();` with

```rust
struct ExpectInFlight {
    id: u64,
    matcher: String,
    locator: Option<crate::locator::LocatorSpec>,
    expected: serde_json::Value,
    remaining: usize,
    last_err: String,
}
```

In the drain arm:

```rust
Command::Expect { matcher, locator, expected, .. } => {
    expects.push(ExpectInFlight {
        id: q.id,
        matcher: matcher.clone(),
        locator: locator.clone(),
        expected: expected.clone(),
        remaining: EXPECT_TIMEOUT_ITERS,
        last_err: String::new(),
    });
}
```

After `app.update()` (step 3 region), evaluate expects each iteration:

```rust
let mut still = Vec::new();
for mut e in expects.drain(..) {
    if e.matcher == "screenshot" {
        // Task 9 handles screenshots; for now pass-through.
        with_ctx(app, |ctx| abi::resolve(ctx, e.id, r#"{"ok":true,"value":null}"#));
        continue;
    }
    match crate::matchers::evaluate(app, &e.matcher, &e.locator, &e.expected) {
        Ok(()) => {
            with_ctx(app, |ctx| abi::resolve(ctx, e.id, r#"{"ok":true,"value":null}"#));
        }
        Err(msg) => {
            e.last_err = msg;
            e.remaining -= 1;
            if e.remaining == 0 {
                let payload = serde_json::json!({ "ok": false, "error": e.last_err }).to_string();
                with_ctx(app, |ctx| abi::resolve(ctx, e.id, &payload));
            } else {
                still.push(e);
            }
        }
    }
}
expects = still;
with_ctx(app, |ctx| { let _ = ctx.run_jobs(); });
```

Update the "Done?" check to also require `expects.is_empty()`.

- [ ] **Step 5: Run to verify it passes**

Run: `cargo test -p superui_test_engine --test matchers -- --nocapture`
Expected: PASS (both tests).

- [ ] **Step 6: Commit**

```bash
git add crates/superui_test_engine
git commit -m "feat(test-engine): expect matchers with auto-wait (visible/text/count/class/attribute)"
```

---

## Task 7: Trace + structured results

**Files:**
- Create: `crates/superui_test_engine/src/trace.rs`
- Modify: `crates/superui_test_engine/src/driver.rs` (record steps; return `TestResult`)
- Modify: `crates/superui_test_engine/src/lib.rs`
- Test: `crates/superui_test_engine/tests/trace.rs`

**Interfaces:**
- Produces:
  ```rust
  #[derive(Clone, Debug, serde::Serialize)]
  pub enum StepStatus { Ok, Failed(String) }
  #[derive(Clone, Debug, serde::Serialize)]
  pub struct Step {
      pub index: usize,
      pub action: String,        // e.g. "click #btn", "expect count=2"
      pub status: StepStatus,
      pub dom_after: String,     // serialized subtree or whole body
      pub screenshot: Option<String>, // filename, filled in Task 9
  }
  #[derive(Clone, Debug, serde::Serialize)]
  pub struct TestResult {
      pub name: String,
      pub passed: bool,
      pub error: Option<String>,
      pub steps: Vec<Step>,
  }
  pub fn serialize_body(dom: &superui_dom::Dom) -> String; // HTML-ish dump of <body>
  ```
- `driver::run_spec` now returns `Vec<TestResult>` (replace `SpecOutcome`). Update Task 5/6 test files to read `results[0].passed` (already compatible field name).

- [ ] **Step 1: Write the failing test**

`crates/superui_test_engine/tests/trace.rs`:

```rust
use superui_test_engine::host::{build_headless_app, HostProject};
use superui_test_engine::driver::run_spec;
use superui_test_engine::transpile::transpile_spec;

#[test]
fn records_a_step_per_action() {
    let mut app = build_headless_app(&HostProject {
        html: "<!doctype html><html><body></body></html>".into(),
        css: String::new(),
        js_or_tsx: r#"
            import { render } from "supersolid";
            render(() => <div id="a" class="x">A</div>, document.body);
        "#.into(),
        tsx: true,
    });
    let spec = r#"
        import { test, expect } from "superui/test";
        test("has step", async ({ page }) => {
            await expect(page.locator("#a")).toHaveText("A");
        });
    "#;
    let js = transpile_spec(spec, "t.spec.ts").unwrap();
    let results = run_spec(&mut app, &js);
    assert!(results[0].passed);
    assert_eq!(results[0].steps.len(), 1);
    assert!(results[0].steps[0].dom_after.contains("id=\"a\""), "{}", results[0].steps[0].dom_after);
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p superui_test_engine --test trace`
Expected: FAIL (`steps` field missing).

- [ ] **Step 3: Implement `trace.rs`**

```rust
use superui_dom::{Dom, NodeId, NodeKind};

#[derive(Clone, Debug, serde::Serialize)]
pub enum StepStatus { Ok, Failed(String) }

#[derive(Clone, Debug, serde::Serialize)]
pub struct Step {
    pub index: usize,
    pub action: String,
    pub status: StepStatus,
    pub dom_after: String,
    pub screenshot: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct TestResult {
    pub name: String,
    pub passed: bool,
    pub error: Option<String>,
    pub steps: Vec<Step>,
}

pub fn serialize_body(dom: &Dom) -> String {
    let doc = dom.document();
    let body = dom.query_selector(doc, "body").unwrap_or(doc);
    let mut out = String::new();
    write_node(dom, body, &mut out, 0);
    out
}

fn write_node(dom: &Dom, node: NodeId, out: &mut String, depth: usize) {
    let pad = "  ".repeat(depth);
    match dom.get(node).map(|n| &n.kind) {
        Some(NodeKind::Element(_)) | Some(NodeKind::Document) => {
            let tag = dom.tag(node).unwrap_or("body");
            let mut attrs = String::new();
            for (k, v) in dom.attributes(node) {
                attrs.push_str(&format!(" {k}=\"{v}\""));
            }
            out.push_str(&format!("{pad}<{tag}{attrs}>\n"));
            for &c in dom.children(node) {
                write_node(dom, c, out, depth + 1);
            }
            out.push_str(&format!("{pad}</{tag}>\n"));
        }
        Some(NodeKind::Text(t)) => {
            if !t.trim().is_empty() {
                out.push_str(&format!("{pad}{}\n", t.trim()));
            }
        }
        None => {}
    }
}
```

- [ ] **Step 4: Record steps in the driver**

In `driver.rs`, change `SpecOutcome` → `crate::trace::TestResult` and, whenever a command resolves (click/fill/press/hover/expect), push a `Step` capturing `action`, `status`, and `serialize_body(&dom)`. Increment a per-test step counter. On test completion set `passed`/`error`. Keep a `Vec<Step>` per test; return it in the `TestResult`.

Minimal helper:

```rust
fn snapshot_body(app: &App) -> String {
    let rt = app.world().non_send_resource::<UiRuntime>();
    crate::trace::serialize_body(&rt.dom.borrow())
}
```

For each resolved command add:

```rust
steps.push(crate::trace::Step {
    index: steps.len(),
    action: action_label,     // format like "click #btn" / "expect count"
    status: crate::trace::StepStatus::Ok, // or Failed(msg)
    dom_after: snapshot_body(app),
    screenshot: None,
});
```

Update Task 5's `actions.rs` and Task 6's `matchers.rs` tests only if they referenced `SpecOutcome` by type name (they read `.passed`/`.error`, which still exist — no change needed).

- [ ] **Step 5: Run to verify it passes**

Run: `cargo test -p superui_test_engine --test trace && cargo test -p superui_test_engine --test actions && cargo test -p superui_test_engine --test matchers`
Expected: PASS all.

- [ ] **Step 6: Commit**

```bash
git add crates/superui_test_engine
git commit -m "feat(test-engine): per-step trace (action, status, DOM-after) + TestResult"
```

---

## Task 8: Offscreen render host + screenshot capture

Integration task (needs a GPU/software adapter). Unit-test the plumbing; verify capture with an ignored-by-default integration test that runs locally.

**Files:**
- Create: `crates/superui_test_engine/src/render.rs`
- Modify: `crates/superui_test_engine/src/host.rs` (add `build_render_app`)
- Modify: `crates/superui_test_engine/src/lib.rs`
- Modify: `crates/superui_test_engine/Cargo.toml` (enable Bevy render features)
- Test: `crates/superui_test_engine/tests/render_capture.rs` (`#[ignore]` by default)

**Interfaces:**
- Produces:
  - `pub struct CapturedImage { pub width: u32, pub height: u32, pub rgba: Vec<u8> }`
  - `pub fn build_render_app(project: &HostProject, width: u32, height: u32) -> App` — like `build_headless_app` but with the render pipeline + a camera targeting an offscreen `Image`.
  - `pub fn capture(app: &mut App) -> Option<CapturedImage>` — spawns `Screenshot::image(target)`, ticks until `ScreenshotCaptured` fires, returns RGBA.

- [ ] **Step 1: Enable render features**

In `Cargo.toml` change the `bevy` dep to include render:

```toml
bevy = { version = "0.17", features = ["bevy_render", "bevy_core_pipeline", "bevy_winit", "png"] }
```

Confirm the exact feature names against `bevy = 0.17` (`bevy_render`, `bevy_core_pipeline` exist; `bevy_winit` needed only if using a window — the offscreen `RenderTarget::Image` path can avoid it). Prefer no window: use `RenderPlugin` + `ImagePlugin` + `CorePipelinePlugin` and a camera with `RenderTarget::Image`.

- [ ] **Step 2: Implement `render.rs`**

```rust
use bevy::prelude::*;
use bevy::render::render_resource::{
    Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
};
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};
use bevy::image::Image;

#[derive(Resource, Clone)]
pub struct RenderTargetHandle(pub Handle<Image>);

pub struct CapturedImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

pub fn make_target_image(width: u32, height: u32) -> Image {
    let size = Extent3d { width, height, depth_or_array_layers: 1 };
    let mut image = Image {
        texture_descriptor: TextureDescriptor {
            label: Some("superui_test_target"),
            size,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8UnormSrgb,
            mip_level_count: 1,
            sample_count: 1,
            usage: TextureUsages::TEXTURE_BINDING
                | TextureUsages::COPY_DST
                | TextureUsages::RENDER_ATTACHMENT
                | TextureUsages::COPY_SRC,
            view_formats: &[],
        },
        ..default()
    };
    image.resize(size);
    image
}

#[derive(Resource, Default)]
struct CaptureSink(std::sync::Arc<std::sync::Mutex<Option<CapturedImage>>>);
```

The `capture` function spawns a `Screenshot::image(handle)` and registers an observer that writes into the sink:

```rust
pub fn capture(app: &mut App) -> Option<CapturedImage> {
    let handle = app.world().resource::<RenderTargetHandle>().0.clone();
    let sink = app.world().resource::<CaptureSink>().0.clone();

    app.world_mut()
        .spawn(Screenshot::image(handle))
        .observe(move |trigger: On<ScreenshotCaptured>, mut commands: Commands| {
            let img = &trigger.event().0; // ScreenshotCaptured(pub Image) OR named field — confirm
            let width = img.width();
            let height = img.height();
            let rgba = img.data.clone().unwrap_or_default();
            *sink.lock().unwrap() = Some(CapturedImage { width, height, rgba });
            // Despawn the screenshot entity next.
            let _ = &mut commands;
        });

    for _ in 0..64 {
        app.update();
        if sink.lock().unwrap().is_some() {
            break;
        }
    }
    sink.lock().unwrap().take()
}
```

Confirm during implementation (against `bevy_render-0.17.3/src/view/window/screenshot.rs`):
- `ScreenshotCaptured` field access: the struct is `pub struct ScreenshotCaptured { pub image: Image, .. }` (line 44 region shows a struct with an `image` field used at line 203 `ScreenshotCaptured { image, entity }`). Use `trigger.event().image`.
- Observer trigger type in Bevy 0.17 is `On<E>` (matches `save_to_disk(path: ...) -> impl FnMut(On<ScreenshotCaptured>)` at line 129). Use that signature exactly.
- `Image::data` is `Option<Vec<u8>>` in 0.17; adjust `.clone().unwrap_or_default()` accordingly. `img.width()`/`img.height()` exist.

- [ ] **Step 3: Implement `build_render_app`**

Mirror `build_headless_app` but add `DefaultPlugins`-equivalent render set without winit, insert the target image asset, and spawn a 2D camera targeting it:

```rust
pub fn build_render_app(project: &HostProject, width: u32, height: u32) -> App {
    // ... same MemoryAssetReader asset registration as build_headless_app ...
    let mut app = App::new();
    // asset source registration (copy from build_headless_app)
    app.add_plugins(bevy::DefaultPlugins.build().disable::<bevy::winit::WinitPlugin>());
    app.add_plugins(SuperUiPlugin);
    app.init_resource::<crate::render::CaptureSink>();

    let image = crate::render::make_target_image(width, height);
    let handle = app.world_mut().resource_mut::<Assets<Image>>().add(image);
    app.insert_resource(crate::render::RenderTargetHandle(handle.clone()));
    app.world_mut().spawn((
        Camera2d,
        Camera { target: bevy::render::camera::RenderTarget::Image(handle.into()), ..default() },
    ));
    app.finish();
    app.insert_resource(HostAssetPaths { js: /* as in headless */, tsx: project.tsx });
    app
}
```

Confirm `RenderTarget::Image` takes an `ImageRenderTarget`/`Handle<Image>.into()` in 0.17 (grep `bevy_camera`/`bevy_render` camera module). Disabling `WinitPlugin` lets the app run offscreen; if `DefaultPlugins` still requires a window, spawn a `Window` with `visible: false` or use the `RenderTarget::Image` camera only.

- [ ] **Step 4: Write an ignored integration test**

`crates/superui_test_engine/tests/render_capture.rs`:

```rust
use superui_test_engine::host::HostProject;
use superui_test_engine::render;

#[test]
#[ignore = "requires a GPU/software render adapter"]
fn captures_nonempty_frame() {
    let project = HostProject {
        html: "<!doctype html><html><body></body></html>".into(),
        css: "#box{width:100px;height:100px;background-color:#ff0000;}".into(),
        js_or_tsx: r#"import { render } from "supersolid";
            render(() => <div id="box"></div>, document.body);"#.into(),
        tsx: true,
    };
    let mut app = render::build_render_app_and_mount(&project, 320, 240);
    let img = render::capture(&mut app).expect("captured");
    assert_eq!(img.width, 320);
    assert!(img.rgba.iter().any(|&b| b != 0), "frame is all zeros");
}
```

Add a `build_render_app_and_mount` convenience in `render.rs`/`host.rs` that builds, mounts, installs ABI, and ticks a few frames so the UI renders.

- [ ] **Step 5: Run**

Run (locally, with adapter): `cargo test -p superui_test_engine --test render_capture -- --ignored --nocapture`
Expected: PASS locally. In CI without an adapter, it stays skipped (`#[ignore]`).

Also run the full suite to ensure render features didn't break headless: `cargo test -p superui_test_engine`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/superui_test_engine
git commit -m "feat(test-engine): offscreen render target + Screenshot::image capture"
```

---

## Task 9: PNG baselines, diff, `--update`, `toHaveScreenshot`

**Files:**
- Create: `crates/superui_test_engine/src/snapshot.rs`
- Modify: `crates/superui_test_engine/src/driver.rs` (handle `screenshot` matcher against captured frames)
- Modify: `crates/superui_test_engine/src/lib.rs`
- Test: `crates/superui_test_engine/src/snapshot.rs` unit tests (pure diff math — no GPU)

**Interfaces:**
- Produces:
  ```rust
  pub struct SnapshotConfig { pub dir: std::path::PathBuf, pub update: bool, pub max_diff_ratio: f64, pub platform: String }
  pub fn baseline_path(cfg: &SnapshotConfig, spec_file: &str, name: &str) -> std::path::PathBuf;
  // Compare `actual` (RGBA) to baseline; write baseline if update or missing.
  // Returns Ok(()) on match/update, Err(msg) on mismatch (writes -actual/-diff).
  pub fn match_screenshot(cfg: &SnapshotConfig, spec_file: &str, name: &str,
      width: u32, height: u32, actual: &[u8]) -> Result<(), String>;
  pub fn diff_ratio(a: &[u8], b: &[u8], tol: u8) -> f64; // fraction of differing pixels
  ```

- [ ] **Step 1: Write the failing test (pure diff math)**

Append to `snapshot.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::diff_ratio;

    #[test]
    fn identical_images_have_zero_diff() {
        let a = vec![10u8; 400];
        assert_eq!(diff_ratio(&a, &a, 0), 0.0);
    }

    #[test]
    fn one_changed_pixel_reports_fraction() {
        let mut a = vec![0u8; 4 * 4]; // 4 RGBA pixels
        let mut b = a.clone();
        b[0] = 255; // change pixel 0 red channel
        let r = diff_ratio(&a, &b, 0);
        assert!((r - 0.25).abs() < 1e-9, "got {r}");
        let _ = &mut a;
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p superui_test_engine snapshot`
Expected: FAIL (`diff_ratio` missing).

- [ ] **Step 3: Implement `snapshot.rs`**

```rust
use std::path::{Path, PathBuf};

pub struct SnapshotConfig {
    pub dir: PathBuf,
    pub update: bool,
    pub max_diff_ratio: f64,
    pub platform: String,
}

pub fn baseline_path(cfg: &SnapshotConfig, spec_file: &str, name: &str) -> PathBuf {
    let stem = name.trim_end_matches(".png");
    cfg.dir
        .join("__snapshots__")
        .join(spec_file)
        .join(format!("{stem}-{}.png", cfg.platform))
}

/// Fraction of pixels differing by more than `tol` in any channel.
pub fn diff_ratio(a: &[u8], b: &[u8], tol: u8) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 1.0;
    }
    let pixels = a.len() / 4;
    let mut diff = 0usize;
    for i in 0..pixels {
        let o = i * 4;
        let d = (0..4).any(|c| a[o + c].abs_diff(b[o + c]) > tol);
        if d { diff += 1; }
    }
    diff as f64 / pixels as f64
}

pub fn match_screenshot(
    cfg: &SnapshotConfig,
    spec_file: &str,
    name: &str,
    width: u32,
    height: u32,
    actual: &[u8],
) -> Result<(), String> {
    let path = baseline_path(cfg, spec_file, name);
    let write_png = |p: &Path, w: u32, h: u32, data: &[u8]| -> Result<(), String> {
        if let Some(parent) = p.parent() { std::fs::create_dir_all(parent).map_err(|e| e.to_string())?; }
        image::save_buffer(p, data, w, h, image::ColorType::Rgba8).map_err(|e| e.to_string())
    };

    if cfg.update || !path.exists() {
        write_png(&path, width, height, actual)?;
        return Ok(());
    }
    let base = image::open(&path).map_err(|e| e.to_string())?.to_rgba8();
    if base.width() != width || base.height() != height {
        let actual_path = path.with_extension("actual.png");
        write_png(&actual_path, width, height, actual)?;
        return Err(format!("size mismatch: baseline {}x{} vs actual {}x{}", base.width(), base.height(), width, height));
    }
    let ratio = diff_ratio(base.as_raw(), actual, 4);
    if ratio <= cfg.max_diff_ratio {
        Ok(())
    } else {
        let actual_path = path.with_extension("actual.png");
        write_png(&actual_path, width, height, actual)?;
        // Simple diff image: white where differing, black otherwise.
        let mut diff = vec![0u8; actual.len()];
        for i in 0..(actual.len() / 4) {
            let o = i * 4;
            let differs = (0..4).any(|c| base.as_raw()[o + c].abs_diff(actual[o + c]) > 4);
            let v = if differs { 255 } else { 0 };
            diff[o] = v; diff[o + 1] = v; diff[o + 2] = v; diff[o + 3] = 255;
        }
        let diff_path = path.with_extension("diff.png");
        write_png(&diff_path, width, height, &diff)?;
        Err(format!("screenshot {name}: diff ratio {ratio:.4} > {:.4}", cfg.max_diff_ratio))
    }
}
```

- [ ] **Step 4: Wire `screenshot` matcher into the driver**

The driver, when it dequeues an `Expect { matcher: "screenshot", expected: <name> }`, must (a) `capture` a frame (only meaningful in a render app; in headless it resolves ok with a note), (b) call `match_screenshot`. Give `run_spec` a `SnapshotConfig` and a `capture` hook. Refactor `run_spec` signature to:

```rust
pub struct RunOptions {
    pub snapshot: Option<crate::snapshot::SnapshotConfig>,
    pub spec_file: String,
    pub render: bool,
}
pub fn run_spec_with(app: &mut App, spec_js: &str, opts: &RunOptions) -> Vec<crate::trace::TestResult>
```

Keep the old `run_spec(app, spec_js)` as a thin wrapper passing `RunOptions { snapshot: None, spec_file: "spec".into(), render: false }`. In the screenshot arm:

```rust
if e.matcher == "screenshot" {
    let name = e.expected.as_str().unwrap_or("screenshot").to_string();
    let result = if opts.render {
        match crate::render::capture(app) {
            Some(img) => match &opts.snapshot {
                Some(cfg) => crate::snapshot::match_screenshot(cfg, &opts.spec_file, &name, img.width, img.height, &img.rgba),
                None => Ok(()),
            },
            None => Err("screenshot capture failed".into()),
        }
    } else {
        Ok(()) // headless: no pixels; treated as pass with a note in the step
    };
    let payload = match &result {
        Ok(()) => r#"{"ok":true,"value":null}"#.to_string(),
        Err(msg) => serde_json::json!({"ok":false,"error":msg}).to_string(),
    };
    with_ctx(app, |ctx| abi::resolve(ctx, e.id, &payload));
    continue;
}
```

Add `pub mod snapshot; pub mod render;` to `lib.rs`.

- [ ] **Step 5: Run to verify it passes**

Run: `cargo test -p superui_test_engine snapshot`
Expected: PASS (diff math). Full suite: `cargo test -p superui_test_engine` → PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/superui_test_engine
git commit -m "feat(test-engine): PNG baseline/diff, __snapshots__ layout, toHaveScreenshot + --update"
```

---

## Task 10: CLI + config + discovery + reporter

**Files:**
- Create: `crates/superui_test_engine/src/config.rs`
- Create: `crates/superui_test_engine/src/report.rs`
- Create: `crates/superui_test_engine/src/bin/superui_test.rs`
- Modify: `crates/superui_test_engine/src/lib.rs`
- Test: `crates/superui_test_engine/src/config.rs` unit tests

**Interfaces:**
- Produces:
  ```rust
  // config.rs
  pub struct TestConfig { pub project: std::path::PathBuf, pub spec_dir: std::path::PathBuf, pub width: u32, pub height: u32, pub max_diff_ratio: f64 }
  pub fn load_config(path: &std::path::Path) -> Result<TestConfig, String>;
  pub fn discover_specs(spec_dir: &std::path::Path) -> Vec<std::path::PathBuf>; // *.spec.ts
  pub fn load_project(project_dir: &std::path::Path) -> Result<crate::host::HostProject, String>; // read index.html/style.css/app.tsx
  // report.rs
  pub fn print_summary(results: &[(String, Vec<crate::trace::TestResult>)]) -> bool; // returns all_passed
  pub fn write_html_report(path: &std::path::Path, results: &[(String, Vec<crate::trace::TestResult>)]) -> std::io::Result<()>;
  ```

- [ ] **Step 1: Write the failing config test**

Append to `config.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::load_config;
    #[test]
    fn parses_toml() {
        let dir = std::env::temp_dir().join("superui_test_cfg");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("superui.test.toml");
        std::fs::write(&p, "project = \"examples/game_menu/assets/ui/game_menu\"\nspecDir = \"examples/game_menu/tests\"\n").unwrap();
        let cfg = load_config(&p).unwrap();
        assert!(cfg.project.ends_with("game_menu"));
        assert_eq!(cfg.width, 1280); // default
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p superui_test_engine config`
Expected: FAIL (`load_config` missing).

- [ ] **Step 3: Implement `config.rs`**

```rust
use std::path::{Path, PathBuf};

pub struct TestConfig {
    pub project: PathBuf,
    pub spec_dir: PathBuf,
    pub width: u32,
    pub height: u32,
    pub max_diff_ratio: f64,
}

#[derive(serde::Deserialize)]
struct Raw {
    project: String,
    #[serde(rename = "specDir")]
    spec_dir: String,
    width: Option<u32>,
    height: Option<u32>,
    #[serde(rename = "maxDiffRatio")]
    max_diff_ratio: Option<f64>,
}

pub fn load_config(path: &Path) -> Result<TestConfig, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let raw: Raw = toml::from_str(&text).map_err(|e| e.to_string())?;
    let base = path.parent().unwrap_or(Path::new("."));
    Ok(TestConfig {
        project: base.join(&raw.project),
        spec_dir: base.join(&raw.spec_dir),
        width: raw.width.unwrap_or(1280),
        height: raw.height.unwrap_or(720),
        max_diff_ratio: raw.max_diff_ratio.unwrap_or(0.01),
    })
}

pub fn discover_specs(spec_dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(spec_dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.file_name().and_then(|n| n.to_str()).map(|n| n.ends_with(".spec.ts")).unwrap_or(false) {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

pub fn load_project(project_dir: &Path) -> Result<crate::host::HostProject, String> {
    let read = |name: &str| std::fs::read_to_string(project_dir.join(name)).map_err(|e| format!("{name}: {e}"));
    // Accept app.tsx (preferred) or app.generated.js.
    let (js, tsx) = if project_dir.join("app.tsx").exists() {
        (read("app.tsx")?, true)
    } else {
        (read("app.generated.js")?, false)
    };
    Ok(crate::host::HostProject {
        html: read("index.html")?,
        css: read("style.css").or_else(|_| read("theme.css")).unwrap_or_default(),
        js_or_tsx: js,
        tsx,
    })
}
```

- [ ] **Step 4: Implement `report.rs`**

```rust
use crate::trace::TestResult;

pub fn print_summary(results: &[(String, Vec<TestResult>)]) -> bool {
    let mut all_passed = true;
    for (file, tests) in results {
        for t in tests {
            let mark = if t.passed { "PASS" } else { all_passed = false; "FAIL" };
            println!("[{mark}] {file} › {}", t.name);
            if let Some(e) = &t.error {
                println!("       {e}");
            }
        }
    }
    all_passed
}

pub fn write_html_report(path: &std::path::Path, results: &[(String, Vec<TestResult>)]) -> std::io::Result<()> {
    let mut html = String::from("<!doctype html><meta charset=utf-8><title>superui-test</title><body>");
    for (file, tests) in results {
        html.push_str(&format!("<h2>{file}</h2>"));
        for t in tests {
            let color = if t.passed { "green" } else { "red" };
            html.push_str(&format!("<h3 style=\"color:{color}\">{} — {}</h3>", t.name, if t.passed {"passed"} else {"failed"}));
            if let Some(e) = &t.error { html.push_str(&format!("<pre>{e}</pre>")); }
            for s in &t.steps {
                html.push_str(&format!("<details><summary>step {}: {}</summary><pre>{}</pre></details>", s.index, s.action, html_escape(&s.dom_after)));
            }
        }
    }
    html.push_str("</body>");
    std::fs::write(path, html)
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}
```

- [ ] **Step 5: Implement the CLI binary**

`crates/superui_test_engine/src/bin/superui_test.rs`:

```rust
use std::path::PathBuf;
use superui_test_engine::{config, driver, report, snapshot, host, render, transpile};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let update = args.iter().any(|a| a == "--update");
    let ui = args.iter().any(|a| a == "--ui");
    let filter: Option<String> = args.iter().find(|a| !a.starts_with("--")).cloned();

    let cfg_path = PathBuf::from("superui.test.toml");
    let cfg = config::load_config(&cfg_path).unwrap_or_else(|e| { eprintln!("config: {e}"); std::process::exit(2); });
    let project = config::load_project(&cfg.project).unwrap_or_else(|e| { eprintln!("project: {e}"); std::process::exit(2); });

    let specs = config::discover_specs(&cfg.spec_dir)
        .into_iter()
        .filter(|p| filter.as_ref().map(|f| p.to_string_lossy().contains(f.as_str())).unwrap_or(true))
        .collect::<Vec<_>>();

    if ui {
        superui_test_engine::ui_mode::run(&cfg, &project, &specs); // Task 11
        return;
    }

    let snap_cfg = snapshot::SnapshotConfig {
        dir: cfg.spec_dir.clone(),
        update,
        max_diff_ratio: cfg.max_diff_ratio,
        platform: std::env::consts::OS.to_string(),
    };

    let mut all: Vec<(String, Vec<superui_test_engine::trace::TestResult>)> = Vec::new();
    for spec in &specs {
        let src = std::fs::read_to_string(spec).unwrap();
        let file = spec.file_name().unwrap().to_string_lossy().to_string();
        let js = transpile::transpile_spec(&src, &file).unwrap();
        // Fresh render app per spec for isolation + screenshots.
        let mut app = render::build_render_app_and_mount(&project, cfg.width, cfg.height);
        let opts = driver::RunOptions {
            snapshot: Some(snapshot::SnapshotConfig { dir: snap_cfg.dir.clone(), update, max_diff_ratio: snap_cfg.max_diff_ratio, platform: snap_cfg.platform.clone() }),
            spec_file: file.clone(),
            render: true,
        };
        let results = driver::run_spec_with(&mut app, &js, &opts);
        all.push((file, results));
    }

    let _ = report::write_html_report(&cfg.spec_dir.join("report.html"), &all);
    let ok = report::print_summary(&all);
    std::process::exit(if ok { 0 } else { 1 });
}
```

Note: per-test isolation currently re-uses one app per spec. For strict Playwright fresh-page-per-test, move the app build inside the per-test loop of `run_spec_with` (re-mount). Add that as the last refinement in this task: `run_spec_with` should re-mount between tests by rebuilding — or simpler, reset by re-running the app JS. Document the chosen approach in a code comment. Minimum bar: tests within a spec must not leak state; achieve by re-building the app per test if needed.

Add `pub mod config; pub mod report;` to `lib.rs`.

- [ ] **Step 6: Run to verify it passes**

Run: `cargo test -p superui_test_engine config`
Expected: PASS. Build the binary: `cargo build -p superui_test_engine --bin superui_test` → compiles (ui_mode may be a stub until Task 11; add `pub mod ui_mode;` with a `pub fn run(..)` stub that prints "ui mode not built yet" so this compiles).

- [ ] **Step 7: Commit**

```bash
git add crates/superui_test_engine
git commit -m "feat(test-engine): CLI, superui.test.toml config, spec discovery, reporter + HTML report"
```

---

## Task 11: egui UI mode

Integration task. UI is not unit-tested; correctness is verified by launching and interacting. Keep the trace model (Task 7) as the data source so the panel logic is thin.

**Files:**
- Create: `crates/superui_test_engine/src/ui_mode.rs` (replace the stub)
- Modify: `crates/superui_test_engine/Cargo.toml` (add `bevy_egui`)

**Interfaces:**
- Consumes: `TestConfig`, `HostProject`, spec paths, `render::build_render_app_and_mount`, `render::RenderTargetHandle`, `driver::run_spec_with`, `trace::TestResult`.
- Produces: `pub fn run(cfg: &TestConfig, project: &HostProject, specs: &[std::path::PathBuf])`.

- [ ] **Step 1: Add bevy_egui**

In `Cargo.toml`:

```toml
bevy_egui = "0.31"
```

Determine and pin the `bevy_egui` version compatible with Bevy 0.17 (check crates.io / the bevy_egui compatibility table). Run `cargo build -p superui_test_engine` to confirm the version resolves against Bevy 0.17 before proceeding. If none is compatible, note it and fall back to a minimal `bevy_ui`-based panel (spec list + live image) — the trace/live-view still works without egui.

- [ ] **Step 2: Implement `ui_mode::run`**

Build a render app (visible window this time: full `DefaultPlugins`, keep `WinitPlugin`). Add `EguiPlugin`. Insert app state holding: `specs`, `selected_spec`, `last_results: Vec<TestResult>`, `selected_step`. Draw:

- Left `egui::SidePanel`: list `specs`; a "Run" button per spec that transpiles + runs `run_spec_with` against the current app (or a child app) and stores results.
- Central panel: an `egui::Image` of the render target (register the `RenderTargetHandle` texture with egui via `bevy_egui`'s texture registration), plus a slider over `last_results[selected].steps` (time-travel). Selecting a step shows `step.dom_after` in a scrollable text area.
- Right panel: the selected step's status + error + screenshot diff (load `-actual.png`/`-diff.png` if present).
- A "Watch" checkbox: when on, re-read spec files each frame and re-run on change (compare mtime).

Because running a spec drives its own `app.update()` loop, the simplest robust structure is: the egui window app is separate from the UI-under-test app; "Run" builds a fresh headless/render app for the selected spec, runs it to completion (blocking), captures the trace + final screenshot into the egui app's state, and displays it. Live "view" shows the last captured frame per step (from the trace's screenshots). This avoids interleaving two Bevy loops.

Concrete run handler:

```rust
fn run_selected(state: &mut UiState, project: &HostProject, cfg: &TestConfig, spec: &std::path::Path) {
    let src = std::fs::read_to_string(spec).unwrap_or_default();
    let file = spec.file_name().unwrap().to_string_lossy().to_string();
    let js = match crate::transpile::transpile_spec(&src, &file) { Ok(j) => j, Err(e) => { state.error = Some(e); return; } };
    let mut app = crate::render::build_render_app_and_mount(project, cfg.width, cfg.height);
    let opts = crate::driver::RunOptions {
        snapshot: Some(crate::snapshot::SnapshotConfig {
            dir: cfg.spec_dir.clone(), update: false, max_diff_ratio: cfg.max_diff_ratio,
            platform: std::env::consts::OS.to_string(),
        }),
        spec_file: file.clone(),
        render: true,
    };
    state.last_results = crate::driver::run_spec_with(&mut app, &js, &opts);
    state.selected_step = 0;
}
```

Note: capturing a screenshot per step (for full time-travel) requires the driver to `capture` after each step when `render && ui`. Extend `RunOptions` with `capture_each_step: bool`; when set, after pushing each `Step`, call `render::capture` and save the PNG to a temp dir, storing the path in `Step.screenshot`. Guard with the flag so headless CI stays fast.

- [ ] **Step 3: Manual verification**

Run: `cargo run -p superui_test_engine --bin superui_test -- --ui` from a directory containing a valid `superui.test.toml` (use the game_menu one from Task 12).
Expected: a window opens; the spec list shows discovered specs; clicking "Run" executes and shows per-step DOM + the rendered frame; the time-travel slider scrubs steps.

Document this manual check in the task; there is no automated assertion for the egui surface.

- [ ] **Step 4: Commit**

```bash
git add crates/superui_test_engine
git commit -m "feat(test-engine): egui UI mode — spec list, live view, time-travel, DOM inspector"
```

---

## Task 12: game_menu acceptance spec + committed baselines

**Files:**
- Create: `examples/game_menu/superui.test.toml`
- Create: `examples/game_menu/tests/game_menu.spec.ts`
- Create (generated by `--update`): `examples/game_menu/tests/__snapshots__/game_menu.spec.ts/*.png`

**Interfaces:** none (top-level acceptance).

- [ ] **Step 1: Write the config**

`examples/game_menu/superui.test.toml`:

```toml
project = "assets/ui/game_menu"
specDir = "tests"
width = 1280
height = 720
maxDiffRatio = 0.02
```

Note: `project` points at the dir holding `index.html`, `style.css`, `app.tsx`. Confirm those filenames exist (they do: `examples/game_menu/assets/ui/game_menu/{index.html,style.css,app.tsx}`).

- [ ] **Step 2: Write the spec (grounded in real selectors)**

`examples/game_menu/tests/game_menu.spec.ts`:

```ts
import { test, expect } from "superui/test";

test("main menu renders", async ({ page }) => {
  await expect(page.locator(".screen.main")).toBeVisible();
  await expect(page).toHaveScreenshot("main.png");
});

test("tab bar navigates to settings", async ({ page }) => {
  await page.locator(".tabs .tab", { hasText: "SETTINGS" }).click();
  await expect(page.locator(".settings-card")).toBeVisible();
  await expect(page.locator(".tabs .tab.active")).toHaveText("SETTINGS");
  await expect(page).toHaveScreenshot("settings.png");
});

test("toggling vsync flips the switch", async ({ page }) => {
  await page.locator(".tabs .tab", { hasText: "SETTINGS" }).click();
  const vsync = page.locator(".cfg-row", { hasText: "VSYNC" }).locator(".toggle");
  await vsync.click();
  await expect(vsync).toHaveClass(/on/);
});
```

Before finalizing, verify the tab labels: open `examples/game_menu/assets/ui/game_menu/app.tsx` and confirm the tab `.tab` text values (they come from a `t.value`/label list near line 284–289) and the settings row label (`VSYNC`). Adjust `hasText` strings to the exact rendered text. If a tab's visible text differs (e.g. lowercased/expanded), use the real text.

- [ ] **Step 3: Run headless to see it pass (DOM assertions) and generate baselines**

From `examples/game_menu/`:

Run: `cargo run -p superui_test_engine --bin superui_test -- --update`
Expected: all three tests PASS; baseline PNGs written under `tests/__snapshots__/game_menu.spec.ts/` (e.g. `main-windows.png`, `settings-windows.png`).

If a DOM assertion fails, fix the spec's selectors/text to match the real markup (this is the intended debugging loop — the spec must reflect the actual UI).

- [ ] **Step 4: Re-run without `--update` to confirm baselines match**

Run: `cargo run -p superui_test_engine --bin superui_test`
Expected: exit code 0; all pass; screenshots match committed baselines.

- [ ] **Step 5: Verify in UI mode (manual)**

Run: `cargo run -p superui_test_engine --bin superui_test -- --ui`
Expected: window opens, all three specs listed, each runs green, time-travel shows the tab-switch producing the `.settings-card`.

- [ ] **Step 6: Commit (spec + baselines)**

```bash
git add examples/game_menu/superui.test.toml examples/game_menu/tests
git commit -m "test(game_menu): E2E spec + committed screenshot baselines (superui-test acceptance)"
```

---

## Self-review notes (addressed in this plan)

- **Spec coverage:** TS-through-Boa pipeline (Task 2), async bridge (Task 3), locators (Task 4), actions (Task 5), auto-wait assertions (Task 6), trace (Task 7), offscreen render + screenshots (Tasks 8–9), CLI + config + reporter (Task 10), egui UI mode with live view/time-travel (Task 11), game_menu acceptance with committed baselines (Task 12). All spec sections map to a task.
- **Determinism/clock:** the driver owns ticking (`app.update()`); no wall-clock waits. Timeouts are iteration budgets. Documented in Tasks 5–6.
- **Windows stack:** inherited from `.cargo/config.toml` (Global Constraints); no per-task action.
- **Known Phase-1 simplifications made explicit in code comments:** class matcher is substring (not full regex); `toBeVisible` headless uses attached+not-`display:none` (ComputedNode size check is an available refinement once the render host is used); per-test isolation defaults to per-spec app reuse with a documented upgrade path to per-test re-mount.
- **Boa/Bevy API confirmations** are called out inline at each risky call (Task 3 promise/native-fn names; Task 8 `ScreenshotCaptured` field + `On<>` observer signature + `RenderTarget::Image`), each with the source file to check.
