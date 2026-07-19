# TodoMVC example + capability ledger Implementation Plan (Phase 1, Plan 6 of 6)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the last Phase 1 deliverables: a **runnable, hot-reloadable TodoMVC** authored as plain `index.html` + `style.css` + `app.js` on top of `SuperUiPlugin` (native run + `wasm32-unknown-unknown` build), plus the **capability ledger** (`docs/support/{README,html,css,js-dom}.md`) documenting exactly what the HTML/CSS/JS subset supports.

**Architecture:** A new workspace-member example crate `examples/todomvc/` holds a tiny Bevy binary (`DefaultPlugins` + `SuperUiPlugin` + a camera + one `SuperUiRoot` loading the three authored assets from `assets/ui/todomvc/`). All TodoMVC behaviour lives in the authored `app.js` / `index.html` / `style.css` — no bespoke Rust UI. The example doubles as the Phase-1 integration test: headless tests parse the **actual authored files** (`include_str!`), mount them through the real `superui` runtime, drive synthetic DOM events, and assert the resulting DOM. One small `superui_bridge` reconciler enhancement (render an `<input>`'s `value`/`placeholder` as visible text) makes the text field usable; everything else uses the already-implemented surface.

**Tech Stack:** Rust edition 2021, Bevy 0.17.3. `superui` / `superui_bridge` / `superui_css` / `superui_html` / `superui_dom` / `superui_js` / `superui_api` (all in-tree, merged). `serde`/`serde_json` for the one `window.bevy` round-trip demo. Docs are plain Markdown.

## Global Constraints

- **Bevy version: 0.17** everywhere (design §5). Do not bump to 0.18/0.19. The example binary uses `bevy = "0.17"` with `default-features = true` (it needs windowing/rendering, unlike the headless library crates).
- **Boundary discipline (design §4):** the only Rust *library* change in this plan is in `superui_bridge` (Task 1). `superui_dom`, `superui_html`, `superui_js`, `superui_api` MUST NOT gain any `bevy_*` dependency. The new example crate may depend on `bevy` + the `superui` umbrella crate.
- **Source of truth is the arena DOM, not the ECS** (design §3). Task 1's synthetic input-text entity is reconciler-managed state derived from the DOM `value`/`placeholder`; it is never read back as DOM.
- **Graceful degradation over throwing** (design §1): the example must never rely on a feature that throws. It uses only the verified-implemented DOM/CSS/JS surface enumerated below. Unsupported CSS in `style.css` is skipped by flair, never fatal — but prefer supported properties so the UI actually renders.
- **Authored surface only (design §1 north-star):** TodoMVC is authored in standards-shaped HTML/CSS/JS. No bespoke markup/widget API. The only non-web call permitted is the single `window.bevy` demo round-trip (`bevy.send`) that proves the seam (design §9).
- **The ledger is a first-class deliverable (design §7):** `docs/support/` is authored covering the *full landscape* (even unimplemented rows). Rows implemented in Phase 1 are ✅; achievable-but-unbuilt are 🟡 Roadmap; fundamentally out-of-scope are ⛔. Each row carries Status + Priority tier (T0–T3) + Notes.
- **`wasm32-unknown-unknown` must compile** for the example (design §5); the repo-root `.cargo/config.toml` already sets the `getrandom_backend="wasm_js"` rustflag, and every Boa-pulling crate declares the wasm `getrandom` dep. The example crate must add the same wasm `getrandom` dep.
- **TDD, DRY, YAGNI, frequent commits** — every task ends green with a commit.

### Verified implemented surface (read from the in-tree crates on 2026-07-19 — used verbatim below)

**DOM/JS API actually installed by `superui_api::install` (do not use anything not on this list in `app.js`):**
- `document`: `getElementById(id)`, `querySelector(sel)`, `querySelectorAll(sel)` (returns a real JS array), `createElement(tag)`, `createTextNode(data)`.
- Node/Element structural: `appendChild(child)`, `removeChild(child)`, `insertBefore(new, ref)`, `replaceChild(new, old)`; accessors `parentNode`, `firstChild`, `nextSibling`, `previousSibling`, `childNodes`, `children`, `nodeType`, `tagName` (element-only, upper-cased).
- Element attrs/content: `getAttribute`, `setAttribute`, `removeAttribute`, `hasAttribute`; accessors `id`, `className`, `textContent`, `innerText`, `value`, `checked`, `classList` (`add`/`remove`/`toggle`/`contains`), `style` (`setProperty`/`getPropertyValue`).
- Events: `addEventListener(type, cb, capture?)`, `removeEventListener(type, cb, capture?)`. The event object exposes `type`, `target`, `currentTarget`, `defaultPrevented`, `preventDefault()`, `stopPropagation()`, `stopImmediatePropagation()`. **It does NOT expose `.key`, `.keyCode`, `.code`, or `.which`** — a `keydown`/`keyup` handler cannot identify the key. Wired event types reaching JS: `click`, `change` (checkbox toggle, via the picking observer), `keydown`/`keyup` (to the focused node), `input` (character typed into a focused text input).
- Globals: `console.*`, `setTimeout`/`setInterval`/`clearTimeout`/`clearInterval`, `fetch` (warn-and-reject stub), and `window`/`bevy` (`bevy.send(name, data)` / `bevy.on(name, cb)`).

**`window.bevy` registration (design §8, `superui_bridge::SuperUiApp` on `App`):**
`app.add_superui_command::<T>("Name")` — JS `bevy.send("Name", payload)` deserializes `payload` into `T` (must be `Event + DeserializeOwned`) and `world.trigger(T)`s it. `app.add_superui_event::<T>("Name")` — a game-triggered `T` (`Event + Serialize`) is forwarded to JS `bevy.on("Name", cb)`.

**`superui` mount surface:** spawn one entity with `SuperUiRoot { html: Handle<HtmlSource>, css: Handle<StyleSheet>, js: Handle<JsSource> }` (all loaded via `AssetServer::load`). `SuperUiPlugin` mounts it, reconciles per frame, wires input, and hot-reloads on `AssetEvent::Modified`. The `<body>` reconciles into the `SuperUiRoot` entity.

**`superui_dom` reads used by Task 1 (all on `Dom`):** `value(id) -> String`, `get_attribute(id, "placeholder") -> Option<&str>`, `children(id)`, `get(id) -> Option<&NodeData>` with `NodeData::kind: NodeKind::{Document, Element(ElementData{ tag: String, .. }), Text(String)}`, `tag(id) -> Option<&str>`.

**CSS properties flair 0.6 supports (design §9 CSS subset — safe set for `style.css`):** the taffy/`bevy_ui` layout + visual subset — `display` (`flex`/`none`), `flex-direction`, `flex-grow`, `flex-shrink`, `flex-basis`, `justify-content`, `align-items`, `align-content`, `flex-wrap`, `width`/`height`/`min-width`/`min-height`/`max-width`/`max-height`, `margin`(+sides), `padding`(+sides), `border`(width, via `border`/`border-*-width`), `border-color`, `border-radius`, `position`(`relative`/`absolute`), `top`/`right`/`bottom`/`left`, `color`, `background-color`, `font-size`, `font-family`, `box-shadow`, `overflow`, `column-gap`/`row-gap`/`gap`. Selectors: type / class / id / descendant, plus wired `:hover`/`:focus`/`:checked`. If a property is uncertain, prefer one from this list; unknowns are skipped harmlessly.

---

## File Structure

```
crates/superui_bridge/
  src/runtime.rs                   # MODIFY: add `input_texts: HashMap<NodeId, Entity>` field + accessor
  src/reconcile.rs                 # MODIFY: render <input> value/placeholder as a managed Text child
  tests/reconcile.rs               # MODIFY: add input-text-rendering test

examples/todomvc/
  Cargo.toml                       # NEW: member crate; bevy(full) + superui + serde; wasm getrandom
  src/main.rs                      # NEW: DefaultPlugins + SuperUiPlugin + camera + SuperUiRoot + bevy.send demo
  assets/ui/todomvc/index.html     # NEW: authored TodoMVC markup
  assets/ui/todomvc/style.css      # NEW: authored TodoMVC styling (flair subset)
  assets/ui/todomvc/app.js         # NEW: authored TodoMVC behaviour
  tests/support/mod.rs             # NEW: headless harness (mount real assets, drive events)
  tests/todomvc.rs                 # NEW: integration tests over the REAL authored files (include_str!)

Cargo.toml                         # MODIFY: workspace `members` to include `examples/*`

docs/support/
  README.md                        # NEW: legend, tiers, scope, AI-context + sync policy
  html.md                          # NEW: HTML element/attribute ledger
  css.md                           # NEW: CSS property/selector/pseudo ledger
  js-dom.md                        # NEW: DOM/Web API ledger
docs/support/tests/ (or a crate test) # NEW: ledger-sync best-effort check (Task 8)

docs/superpowers/plans/README.md   # MODIFY (Task 9): flip Plan 6 → Done; mark Phase 1 complete
```

---

### Task 1: `superui_bridge` — render `<input>` value/placeholder as visible text

**Files:**
- Modify: `crates/superui_bridge/src/runtime.rs`
- Modify: `crates/superui_bridge/src/reconcile.rs`
- Modify: `crates/superui_bridge/tests/reconcile.rs`

**Interfaces:**
- Consumes: `superui_dom::Dom::{value, get_attribute, tag}`, `bevy::ui::widget::Text`.
- Produces:
  - New `UiRuntime` field `input_texts: HashMap<NodeId, Entity>` (private) mapping a text-`<input>` node to its reconciler-managed synthetic `Text` child entity.
  - `UiRuntime::ensure_input_text(&mut self, world, dom, input_node, input_entity) -> Entity` — get-or-spawn the synthetic `Text` child (marker `InputValueText`), set its content to `value` (or `placeholder` when value is empty), return it.
  - `pub struct InputValueText;` marker component (so the entity is identifiable/queryable in tests).
  - Behaviour: `sync_children` appends the synthetic text entity to a text-input element's children list, so it renders and survives `replace_children`. Stale-sweep removal of an input node also drops its `input_texts` entry.

- [ ] **Step 1: Write the failing test** — append to `crates/superui_bridge/tests/reconcile.rs`:

```rust
#[test]
fn input_renders_placeholder_then_value_as_text() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<input id='new' type='text' placeholder='What needs doing?'>",
    )));
    let mut app = test_app();
    let _root = mount(&mut app, dom.clone());
    app.update();

    let input_node = dom
        .borrow()
        .query_selector(dom.borrow().document(), "input")
        .unwrap();
    let input_ent = {
        let mut q = app.world_mut().query::<(Entity, &DomNode)>();
        q.iter(app.world())
            .find(|(_, d)| d.0 == input_node)
            .map(|(e, _)| e)
            .unwrap()
    };

    // Empty value -> the managed text child shows the placeholder.
    let text_of_input = |app: &mut App, input_ent: Entity| -> String {
        let kids = app.world().get::<Children>(input_ent).unwrap().to_vec();
        for k in kids {
            if app.world().get::<superui_bridge::InputValueText>(k).is_some() {
                return app.world().get::<Text>(k).unwrap().0.clone();
            }
        }
        panic!("input has no managed InputValueText child");
    };
    assert_eq!(text_of_input(&mut app, input_ent), "What needs doing?");

    // Type into the DOM value (as the keyboard seam would) and re-reconcile:
    // the SAME managed text child now shows the value.
    dom.borrow_mut().set_value(input_node, "Buy milk");
    app.world_mut()
        .non_send_resource_mut::<UiRuntime>()
        .dirty = true;
    app.update();
    assert_eq!(text_of_input(&mut app, input_ent), "Buy milk");
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p superui_bridge --test reconcile input_renders_placeholder_then_value_as_text`
Expected: FAIL — `superui_bridge::InputValueText` does not exist / no managed child.

- [ ] **Step 3: Add the field + marker + accessor** — in `crates/superui_bridge/src/runtime.rs`:

Add the marker near `DomNode`:

```rust
/// Marks the reconciler-managed synthetic `Text` child that renders a text
/// `<input>`'s current `value` (or its `placeholder` when the value is empty).
/// It has no DOM `NodeId` — it is derived state, keyed off the input node.
#[derive(Component, Clone, Copy, Debug)]
pub struct InputValueText;
```

Add the field to the `UiRuntime` struct (next to `node_to_entity`):

```rust
    /// Text-`<input>` node -> its managed `InputValueText` child entity.
    input_texts: HashMap<NodeId, Entity>,
```

Initialise it in `UiRuntime::new` (next to `entity_to_node: HashMap::new(),`):

```rust
            input_texts: HashMap::new(),
```

Export the marker — in `crates/superui_bridge/src/lib.rs`, extend the runtime re-export:

```rust
pub use runtime::{DomNode, InputValueText, UiRuntime};
```

- [ ] **Step 4: Implement the reconciler enhancement** — in `crates/superui_bridge/src/reconcile.rs`:

Add imports at the top (join the existing `runtime` use):

```rust
use crate::runtime::{DomNode, InputValueText, UiRuntime};
```

Add this helper inside `impl UiRuntime` (below `sync_identity`):

```rust
    /// Does `node` name a text-entry `<input>` (i.e. an `input` whose `type`
    /// is not `checkbox`)? Such inputs get a managed visible text child.
    fn is_text_input(dom: &superui_dom::Dom, node: NodeId) -> bool {
        matches!(dom.tag(node), Some("input"))
            && dom.get_attribute(node, "type") != Some("checkbox")
    }

    /// Ensure a text `<input>` has its managed `InputValueText` child, update its
    /// content (value, or placeholder when empty), and return the child entity.
    fn ensure_input_text(
        &mut self,
        world: &mut World,
        dom: &superui_dom::Dom,
        input_node: NodeId,
        input_entity: Entity,
    ) -> Entity {
        let value = dom.value(input_node);
        let content = if value.is_empty() {
            dom.get_attribute(input_node, "placeholder")
                .unwrap_or("")
                .to_string()
        } else {
            value
        };

        // Reuse the existing managed child if it is still alive; else spawn one.
        let existing = self
            .input_texts
            .get(&input_node)
            .copied()
            .filter(|e| world.get_entity(*e).is_ok());

        let entity = match existing {
            Some(e) => {
                if let Some(mut t) = world.get_mut::<Text>(e) {
                    if t.0 != content {
                        t.0 = content;
                    }
                }
                e
            }
            None => {
                let e = world
                    .spawn((Text::new(content), InputValueText))
                    .id();
                self.input_texts.insert(input_node, e);
                // Parent it under the input so recursive despawn cleans it up.
                world.entity_mut(input_entity).add_child(e);
                e
            }
        };
        entity
    }
```

In `sync_children`, append the managed text child to a text-input's child list so `replace_children` keeps it. Change the `child_entities` assembly so that, right **before** the final `replace_children(parent_entity, ...)`, we inject the managed child when the parent is a text input:

```rust
        // If this parent is a text <input>, append its managed value/placeholder
        // text child so it renders and survives replace_children.
        if Self::is_text_input(dom, parent_node) {
            let managed = self.ensure_input_text(world, dom, parent_node, parent_entity);
            child_entities.push(managed);
        }

        world
            .entity_mut(parent_entity)
            .replace_children(&child_entities);
```

In `reconcile`, when despawning a stale node, also drop its managed input-text mapping. In the stale-despawn loop, after `self.unbind(node, entity);` add:

```rust
            self.input_texts.remove(&node);
```

(The managed `Text` child is a Bevy child of the input entity, so `ec.despawn()` on the input removes it recursively; we only need to forget the map entry.)

If `world.get_entity(e)` has a different signature in 0.17 (e.g. returns `Result`), adjust the `.filter(|e| world.get_entity(*e).is_ok())` accordingly — the intent is "is this entity still alive?". If `add_child` is spelled differently, follow the compiler (`add_children(&[e])`).

- [ ] **Step 5: Run the reconcile tests**

Run: `cargo test -p superui_bridge --test reconcile`
Expected: PASS — all existing reconcile tests plus `input_renders_placeholder_then_value_as_text`.

- [ ] **Step 6: Verify boundary discipline unchanged**

Run (Bash): `cargo tree -p superui_dom -e normal | grep -i bevy || echo CLEAN`
Expected: `CLEAN` (Task 1 touched only `superui_bridge`).

- [ ] **Step 7: Commit**

```bash
git add crates/superui_bridge
git commit -m "feat(bridge): render <input> value/placeholder as visible text child"
```

---

### Task 2: Example crate scaffold + native binary + empty authored assets

**Files:**
- Modify: `Cargo.toml` (workspace `members`)
- Create: `examples/todomvc/Cargo.toml`
- Create: `examples/todomvc/src/main.rs`
- Create: `examples/todomvc/assets/ui/todomvc/index.html` (minimal shell)
- Create: `examples/todomvc/assets/ui/todomvc/style.css` (minimal)
- Create: `examples/todomvc/assets/ui/todomvc/app.js` (minimal)

**Interfaces:**
- Consumes: `superui::prelude::{SuperUiPlugin, SuperUiRoot, HtmlSource, JsSource}`, `superui_css::style::StyleSheet`, `bevy::prelude::*`.
- Produces: a runnable binary `todomvc` that mounts the three assets. No test yet (Task 3 adds the harness + tests).

- [ ] **Step 1: Add `examples/*` to the workspace members** — edit the root `Cargo.toml`:

```toml
[workspace]
resolver = "2"
members = ["crates/*", "examples/*"]
```

- [ ] **Step 2: Create the example crate manifest** — `examples/todomvc/Cargo.toml`:

```toml
[package]
name = "todomvc"
edition.workspace = true
version.workspace = true
license.workspace = true
publish = false

[dependencies]
superui = { path = "../../crates/superui" }
superui_css = { path = "../../crates/superui_css" }
# Full Bevy (windowing + rendering) — this is the runnable app, not a headless lib.
bevy = "0.17"
serde = { version = "1", features = ["derive"] }

[dev-dependencies]
# Headless integration tests reuse the browser stack without a window.
superui_bridge = { path = "../../crates/superui_bridge" }
superui_dom = { path = "../../crates/superui_dom" }
superui_html = { path = "../../crates/superui_html" }

# Boa (pulled transitively via superui) needs the JS getrandom backend on wasm.
[target.'cfg(target_arch = "wasm32")'.dependencies]
getrandom = { version = "0.3", features = ["wasm_js"] }
```

- [ ] **Step 3: Create the native binary** — `examples/todomvc/src/main.rs`:

```rust
//! Runnable TodoMVC — authored in plain HTML/CSS/JS under `assets/ui/todomvc/`,
//! mounted on `SuperUiPlugin`. `cargo run -p todomvc` (native, hot-reloading);
//! `cargo build -p todomvc --target wasm32-unknown-unknown` (web build).
//!
//! The only non-web wiring is the `window.bevy` demo: `app.js` fires
//! `bevy.send("TodoAdded", { label })` when a todo is added, which this binary
//! registers as a Bevy command and logs — proving the ECS seam (design §9).

use bevy::prelude::*;
use serde::Deserialize;
use superui::prelude::{SuperUiPlugin, SuperUiRoot};
use superui_css::style::StyleSheet;

/// Fired from JS via `bevy.send("TodoAdded", { label })`.
#[derive(Event, Deserialize, Debug, Clone)]
struct TodoAdded {
    label: String,
}

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(AssetPlugin {
        // Enable native hot reload (design §6). Inert on wasm.
        watch_for_changes_override: Some(true),
        ..default()
    }))
    .add_plugins(SuperUiPlugin);

    // Register the one demo command so `bevy.send("TodoAdded", ...)` reaches ECS.
    use superui::prelude::SuperUiApp;
    app.add_superui_command::<TodoAdded>("TodoAdded");
    app.add_observer(|ev: On<TodoAdded>| info!("todo added: {}", ev.event().label));

    app.add_systems(Startup, setup);
    app.run();
}

fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    commands.spawn(Camera2d);
    commands.spawn(SuperUiRoot {
        html: assets.load("ui/todomvc/index.html"),
        css: assets.load::<StyleSheet>("ui/todomvc/style.css"),
        js: assets.load("ui/todomvc/app.js"),
    });
}
```

If `watch_for_changes_override` is spelled differently in 0.17, follow the compiler (it is the `AssetPlugin` field controlling the file watcher). If `Camera2d` needs the `bevy_render`/`bevy_core_pipeline` features, `DefaultPlugins` already includes them.

- [ ] **Step 4: Create minimal placeholder assets** (Tasks 4–7 flesh these out) — `examples/todomvc/assets/ui/todomvc/index.html`:

```html
<div id="app"><h1>todos</h1></div>
```

`examples/todomvc/assets/ui/todomvc/style.css`:

```css
#app { display: flex; flex-direction: column; }
```

`examples/todomvc/assets/ui/todomvc/app.js`:

```js
// Behaviour added in Tasks 4-6.
console.log("todomvc loaded");
```

- [ ] **Step 5: Verify it compiles (native)**

Run: `cargo build -p todomvc`
Expected: builds (first build compiles full Bevy — slow once). Do NOT `cargo run` in CI/headless (it opens a window); compilation is the automated gate. A human runs `cargo run -p todomvc` to see it.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock examples/todomvc
git commit -m "feat(example): todomvc crate scaffold — SuperUiPlugin binary + asset shell"
```

---

### Task 3: Headless test harness for the example (mount real assets, drive events)

**Files:**
- Create: `examples/todomvc/tests/support/mod.rs`
- Create: `examples/todomvc/tests/todomvc.rs` (first smoke test)

**Interfaces:**
- Produces test helpers:
  - `fn app() -> App` — headless Bevy app with `SuperUiPlugin` on the CSS+UI+input stack, no window.
  - `fn mount_todomvc(app: &mut App) -> Entity` — put the **real** authored asset files (`include_str!`) into an in-memory asset source, spawn the `SuperUiRoot`, tick until mounted; return the root entity.
  - `fn tick(app: &mut App, n: usize)` — run `n` updates.
  - `fn click(app: &mut App, node: NodeId)` — enqueue a `click` DOM event on `node`.
  - `fn set_value(app: &mut App, node: NodeId, v: &str)` — set an input's DOM value via the engine (stands in for typing; the keyboard seam itself is unit-tested in `superui_bridge`).
  - `fn node_by_selector(app, sel) -> NodeId` / `fn text_content(app, node) -> String` — DOM read helpers.

- [ ] **Step 1: Create the harness** — `examples/todomvc/tests/support/mod.rs`:

```rust
//! Headless harness: mount the REAL authored TodoMVC assets through the real
//! `superui` runtime, then drive synthetic DOM events and read the DOM back.
#![allow(dead_code)]

use bevy::asset::io::memory::{Dir, MemoryAssetReader};
use bevy::asset::io::{AssetSource, AssetSourceId};
use bevy::asset::{AssetPlugin, LoadState};
use bevy::input::InputPlugin;
use bevy::input_focus::{InputFocus, InputFocusVisible};
use bevy::picking::{InteractionPlugin, PickingPlugin};
use bevy::prelude::*;
use bevy::text::TextPlugin;
use bevy::ui::UiPlugin;
use superui::prelude::{SuperUiRoot, SuperUiPlugin};
use superui_bridge::{PendingDomEvent, PendingDomEvents, UiRuntime};
use superui_css::style::StyleSheet;
use superui_dom::NodeId;

/// The real authored files — compiled into the test so the test exercises
/// exactly what ships.
pub const HTML: &str = include_str!("../../assets/ui/todomvc/index.html");
pub const CSS: &str = include_str!("../../assets/ui/todomvc/style.css");
pub const JS: &str = include_str!("../../assets/ui/todomvc/app.js");

/// A headless app with the full SuperUi stack and an in-memory asset source
/// holding the authored files.
pub fn app() -> App {
    let dir = Dir::new("assets".into());
    dir.insert_asset("ui/todomvc/index.html".as_ref(), HTML.as_bytes());
    dir.insert_asset("ui/todomvc/style.css".as_ref(), CSS.as_bytes());
    dir.insert_asset("ui/todomvc/app.js".as_ref(), JS.as_bytes());

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
        TextPlugin,
        (InputPlugin, PickingPlugin, InteractionPlugin, UiPlugin),
    ));
    app.init_resource::<InputFocus>()
        .init_resource::<InputFocusVisible>();
    app.add_plugins(SuperUiPlugin);
    app.finish();
    app
}

/// Spawn the `SuperUiRoot` and tick until the runtime is mounted.
pub fn mount_todomvc(app: &mut App) -> Entity {
    let (html, css, js) = {
        let server = app.world().resource::<AssetServer>().clone();
        (
            server.load("ui/todomvc/index.html"),
            server.load::<StyleSheet>("ui/todomvc/style.css"),
            server.load("ui/todomvc/app.js"),
        )
    };
    let root = app
        .world_mut()
        .spawn(SuperUiRoot { html, css, js })
        .id();
    for _ in 0..128 {
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

/// Resolve a selector against the live DOM.
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

/// Set an input's DOM value (stands in for typing — the keyboard seam is
/// unit-tested in `superui_bridge`). Marks the runtime dirty so it reconciles.
pub fn set_value(app: &mut App, node: NodeId, v: &str) {
    let mut rt = app.world_mut().non_send_resource_mut::<UiRuntime>();
    rt.dom.borrow_mut().set_value(node, v);
    rt.dirty = true;
}
```

If a plugin/type path differs (same as the `superui`/`superui_bridge` harnesses), follow the compiler. Note `nodes_by_selector` relies on `Dom::query_selector_all` (used by `superui_api`, so it exists).

- [ ] **Step 2: Write the smoke test** — `examples/todomvc/tests/todomvc.rs`:

```rust
//! Integration tests over the REAL authored TodoMVC files (compiled in via
//! `include_str!` in the harness), driven headlessly through the `superui` stack.
mod support;
use support::*;

#[test]
fn mounts_and_shows_title() {
    let mut app = app();
    let _root = mount_todomvc(&mut app);
    // The app mounted (a UiRuntime exists) and the <h1> title is present.
    let h1 = node_by_selector(&app, "h1");
    assert_eq!(text_content(&app, h1), "todos");
}
```

- [ ] **Step 3: Run it**

Run: `cargo test -p todomvc --test todomvc mounts_and_shows_title`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add examples/todomvc/tests
git commit -m "test(example): headless harness mounting the real authored assets"
```

---

### Task 4: `index.html` structure + `app.js` add-todo (Add button) + rendering

**Files:**
- Modify: `examples/todomvc/assets/ui/todomvc/index.html`
- Modify: `examples/todomvc/assets/ui/todomvc/app.js`
- Modify: `examples/todomvc/tests/todomvc.rs`

**Interfaces:**
- Produces the DOM contract the rest of the plan relies on (ids/classes): `#new-todo` (text input), `#add` (Add button), `#todo-list` (`ul`), `#count` (items-left text), `.filters` with `#filter-all`/`#filter-active`/`#filter-completed`, each todo `li.todo` containing `input.toggle[type=checkbox]`, `span.label`, `button.destroy`.

- [ ] **Step 1: Author the full markup** — replace `examples/todomvc/assets/ui/todomvc/index.html`:

```html
<div id="app">
  <h1>todos</h1>
  <div id="new-todo-row">
    <input id="new-todo" type="text" placeholder="What needs to be done?">
    <button id="add">Add</button>
  </div>
  <ul id="todo-list"></ul>
  <div id="footer">
    <span id="count">0 items left</span>
    <div class="filters">
      <button id="filter-all" class="filter selected">All</button>
      <button id="filter-active" class="filter">Active</button>
      <button id="filter-completed" class="filter">Completed</button>
    </div>
  </div>
</div>
```

- [ ] **Step 2: Author add-todo + render in `app.js`** — replace `examples/todomvc/assets/ui/todomvc/app.js`:

```js
// TodoMVC on bevy_superui. Authored in plain DOM/JS against the implemented
// subset (no `.key` on key events, so we add via the Add button, not Enter).

(function () {
  var todos = []; // { id, label, done }
  var nextId = 1;
  var filter = "all"; // all | active | completed

  var input = document.getElementById("new-todo");
  var list = document.getElementById("todo-list");
  var count = document.getElementById("count");

  function visible(t) {
    if (filter === "active") return !t.done;
    if (filter === "completed") return t.done;
    return true;
  }

  function render() {
    // Rebuild the list from state (simple + correct for Phase 1).
    while (list.firstChild) list.removeChild(list.firstChild);

    for (var i = 0; i < todos.length; i++) {
      var t = todos[i];
      if (!visible(t)) continue;

      var li = document.createElement("li");
      li.className = t.done ? "todo completed" : "todo";
      li.setAttribute("data-id", String(t.id));

      var toggle = document.createElement("input");
      toggle.setAttribute("type", "checkbox");
      toggle.className = "toggle";
      if (t.done) toggle.checked = true;
      toggle.addEventListener("change", makeToggleHandler(t.id));

      var label = document.createElement("span");
      label.className = "label";
      label.textContent = t.label;

      var destroy = document.createElement("button");
      destroy.className = "destroy";
      destroy.textContent = "x";
      destroy.addEventListener("click", makeDestroyHandler(t.id));

      li.appendChild(toggle);
      li.appendChild(label);
      li.appendChild(destroy);
      list.appendChild(li);
    }

    var left = todos.filter(function (t) { return !t.done; }).length;
    count.textContent = left + (left === 1 ? " item left" : " items left");
  }

  function makeToggleHandler(id) {
    return function () {
      for (var i = 0; i < todos.length; i++) {
        if (todos[i].id === id) { todos[i].done = !todos[i].done; break; }
      }
      render();
    };
  }
  function makeDestroyHandler(id) {
    return function () {
      todos = todos.filter(function (t) { return t.id !== id; });
      render();
    };
  }

  function addTodo() {
    var label = (input.value || "").trim();
    if (!label) return;
    todos.push({ id: nextId++, label: label, done: false });
    input.value = "";
    bevy.send("TodoAdded", { label: label }); // demo the ECS seam (design §9)
    render();
  }

  document.getElementById("add").addEventListener("click", addTodo);

  // Filters.
  function setFilter(name) {
    filter = name;
    var buttons = document.querySelectorAll(".filter");
    for (var i = 0; i < buttons.length; i++) buttons[i].classList.remove("selected");
    document.getElementById("filter-" + name).classList.add("selected");
    render();
  }
  document.getElementById("filter-all").addEventListener("click", function () { setFilter("all"); });
  document.getElementById("filter-active").addEventListener("click", function () { setFilter("active"); });
  document.getElementById("filter-completed").addEventListener("click", function () { setFilter("completed"); });

  render();
})();
```

- [ ] **Step 3: Write the add-todo test** — append to `examples/todomvc/tests/todomvc.rs`:

```rust
fn li_labels(app: &App) -> Vec<String> {
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
use superui_bridge::UiRuntime;

#[test]
fn add_button_appends_a_todo() {
    let mut app = app();
    let _root = mount_todomvc(&mut app);

    let input = node_by_selector(&app, "#new-todo");
    let add = node_by_selector(&app, "#add");

    set_value(&mut app, input, "Buy milk");
    click(&mut app, add);

    assert_eq!(li_labels(&app), vec!["Buy milk".to_string()]);
    // Input cleared after add; placeholder shows again in the rendered text.
    assert_eq!(value_of(&app, input), "");
}
```

If `Dom::query_selector` scoped to a subtree root (`li`) is available (it is — `query_selector(root, sel)`), the `.label` lookup works. If descendant scoping behaves globally, select `li .label` from document and index instead.

- [ ] **Step 4: Run the test**

Run: `cargo test -p todomvc --test todomvc add_button_appends_a_todo`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add examples/todomvc/assets examples/todomvc/tests
git commit -m "feat(example): todomvc markup + add-todo via Add button, list render"
```

---

### Task 5: Toggle-complete + delete tests

**Files:**
- Modify: `examples/todomvc/tests/todomvc.rs`

(`app.js` already implements toggle + delete in Task 4; this task proves them.)

- [ ] **Step 1: Write the toggle + delete tests** — append to `examples/todomvc/tests/todomvc.rs`:

```rust
fn add(app: &mut App, label: &str) {
    let input = node_by_selector(app, "#new-todo");
    let add = node_by_selector(app, "#add");
    set_value(app, input, label);
    click(app, add);
}

#[test]
fn toggle_marks_completed_and_updates_count() {
    let mut app = app();
    let _root = mount_todomvc(&mut app);
    add(&mut app, "a");
    add(&mut app, "b");

    let count = node_by_selector(&app, "#count");
    assert_eq!(text_content(&app, count), "2 items left");

    // Click the first todo's checkbox -> completed; count drops to 1.
    let first_toggle = nodes_by_selector(&app, "li .toggle")[0];
    click(&mut app, first_toggle); // click on a checkbox toggles it + fires change

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

#[test]
fn destroy_removes_a_todo() {
    let mut app = app();
    let _root = mount_todomvc(&mut app);
    add(&mut app, "a");
    add(&mut app, "b");
    assert_eq!(li_labels(&app).len(), 2);

    // Click the destroy button of the first todo.
    let first_destroy = nodes_by_selector(&app, "li .destroy")[0];
    click(&mut app, first_destroy);

    assert_eq!(li_labels(&app), vec!["b".to_string()]);
}
```

`click` on a checkbox routes through the picking-observer path in a real run; in the headless harness we enqueue a `click` PendingDomEvent, and the checkbox toggle+`change` also needs to fire. **Note:** the harness's `click` enqueues only a `click` event — the checkbox toggle/`change` mirroring happens in the picking *observer* (`on_pointer_click`), which the direct-enqueue path bypasses. So for checkboxes the enqueued `click` alone will NOT toggle `checked` or fire `change`.

Resolve this in the harness: add a `click_checkbox` helper that mirrors the observer's native behaviour (flip DOM `checked`, enqueue `change`) so tests exercise the same JS `change` listener a real click would. Add to `tests/support/mod.rs`:

```rust
/// Simulate a real pointer click on a checkbox: mirror the native toggle the
/// picking observer performs (flip DOM `checked`), then dispatch `change`.
pub fn click_checkbox(app: &mut App, node: NodeId) {
    {
        let mut rt = app.world_mut().non_send_resource_mut::<UiRuntime>();
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

Then in `toggle_marks_completed_and_updates_count`, replace `click(&mut app, first_toggle);` with `click_checkbox(&mut app, first_toggle);`.

- [ ] **Step 2: Run the tests**

Run: `cargo test -p todomvc --test todomvc`
Expected: PASS — all example tests.

- [ ] **Step 3: Commit**

```bash
git add examples/todomvc/tests
git commit -m "test(example): toggle-complete + destroy behaviours"
```

---

### Task 6: Filters + counter + `window.bevy` round-trip test

**Files:**
- Modify: `examples/todomvc/tests/todomvc.rs`

(`app.js` already implements filters + the `bevy.send` demo in Task 4; this task proves them.)

- [ ] **Step 1: Write filter + bridge tests** — append to `examples/todomvc/tests/todomvc.rs`:

```rust
#[test]
fn filters_show_active_and_completed_subsets() {
    let mut app = app();
    let _root = mount_todomvc(&mut app);
    add(&mut app, "a");
    add(&mut app, "b");
    // Complete "a".
    let first_toggle = nodes_by_selector(&app, "li .toggle")[0];
    click_checkbox(&mut app, first_toggle);

    // Active filter -> only "b".
    click(&mut app, node_by_selector(&app, "#filter-active"));
    assert_eq!(li_labels(&app), vec!["b".to_string()]);

    // Completed filter -> only "a".
    click(&mut app, node_by_selector(&app, "#filter-completed"));
    assert_eq!(li_labels(&app), vec!["a".to_string()]);

    // Back to All -> both.
    click(&mut app, node_by_selector(&app, "#filter-all"));
    assert_eq!(li_labels(&app).len(), 2);
}

#[test]
fn adding_a_todo_fires_bevy_send_into_ecs() {
    use bevy::prelude::*;
    use serde::Deserialize;
    use superui_bridge::SuperUiApp;

    #[derive(Event, Deserialize, Clone, Debug, PartialEq)]
    struct TodoAdded {
        label: String,
    }
    #[derive(Resource, Default)]
    struct Seen(Vec<String>);

    let mut app = app();
    app.add_superui_command::<TodoAdded>("TodoAdded");
    app.init_resource::<Seen>();
    app.add_observer(|ev: On<TodoAdded>, mut s: ResMut<Seen>| s.0.push(ev.event().label.clone()));

    let _root = mount_todomvc(&mut app);
    add(&mut app, "Ship it");
    tick(&mut app, 2);

    assert_eq!(app.world().resource::<Seen>().0, vec!["Ship it".to_string()]);
}
```

- [ ] **Step 2: Run the tests**

Run: `cargo test -p todomvc --test todomvc`
Expected: PASS — filters + the `window.bevy` round trip.

- [ ] **Step 3: Commit**

```bash
git add examples/todomvc/tests
git commit -m "test(example): filters + window.bevy round-trip"
```

---

### Task 7: `style.css` — full TodoMVC styling within the flair subset + wasm build check

**Files:**
- Modify: `examples/todomvc/assets/ui/todomvc/style.css`
- Modify: `examples/todomvc/tests/todomvc.rs` (a "css loads, no fatal" assertion)

**Interfaces:**
- Produces the visual layer using only the design-§9 CSS subset (flex, color, spacing, border, radius, font, sizing) + `:hover`/`:checked`. Correct *appearance* is verified by a human running the app; the automated gate is "the stylesheet loads and mounts without a fatal error, and a class-driven rule is present."

- [ ] **Step 1: Author the stylesheet** — replace `examples/todomvc/assets/ui/todomvc/style.css`:

```css
#app {
  display: flex;
  flex-direction: column;
  width: 420px;
  margin: 40px;
  padding: 16px;
  background-color: #ffffff;
  border: 1px solid #e0e0e0;
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
  margin: 8px;
}

#new-todo {
  flex-grow: 1;
  height: 32px;
  padding: 8px;
  border: 1px solid #cccccc;
  border-radius: 4px;
  color: #333333;
  font-size: 18px;
}

#add {
  height: 32px;
  padding: 8px;
  margin: 4px;
  background-color: #b83f45;
  color: #ffffff;
  border-radius: 4px;
}

#add:hover {
  background-color: #983035;
}

#todo-list {
  display: flex;
  flex-direction: column;
}

.todo {
  display: flex;
  flex-direction: row;
  align-items: center;
  padding: 8px;
  border-bottom: 1px solid #ededed;
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
  width: 24px;
  height: 24px;
  margin: 4px;
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
  border: 1px solid #ffffff;
  border-radius: 4px;
}

.filter.selected {
  border: 1px solid #b83f45;
  color: #b83f45;
}
```

- [ ] **Step 2: Assert the stylesheet mounts without fatal** — append to `examples/todomvc/tests/todomvc.rs`:

```rust
#[test]
fn stylesheet_loads_and_ui_reconciles_with_it() {
    // If style.css contained a fatal parse error, flair would fail to produce a
    // StyleSheet and mount would stall; reaching a mounted runtime + rendered
    // list proves the CSS loaded and cascaded without aborting.
    let mut app = app();
    let _root = mount_todomvc(&mut app);
    add(&mut app, "styled");
    // The todo rendered under the styled tree.
    assert_eq!(li_labels(&app), vec!["styled".to_string()]);
    // And the app entity carries a TypeName (reconciled body subtree exists).
    let has_h1 = {
        let mut q = app
            .world_mut()
            .query::<&superui_css::prelude::TypeName>();
        q.iter(app.world()).any(|t| t.0 == "h1")
    };
    assert!(has_h1);
}
```

- [ ] **Step 3: Run tests + the wasm build check**

Run: `cargo test -p todomvc --test todomvc`
Expected: PASS.

Run (Bash): `cargo build -p todomvc --target wasm32-unknown-unknown`
Expected: builds clean (the `.cargo/config.toml` getrandom rustflag + the crate's wasm `getrandom` dep make Boa compile for wasm). This is the design-§11 "wasm build check." Actually *running* in a browser needs `wasm-bindgen`/`trunk` (documented in Task 8's README), which is a manual step, not part of this automated gate.

If the wasm build fails on a missing getrandom backend, confirm the crate has the `[target.'cfg(target_arch = "wasm32")'.dependencies] getrandom = { version = "0.3", features = ["wasm_js"] }` block (Task 2 Step 2) and that `.cargo/config.toml` is unchanged.

- [ ] **Step 4: Commit**

```bash
git add examples/todomvc/assets examples/todomvc/tests
git commit -m "feat(example): todomvc stylesheet (flair subset) + wasm build check"
```

---

### Task 8: Capability ledger — `docs/support/README.md` + `js-dom.md` + sync check

**Files:**
- Create: `docs/support/README.md`
- Create: `docs/support/js-dom.md`
- Create: `examples/todomvc/tests/ledger.rs` (best-effort ledger↔impl sync check)

**Interfaces:**
- Produces the ledger legend/policy (README) and the DOM/Web API ledger (js-dom.md). Rows are Status (✅/🟡/⛔) + Tier (T0–T3) + Notes. ✅ rows are exactly the surface verified in this plan's "Verified implemented surface" block.

- [ ] **Step 1: Author `docs/support/README.md`**:

```markdown
# bevy_superui capability ledger

This directory is the **authoritative, machine-loadable record** of what the
`bevy_superui` HTML/CSS/JS subset supports. It is both the **AI-context document**
(load it so generated UI stays inside the supported surface) and the **roadmap
tracker**. See the design: `../superpowers/specs/2026-07-18-bevy-superui-design.md` §7.

## Legend

**Status**
- ✅ **Supported** — implemented and covered by tests.
- 🟡 **Roadmap** — achievable in theory (regardless of what `bevy_ui` can do
  today) and planned; currently degrades gracefully (no-op / skipped / warn).
- ⛔ **Won't support** — fundamentally out of scope (network, cookies, real
  navigation, multi-document).

**Priority tier** (by game-UI usefulness, for ordering work)
- **T0** essential — layout, text, click, class toggling.
- **T1** common — inputs, lists, hover/focus, transitions.
- **T2** advanced — SVG, canvas, animations, transforms.
- **T3** niche.

Rows are ordered T0 first, so the top of each file is the highest-value surface.

## Files
- `html.md` — HTML elements & attributes.
- `css.md` — CSS properties, selectors, pseudo-classes, at-rules.
- `js-dom.md` — DOM/Web API objects, methods, events.

## Graceful degradation

Unsupported features never hard-crash (design §1): unknown tags render as plain
boxes, unknown CSS is skipped, unimplemented JS methods no-op/warn, `fetch` warns
and rejects. AI-generated code that touches an unimplemented corner keeps running.

## Sync policy

The ledger is kept in step with the implementation. `examples/todomvc/tests/ledger.rs`
is a best-effort check that a sample of ✅ DOM rows have a live binding (executing a
snippet using them does not throw). Tighten over time; it is a smoke test, not a proof.

## Phase 1 status

Phase 1 (TodoMVC) is complete. The ✅ rows below are what shipped; 🟡 rows are the
Phase 2/3 roadmap. TodoMVC itself exercises the T0/T1 ✅ core.
```

- [ ] **Step 2: Author `docs/support/js-dom.md`** — enumerate the DOM/Web API surface. ✅ rows are the verified-implemented set from this plan's header; include the key 🟡/⛔ landscape rows:

```markdown
# JS / DOM / Web API ledger

Status ✅ Supported · 🟡 Roadmap · ⛔ Won't support. Tier T0–T3.
Engine: Boa on every target (design §5). ✅ = installed by `superui_api` +
`superui_bridge` and covered by tests.

## document

| API | Status | Tier | Notes |
|---|---|---|---|
| `document.getElementById(id)` | ✅ | T0 | |
| `document.querySelector(sel)` | ✅ | T0 | type/class/id/descendant selectors |
| `document.querySelectorAll(sel)` | ✅ | T0 | returns a real JS array |
| `document.createElement(tag)` | ✅ | T0 | |
| `document.createTextNode(data)` | ✅ | T0 | |
| `document.body` / `document.head` | 🟡 | T1 | reachable via `querySelector("body")` today |
| `document.createDocumentFragment()` | 🟡 | T2 | |

## Node / Element — structure

| API | Status | Tier | Notes |
|---|---|---|---|
| `appendChild` / `removeChild` | ✅ | T0 | |
| `insertBefore` / `replaceChild` | ✅ | T0 | |
| `parentNode` / `childNodes` / `children` | ✅ | T0 | `children` = element children only |
| `firstChild` / `nextSibling` / `previousSibling` | ✅ | T1 | |
| `nodeType` / `tagName` | ✅ | T1 | `tagName` upper-cased, element-only |
| `cloneNode` | 🟡 | T2 | |
| `innerHTML` (get/set) | 🟡 | T1 | parse-on-set is roadmap |

## Element — attributes / content / state

| API | Status | Tier | Notes |
|---|---|---|---|
| `getAttribute` / `setAttribute` / `removeAttribute` / `hasAttribute` | ✅ | T0 | |
| `id` / `className` | ✅ | T0 | |
| `textContent` / `innerText` | ✅ | T0 | |
| `value` (get/set) | ✅ | T1 | text inputs render value; see reconciler |
| `checked` (get/set) | ✅ | T1 | drives `:checked` |
| `classList.add/remove/toggle/contains` | ✅ | T0 | |
| `style.setProperty / getPropertyValue` | ✅ | T1 | inline style, cascaded by flair |
| `getBoundingClientRect()` | 🟡 | T2 | needs post-layout read-back |
| `focus()` / `blur()` | 🟡 | T1 | focus is set on click today |

## Events

| API | Status | Tier | Notes |
|---|---|---|---|
| `addEventListener` / `removeEventListener` | ✅ | T0 | capture flag honored |
| capture → target → bubble dispatch | ✅ | T0 | W3C order |
| `event.target` / `currentTarget` | ✅ | T0 | |
| `event.type` / `defaultPrevented` | ✅ | T0 | |
| `event.preventDefault` / `stopPropagation` / `stopImmediatePropagation` | ✅ | T0 | |
| `click` | ✅ | T0 | via `bevy_picking` |
| `change` (checkbox) | ✅ | T1 | fired on checkbox toggle |
| `input` (text field) | ✅ | T1 | fired on character typed |
| `keydown` / `keyup` | ✅ | T1 | dispatched to focused node |
| `event.key` / `keyCode` / `code` | 🟡 | T1 | **not exposed yet** — key identity unavailable to JS; add an Add button instead of Enter |
| `submit` | 🟡 | T1 | no `<form>` submit wiring yet |
| `mouseover` / `mouseout` / `focus` / `blur` events | 🟡 | T1 | hover state exists in CSS; JS events roadmap |

## Globals

| API | Status | Tier | Notes |
|---|---|---|---|
| `console.log/warn/error/info/debug` | ✅ | T0 | |
| `setTimeout` / `setInterval` / `clearTimeout` / `clearInterval` | ✅ | T1 | driven by Bevy's clock |
| `window` (alias of `globalThis`) | ✅ | T1 | |
| `window.bevy.send(name, data)` | ✅ | T1 | JS → ECS (design §8) |
| `window.bevy.on(name, cb)` | ✅ | T1 | ECS → JS |
| `window.bevy.query(path)` | 🟡 | T2 | async state read — Phase 2 |
| `history.pushState` / `replaceState` / `popstate` / `location` | 🟡 | T3 | in-memory routing state (design §7) |
| `fetch` / `XMLHttpRequest` | ⛔ | — | network; warn-and-reject stub only |
| `localStorage` / `cookie` | ⛔ | — | out of scope (games persist via ECS) |
```

- [ ] **Step 3: Write the ledger-sync smoke test** — `examples/todomvc/tests/ledger.rs`:

```rust
//! Best-effort ledger↔impl sync (design §11): a handful of ✅ DOM rows must have
//! a live binding — executing a snippet that uses them must not throw.
mod support;
use support::*;
use superui_bridge::UiRuntime;

#[test]
fn sampled_supported_dom_apis_have_live_bindings() {
    let mut app = app();
    let _root = mount_todomvc(&mut app);

    // Exercise a representative slice of the ✅ js-dom.md surface. If any of
    // these were secretly unimplemented, the eval would throw and `run_script`
    // would warn+swallow — so we assert an observable side effect instead.
    let mut rt = app.world_mut().non_send_resource_mut::<UiRuntime>();
    rt.run_script(
        "var d = document.createElement('div'); \
         d.id = 'ledger-probe'; d.className = 'x'; \
         d.setAttribute('data-k', 'v'); \
         d.classList.add('y'); \
         var t = document.createTextNode('hi'); d.appendChild(t); \
         document.getElementById('app').appendChild(d);",
    );
    drop(rt);
    tick(&mut app, 2);

    // The probe node materialised with its attributes -> the sampled APIs work.
    let probe = node_by_selector(&app, "#ledger-probe");
    let (has_class, attr, text) = {
        let rt = app.world().non_send_resource::<UiRuntime>();
        let d = rt.dom.borrow();
        (
            d.classes(probe).iter().any(|c| c == "y"),
            d.get_attribute(probe, "data-k").map(|s| s.to_string()),
            d.text_content(probe),
        )
    };
    assert!(has_class);
    assert_eq!(attr.as_deref(), Some("v"));
    assert_eq!(text, "hi");
}
```

- [ ] **Step 4: Run the ledger test**

Run: `cargo test -p todomvc --test ledger`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add docs/support/README.md docs/support/js-dom.md examples/todomvc/tests/ledger.rs
git commit -m "docs(ledger): README + js-dom.md + best-effort sync smoke test"
```

---

### Task 9: Capability ledger — `html.md` + `css.md`, and flip the plan series to Done

**Files:**
- Create: `docs/support/html.md`
- Create: `docs/support/css.md`
- Modify: `docs/superpowers/plans/README.md`

**Interfaces:** completes the ledger landscape and marks Phase 1 complete.

- [ ] **Step 1: Author `docs/support/html.md`**:

```markdown
# HTML element / attribute ledger

Status ✅ Supported · 🟡 Roadmap · ⛔ Won't support. Tier T0–T3.
Parser: `html5ever` (`superui_html`). Unknown tags render as plain boxes;
unknown attributes are ignored (design §1).

## Elements

| Element | Status | Tier | Notes |
|---|---|---|---|
| `div` / `span` / `p` | ✅ | T0 | generic boxes |
| `h1`–`h6` | ✅ | T0 | text sizing via CSS |
| `ul` / `ol` / `li` | ✅ | T0 | plain flex boxes (no list markers yet) |
| `button` | ✅ | T0 | clickable |
| `input type=text` | ✅ | T1 | value renders as text; typed via keyboard seam |
| `input type=checkbox` | ✅ | T1 | toggles `checked`, drives `:checked` |
| `label` | ✅ | T1 | plain box (no implicit `for` focus yet) |
| text nodes | ✅ | T0 | rendered via `bevy_ui` `Text` |
| `a` (anchor) | 🟡 | T1 | renders; no navigation (no network) |
| `img` | 🟡 | T2 | needs image asset wiring |
| `form` | 🟡 | T1 | renders; no `submit` semantics yet |
| `select` / `option` / `textarea` | 🟡 | T2 | |
| `table`/`tr`/`td` | 🟡 | T2 | via flex/grid approximation |
| `svg` + children | 🟡 | T2 | AI emits it often; planned |
| `canvas` | 🟡 | T3 | |
| `iframe` (to a server) | ⛔ | — | multi-document / network |

## Attributes

| Attribute | Status | Tier | Notes |
|---|---|---|---|
| `id` | ✅ | T0 | id selector (`#x`) |
| `class` | ✅ | T0 | class selector (`.x`) |
| `type` (input) | ✅ | T1 | `text` / `checkbox` |
| `value` (input) | ✅ | T1 | |
| `checked` (input) | ✅ | T1 | |
| `placeholder` (input) | ✅ | T1 | shown when value empty |
| `style` (inline) | ✅ | T1 | cascaded by flair |
| `data-*` | ✅ | T1 | stored, readable via `getAttribute` |
| `href` | 🟡 | T1 | stored; no navigation |
| `disabled` | 🟡 | T1 | |
| `title` / `alt` | 🟡 | T3 | |
```

- [ ] **Step 2: Author `docs/support/css.md`**:

```markdown
# CSS property / selector ledger

Status ✅ Supported · 🟡 Roadmap · ⛔ Won't support. Tier T0–T3.
Engine: forked `bevy_flair` 0.6 (`superui_css`) over taffy + `bevy_ui`. Supported
surface = { standards-shaped CSS } ∩ { what taffy/bevy_ui express } (design §1).
Unknown properties/rules are skipped, never fatal.

## Selectors

| Selector | Status | Tier | Notes |
|---|---|---|---|
| type (`li`) | ✅ | T0 | |
| class (`.todo`) | ✅ | T0 | |
| id (`#app`) | ✅ | T0 | matches on entity `Name` |
| descendant (`.todo .label`) | ✅ | T0 | |
| compound (`.todo.completed`) | ✅ | T1 | |
| `:hover` | ✅ | T1 | via `bevy_picking` hover |
| `:checked` | ✅ | T1 | checkbox state |
| `:focus` | ✅ | T1 | focus set on click |
| child (`>`) / sibling (`+`, `~`) | 🟡 | T2 | |
| `:nth-child`, `::before/::after` | 🟡 | T2 | |

## Properties (layout)

| Property | Status | Tier | Notes |
|---|---|---|---|
| `display: flex / none` | ✅ | T0 | taffy flexbox |
| `flex-direction` / `flex-wrap` | ✅ | T0 | |
| `flex-grow` / `flex-shrink` / `flex-basis` | ✅ | T1 | |
| `justify-content` / `align-items` / `align-content` | ✅ | T0 | |
| `gap` / `row-gap` / `column-gap` | ✅ | T1 | |
| `width` / `height` (+ `min`/`max`) | ✅ | T0 | px, %, auto, vw/vh |
| `margin` / `padding` (+ sides) | ✅ | T0 | |
| `position: relative / absolute` + `top/right/bottom/left` | ✅ | T1 | |
| `overflow` | ✅ | T1 | |
| `display: grid` | 🟡 | T2 | taffy supports grid; wiring roadmap |
| `float` | ⛔ | — | not in taffy's box model (design §2) |

## Properties (visual / text)

| Property | Status | Tier | Notes |
|---|---|---|---|
| `color` | ✅ | T0 | named + hex + rgb/oklch |
| `background-color` | ✅ | T0 | |
| `border` / `border-*-width` / `border-color` | ✅ | T1 | |
| `border-radius` | ✅ | T1 | |
| `box-shadow` | ✅ | T2 | |
| `font-size` / `font-family` | ✅ | T1 | |
| `opacity` | 🟡 | T2 | |
| `transition` | 🟡 | T2 | flair has animation infra |
| `transform` (translate/scale/rotate) | 🟡 | T2 | |
| `background-image` (gradient) | 🟡 | T2 | flair parses gradients |
| `text-align` / `line-height` | 🟡 | T1 | |
| `background-image: url()` | 🟡 | T2 | needs asset wiring |

## At-rules

| At-rule | Status | Tier | Notes |
|---|---|---|---|
| `@media` | 🟡 | T2 | flair supports media selectors |
| `@keyframes` / `@import` / `@layer` | 🟡 | T2 | flair infra present |
```

- [ ] **Step 3: Flip the plan series to Done** — in `docs/superpowers/plans/README.md`, edit the table's Plan 6 row from `⬜ Not started` to:

```markdown
| 6 | `examples/todomvc` + `docs/support/` | The runnable TodoMVC example (native + wasm) and the capability ledger (`html.md` / `css.md` / `js-dom.md`, status ✅/🟡 Roadmap/⛔). | ✅ Done — merged to `main` ([plan](./2026-07-19-superui-phase1-06-todomvc.md)) |
```

Then replace the "Resuming in a fresh session" block's Plan-6-targeted prompt with a short **Phase 1 complete** note pointing at the component-framework direction doc as the next brainstorm input:

```markdown
## Phase 1 complete

All 6 plans are done and merged to `main`. Phase 1's deliverable — a runnable,
hot-reloadable TodoMVC authored in plain HTML/CSS/JS (native + wasm) plus the
`docs/support/` capability ledger — has shipped. Run it with `cargo run -p todomvc`.

**Next:** Phase 2 (browser-ish completeness) and Phase 3 (the component/reactivity
framework) — the latter has an agreed direction in
[`../specs/2026-07-19-superui-component-framework-direction.md`](../specs/2026-07-19-superui-component-framework-direction.md),
which begins its own brainstorm → spec → plan cycle when the team is ready.
```

- [ ] **Step 4: Verify the whole workspace is green + docs render**

Run: `cargo test --workspace`
Expected: PASS across all crates + the example.

Run (Bash): `cargo build -p todomvc --target wasm32-unknown-unknown`
Expected: builds.

- [ ] **Step 5: Commit**

```bash
git add docs/support/html.md docs/support/css.md docs/superpowers/plans/README.md
git commit -m "docs(ledger): html.md + css.md; mark Phase 1 (plan series) complete"
```

---

## Self-Review

**Spec coverage (design §9 Definition of Done):**
- "Add / toggle / delete todos" → Tasks 4 (add), 5 (toggle, delete). ✅
- "filter all / active / completed" → Task 6. ✅
- "live 'N items left' counter" → Task 4 (`render()` count) + asserted in Task 5. ✅
- "Runs native" → Task 2 (binary builds; human runs `cargo run -p todomvc`). ✅ (windowed run is manual — noted honestly).
- "builds and loads on wasm" → Task 7 build check (build ✅; browser *load* via trunk/wasm-bindgen is a documented manual step, not an automated gate — stated plainly).
- "Editing app.js/style.css on native hot-reloads" → inherited from Plan 5's `SuperUiPlugin` (verified there); the example enables the file watcher (Task 2). No new code needed.
- "`docs/support/` ledger exists, full landscape, ✅/🟡/⛔" → Tasks 8–9. ✅

**Naming/type consistency:** `SuperUiRoot { html, css, js }`, `add_superui_command::<T>(name)`, `PendingDomEvent::new`, `PendingDomEvents`, `UiRuntime`, `DomNode`, `InputValueText`, `Dom::{value, set_value, checked, set_checked, classes, get_attribute, tag, query_selector, query_selector_all, text_content}` — all match the surfaces read from the crates on 2026-07-19. The DOM contract ids/classes (`#new-todo`, `#add`, `#todo-list`, `#count`, `.filter`, `.toggle`, `.label`, `.destroy`, `.completed`) are defined in Task 4 and used consistently in Tasks 5–7.

**Placeholder scan:** every code/asset/doc step carries complete content. The ledger tables are concrete rows (not "enumerate the rest"); they cover the landscape the design asks for at Phase-1 granularity and can be extended later without being placeholders now.

**Known honest limitations surfaced (not hidden):** (1) Enter-to-add is not possible (no `event.key`) → Add button + 🟡 ledger row; (2) checkbox `change` in headless tests is driven by a harness helper mirroring the picking observer, because the direct event-enqueue path bypasses the observer's native toggle; (3) windowed native run and browser wasm load are manual, with build-level automated gates.

## Execution Handoff

Plan complete. Two execution options:

1. **Subagent-Driven (recommended, matches this series' convention)** — dispatch a fresh subagent per task, review between tasks, plus a whole-branch review before merge.
2. **Inline Execution** — execute tasks in this session with checkpoints.
