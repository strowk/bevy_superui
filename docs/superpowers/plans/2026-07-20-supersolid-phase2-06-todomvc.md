# Supersolid Phase 2 — Plan 6: runnable Supersolid TodoMVC — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship `examples/todomvc_supersolid/` — a runnable, hot-reloadable TodoMVC authored in idiomatic Solid-style `.tsx`, composing Plans 1–5, running native (live HMR) and `wasm32-unknown-unknown` (pre-transpiled), with the existing plain `examples/todomvc` retained.

**Architecture:** A new example crate mounts a Supersolid app through the existing `SuperUiPlugin`. All app state lives in a stable top-level `App` component (three signals: `todos`, `filter`, `draft`); `Header`/`TodoItem`/`Footer` are stateless prop-driven views. On native+`hmr` the binary loads `app.tsx` through the native `TsxLoader` (state-preserving HMR); everywhere else (wasm, native release/no-`hmr`) it loads a `build.rs`-generated `app.generated.js`. Headless integration tests drive the *real* `app.tsx` through the runtime and assert the reconciled ECS state; a separate HMR test re-execs the module and asserts state survives. Everything downstream of the DOM is untouched Phase-1 machinery (direction §7).

**Tech Stack:** Solid-style `.tsx` (transpiled by `supersolid`/oxc, run in Boa 0.21), Rust edition 2021, Bevy 0.17, the `superui` umbrella plugin + `superui_css`/`superui_bridge`, the `supersolid` transpiler (build-dependency + test-only dev-dependency).

Design spec: [`../specs/2026-07-20-supersolid-todomvc-plan6-design.md`](../specs/2026-07-20-supersolid-todomvc-plan6-design.md).

## Global Constraints

- **Bevy 0.17**, edition 2021. The example is a `publish = false` workspace member under `examples/*`.
- **Pure composition of Plans 1–5** — no changes to `superui*`/`supersolid*` crates, no browser/ledger capability additions. The app stays inside the ✅ subset of `docs/support/`.
- **oxc must never enter the wasm binary.** The transpiler is reachable only as a **build-dependency** (host-only) and a **dev-dependency** (native tests). `TsxLoader` is already `#[cfg(not(target_arch = "wasm32"))]` in `superui`.
- **Feature set (in-subset):** add (Add button, not Enter — `event.key` is 🟡), toggle complete, delete, toggle-all, filter all/active/completed, clear-completed, "N items left" count. **No editing** (`dblclick`/`event.key`), **no persistence** (`localStorage` ⛔), **no `window.bevy`** demo.
- **State placement:** all state in `App`; children are stateless. Immutable, identity-preserving updates so `<For>` (keyed by identity) reuses rows.
- **TDD** throughout; **frequent commits**; work on `main` (project CLAUDE.md — no feature branch).
- **Graceful degradation:** author-script/transpile errors already log-and-swallow; `build.rs` must not fail the build on a transpile *warning*.
- The existing `examples/todomvc/` is **not modified**.

### Runtime globals available to `app.tsx` (injected by Plans 3–4; their imports are stripped)

`createSignal` → `[get, set]`, `createEffect`, `createMemo`, `onMount`, `onCleanup`, `createContext`, `useContext`, `render`, `Show`, `For`, `Index`, `Switch`, `Match`. DOM: `document.getElementById`, `document.createElement`, `event.target`, `element.value`/`checked`, standard `Array`/`Math`/`Object`. Events wired via JSX: `onClick`→`click`, `onInput`→`input`, `onChange`→`change`.

### The final `app.tsx` (built up across Tasks 1–6; shown in full at each change)

Tasks replace the whole `assets/ui/todomvc_supersolid/app.tsx` file at each stage (clearest for a fresh implementer). This is its final form, for reference — do not paste it until Task 6:

```tsx
import { createSignal, createMemo, For, Show, render } from "supersolid";

interface Todo { id: number; title: string; done: boolean; }
type Filter = "all" | "active" | "completed";

function Header(props) {
  return (
    <div id="new-todo-row">
      <input id="new-todo" type="text" placeholder="What needs to be done?"
             value={props.draft} onInput={(e) => props.onInput(e.target.value)} />
      <button id="add" onClick={() => props.onAdd()}>Add</button>
    </div>
  );
}

function TodoItem(props) {
  return (
    <li class={props.todo.done ? "todo completed" : "todo"} data-id={props.todo.id}>
      <input class="toggle" type="checkbox" checked={props.todo.done}
             onChange={() => props.onToggle(props.todo.id)} />
      <span class="label">{props.todo.title}</span>
      <button class="destroy" onClick={() => props.onRemove(props.todo.id)}>x</button>
    </li>
  );
}

function Footer(props) {
  return (
    <div id="footer">
      <span id="count">
        {props.remaining + (props.remaining === 1 ? " item left" : " items left")}
      </span>
      <div class="filters">
        <button id="filter-all" class={props.filter === "all" ? "filter selected" : "filter"}
                onClick={() => props.onFilter("all")}>All</button>
        <button id="filter-active" class={props.filter === "active" ? "filter selected" : "filter"}
                onClick={() => props.onFilter("active")}>Active</button>
        <button id="filter-completed"
                class={props.filter === "completed" ? "filter selected" : "filter"}
                onClick={() => props.onFilter("completed")}>Completed</button>
      </div>
      <button id="clear-completed" class="clear-completed"
              onClick={() => props.onClearCompleted()}>Clear completed</button>
    </div>
  );
}

function App() {
  const [todos, setTodos] = createSignal<Todo[]>([]);
  const [filter, setFilter] = createSignal<Filter>("all");
  const [draft, setDraft] = createSignal("");

  const remaining = createMemo(() => todos().filter((t) => !t.done).length);
  const filtered = createMemo(() => {
    const f = filter();
    return todos().filter((t) => (f === "all" ? true : f === "active" ? !t.done : t.done));
  });

  const addTodo = () => {
    const title = draft().trim();
    if (!title) return;
    const id = todos().reduce((m, t) => Math.max(m, t.id), 0) + 1;
    setTodos([...todos(), { id, title, done: false }]);
    setDraft("");
  };
  const toggle = (id) =>
    setTodos(todos().map((t) => (t.id === id ? { ...t, done: !t.done } : t)));
  const remove = (id) => setTodos(todos().filter((t) => t.id !== id));
  const clearCompleted = () => setTodos(todos().filter((t) => !t.done));
  const toggleAll = () => {
    const allDone = todos().length > 0 && todos().every((t) => t.done);
    setTodos(todos().map((t) => ({ ...t, done: !allDone })));
  };

  return (
    <div id="app">
      <h1>todos</h1>
      <Header draft={draft()} onInput={setDraft} onAdd={addTodo} />
      <div id="main">
        <input id="toggle-all" type="checkbox" onChange={() => toggleAll()} />
        <ul id="todo-list">
          <For each={filtered()}>
            {(todo) => <TodoItem todo={todo} onToggle={toggle} onRemove={remove} />}
          </For>
        </ul>
      </div>
      <Show when={todos().length > 0}>
        <Footer remaining={remaining()} filter={filter()}
                onFilter={setFilter} onClearCompleted={clearCompleted} />
      </Show>
    </div>
  );
}

render(() => <App />, document.getElementById("root"));
```

> **Boa note:** array spread (`[...a, x]`), object spread (`{ ...t, done }`), arrow fns, `reduce`/`map`/`filter`/`every`, template-free string concat, and generic call type args (`createSignal<Todo[]>` — stripped) are all supported by Boa 0.21 / oxc. If a TDD step surfaces a Boa gap on object spread, fall back to `Object.assign({}, t, { done: !t.done })`; on array spread, `todos().concat([{ id, title, done: false }])`. Let the tests decide.

---

## Task 1: Crate scaffold — compiles, mounts, shows the title

Create the example crate so it compiles on native, generates the wasm JS via `build.rs`, and a headless test proves the full pipeline (`.tsx` → `TsxLoader` → `render()` → reconcile) by mounting a minimal `App` and reading back the `<h1>` title.

**Files:**
- Create: `examples/todomvc_supersolid/Cargo.toml`
- Create: `examples/todomvc_supersolid/build.rs`
- Create: `examples/todomvc_supersolid/.gitignore`
- Create: `examples/todomvc_supersolid/src/main.rs`
- Create: `examples/todomvc_supersolid/assets/ui/todomvc_supersolid/index.html`
- Create: `examples/todomvc_supersolid/assets/ui/todomvc_supersolid/style.css`
- Create: `examples/todomvc_supersolid/assets/ui/todomvc_supersolid/app.tsx`
- Create: `examples/todomvc_supersolid/tests/support/mod.rs`
- Create: `examples/todomvc_supersolid/tests/todomvc.rs`

**Interfaces:**
- Consumes: `superui::prelude::{SuperUiPlugin, SuperUiRoot}`; `superui::JsSource`; `superui_css::style::StyleSheet`; the `TsxLoader` registered by `SuperUiPlugin` (native). The harness mirrors `examples/todomvc/tests/support/mod.rs`.
- Produces (for later tasks): the test harness `support::{app, mount, node_by_selector, nodes_by_selector, text_content, value_of, checked_of, click, type_into, click_checkbox, tick}`; the real asset files loaded via `include_str!`.

- [ ] **Step 1: Create `Cargo.toml`**

`examples/todomvc_supersolid/Cargo.toml`:

```toml
[package]
name = "todomvc_supersolid"
edition.workspace = true
version.workspace = true
license.workspace = true
publish = false

[features]
# Native state-preserving hot reload: live `.tsx` via TsxLoader + the asset watcher.
# `cargo run -p todomvc_supersolid --features hmr`.
hmr = ["superui/hmr", "bevy/file_watcher"]
# Opt-in diagnostics (text + color dump, click/key logging).
debug-ui = []
# Bevy Remote Protocol + BRP extras so bevy_brp_mcp can drive/inspect the app.
mcp_debug = ["dep:bevy_brp_extras", "bevy/bevy_remote"]

[dependencies]
superui = { path = "../../crates/superui" }
superui_css = { path = "../../crates/superui_css" }
# Pulled transitively via `superui`; named directly for the mcp_debug click injector.
superui_bridge = { path = "../../crates/superui_bridge" }
bevy = { version = "0.17" }
serde = { version = "1", features = ["derive"] }

[dependencies.bevy_brp_extras]
optional = true
version = "0.17.3"

# Host-only: pre-transpile app.tsx -> app.generated.js for wasm / no-HMR native.
[build-dependencies]
supersolid = { path = "../../crates/supersolid" }

[dev-dependencies]
# Headless tests reuse the browser stack without a window.
superui_dom = { path = "../../crates/superui_dom" }
superui_html = { path = "../../crates/superui_html" }
# The HMR test transpiles app.tsx directly and drives a UiRuntime.
supersolid = { path = "../../crates/supersolid" }

# Boa (via superui) needs the JS getrandom backend on wasm.
[target.'cfg(target_arch = "wasm32")'.dependencies]
getrandom = { version = "0.3", features = ["wasm_js"] }
```

> **Note:** unlike the plain example, `bevy` here does **not** unconditionally enable `file_watcher` — it's pulled in only by the `hmr` feature, so a default `cargo run` / wasm build stays on the pre-transpiled path.

- [ ] **Step 2: Create `build.rs` (host-only pre-transpile)**

`examples/todomvc_supersolid/build.rs`:

```rust
//! Pre-transpile the Supersolid app so wasm and no-HMR native builds have plain
//! `.js` to load (direction spec §11.3). Build scripts + their deps compile for
//! the HOST, so `supersolid` (oxc) never enters the wasm binary. Runs on every
//! build (transpiling one file is cheap) to keep the output fresh and present.

use std::path::PathBuf;

fn main() {
    let dir = PathBuf::from("assets/ui/todomvc_supersolid");
    let input = dir.join("app.tsx");
    let output = dir.join("app.generated.js");
    println!("cargo:rerun-if-changed={}", input.display());
    match supersolid::transpile_file(&input, &output) {
        Ok(result) => {
            for d in &result.diagnostics {
                // Warn-only: never fail the build on a transpile diagnostic.
                println!("cargo:warning=supersolid: {}", d.message);
            }
        }
        Err(e) => {
            // Missing input etc. — surface as a warning, don't hard-fail the build.
            println!("cargo:warning=supersolid: could not transpile {}: {e}", input.display());
        }
    }
}
```

- [ ] **Step 3: Create `.gitignore`**

`examples/todomvc_supersolid/.gitignore`:

```gitignore
# Generated by build.rs from app.tsx (wasm / no-HMR native path).
assets/ui/todomvc_supersolid/app.generated.js
```

- [ ] **Step 4: Create the shell HTML**

`examples/todomvc_supersolid/assets/ui/todomvc_supersolid/index.html` (minimal mount point — `App` builds everything):

```html
<div id="root"></div>
```

- [ ] **Step 5: Create the stylesheet**

`examples/todomvc_supersolid/assets/ui/todomvc_supersolid/style.css` (adapted from the plain example, plus `#main`/`#toggle-all`/`.clear-completed`):

```css
#app {
  display: flex;
  flex-direction: column;
  width: 420px;
  margin: 40px;
  padding: 16px;
  background-color: #ffffff;
  border: 1px #e0e0e0;
  border-radius: 8px;
}

h1 {
  color: #b83f45;
  font-size: 48px;
  margin: 8px;
}

#new-todo-row {
  display: flex;
  flex-direction: row;
  align-items: center;
  column-gap: 8px;
  margin: 8px;
}

#new-todo {
  flex-grow: 1;
  padding: 12px 12px;
  background-color: #f7f7f7;
  border: 1px #888888;
  border-radius: 4px;
  color: #333333;
  font-size: 16px;
  overflow: hidden;
}

#add {
  padding: 12px 20px;
  margin: 4px;
  background-color: #b83f45;
  color: #ffffff;
  border-radius: 4px;
  font-size: 16px;
  align-items: center;
  justify-content: center;
}

#add:hover {
  background-color: #983035;
}

#main {
  display: flex;
  flex-direction: row;
  align-items: flex-start;
}

#toggle-all {
  width: 22px;
  height: 22px;
  margin: 8px;
  background-color: #ffffff;
  border: 2px #cccccc;
  border-radius: 4px;
}

#todo-list {
  display: flex;
  flex-direction: column;
  flex-grow: 1;
}

.todo {
  display: flex;
  flex-direction: row;
  align-items: center;
  padding: 8px;
  border-bottom-width: 1px;
  border-bottom-color: #ededed;
}

.todo .label {
  flex-grow: 1;
  color: #333333;
  font-size: 18px;
}

.todo.completed .label {
  color: #aaaaaa;
}

.toggle {
  width: 22px;
  height: 22px;
  margin: 4px;
  background-color: #ffffff;
  border: 2px #cccccc;
  border-radius: 4px;
  align-items: center;
  justify-content: center;
}

.toggle:checked {
  background-color: #b83f45;
  border: 2px #b83f45;
}

.destroy {
  width: 28px;
  height: 28px;
  background-color: #ffffff;
  color: #b83f45;
  border-radius: 4px;
}

.destroy:hover {
  background-color: #f4f4f4;
}

#footer {
  display: flex;
  flex-direction: row;
  align-items: center;
  justify-content: space-between;
  padding: 8px;
}

#count {
  flex-shrink: 0;
  color: #777777;
  font-size: 14px;
}

.filters {
  display: flex;
  flex-direction: row;
}

.filter {
  margin: 2px;
  padding: 4px;
  color: #777777;
  border: 1px #ffffff;
  border-radius: 4px;
  font-size: 14px;
  align-items: center;
  justify-content: center;
}

.filter.selected {
  border: 1px #b83f45;
  color: #b83f45;
}

.clear-completed {
  padding: 4px;
  color: #777777;
  border-radius: 4px;
  font-size: 14px;
}

.clear-completed:hover {
  color: #b83f45;
}
```

- [ ] **Step 6: Create the minimal `app.tsx`**

`examples/todomvc_supersolid/assets/ui/todomvc_supersolid/app.tsx` (title only — grows in later tasks):

```tsx
import { render } from "supersolid";

function App() {
  return (
    <div id="app">
      <h1>todos</h1>
    </div>
  );
}

render(() => <App />, document.getElementById("root"));
```

- [ ] **Step 7: Create `src/main.rs`**

`examples/todomvc_supersolid/src/main.rs`:

```rust
//! Runnable Supersolid TodoMVC — authored in Solid-style `.tsx` under
//! `assets/ui/todomvc_supersolid/`, mounted on `SuperUiPlugin`.
//!
//! - `cargo run -p todomvc_supersolid --features hmr` — native, live `.tsx` via
//!   the transpiling asset loader, state-preserving hot reload.
//! - `cargo run -p todomvc_supersolid` — native, loads the pre-transpiled
//!   `app.generated.js` (build.rs output); no HMR.
//! - `cargo build -p todomvc_supersolid --target wasm32-unknown-unknown` — web
//!   build, loads `app.generated.js` (the transpiler never enters wasm).

use bevy::prelude::*;
use superui::prelude::{SuperUiPlugin, SuperUiRoot};
use superui::JsSource;
use superui_css::style::StyleSheet;

/// Live `.tsx` (transpiled at load, hot-reloadable) is used only on native builds
/// with the `hmr` feature; every other build loads the pre-transpiled `.js`.
const USE_LIVE_TSX: bool = cfg!(all(not(target_arch = "wasm32"), feature = "hmr"));

fn main() {
    let mut app = App::new();

    let asset_plugin = AssetPlugin {
        // Only meaningful with `bevy/file_watcher` (pulled by the `hmr` feature).
        watch_for_changes_override: Some(USE_LIVE_TSX),
        ..default()
    };
    app.add_plugins(DefaultPlugins.set(asset_plugin))
        .add_plugins(SuperUiPlugin);

    #[cfg(feature = "mcp_debug")]
    {
        app.add_plugins(bevy_brp_extras::BrpExtrasPlugin)
            .register_type::<mcp_debug::DebugClick>()
            .init_resource::<mcp_debug::DebugClick>()
            .add_systems(Update, mcp_debug::debug_click_system);
    }

    app.add_systems(Startup, setup);
    app.run();
}

fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    commands.spawn(Camera2d);

    // Native+hmr loads `app.tsx` through the transpiling TsxLoader (live HMR);
    // wasm / no-hmr loads the build.rs-generated `.js`. Both yield Handle<JsSource>.
    let js: Handle<JsSource> = if USE_LIVE_TSX {
        assets.load("ui/todomvc_supersolid/app.tsx")
    } else {
        assets.load("ui/todomvc_supersolid/app.generated.js")
    };

    commands.spawn((
        Node::default(),
        SuperUiRoot {
            html: assets.load("ui/todomvc_supersolid/index.html"),
            css: assets.load::<StyleSheet>("ui/todomvc_supersolid/style.css"),
            js,
        },
    ));
}
```

> **Guidance:** `SuperUiRoot.js` is `Handle<JsSource>`; `TsxLoader` and `JsLoader` both produce `JsSource`, so the `if` picks the source by asset path and the rest of the pipeline is identical. `mcp_debug`/`debug-ui` module bodies are added in Task 8 (this compiles now because their `#[cfg]` blocks are absent).

- [ ] **Step 8: Create the test harness**

`examples/todomvc_supersolid/tests/support/mod.rs` (mirrors the plain example's harness; loads the real `.tsx` through `TsxLoader`; adds `type_into` for controlled inputs):

```rust
//! Headless harness: mount the REAL authored Supersolid assets through the real
//! `superui` runtime (the `.tsx` transpiled by the native TsxLoader), then drive
//! synthetic DOM events and read the DOM back.
#![allow(dead_code)]

use bevy::asset::io::memory::{Dir, MemoryAssetReader};
use bevy::asset::io::{AssetSource, AssetSourceId};
use bevy::asset::AssetPlugin;
use bevy::image::TextureAtlasPlugin;
use bevy::input::InputPlugin;
use bevy::input_focus::{InputFocus, InputFocusVisible};
use bevy::picking::{InteractionPlugin, PickingPlugin};
use bevy::prelude::*;
use bevy::text::TextPlugin;
use bevy::ui::UiPlugin;
use superui::prelude::{SuperUiPlugin, SuperUiRoot};
use superui::JsSource;
use superui_bridge::{PendingDomEvent, PendingDomEvents, UiRuntime};
use superui_css::style::StyleSheet;
use superui_dom::NodeId;

pub const HTML: &str = include_str!("../../assets/ui/todomvc_supersolid/index.html");
pub const CSS: &str = include_str!("../../assets/ui/todomvc_supersolid/style.css");
pub const TSX: &str = include_str!("../../assets/ui/todomvc_supersolid/app.tsx");

/// A headless app with the full SuperUi stack and an in-memory asset source
/// holding the authored files (the `.tsx` is transpiled by the native TsxLoader).
pub fn app() -> App {
    let dir = Dir::new("assets".into());
    dir.insert_asset("ui/todomvc_supersolid/index.html".as_ref(), HTML.as_bytes());
    dir.insert_asset("ui/todomvc_supersolid/style.css".as_ref(), CSS.as_bytes());
    dir.insert_asset("ui/todomvc_supersolid/app.tsx".as_ref(), TSX.as_bytes());

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
    ));
    app.init_resource::<InputFocus>()
        .init_resource::<InputFocusVisible>();
    app.add_plugins(SuperUiPlugin);
    app.finish();
    app
}

/// Spawn the `SuperUiRoot` (loading the `.tsx` as JsSource) and tick until mounted.
pub fn mount(app: &mut App) -> Entity {
    let (html, css, js) = {
        let server = app.world().resource::<AssetServer>().clone();
        (
            server.load("ui/todomvc_supersolid/index.html"),
            server.load::<StyleSheet>("ui/todomvc_supersolid/style.css"),
            server.load::<JsSource>("ui/todomvc_supersolid/app.tsx"),
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

pub fn node_by_selector(app: &App, sel: &str) -> NodeId {
    let rt = app.world().non_send_resource::<UiRuntime>();
    let d = rt.dom.borrow();
    d.query_selector(d.document(), sel)
        .unwrap_or_else(|| panic!("selector matched nothing: {sel}"))
}

pub fn nodes_by_selector(app: &App, sel: &str) -> Vec<NodeId> {
    let rt = app.world().non_send_resource::<UiRuntime>();
    let d = rt.dom.borrow();
    d.query_selector_all(d.document(), sel)
}

pub fn text_content(app: &App, node: NodeId) -> String {
    let rt = app.world().non_send_resource::<UiRuntime>();
    let t = rt.dom.borrow().text_content(node);
    t
}

pub fn value_of(app: &App, node: NodeId) -> String {
    let rt = app.world().non_send_resource::<UiRuntime>();
    let v = rt.dom.borrow().value(node);
    v
}

pub fn checked_of(app: &App, node: NodeId) -> bool {
    let rt = app.world().non_send_resource::<UiRuntime>();
    let c = rt.dom.borrow().checked(node);
    c
}

/// Enqueue a synthetic `click` DOM event on `node` (drained + dispatched next tick).
pub fn click(app: &mut App, node: NodeId) {
    app.world_mut()
        .resource_mut::<PendingDomEvents>()
        .0
        .push(PendingDomEvent::new(node, "click"));
    tick(app, 2);
}

/// Type into a controlled input: set the DOM value, then fire `input` so the
/// component's `onInput` handler (reading `e.target.value`) updates its signal.
pub fn type_into(app: &mut App, node: NodeId, v: &str) {
    {
        let mut rt = app.world_mut().non_send_resource_mut::<UiRuntime>();
        rt.dom.borrow_mut().set_value(node, v);
        rt.dirty = true;
    }
    app.world_mut()
        .resource_mut::<PendingDomEvents>()
        .0
        .push(PendingDomEvent::new(node, "input"));
    tick(app, 2);
}

/// Simulate a pointer click on a checkbox: flip DOM `checked` (as the picking
/// observer does natively), then dispatch `change`.
pub fn click_checkbox(app: &mut App, node: NodeId) {
    {
        let rt = app.world_mut().non_send_resource_mut::<UiRuntime>();
        let now = !rt.dom.borrow().checked(node);
        rt.dom.borrow_mut().set_checked(node, now);
    }
    app.world_mut()
        .resource_mut::<PendingDomEvents>()
        .0
        .push(PendingDomEvent::new(node, "change"));
    tick(app, 2);
}
```

> **Guidance:** confirm `PendingDomEvent::new`, `Dom::{query_selector, query_selector_all, text_content, value, checked, set_value, set_checked, document}` signatures against `examples/todomvc/tests/support/mod.rs` (identical usage there). The mount loop uses 256 iterations because a `.tsx` load runs the transpiler.

- [ ] **Step 9: Write the failing mount test**

`examples/todomvc_supersolid/tests/todomvc.rs`:

```rust
//! Integration tests over the REAL authored Supersolid TodoMVC (`app.tsx`
//! transpiled by the native TsxLoader), driven headlessly through `superui`.
mod support;
use support::*;

#[test]
fn mounts_and_shows_title() {
    let mut app = app();
    let _root = mount(&mut app);
    // The app mounted (a UiRuntime exists) and App's <h1> title rendered.
    let h1 = node_by_selector(&app, "h1");
    assert_eq!(text_content(&app, h1), "todos");
}
```

- [ ] **Step 10: Run the test to verify it fails, then passes**

Run: `cargo test -p todomvc_supersolid --test todomvc mounts_and_shows_title`
Expected first run: it should actually PASS once all files are in place (the pipeline is real). If it FAILS, diagnose: a panic in `mount` (transpile/eval error) means the `.tsx` or harness is wrong; "selector matched nothing: h1" means `render()` didn't reconcile. This is the scaffold's smoke test — get it green before moving on.

- [ ] **Step 11: Verify the crate builds as a binary**

Run: `cargo build -p todomvc_supersolid`
Expected: PASS. `build.rs` runs the transpiler and writes `app.generated.js` (gitignored). `src/main.rs` compiles.

- [ ] **Step 12: Commit**

```bash
git add examples/todomvc_supersolid/Cargo.toml examples/todomvc_supersolid/build.rs \
  examples/todomvc_supersolid/.gitignore examples/todomvc_supersolid/src/main.rs \
  examples/todomvc_supersolid/assets examples/todomvc_supersolid/tests
git commit -m "feat(todomvc_supersolid): crate scaffold — mounts a Supersolid App, shows title

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 2: Add a todo (Header + App state)

Introduce `App`'s state (`todos`, `draft` signals) and the `Header` component (controlled input + Add button), and the `addTodo` handler. Deliverable: typing + Add appends a `<li>` and clears the input.

**Files:**
- Modify: `examples/todomvc_supersolid/assets/ui/todomvc_supersolid/app.tsx`
- Test: `examples/todomvc_supersolid/tests/todomvc.rs`

**Interfaces:**
- Consumes: `createSignal`, `For`, `render` globals; harness `type_into`, `click`, `value_of`, `nodes_by_selector`.
- Produces: DOM ids `#new-todo`, `#add`; `<li>` rows each containing `.label`; the `App` state signals used by all later tasks.

- [ ] **Step 1: Write the failing test**

Add to `tests/todomvc.rs`:

```rust
use superui_bridge::UiRuntime;

/// Labels of the currently rendered `<li>` rows.
fn li_labels(app: &bevy::prelude::App) -> Vec<String> {
    nodes_by_selector(app, "li")
        .into_iter()
        .map(|li| {
            let label = {
                let rt = app.world().non_send_resource::<UiRuntime>();
                let d = rt.dom.borrow();
                d.query_selector(li, ".label").unwrap()
            };
            text_content(app, label)
        })
        .collect()
}

/// Type a label into the new-todo input and click Add.
fn add(app: &mut bevy::prelude::App, label: &str) {
    let input = node_by_selector(app, "#new-todo");
    type_into(app, input, label);
    let add_btn = node_by_selector(app, "#add");
    click(app, add_btn);
}

#[test]
fn add_button_appends_a_todo() {
    let mut app = app();
    let _root = mount(&mut app);

    add(&mut app, "Buy milk");

    assert_eq!(li_labels(&app), vec!["Buy milk".to_string()]);
    // Controlled input cleared after add (draft signal reset -> value binding).
    let input = node_by_selector(&app, "#new-todo");
    assert_eq!(value_of(&app, input), "");
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p todomvc_supersolid --test todomvc add_button_appends`
Expected: FAIL — `#new-todo` selector matches nothing (Header not built yet).

- [ ] **Step 3: Update `app.tsx` with Header + state + add**

Replace `assets/ui/todomvc_supersolid/app.tsx` with:

```tsx
import { createSignal, For, render } from "supersolid";

interface Todo { id: number; title: string; done: boolean; }

function Header(props) {
  return (
    <div id="new-todo-row">
      <input id="new-todo" type="text" placeholder="What needs to be done?"
             value={props.draft} onInput={(e) => props.onInput(e.target.value)} />
      <button id="add" onClick={() => props.onAdd()}>Add</button>
    </div>
  );
}

function App() {
  const [todos, setTodos] = createSignal<Todo[]>([]);
  const [draft, setDraft] = createSignal("");

  const addTodo = () => {
    const title = draft().trim();
    if (!title) return;
    const id = todos().reduce((m, t) => Math.max(m, t.id), 0) + 1;
    setTodos([...todos(), { id, title, done: false }]);
    setDraft("");
  };

  return (
    <div id="app">
      <h1>todos</h1>
      <Header draft={draft()} onInput={setDraft} onAdd={addTodo} />
      <ul id="todo-list">
        <For each={todos()}>
          {(todo) => (
            <li class="todo" data-id={todo.id}>
              <span class="label">{todo.title}</span>
            </li>
          )}
        </For>
      </ul>
    </div>
  );
}

render(() => <App />, document.getElementById("root"));
```

- [ ] **Step 4: Run to verify it passes (+ the title test still green)**

Run: `cargo test -p todomvc_supersolid --test todomvc`
Expected: PASS (`mounts_and_shows_title` + `add_button_appends_a_todo`).

> **If `add` yields an empty label:** the `input` event isn't updating `draft`. Confirm the JSX lowered `onInput` to `$ss.on(input, "input", ...)` and that `e.target.value` reads the set value — inspect the transpiled `app.generated.js` (from `build.rs`) if needed.

- [ ] **Step 5: Commit**

```bash
git add examples/todomvc_supersolid/assets/ui/todomvc_supersolid/app.tsx examples/todomvc_supersolid/tests/todomvc.rs
git commit -m "feat(todomvc_supersolid): add-a-todo via Header + App state signals

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 3: Toggle complete + item count (TodoItem + Footer count)

Add the `TodoItem` component (checkbox + label), the `toggle` handler (immutable, identity-preserving), the `remaining` memo, and a `Footer` showing the count. Deliverable: toggling a checkbox marks the row `completed` and updates "N items left".

**Files:**
- Modify: `examples/todomvc_supersolid/assets/ui/todomvc_supersolid/app.tsx`
- Test: `examples/todomvc_supersolid/tests/todomvc.rs`

**Interfaces:**
- Consumes: `createMemo` global; harness `click_checkbox`, `text_content`; `Dom::classes`.
- Produces: `.toggle` checkbox per row, `.completed` class on done rows, `#count` text `"N item(s) left"`.

- [ ] **Step 1: Write the failing test**

Add to `tests/todomvc.rs`:

```rust
#[test]
fn toggle_marks_completed_and_updates_count() {
    let mut app = app();
    let _root = mount(&mut app);
    add(&mut app, "a");
    add(&mut app, "b");

    let count = node_by_selector(&app, "#count");
    assert_eq!(text_content(&app, count), "2 items left");

    // Toggle the first todo's checkbox -> completed; count drops to 1.
    let first_toggle = nodes_by_selector(&app, "li .toggle")[0];
    click_checkbox(&mut app, first_toggle);

    let count = node_by_selector(&app, "#count");
    assert_eq!(text_content(&app, count), "1 item left");

    // The first li carries the `completed` class.
    let first_li = nodes_by_selector(&app, "li")[0];
    let classes = {
        let rt = app.world().non_send_resource::<UiRuntime>();
        let c = rt.dom.borrow().classes(first_li);
        c
    };
    assert!(classes.iter().any(|c| c == "completed"));
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p todomvc_supersolid --test todomvc toggle_marks_completed`
Expected: FAIL — no `#count` / `.toggle` yet.

- [ ] **Step 3: Update `app.tsx` with TodoItem + Footer count + toggle**

Replace `assets/ui/todomvc_supersolid/app.tsx` with:

```tsx
import { createSignal, createMemo, For, render } from "supersolid";

interface Todo { id: number; title: string; done: boolean; }

function Header(props) {
  return (
    <div id="new-todo-row">
      <input id="new-todo" type="text" placeholder="What needs to be done?"
             value={props.draft} onInput={(e) => props.onInput(e.target.value)} />
      <button id="add" onClick={() => props.onAdd()}>Add</button>
    </div>
  );
}

function TodoItem(props) {
  return (
    <li class={props.todo.done ? "todo completed" : "todo"} data-id={props.todo.id}>
      <input class="toggle" type="checkbox" checked={props.todo.done}
             onChange={() => props.onToggle(props.todo.id)} />
      <span class="label">{props.todo.title}</span>
    </li>
  );
}

function Footer(props) {
  return (
    <div id="footer">
      <span id="count">
        {props.remaining + (props.remaining === 1 ? " item left" : " items left")}
      </span>
    </div>
  );
}

function App() {
  const [todos, setTodos] = createSignal<Todo[]>([]);
  const [draft, setDraft] = createSignal("");

  const remaining = createMemo(() => todos().filter((t) => !t.done).length);

  const addTodo = () => {
    const title = draft().trim();
    if (!title) return;
    const id = todos().reduce((m, t) => Math.max(m, t.id), 0) + 1;
    setTodos([...todos(), { id, title, done: false }]);
    setDraft("");
  };
  const toggle = (id) =>
    setTodos(todos().map((t) => (t.id === id ? { ...t, done: !t.done } : t)));

  return (
    <div id="app">
      <h1>todos</h1>
      <Header draft={draft()} onInput={setDraft} onAdd={addTodo} />
      <ul id="todo-list">
        <For each={todos()}>
          {(todo) => <TodoItem todo={todo} onToggle={toggle} />}
        </For>
      </ul>
      <Footer remaining={remaining()} />
    </div>
  );
}

render(() => <App />, document.getElementById("root"));
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p todomvc_supersolid --test todomvc`
Expected: PASS (all three tests).

- [ ] **Step 5: Commit**

```bash
git add examples/todomvc_supersolid/assets/ui/todomvc_supersolid/app.tsx examples/todomvc_supersolid/tests/todomvc.rs
git commit -m "feat(todomvc_supersolid): TodoItem toggle + reactive item count

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 4: Delete a todo (destroy button)

Add the destroy `<button>` to `TodoItem` and the `remove` handler. Deliverable: clicking destroy removes that row.

**Files:**
- Modify: `examples/todomvc_supersolid/assets/ui/todomvc_supersolid/app.tsx`
- Test: `examples/todomvc_supersolid/tests/todomvc.rs`

**Interfaces:**
- Consumes: harness `click`, `nodes_by_selector`.
- Produces: `.destroy` button per row; `remove(id)` handler.

- [ ] **Step 1: Write the failing test**

Add to `tests/todomvc.rs`:

```rust
#[test]
fn destroy_removes_a_todo() {
    let mut app = app();
    let _root = mount(&mut app);
    add(&mut app, "a");
    add(&mut app, "b");
    assert_eq!(li_labels(&app).len(), 2);

    // Click the destroy button of the first todo.
    let first_destroy = nodes_by_selector(&app, "li .destroy")[0];
    click(&mut app, first_destroy);

    assert_eq!(li_labels(&app), vec!["b".to_string()]);
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p todomvc_supersolid --test todomvc destroy_removes`
Expected: FAIL — no `.destroy` button yet.

- [ ] **Step 3: Add destroy to `TodoItem` + `remove` to `App`**

In `app.tsx`, replace the `TodoItem` function with:

```tsx
function TodoItem(props) {
  return (
    <li class={props.todo.done ? "todo completed" : "todo"} data-id={props.todo.id}>
      <input class="toggle" type="checkbox" checked={props.todo.done}
             onChange={() => props.onToggle(props.todo.id)} />
      <span class="label">{props.todo.title}</span>
      <button class="destroy" onClick={() => props.onRemove(props.todo.id)}>x</button>
    </li>
  );
}
```

Add the `remove` handler inside `App` (next to `toggle`):

```tsx
  const remove = (id) => setTodos(todos().filter((t) => t.id !== id));
```

And pass it to `TodoItem` in the `<For>`:

```tsx
          {(todo) => <TodoItem todo={todo} onToggle={toggle} onRemove={remove} />}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p todomvc_supersolid --test todomvc`
Expected: PASS (all four tests).

- [ ] **Step 5: Commit**

```bash
git add examples/todomvc_supersolid/assets/ui/todomvc_supersolid/app.tsx examples/todomvc_supersolid/tests/todomvc.rs
git commit -m "feat(todomvc_supersolid): delete a todo via the destroy button

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 5: Filters + footer visibility (`Footer` filter buttons, `<Show>`)

Add the `filter` signal, the `filtered` memo feeding the `<For>`, the three filter buttons with the reactive `selected` class, and wrap the footer in `<Show when={todos().length > 0}>` (classic hide-when-empty). Deliverable: switching filters shows the matching subset; footer hidden when empty.

**Files:**
- Modify: `examples/todomvc_supersolid/assets/ui/todomvc_supersolid/app.tsx`
- Test: `examples/todomvc_supersolid/tests/todomvc.rs`

**Interfaces:**
- Consumes: `Show` global; harness helpers.
- Produces: `#filter-all`/`#filter-active`/`#filter-completed` buttons; `filter` signal; `filtered` memo driving the list; footer gated by `<Show>`.

- [ ] **Step 1: Write the failing tests**

Add to `tests/todomvc.rs`:

```rust
#[test]
fn filters_show_active_and_completed_subsets() {
    let mut app = app();
    let _root = mount(&mut app);
    add(&mut app, "a");
    add(&mut app, "b");
    // Complete "a".
    let first_toggle = nodes_by_selector(&app, "li .toggle")[0];
    click_checkbox(&mut app, first_toggle);

    // Active filter -> only "b".
    let btn_active = node_by_selector(&app, "#filter-active");
    click(&mut app, btn_active);
    assert_eq!(li_labels(&app), vec!["b".to_string()]);

    // Completed filter -> only "a".
    let btn_completed = node_by_selector(&app, "#filter-completed");
    click(&mut app, btn_completed);
    assert_eq!(li_labels(&app), vec!["a".to_string()]);

    // Back to All -> both.
    let btn_all = node_by_selector(&app, "#filter-all");
    click(&mut app, btn_all);
    assert_eq!(li_labels(&app).len(), 2);
}

#[test]
fn footer_hidden_until_first_todo() {
    let mut app = app();
    let _root = mount(&mut app);
    // No todos yet -> <Show> renders nothing, so #count is absent.
    assert!(nodes_by_selector(&app, "#count").is_empty(), "footer hidden when empty");

    add(&mut app, "a");
    // Now the footer (and its count) appears.
    let count = node_by_selector(&app, "#count");
    assert_eq!(text_content(&app, count), "1 item left");
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p todomvc_supersolid --test todomvc filters_show_active footer_hidden`
Expected: FAIL — no filter buttons; footer always present.

- [ ] **Step 3: Update `app.tsx` with filter + `<Show>`**

Replace `assets/ui/todomvc_supersolid/app.tsx` with:

```tsx
import { createSignal, createMemo, For, Show, render } from "supersolid";

interface Todo { id: number; title: string; done: boolean; }
type Filter = "all" | "active" | "completed";

function Header(props) {
  return (
    <div id="new-todo-row">
      <input id="new-todo" type="text" placeholder="What needs to be done?"
             value={props.draft} onInput={(e) => props.onInput(e.target.value)} />
      <button id="add" onClick={() => props.onAdd()}>Add</button>
    </div>
  );
}

function TodoItem(props) {
  return (
    <li class={props.todo.done ? "todo completed" : "todo"} data-id={props.todo.id}>
      <input class="toggle" type="checkbox" checked={props.todo.done}
             onChange={() => props.onToggle(props.todo.id)} />
      <span class="label">{props.todo.title}</span>
      <button class="destroy" onClick={() => props.onRemove(props.todo.id)}>x</button>
    </li>
  );
}

function Footer(props) {
  return (
    <div id="footer">
      <span id="count">
        {props.remaining + (props.remaining === 1 ? " item left" : " items left")}
      </span>
      <div class="filters">
        <button id="filter-all" class={props.filter === "all" ? "filter selected" : "filter"}
                onClick={() => props.onFilter("all")}>All</button>
        <button id="filter-active" class={props.filter === "active" ? "filter selected" : "filter"}
                onClick={() => props.onFilter("active")}>Active</button>
        <button id="filter-completed"
                class={props.filter === "completed" ? "filter selected" : "filter"}
                onClick={() => props.onFilter("completed")}>Completed</button>
      </div>
    </div>
  );
}

function App() {
  const [todos, setTodos] = createSignal<Todo[]>([]);
  const [filter, setFilter] = createSignal<Filter>("all");
  const [draft, setDraft] = createSignal("");

  const remaining = createMemo(() => todos().filter((t) => !t.done).length);
  const filtered = createMemo(() => {
    const f = filter();
    return todos().filter((t) => (f === "all" ? true : f === "active" ? !t.done : t.done));
  });

  const addTodo = () => {
    const title = draft().trim();
    if (!title) return;
    const id = todos().reduce((m, t) => Math.max(m, t.id), 0) + 1;
    setTodos([...todos(), { id, title, done: false }]);
    setDraft("");
  };
  const toggle = (id) =>
    setTodos(todos().map((t) => (t.id === id ? { ...t, done: !t.done } : t)));
  const remove = (id) => setTodos(todos().filter((t) => t.id !== id));

  return (
    <div id="app">
      <h1>todos</h1>
      <Header draft={draft()} onInput={setDraft} onAdd={addTodo} />
      <ul id="todo-list">
        <For each={filtered()}>
          {(todo) => <TodoItem todo={todo} onToggle={toggle} onRemove={remove} />}
        </For>
      </ul>
      <Show when={todos().length > 0}>
        <Footer remaining={remaining()} filter={filter()} onFilter={setFilter} />
      </Show>
    </div>
  );
}

render(() => <App />, document.getElementById("root"));
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p todomvc_supersolid --test todomvc`
Expected: PASS (all tests). If `footer_hidden_until_first_todo` fails on the empty case, confirm `<Show>` lowered to `$ss.cmp(Show, { get when() {...}, get children() {...} })` and that `when` reads `todos().length`.

- [ ] **Step 5: Commit**

```bash
git add examples/todomvc_supersolid/assets/ui/todomvc_supersolid/app.tsx examples/todomvc_supersolid/tests/todomvc.rs
git commit -m "feat(todomvc_supersolid): filters (all/active/completed) + <Show> footer

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 6: Clear-completed + toggle-all

Add the `clearCompleted` and `toggleAll` handlers, the `#clear-completed` button in `Footer`, and the `#toggle-all` checkbox in `App`. This brings `app.tsx` to its final form (the reference at the top of this plan).

**Files:**
- Modify: `examples/todomvc_supersolid/assets/ui/todomvc_supersolid/app.tsx`
- Test: `examples/todomvc_supersolid/tests/todomvc.rs`

**Interfaces:**
- Consumes: harness helpers.
- Produces: `#clear-completed` button, `#toggle-all` checkbox; `clearCompleted`/`toggleAll` handlers.

- [ ] **Step 1: Write the failing tests**

Add to `tests/todomvc.rs`:

```rust
#[test]
fn clear_completed_removes_done_todos() {
    let mut app = app();
    let _root = mount(&mut app);
    add(&mut app, "a");
    add(&mut app, "b");
    // Complete "a".
    let first_toggle = nodes_by_selector(&app, "li .toggle")[0];
    click_checkbox(&mut app, first_toggle);

    let clear = node_by_selector(&app, "#clear-completed");
    click(&mut app, clear);

    assert_eq!(li_labels(&app), vec!["b".to_string()]);
}

#[test]
fn toggle_all_completes_then_clears_all() {
    let mut app = app();
    let _root = mount(&mut app);
    add(&mut app, "a");
    add(&mut app, "b");

    let toggle_all = node_by_selector(&app, "#toggle-all");
    // First change -> all complete -> 0 items left.
    click_checkbox(&mut app, toggle_all);
    let count = node_by_selector(&app, "#count");
    assert_eq!(text_content(&app, count), "0 items left");

    // Second change -> all active again -> 2 items left.
    click_checkbox(&mut app, toggle_all);
    let count = node_by_selector(&app, "#count");
    assert_eq!(text_content(&app, count), "2 items left");
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p todomvc_supersolid --test todomvc clear_completed toggle_all`
Expected: FAIL — no `#clear-completed` / `#toggle-all` yet.

- [ ] **Step 3: Replace `app.tsx` with the final form**

Replace `assets/ui/todomvc_supersolid/app.tsx` with the **final `app.tsx`** shown in full at the top of this plan (the "final form" code block under Global Constraints). It adds:
- `clearCompleted` and `toggleAll` handlers in `App`,
- the `#toggle-all` checkbox inside a `#main` wrapper around the list,
- `onClearCompleted` prop + `#clear-completed` button in `Footer`.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p todomvc_supersolid --test todomvc`
Expected: PASS (all functional tests).

- [ ] **Step 5: Commit**

```bash
git add examples/todomvc_supersolid/assets/ui/todomvc_supersolid/app.tsx examples/todomvc_supersolid/tests/todomvc.rs
git commit -m "feat(todomvc_supersolid): clear-completed + toggle-all (final app.tsx)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 7: HMR state-preservation test

Prove the headline: a hot reload (re-exec of the same module on the same runtime) preserves the live `App` state — todos, the active filter, and the half-typed draft — through a DOM rebuild → reconcile. This drives a `UiRuntime` directly with HMR enabled (mirroring `superui_bridge/tests/supersolid_render.rs`), transpiling the real `app.tsx` via the `supersolid` dev-dependency.

**Files:**
- Create: `examples/todomvc_supersolid/tests/hmr.rs`

**Interfaces:**
- Consumes: `supersolid::{transpile, TranspileOptions}`; `superui_bridge::{UiRuntime, reconcile_system, PendingDomEvent, PendingDomEvents}`; `superui_dom::Dom`; `superui_html::parse_document`; the same headless plugin set as the functional harness.
- Produces: an `hmr` integration test binary.

- [ ] **Step 1: Write the failing test**

`examples/todomvc_supersolid/tests/hmr.rs`:

```rust
//! State-preserving HMR over the REAL app.tsx: re-exec the transpiled module on
//! the same HMR-enabled runtime and assert todos + filter + draft survive the
//! DOM rebuild -> reconcile. Mirrors superui_bridge/tests/supersolid_render.rs.

use std::cell::RefCell;
use std::rc::Rc;

use bevy::asset::AssetPlugin;
use bevy::image::{ImagePlugin, TextureAtlasPlugin};
use bevy::input::InputPlugin;
use bevy::input_focus::{InputFocus, InputFocusVisible};
use bevy::picking::{InteractionPlugin, PickingPlugin};
use bevy::prelude::*;
use bevy::text::TextPlugin;
use bevy::ui::UiPlugin;
use superui_bridge::{reconcile_system, PendingDomEvent, PendingDomEvents, UiRuntime};
use superui_css::style::StyleSheet;
use superui_css::SuperUiCssPlugin;
use superui_dom::{Dom, NodeId};

const TSX: &str = include_str!("../assets/ui/todomvc_supersolid/app.tsx");

/// Transpile the real app.tsx exactly as the loader would (with a module_id so
/// component HMR ids are path-qualified).
fn transpile_app() -> String {
    let opts = supersolid::TranspileOptions {
        module_id: Some("ui/todomvc_supersolid/app.tsx".into()),
        ..Default::default()
    };
    supersolid::transpile(TSX, &opts).code
}

fn test_app() -> App {
    let mut app = App::new();
    app.add_plugins((
        bevy::time::TimePlugin,
        bevy::app::TaskPoolPlugin::default(),
        AssetPlugin::default(),
        WindowPlugin::default(),
        ImagePlugin::default(),
        TextureAtlasPlugin,
        TextPlugin,
        (InputPlugin, PickingPlugin, InteractionPlugin, UiPlugin),
        SuperUiCssPlugin,
    ));
    app.init_resource::<InputFocus>()
        .init_resource::<InputFocusVisible>();
    app.init_resource::<PendingDomEvents>();
    app.finish();
    app
}

/// Build an HMR-enabled UiRuntime around a fresh shell DOM, insert it, and add
/// the reconcile + event-drain systems.
fn mount_hmr(app: &mut App) -> Rc<RefCell<Dom>> {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<div id='root'></div>",
    )));
    let root = app.world_mut().spawn(Node::default()).id();
    let stylesheet: Handle<StyleSheet> = Handle::default();
    let rt = UiRuntime::new(dom.clone(), root, stylesheet, /* hmr */ true);
    app.world_mut().insert_non_send_resource(rt);
    app.add_systems(
        Update,
        (superui_bridge::drain_dom_events_system, reconcile_system).chain(),
    );
    dom
}

fn run(app: &mut App, js: &str) {
    app.world_mut()
        .non_send_resource_mut::<UiRuntime>()
        .run_script(js);
}

fn node(dom: &Rc<RefCell<Dom>>, sel: &str) -> NodeId {
    let d = dom.borrow();
    d.query_selector(d.document(), sel).unwrap()
}

fn label_texts(dom: &Rc<RefCell<Dom>>) -> Vec<String> {
    let d = dom.borrow();
    d.query_selector_all(d.document(), ".label")
        .into_iter()
        .map(|n| d.text_content(n))
        .collect()
}

#[test]
fn hot_reload_preserves_todos_filter_and_draft() {
    let js = transpile_app();
    let mut app = test_app();
    let dom = mount_hmr(&mut app);

    // Initial mount.
    run(&mut app, &js);
    app.update();

    // Add two todos by driving the real controlled input + Add button.
    let input = node(&dom, "#new-todo");
    dom.borrow_mut().set_value(input, "alpha");
    app.world_mut().non_send_resource_mut::<UiRuntime>().dirty = true;
    push_event(&mut app, input, "input");
    app.update();
    click(&mut app, &dom, "#add");

    let input = node(&dom, "#new-todo");
    dom.borrow_mut().set_value(input, "beta");
    app.world_mut().non_send_resource_mut::<UiRuntime>().dirty = true;
    push_event(&mut app, input, "input");
    app.update();
    click(&mut app, &dom, "#add");

    assert_eq!(label_texts(&dom), vec!["alpha".to_string(), "beta".to_string()]);

    // Switch the filter to "active" and type an un-submitted draft.
    click(&mut app, &dom, "#filter-active");
    let filter_active = node(&dom, "#filter-active");
    let sel_before = dom.borrow().classes(filter_active);
    assert!(sel_before.iter().any(|c| c == "selected"), "active filter selected");

    let input = node(&dom, "#new-todo");
    dom.borrow_mut().set_value(input, "half-typed");
    app.world_mut().non_send_resource_mut::<UiRuntime>().dirty = true;
    push_event(&mut app, input, "input");
    app.update();

    // Hot reload: re-exec the SAME module on the SAME runtime (as apply_hot_reload
    // does for a JsSource Modified event).
    run(&mut app, &js);
    app.update();

    // Todos preserved.
    assert_eq!(
        label_texts(&dom),
        vec!["alpha".to_string(), "beta".to_string()],
        "todos preserved across hot reload"
    );
    // Filter preserved (active still selected).
    let filter_active = node(&dom, "#filter-active");
    let sel_after = dom.borrow().classes(filter_active);
    assert!(sel_after.iter().any(|c| c == "selected"), "filter preserved across reload");
    // Draft preserved (controlled input value survived).
    let input = node(&dom, "#new-todo");
    assert_eq!(dom.borrow().value(input), "half-typed", "draft preserved across reload");
}

// --- small local event helpers (avoid a second support module) ---

fn push_event(app: &mut App, n: NodeId, ty: &str) {
    app.world_mut()
        .resource_mut::<PendingDomEvents>()
        .0
        .push(PendingDomEvent::new(n, ty));
}

fn click(app: &mut App, dom: &Rc<RefCell<Dom>>, sel: &str) {
    let n = node(dom, sel);
    push_event(app, n, "click");
    app.update();
    app.update();
}
```

- [ ] **Step 2: Run to verify it fails, then passes**

Run: `cargo test -p todomvc_supersolid --test hmr`
Expected: FAIL first if any harness signature is off (fix against `superui_bridge/tests/support/mod.rs` + `supersolid_render.rs`). Then PASS: `alpha`/`beta`, the active filter, and the `half-typed` draft all survive the reload.

> **Guidance:** the draft preservation is the sharpest assertion — it works only because `draft` is an `App` signal (preserved by Plan 5's cell rehydration) *and* the input `value` is reactively bound to it. If `value` shows `""` after reload, the `value={props.draft}` binding regressed to a static attr; inspect the transpiled output. Confirm `drain_dom_events_system` is exported from `superui_bridge` (it is used by `superui`'s plugin); if the event-drain system name differs, push events and call `app.update()` twice as `click` does.

- [ ] **Step 3: Commit**

```bash
git add examples/todomvc_supersolid/tests/hmr.rs
git commit -m "test(todomvc_supersolid): hot reload preserves todos + filter + draft

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 8: Runnable-binary finish — debug features, wasm build, docs, ledger

Add the `mcp_debug` + `debug-ui` module bodies to `main.rs` (so the requested BRP-driven inspection works), verify the wasm build, write the example README, and record the example in the capability ledger. Deliverable: all three run modes build, and the example is documented.

**Files:**
- Modify: `examples/todomvc_supersolid/src/main.rs`
- Create: `examples/todomvc_supersolid/README.md`
- Modify: `docs/support/README.md`

**Interfaces:**
- Consumes: `bevy_brp_extras::BrpExtrasPlugin` (already wired under `#[cfg(feature="mcp_debug")]` in `main`); patterns copied from `examples/todomvc/src/main.rs`.
- Produces: the `debug_ui` + `mcp_debug` modules; docs.

- [ ] **Step 1: Add the `debug-ui` + `mcp_debug` modules to `main.rs`**

Append to `examples/todomvc_supersolid/src/main.rs` the two modules from the plain example (`examples/todomvc/src/main.rs` lines 54–160), verbatim except adapting nothing (they reference only Bevy + `superui_css` types). Also add the `debug-ui` plugin registration to `main` under `#[cfg(feature = "debug-ui")]` exactly as the plain example does:

```rust
    #[cfg(feature = "debug-ui")]
    app.add_plugins(debug_ui::plugin);
```

Copy the `mod debug_ui { … }` block (`#[cfg(feature = "debug-ui")]`) and the `mod mcp_debug { … }` block (`#[cfg(feature = "mcp_debug")]`) from `examples/todomvc/src/main.rs` unchanged.

> **Guidance:** these modules are self-contained and compile-gated; they don't touch the Supersolid app. Do not duplicate the `use` imports already at the top of `main.rs`.

- [ ] **Step 2: Verify all feature combinations compile**

Run each and confirm PASS:

```bash
cargo build -p todomvc_supersolid
cargo build -p todomvc_supersolid --features hmr
cargo build -p todomvc_supersolid --features "mcp_debug debug-ui"
cargo test -p todomvc_supersolid
```

Expected: all PASS. (`cargo test` re-runs the functional + hmr suites.)

- [ ] **Step 3: Verify the wasm target builds**

Run (only if the target is installed; otherwise note it for the reviewer):

```bash
rustup target add wasm32-unknown-unknown
cargo build -p todomvc_supersolid --target wasm32-unknown-unknown
```

Expected: PASS. This proves the loader path (`app.generated.js`, no oxc in the binary) and the `getrandom` wasm backend are correct. If the target isn't available in this environment, record that Steps 2 passed and flag the wasm build for manual verification.

- [ ] **Step 4: Write the example README**

`examples/todomvc_supersolid/README.md`:

```markdown
# todomvc_supersolid

A runnable, hot-reloadable **TodoMVC authored in Solid-style `.tsx`** on
`bevy_superui` — the capstone of the Phase-2 / Supersolid series. Composes the
Supersolid transpiler, reactive core, render/control-flow layer, and
state-preserving HMR (Plans 1–5). The plain-HTML/CSS/JS `examples/todomvc` is the
Phase-1 counterpart.

## Run

| Command | Target | Source | Hot reload |
|---|---|---|---|
| `cargo run -p todomvc_supersolid --features hmr` | native | `app.tsx` (live) | ✅ state-preserving |
| `cargo run -p todomvc_supersolid` | native | `app.generated.js` | — |
| `cargo build -p todomvc_supersolid --target wasm32-unknown-unknown` | web | `app.generated.js` | — |

With `--features hmr`, edit `assets/ui/todomvc_supersolid/app.tsx` (or `style.css`)
while it runs: the view updates and your todos, active filter, and half-typed
new-todo text are preserved. `app.generated.js` is produced by `build.rs` from
`app.tsx` (gitignored) for the wasm / no-HMR paths — the transpiler (oxc) never
enters the wasm binary.

## Authoring

`app.tsx` is a small Solid app: all state lives in the top-level `App`
component (`todos`, `filter`, `draft` signals); `Header`, `TodoItem`, and `Footer`
are stateless views driven by props. State updates are immutable and
identity-preserving, so the keyed `<For>` reuses unchanged rows.

## Scope

Add / toggle / delete / toggle-all / filter (all·active·completed) /
clear-completed / item count — everything within the supported DOM subset
(`docs/support/`). Editing (needs `dblclick`/`event.key`) and persistence
(`localStorage` ⛔) are out of scope for this example.

## Debugging

`--features debug-ui` logs rendered text + colors and each click/key. `--features
mcp_debug` enables the Bevy Remote Protocol + BRP extras so the `bevy_brp_mcp`
server can screenshot, inject input, and inspect the live ECS world.
```

- [ ] **Step 5: Record the example in the ledger**

In `docs/support/README.md`, update the "Phase 1 status" section (append a Phase-2 note). Replace:

```markdown
## Phase 1 status

Phase 1 (TodoMVC) is complete. The ✅ rows below are what shipped; 🟡 rows are the
Phase 2/3 roadmap. TodoMVC itself exercises the T0/T1 ✅ core.
```

with:

```markdown
## Phase 1 status

Phase 1 (TodoMVC) is complete. The ✅ rows below are what shipped; 🟡 rows are the
Phase 2/3 roadmap. TodoMVC itself exercises the T0/T1 ✅ core.

## Phase 2 status

Phase 2 (Supersolid) is complete. `examples/todomvc_supersolid/` — a Solid-style
`.tsx` TodoMVC — now exercises the Supersolid ✅ rows in `js-dom.md`
(`createSignal`/`createMemo`, `render`, the `$ss.*` JSX runtime, `<For>`/`<Show>`,
and state-preserving HMR), all within the existing HTML/CSS/DOM subset (no new
capabilities were added for it).
```

- [ ] **Step 6: Commit**

```bash
git add examples/todomvc_supersolid/src/main.rs examples/todomvc_supersolid/README.md docs/support/README.md
git commit -m "feat(todomvc_supersolid): debug features, wasm verification, README + ledger note

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

- [ ] **Step 7: Update the plans README status row**

In `docs/superpowers/plans/README.md`, change the Plan 6 row status from `⏳ Just-in-time` to `✅ Done` with a link to this plan, and update the Phase-2 header note if it marks the phase incomplete. Commit:

```bash
git add docs/superpowers/plans/README.md
git commit -m "docs(supersolid): mark Phase-2 Plan 6 (Supersolid TodoMVC) done

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Final verification (whole-plan review)

After all tasks, run the whole suite and the build matrix once more:

```bash
cargo test -p todomvc_supersolid
cargo build -p todomvc_supersolid --features hmr
cargo build -p todomvc_supersolid --features "mcp_debug debug-ui"
cargo build -p todomvc_supersolid --target wasm32-unknown-unknown   # if target installed
```

Then a manual smoke run (native): `cargo run -p todomvc_supersolid --features hmr`, add/toggle/filter a few todos, edit `app.tsx` live, confirm state is preserved. Use `--features mcp_debug` + the `bevy_brp_mcp` tools to screenshot/inspect if issues are reported.
