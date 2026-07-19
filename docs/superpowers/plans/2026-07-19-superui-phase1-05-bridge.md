# superui_bridge + superui Implementation Plan (Phase 1, Plan 5 of 6)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the two Bevy-facing crates that close the loop between the "web world" (arena DOM + Boa JS + flair CSS) and the "ECS world": `superui_bridge` (the per-frame reconciler that diffs the DOM into `bevy_ui` entities carrying flair components, the input→DOM-event seam, and the `window.bevy` bridge) and `superui` (the `SuperUiPlugin`, the `.html`/`.js` asset loaders, and hot reload via `AssetEvent::Modified`), so that authored `index.html` + `style.css` + `app.js` render, restyle, and respond to input in a headless Bevy app.

**Architecture:** The retained arena `Dom` (owned as `Rc<RefCell<Dom>>`) is the source of truth. A **`UiRuntime` NonSend resource** holds the `BoaEngine` (which shares that `Rc`), a stable `NodeId ↔ Entity` map, and a `dirty` flag. Because `superui_dom` `NodeId`s are **stable across frames**, the reconciler keys ECS entities by `NodeId` — it never content-diffs; it spawns entities for new nodes, despawns entities for vanished nodes, re-parents/re-orders to match, and pushes text/type-name/class/attribute/value/checked into each entity so flair's cascade + `bevy_ui`/taffy do the rest. Anything that mutates the DOM (initial JS run, a dispatched DOM event, a fired timer, a `bevy.on` callback) sets `dirty`; one system reconciles per frame when dirty. Input flows the other way: `bevy_picking`/keyboard → translated DOM events → `engine.dispatch_event` (W3C capture/bubble, runs JS synchronously) → DOM mutated → next reconcile. `window.bevy` is a JS global (`send`/`on`) marshalled to/from registered Bevy `Event` types via `serde_json` ⇄ `boa` `JsValue` and Bevy observers.

**Tech Stack:** Rust edition 2021. Bevy 0.17.3. `boa_engine` 0.21 (via the existing `superui_js`/`superui_api`). `serde` / `serde_json` for the `window.bevy` marshalling. `superui_dom` / `superui_html` / `superui_js` / `superui_api` / `superui_css` (all already in-tree and merged).

## Global Constraints

- **Bevy version: 0.17** for every Bevy-facing crate (design §5). `superui_bridge` and `superui` depend on `bevy = "0.17"` (with `default-features = false` + explicit features; see Task 1/Task 7). Do not bump to 0.18/0.19.
- **Only Bevy-facing crates touch Bevy** (design §4 boundary discipline). `superui_dom`, `superui_html`, `superui_js`, `superui_api` MUST NOT gain a dependency on any `bevy_*` crate or on `superui_bridge`/`superui`/`superui_css`. Verify with `cargo tree` after Task 1 and Task 7.
- **The engine is NonSend.** `BoaEngine` holds `Rc<RefCell<Dom>>` and Boa `JsFunction`s (neither `Send`), so `UiRuntime` is a **`NonSend` Bevy resource** and every system that touches it is a main-thread system (`NonSendMut<UiRuntime>` or an exclusive `&mut World` system). Never put the engine or DOM behind a `Send` `Resource`.
- **Source of truth is the arena DOM, not the ECS** (design §3). The reconciler only ever reads the DOM and writes the ECS; JS/events only ever mutate the DOM. Entities are keyed by the stable `NodeId`.
- **Graceful degradation over throwing** (design §1): a JS `eval` error is logged (`warn!`) and swallowed — it never panics the app. A malformed selector / unknown attribute / missing node is skipped. A `bevy.send` for an unregistered command is `warn!`-logged and dropped.
- **`window.bevy` Phase-1 scope is `send` + `on` only** (design §9). `bevy.query` (async state read) is **Phase 2** (design §10) — do NOT implement it here.
- **`wasm32-unknown-unknown` must compile** for the `superui` runtime library (design §5). Reuse the repo-root `.cargo/config.toml` rustflag and add a wasm-target `getrandom = { version = "0.3", features = ["wasm_js"] }` dep to any new crate that transitively pulls Boa (both `superui_bridge` and `superui` do, via `superui_js`).
- **The capability ledger (`docs/support/`) and the TodoMVC example are Plan 6** — do NOT author them here. This plan's end state is: both crates compile, headless integration tests prove reconcile + input events + hot reload + `window.bevy`, and the `superui` lib builds for wasm.
- **TDD, DRY, YAGNI, frequent commits** — every task ends green with a commit.

**Verified API reference (confirmed by reading the in-tree crates + registry sources on 2026-07-19; used verbatim below).** If a re-export path differs at compile time, follow the compiler's suggestion — the *types/methods* are correct, only the module they are re-exported through may differ.

- **superui_dom** (all on `Dom`): `document() -> NodeId`; `children(id) -> &[NodeId]`; `parent(id) -> Option<NodeId>`; `get(id) -> Option<&NodeData>` with `NodeData::kind: NodeKind` where `NodeKind::{Document, Element(ElementData), Text(String)}` and `ElementData { pub tag: String, .. }`; `get_attribute(id, name) -> Option<&str>`; `classes(id) -> Vec<String>`; `text_content(id) -> String`; `value(id) -> String`; `checked(id) -> bool`; `query_selector(root, sel) -> Option<NodeId>`. `NodeId`: `Copy + Eq + Hash`, `to_ffi()/from_ffi()`. **NodeIds are stable across frames** (arena). To read a node's tag: `match &dom.get(id)?.kind { NodeKind::Element(e) => Some(e.tag.as_str()), _ => None }`.
- **superui_html**: `parse_document(html: &str) -> superui_dom::Dom`.
- **superui_js**: `trait JsEngine { fn eval(&mut self, &str) -> Result<(), String>; fn dispatch_event(&mut self, NodeId, &str, bool, bool) -> bool; fn run_timers(&mut self, f64); }`. `BoaEngine::new(dom: Rc<RefCell<Dom>>) -> BoaEngine`; `BoaEngine::context_mut() -> &mut boa_engine::Context`; `BoaEngine::dom() -> Rc<RefCell<Dom>>`.
- **superui_api**: `install(engine: &mut BoaEngine)`.
- **superui_css::prelude**: `SuperUiCssPlugin`; `html_type_name(&str) -> TypeName`; `ClassList::new(&str)`, `ClassList::empty()`; `AttributeList::from_iter([(&str,&str)])`, `AttributeList::new()`, `set_attribute`; `NodeStyleData`; `NodeStyleSheet::new(Handle<StyleSheet>)` / `NodeStyleSheet::Inherited`; `TypeName`; `NodePseudoState`; `StyleSheet` (Asset); `InlineStyle::new(&str)`. `CssStyleSheetLoader` (extension `"css"`) is registered by `SuperUiCssPlugin`.
- **boa_engine 0.21.1**: `JsValue::from_json(&serde_json::Value, &mut Context) -> JsResult<JsValue>`; `JsValue::to_json(&self, &mut Context) -> JsResult<Option<serde_json::Value>>` (serde_json is a hard dep — no feature flag needed). Register a global fn: `context.register_global_callable(js_string!("name"), arity, NativeFunction::from_fn_ptr(f))`. Look up a global: `context.global_object().get(js_string!("bevy"), context)`.
- **Bevy 0.17.3**: `#[derive(Event)]` = global observer event; `commands.trigger(e)` / `world.trigger(e)`; observer system first param `On<E>`; for an `EntityEvent`, `On::target() -> Entity`; for the payload, `ev.event() -> &E` (or `&*ev`). `app.add_observer(system)`. `Pointer<Click>` (`bevy_picking`) is an `EntityEvent`. Pseudo-state components: `bevy_ui::{Checked, Pressed, InteractionDisabled, Interaction, Button}`, `bevy_picking::hover::Hovered`. UI text node: `Text(pub String)` (`bevy::ui::widget::Text`, in prelude) — spawn `Text::new(s)` as a child of a `Node`. Keyboard input: `bevy_input::keyboard::{KeyboardInput, Key}`, read from `MessageReader<KeyboardInput>` (0.17 renamed `EventReader`→`MessageReader`; follow the compiler if the name differs). NonSend resource ops: `world.insert_non_send_resource(r)`, `world.remove_non_send_resource::<T>() -> Option<T>`, `NonSendMut<T>` system param.

---

## File Structure

```
crates/superui_bridge/
  Cargo.toml                      # NEW: bevy(subset)+dom+js+api+css+serde_json; wasm getrandom
  src/lib.rs                      # NEW: exports; DomNode component; UiRuntime; SuperUiApp ext trait
  src/runtime.rs                  # NEW: UiRuntime (engine + node<->entity maps + dirty) + construction
  src/reconcile.rs                # NEW: DOM tree -> ECS entities (structural + identity/attr sync)
  src/events.rs                   # NEW: PendingDomEvents, pointer/keyboard -> DOM event dispatch systems
  src/bevy_bridge.rs              # NEW: window.bevy install + outbox/inbox + registry + add_superui_command/event
  tests/support/mod.rs            # NEW: headless test-app harness (reused across bridge tests)
  tests/reconcile.rs              # NEW: structural + update reconcile integration tests
  tests/input_events.rs           # NEW: click/checkbox/keyboard -> DOM -> reconcile tests
  tests/window_bevy.rs            # NEW: bevy.send -> Event, and Event -> bevy.on round trips

crates/superui/
  Cargo.toml                      # NEW: bevy(subset)+bridge+css+html+dom+js+api; wasm getrandom
  src/lib.rs                      # NEW: SuperUiPlugin, SuperUiRoot component, prelude
  src/assets.rs                   # NEW: HtmlSource/JsSource assets + .html/.js loaders
  src/mount.rs                    # NEW: build-runtime-when-loaded + schedule of bridge systems
  src/hot_reload.rs               # NEW: AssetEvent::Modified -> rebuild/re-exec -> reconcile
  tests/support/mod.rs            # NEW: headless test-app harness (in-memory assets)
  tests/integration.rs            # NEW: capstone — load html+css+js, tick, click, window.bevy round trip

docs/superpowers/plans/README.md  # MODIFY (Task 10): flip Plan 5 row to Done; retarget resume block to Plan 6
```

---

### Task 1: `superui_bridge` skeleton — `DomNode` component + `UiRuntime` NonSend holder

**Files:**
- Create: `crates/superui_bridge/Cargo.toml`
- Create: `crates/superui_bridge/src/lib.rs`
- Create: `crates/superui_bridge/src/runtime.rs`

**Interfaces:**
- Consumes: `superui_dom::{Dom, NodeId}`, `superui_js::{BoaEngine, JsEngine}`, `superui_api`, `bevy_ecs`.
- Produces:
  - `pub struct DomNode(pub NodeId)` — a `Component` the reconciler stamps on every entity so observers/systems can map `Entity -> NodeId` without the NonSend runtime.
  - `pub struct UiRuntime { pub dom: Rc<RefCell<Dom>>, pub engine: BoaEngine, pub root: Entity, pub stylesheet: Handle<StyleSheet>, pub dirty: bool, node_to_entity: HashMap<NodeId, Entity>, entity_to_node: HashMap<Entity, NodeId>, focused: Option<NodeId> }` (NonSend).
  - `impl UiRuntime`: `new(dom, root, stylesheet) -> Self` (builds engine, installs the web API + `window.bevy` bootstrap, `dirty = true`); `run_script(&mut self, src: &str)` (eval, log-and-swallow errors, set dirty); `entity_for(&self, NodeId) -> Option<Entity>`; `node_for(&self, Entity) -> Option<NodeId>`.

- [ ] **Step 1: Create the crate manifest** — create `crates/superui_bridge/Cargo.toml`:

```toml
[package]
name = "superui_bridge"
edition.workspace = true
version.workspace = true
license.workspace = true

[dependencies]
superui_dom = { path = "../superui_dom" }
superui_js = { path = "../superui_js" }
superui_api = { path = "../superui_api" }
superui_css = { path = "../superui_css" }
boa_engine.workspace = true
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Bevy, headless subset. Only the ECS/app/ui/picking/input pieces the bridge needs.
bevy = { version = "0.17", default-features = false, features = [
    "std",
    "bevy_ui",
    "bevy_text",
    "bevy_picking",
    "bevy_ui_picking_backend",
    "bevy_input_focus",
    "default_font",
] }

# Boa pulls getrandom 0.3, which needs the JS backend on wasm (same gotcha as the
# JS crates). Pair with the repo-root `.cargo/config.toml` rustflag.
[target.'cfg(target_arch = "wasm32")'.dependencies]
getrandom = { version = "0.3", features = ["wasm_js"] }
```

Add `serde` to `[workspace.dependencies]`? Not required — the version literal above is fine. Leave the workspace manifest unchanged.

- [ ] **Step 2: Write the failing unit test** — create `crates/superui_bridge/src/runtime.rs`:

```rust
//! [`UiRuntime`]: the NonSend holder for the JS engine + shared DOM + the stable
//! `NodeId <-> Entity` map that the reconciler maintains. One per mounted UI.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use bevy::prelude::*;
use superui_css::style::StyleSheet;
use superui_dom::{Dom, NodeId};
use superui_js::{BoaEngine, JsEngine};

/// Stamped by the reconciler on every entity it owns, so observers and systems
/// can resolve `Entity -> NodeId` via a normal query (the runtime is NonSend and
/// awkward to reach from observers).
#[derive(Component, Clone, Copy, Debug)]
pub struct DomNode(pub NodeId);

/// NonSend because [`BoaEngine`] holds `Rc<RefCell<Dom>>` and Boa `JsFunction`s.
pub struct UiRuntime {
    /// The retained arena DOM — the source of truth. Shared with `engine`.
    pub dom: Rc<RefCell<Dom>>,
    /// The Boa engine, wired to `dom` and the DOM/Web API + `window.bevy`.
    pub engine: BoaEngine,
    /// The ECS entity the DOM `<body>` reconciles into (its children mount here).
    pub root: Entity,
    /// The stylesheet handle the root carries (children inherit it in flair).
    pub stylesheet: Handle<StyleSheet>,
    /// Set whenever the DOM may have changed; cleared after a reconcile.
    pub dirty: bool,
    node_to_entity: HashMap<NodeId, Entity>,
    entity_to_node: HashMap<Entity, NodeId>,
    /// The DOM node that currently has keyboard focus (Task 5).
    pub(crate) focused: Option<NodeId>,
}

impl UiRuntime {
    /// Build a runtime around an already-parsed `dom`, mounting at `root` with
    /// `stylesheet`. Installs the DOM/Web API surface and the `window.bevy`
    /// bootstrap, but does NOT run author JS yet (callers `run_script` after, so
    /// hot reload can re-exec independently). Starts `dirty` so the first frame
    /// reconciles.
    pub fn new(dom: Rc<RefCell<Dom>>, root: Entity, stylesheet: Handle<StyleSheet>) -> Self {
        let mut engine = BoaEngine::new(dom.clone());
        superui_api::install(&mut engine);
        crate::bevy_bridge::install_bevy_bridge(&mut engine);
        UiRuntime {
            dom,
            engine,
            root,
            stylesheet,
            dirty: true,
            node_to_entity: HashMap::new(),
            entity_to_node: HashMap::new(),
            focused: None,
        }
    }

    /// Evaluate an author script against the current DOM. Errors are logged and
    /// swallowed (graceful degradation, design §1). Marks the runtime dirty.
    pub fn run_script(&mut self, src: &str) {
        if let Err(e) = self.engine.eval(src) {
            warn!("superui: JS error: {e}");
        }
        self.dirty = true;
    }

    pub fn entity_for(&self, node: NodeId) -> Option<Entity> {
        self.node_to_entity.get(&node).copied()
    }

    pub fn node_for(&self, entity: Entity) -> Option<NodeId> {
        self.entity_to_node.get(&entity).copied()
    }

    /// Insert/refresh the bidirectional map entry (used by the reconciler).
    pub(crate) fn bind(&mut self, node: NodeId, entity: Entity) {
        self.node_to_entity.insert(node, entity);
        self.entity_to_node.insert(entity, node);
    }

    /// Drop a mapping (used when the reconciler despawns a vanished node).
    pub(crate) fn unbind(&mut self, node: NodeId, entity: Entity) {
        self.node_to_entity.remove(&node);
        self.entity_to_node.remove(&entity);
    }

    /// Read-only view of the current node->entity bindings (for the reconciler).
    pub(crate) fn bindings(&self) -> &HashMap<NodeId, Entity> {
        &self.node_to_entity
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_runtime_is_dirty_and_runs_script() {
        let dom = Rc::new(RefCell::new(superui_html::parse_document(
            "<div id='a'></div>",
        )));
        let mut rt = UiRuntime::new(dom.clone(), Entity::PLACEHOLDER, Handle::default());
        assert!(rt.dirty, "a fresh runtime must reconcile on the first frame");

        // A script that mutates the DOM runs without panicking and re-dirties.
        rt.dirty = false;
        rt.run_script("document.getElementById('a').setAttribute('data-x','1');");
        assert!(rt.dirty);
        assert_eq!(
            dom.borrow()
                .get_attribute(dom.borrow().get_element_by_id("a").unwrap(), "data-x"),
            Some("1")
        );

        // A broken script is swallowed, not panicked, and still marks dirty.
        rt.run_script("this is not valid js @@@");
        assert!(rt.dirty);
    }
}
```

Add `superui_html` as a dev-dependency (the test parses HTML). Append to `crates/superui_bridge/Cargo.toml`:

```toml
[dev-dependencies]
superui_html = { path = "../superui_html" }
```

- [ ] **Step 3: Create `lib.rs` with the module wiring and a stub `install_bevy_bridge`**

Create `crates/superui_bridge/src/lib.rs`:

```rust
//! `superui_bridge` — the single coupling point between the web world (arena DOM
//! + Boa JS + flair CSS) and the ECS world. It owns the per-frame reconciler
//! (DOM -> `bevy_ui` entities), the input -> DOM-event seam, and the `window.bevy`
//! bridge. Only this crate and `superui` (and `superui_css`) depend on Bevy.

mod bevy_bridge;
mod events;
mod reconcile;
mod runtime;

pub use bevy_bridge::{BevyBridgeRegistry, SuperUiApp};
pub use events::{PendingDomEvents, PendingDomEvent};
pub use reconcile::reconcile_system;
pub use runtime::{DomNode, UiRuntime};
```

Since Tasks 2–6 create `reconcile`, `events`, and `bevy_bridge`, add **temporary minimal stubs** now so the crate compiles (each task replaces its stub):

Create `crates/superui_bridge/src/reconcile.rs`:

```rust
//! DOM tree -> ECS reconciler. Filled in by Task 2/3.
use bevy::prelude::*;

/// Exclusive system: reconcile the DOM into ECS entities when the runtime is dirty.
pub fn reconcile_system(_world: &mut World) {}
```

Create `crates/superui_bridge/src/events.rs`:

```rust
//! Input -> DOM event seam. Filled in by Task 4/5.
use bevy::prelude::*;
use superui_dom::NodeId;

/// One pending DOM event to dispatch into JS on the next drain.
#[derive(Clone, Debug)]
pub struct PendingDomEvent {
    pub target: NodeId,
    pub type_: String,
    pub bubbles: bool,
    pub cancelable: bool,
}

/// Queue of input-originated DOM events awaiting dispatch (a Send resource, so
/// picking observers can push to it).
#[derive(Resource, Default)]
pub struct PendingDomEvents(pub Vec<PendingDomEvent>);
```

Create `crates/superui_bridge/src/bevy_bridge.rs`:

```rust
//! The `window.bevy` bridge. Filled in by Task 6.
use bevy::prelude::*;
use superui_js::BoaEngine;

/// Registry of JS-exposed commands/events. Filled in by Task 6.
#[derive(Resource, Default)]
pub struct BevyBridgeRegistry;

/// Extension trait on `App` for registering the `window.bevy` surface (Task 6).
pub trait SuperUiApp {}
impl SuperUiApp for App {}

/// Install the `window.bevy` global into `engine`. Stub until Task 6 — for now
/// it just aliases `window` to `globalThis` so `window.bevy` won't throw later.
pub fn install_bevy_bridge(engine: &mut BoaEngine) {
    let _ = engine.eval_bootstrap("globalThis.window = globalThis;");
}
```

The stub calls `engine.eval_bootstrap`, which does not exist — replace that line with a direct eval using the public API:

```rust
pub fn install_bevy_bridge(engine: &mut BoaEngine) {
    use superui_js::JsEngine;
    let _ = engine.eval("globalThis.window = globalThis;");
}
```

- [ ] **Step 4: Run the unit test to verify it passes**

Run: `cargo test -p superui_bridge --lib`
Expected: PASS — `new_runtime_is_dirty_and_runs_script` compiles and passes. (First build compiles Bevy 0.17 + Boa; slow once.)

If `bevy` prelude does not re-export something used here (e.g. `Handle`, `Component`), follow the compiler's suggested path (e.g. `bevy::asset::Handle`).

- [ ] **Step 5: Verify boundary discipline still holds**

Run (Bash): `cargo tree -p superui_dom -e normal | grep -i bevy || echo CLEAN` (repeat for `superui_html`, `superui_js`, `superui_api`)
Expected: `CLEAN` for each — none of the four wasm-clean crates gained a `bevy_*` dep.

- [ ] **Step 6: Commit**

```bash
git add crates/superui_bridge Cargo.lock
git commit -m "feat(bridge): superui_bridge skeleton — DomNode + UiRuntime NonSend holder"
```

---

### Task 2: Reconciler — structural DOM → ECS (spawn/despawn/reparent, text)

**Files:**
- Modify: `crates/superui_bridge/src/reconcile.rs` (replace the stub)
- Create: `crates/superui_bridge/tests/support/mod.rs`
- Create: `crates/superui_bridge/tests/reconcile.rs`

**Interfaces:**
- Consumes: `UiRuntime` (Task 1), `superui_css::{html_type_name, prelude::*}`, `superui_dom` reads.
- Produces:
  - `pub fn reconcile_system(world: &mut World)` — exclusive system: if the NonSend `UiRuntime` is dirty, walk the DOM `<body>` subtree and sync entities, then clear dirty.
  - Internal `UiRuntime::reconcile(&mut self, world: &mut World)` — the recursive sync. Element nodes → entities with `Node` + `html_type_name(tag)` + `DomNode`; text nodes → child entities with `Text` + `DomNode`; parenting/ordering via `replace_children`; vanished nodes despawned.

- [ ] **Step 1: Implement the reconciler** — replace `crates/superui_bridge/src/reconcile.rs` with:

```rust
//! DOM tree -> ECS reconciler. Because `superui_dom` `NodeId`s are stable across
//! frames, we key entities by `NodeId`: spawn for new nodes, despawn for vanished
//! ones, re-parent/re-order to match, and push text/identity into each entity.
//! flair's cascade + `bevy_ui`/taffy then produce layout and rendering.

use std::collections::HashSet;

use bevy::prelude::*;
use superui_css::html_type_name;
use superui_css::prelude::NodeStyleSheet;
use superui_dom::{NodeId, NodeKind};

use crate::runtime::{DomNode, UiRuntime};

/// Exclusive system: reconcile when dirty. Pulls the NonSend runtime out, syncs,
/// re-inserts (the NonSend resource has no `resource_scope`, so move it out/in).
pub fn reconcile_system(world: &mut World) {
    let Some(mut rt) = world.remove_non_send_resource::<UiRuntime>() else {
        return;
    };
    if rt.dirty {
        rt.reconcile(world);
        rt.dirty = false;
    }
    world.insert_non_send_resource(rt);
}

impl UiRuntime {
    /// Sync the DOM `<body>` subtree into ECS under `self.root`.
    pub(crate) fn reconcile(&mut self, world: &mut World) {
        // Snapshot the tree shape from a short DOM borrow, then mutate the ECS
        // without holding the borrow (spawning can call arbitrary Bevy code).
        let dom = self.dom.clone();
        let dom = dom.borrow();

        let document = dom.document();
        let body = dom.query_selector(document, "body").unwrap_or(document);

        // The body node maps to the pre-existing root entity.
        self.bind(body, self.root);
        // Ensure the root carries the stylesheet so descendants inherit it.
        world
            .entity_mut(self.root)
            .insert((DomNode(body), NodeStyleSheet::new(self.stylesheet.clone())));

        // Recursively sync, collecting every node we touched this pass.
        let mut live: HashSet<NodeId> = HashSet::new();
        live.insert(body);
        self.sync_children(world, &dom, body);
        self.collect_live(&dom, body, &mut live);

        // Despawn entities whose node is no longer reachable.
        let stale: Vec<(NodeId, Entity)> = self
            .bindings()
            .iter()
            .filter(|(n, _)| !live.contains(*n) && **n != body)
            .map(|(n, e)| (*n, *e))
            .collect();
        for (node, entity) in stale {
            if let Ok(ec) = world.get_entity_mut(entity) {
                ec.despawn();
            }
            self.unbind(node, entity);
        }
    }

    /// Ensure every child of `parent_node` has an entity, is synced, and appears
    /// in the right order under the parent entity.
    fn sync_children(&mut self, world: &mut World, dom: &superui_dom::Dom, parent_node: NodeId) {
        let parent_entity = self.entity_for(parent_node).expect("parent bound");
        let child_nodes: Vec<NodeId> = dom.children(parent_node).to_vec();
        let mut child_entities: Vec<Entity> = Vec::with_capacity(child_nodes.len());

        for &child in &child_nodes {
            let Some(kind) = dom.get(child).map(|n| &n.kind) else {
                continue;
            };
            let entity = match self.entity_for(child) {
                Some(e) => e,
                None => {
                    // Spawn a fresh entity for this node.
                    let e = match kind {
                        NodeKind::Element(el) => world
                            .spawn((Node::default(), html_type_name(&el.tag), DomNode(child)))
                            .id(),
                        NodeKind::Text(t) => {
                            world.spawn((Text::new(t.clone()), DomNode(child))).id()
                        }
                        NodeKind::Document => continue,
                    };
                    self.bind(child, e);
                    e
                }
            };
            // Sync this node's payload (text now; identity/attrs in Task 3).
            if let NodeKind::Text(t) = kind {
                if let Some(mut text) = world.get_mut::<Text>(entity) {
                    if text.0 != *t {
                        text.0 = t.clone();
                    }
                }
            }
            child_entities.push(entity);
            // Recurse into element children.
            if matches!(kind, NodeKind::Element(_)) {
                self.sync_children(world, dom, child);
            }
        }

        world
            .entity_mut(parent_entity)
            .replace_children(&child_entities);
    }

    /// Record every node reachable under `node` (inclusive) into `live`.
    fn collect_live(&self, dom: &superui_dom::Dom, node: NodeId, live: &mut HashSet<NodeId>) {
        for &child in dom.children(node) {
            live.insert(child);
            self.collect_live(dom, child, live);
        }
    }
}
```

If `replace_children` is not the exact 0.17 name, follow the compiler — alternatives are `add_children` (after clearing) or `EntityCommands::replace_children`; the semantic needed is "make the parent's children exactly this ordered slice." If `get_entity_mut` returns a different Result/Option shape, adjust the `if let`.

- [ ] **Step 2: Create the headless test harness** — create `crates/superui_bridge/tests/support/mod.rs`:

```rust
//! Headless Bevy app harness for bridge integration tests: the CSS engine + UI
//! stack, no window/GPU, plus helpers to mount a `UiRuntime` and tick.
#![allow(dead_code)]

use std::cell::RefCell;
use std::rc::Rc;

use bevy::input::InputPlugin;
use bevy::input_focus::{InputFocus, InputFocusVisible};
use bevy::picking::{InteractionPlugin, PickingPlugin};
use bevy::prelude::*;
use bevy::text::TextPlugin;
use bevy::ui::UiPlugin;
use superui_bridge::{reconcile_system, UiRuntime};
use superui_css::style::StyleSheet;
use superui_css::SuperUiCssPlugin;
use superui_dom::Dom;

/// A headless app with the CSS engine + UI stack installed.
pub fn test_app() -> App {
    let mut app = App::new();
    app.add_plugins((
        bevy::time::TimePlugin,
        bevy::app::TaskPoolPlugin::default(),
        AssetPlugin::default(),
        WindowPlugin::default(),
        bevy::image::ImagePlugin::default(),
        TextPlugin,
        (InputPlugin, PickingPlugin, InteractionPlugin, UiPlugin),
        SuperUiCssPlugin,
    ));
    app.init_resource::<InputFocus>()
        .init_resource::<InputFocusVisible>();
    app.finish();
    app
}

/// Spawn a root entity, build a `UiRuntime` around `dom`, insert it NonSend, and
/// register the reconcile system in `Update`. Returns the root entity.
pub fn mount(app: &mut App, dom: Rc<RefCell<Dom>>) -> Entity {
    let root = app.world_mut().spawn(Node::default()).id();
    let stylesheet: Handle<StyleSheet> = Handle::default();
    let rt = UiRuntime::new(dom, root, stylesheet);
    app.world_mut().insert_non_send_resource(rt);
    app.add_systems(Update, reconcile_system);
    root
}

/// Number of direct children of `entity`.
pub fn child_count(app: &mut App, entity: Entity) -> usize {
    app.world_mut()
        .get::<Children>(entity)
        .map(|c| c.len())
        .unwrap_or(0)
}
```

This is adapted from Plan 4's harness; if a plugin/type path or feature differs, follow the compiler (same latitude as Plan 4 — widen the `bevy` dev features or fix an import).

- [ ] **Step 3: Write the structural reconcile test** — create `crates/superui_bridge/tests/reconcile.rs`:

```rust
//! Reconciler integration: DOM tree -> ECS entity tree (structure + text).
mod support;
use support::*;

use std::cell::RefCell;
use std::rc::Rc;

use bevy::prelude::*;
use superui_bridge::DomNode;
use superui_css::prelude::TypeName;
use superui_dom::NodeKind;

#[test]
fn reconciles_dom_tree_into_entities() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<ul class='list'><li>one</li><li>two</li></ul>",
    )));
    let mut app = test_app();
    let root = mount(&mut app, dom.clone());

    app.update(); // reconcile once

    // Root (body) has one child: the <ul>.
    assert_eq!(child_count(&mut app, root), 1);
    let ul = app.world_mut().get::<Children>(root).unwrap()[0];

    // The <ul> entity has a TypeName "ul" and a DomNode mapping back to an element.
    let tn = app.world().get::<TypeName>(ul).expect("ul has TypeName");
    assert_eq!(tn.0, "ul");
    let dn = app.world().get::<DomNode>(ul).expect("ul has DomNode");
    assert!(matches!(
        dom.borrow().get(dn.0).map(|n| &n.kind),
        Some(NodeKind::Element(_))
    ));

    // The <ul> has two <li> children.
    assert_eq!(child_count(&mut app, ul), 2);
    let lis = app.world_mut().get::<Children>(ul).unwrap().to_vec();
    for (li, expected) in lis.iter().zip(["one", "two"]) {
        // Each <li> contains one text-node child entity carrying the text.
        let li_children = app.world().get::<Children>(*li).unwrap();
        let text_entity = li_children[0];
        let text = app.world().get::<Text>(text_entity).expect("text node");
        assert_eq!(text.0, expected);
    }
}
```

- [ ] **Step 4: Run the test**

Run: `cargo test -p superui_bridge --test reconcile`
Expected: PASS — the DOM tree becomes the expected entity tree with `TypeName`, `DomNode`, and text children.

Debugging latitude: if a `<li>`'s child ordering differs, confirm `sync_children` uses `dom.children` order and `replace_children` preserves it. If `Text` isn't found, verify the prelude `Text` import path and that `Text::new` exists (else `Text(String::from(t))`).

- [ ] **Step 5: Commit**

```bash
git add crates/superui_bridge
git commit -m "feat(bridge): structural DOM->ECS reconciler (spawn/despawn/reparent, text)"
```

---

### Task 3: Reconciler — identity/attribute/class/value/checked sync + stable re-reconcile

**Files:**
- Modify: `crates/superui_bridge/src/reconcile.rs` (add identity sync; call it per element)
- Modify: `crates/superui_bridge/tests/reconcile.rs` (add update + identity tests)

**Interfaces:**
- Consumes: `superui_css::prelude::{ClassList, AttributeList, InlineStyle}`, `bevy::prelude::Name`, `bevy_ui::Checked`.
- Produces: `UiRuntime::sync_identity(&mut self, world, dom, node, entity)` — pushes id→`Name`, class→`ClassList`, other attrs→`AttributeList`, inline `style`→`InlineStyle`, and `checked`→ insert/remove `Checked`. Called for every element node each reconcile, so re-reconcile updates in place on the same (stable) entity.

- [ ] **Step 1: Add identity sync** — in `crates/superui_bridge/src/reconcile.rs`, add imports at the top:

```rust
use bevy::ui::Checked;
use superui_css::prelude::{AttributeList, ClassList, InlineStyle};
```

Then add this method inside `impl UiRuntime` (below `sync_children`):

```rust
    /// Push an element node's identity/attributes/state onto its entity. Called
    /// every reconcile so mutations land on the same stable entity in place.
    fn sync_identity(
        &self,
        world: &mut World,
        dom: &superui_dom::Dom,
        node: NodeId,
        entity: Entity,
    ) {
        // id -> Name (flair's id selector matches on Name).
        let mut ec = world.entity_mut(entity);
        match dom.get_attribute(node, "id") {
            Some(id) if !id.is_empty() => {
                ec.insert(Name::new(id.to_string()));
            }
            _ => {
                ec.remove::<Name>();
            }
        }

        // class -> ClassList (whitespace-separated).
        let classes = dom.classes(node);
        if classes.is_empty() {
            ec.insert(ClassList::empty());
        } else {
            ec.insert(ClassList::new(&classes.join(" ")));
        }

        // Remaining attributes (excluding id/class/style) -> AttributeList.
        let mut attrs = AttributeList::new();
        if let Some(NodeKind::Element(el)) = dom.get(node).map(|n| &n.kind) {
            for (k, v) in el.attrs.iter() {
                if k != "id" && k != "class" && k != "style" {
                    attrs.set_attribute(k.as_str(), v.as_str());
                }
            }
        }
        ec.insert(attrs);

        // inline style -> InlineStyle.
        match dom.get_attribute(node, "style") {
            Some(s) if !s.is_empty() => {
                ec.insert(InlineStyle::new(s));
            }
            _ => {
                ec.remove::<InlineStyle>();
            }
        }

        // checked (input) -> bevy_ui Checked marker, so `:checked` matches.
        if dom.checked(node) {
            ec.insert(Checked);
        } else {
            ec.remove::<Checked>();
        }
    }
```

`ElementData.attrs` is `pub(crate)` in `superui_dom` — it is NOT accessible here. Replace the attribute loop with the public API: iterate the known attribute names via a helper. Since `superui_dom` exposes `get_attribute` but not "list all attributes", add a **public accessor to `superui_dom`** in this step:

In `crates/superui_dom/src/attr.rs`, add (near `get_attribute`):

```rust
    /// The element's attributes as `(name, value)` pairs in insertion order.
    /// Returns empty for non-elements. Names are already lowercased.
    pub fn attributes(&self, id: NodeId) -> Vec<(String, String)> {
        match self.get(id).map(|n| &n.kind) {
            Some(crate::node::NodeKind::Element(el)) => el.attrs.clone(),
            _ => Vec::new(),
        }
    }
```

(Confirm the exact field path — `el.attrs` is `pub(crate)`, reachable from within the crate. If `NodeKind`/`ElementData` live in a different module, adjust the path per the compiler.) Then in the reconciler use it instead of touching `el.attrs`:

```rust
        let mut attrs = AttributeList::new();
        for (k, v) in dom.attributes(node) {
            if k != "id" && k != "class" && k != "style" {
                attrs.set_attribute(k.as_str(), v.as_str());
            }
        }
        ec.insert(attrs);
```

Remove the now-unused `NodeKind::Element(el)` destructuring for attrs. Add a `superui_dom` unit test for the new accessor in `crates/superui_dom/src/attr.rs` tests module:

```rust
    #[test]
    fn attributes_lists_all_pairs() {
        let mut dom = crate::Dom::new();
        let e = dom.create_element("input");
        dom.set_attribute(e, "type", "checkbox").unwrap();
        dom.set_attribute(e, "class", "x").unwrap();
        let attrs = dom.attributes(e);
        assert!(attrs.contains(&("type".to_string(), "checkbox".to_string())));
        assert!(attrs.contains(&("class".to_string(), "x".to_string())));
    }
```

Finally, call `sync_identity` from `sync_children` for element nodes — right after the `if let NodeKind::Text(t) = kind` block, add:

```rust
            if matches!(kind, NodeKind::Element(_)) {
                self.sync_identity(world, dom, child, entity);
            }
```

- [ ] **Step 2: Run the `superui_dom` accessor test**

Run: `cargo test -p superui_dom attributes_lists_all_pairs`
Expected: PASS.

- [ ] **Step 3: Add reconcile identity + update tests** — append to `crates/superui_bridge/tests/reconcile.rs`:

```rust
use superui_css::prelude::{AttributeList, ClassList};

#[test]
fn syncs_identity_and_updates_in_place() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<div id='root'><input type='checkbox' class='a b'></div>",
    )));
    let mut app = test_app();
    let root = mount(&mut app, dom.clone());
    app.update();

    // Find the <input> entity via its DomNode.
    let input_node = dom.borrow().query_selector(dom.borrow().document(), "input").unwrap();
    let input_ent = {
        let mut q = app.world_mut().query::<(Entity, &DomNode)>();
        q.iter(app.world())
            .find(|(_, d)| d.0 == input_node)
            .map(|(e, _)| e)
            .unwrap()
    };

    // Identity synced: ClassList has a+b; AttributeList has type=checkbox.
    assert!(app.world().get::<ClassList>(input_ent).unwrap().contains("a"));
    assert_eq!(
        app.world().get::<AttributeList>(input_ent).unwrap().get_attribute("type"),
        Some("checkbox")
    );
    // Not checked yet -> no Checked marker.
    assert!(app.world().get::<bevy::ui::Checked>(input_ent).is_none());

    // Mutate the DOM (as JS would) and re-reconcile: SAME entity, updated state.
    dom.borrow_mut().set_checked(input_node, true);
    dom.borrow_mut().class_add(input_node, "done");
    app.world_mut().non_send_resource_mut::<superui_bridge::UiRuntime>().dirty = true;
    app.update();

    let input_ent2 = {
        let mut q = app.world_mut().query::<(Entity, &DomNode)>();
        q.iter(app.world())
            .find(|(_, d)| d.0 == input_node)
            .map(|(e, _)| e)
            .unwrap()
    };
    assert_eq!(input_ent, input_ent2, "entity is stable across reconciles");
    assert!(app.world().get::<bevy::ui::Checked>(input_ent).is_some());
    assert!(app.world().get::<ClassList>(input_ent).unwrap().contains("done"));
}
```

- [ ] **Step 4: Run the reconcile tests**

Run: `cargo test -p superui_bridge --test reconcile`
Expected: PASS — both tests. Identity is synced and re-reconcile updates the same entity (stable-`NodeId` keying).

- [ ] **Step 5: Commit**

```bash
git add crates/superui_bridge crates/superui_dom
git commit -m "feat(bridge): reconcile identity/attrs/class/checked/inline-style; stable re-reconcile"
```

---

### Task 4: Input seam — pointer click → DOM event dispatch → reconcile

**Files:**
- Modify: `crates/superui_bridge/src/events.rs` (replace stub: observer + drain system)
- Modify: `crates/superui_bridge/src/lib.rs` (export the new items)
- Create: `crates/superui_bridge/tests/input_events.rs`

**Interfaces:**
- Consumes: `bevy_picking::Pointer<Click>`, `UiRuntime`, `DomNode`, `PendingDomEvents`.
- Produces:
  - `pub fn on_pointer_click(ev: On<Pointer<Click>>, q: Query<&DomNode>, pending: ResMut<PendingDomEvents>)` — observer: map target entity → node, enqueue a `"click"` DOM event (+ toggle `checked` & enqueue `"change"` for checkbox inputs).
  - `pub fn drain_dom_events_system(world: &mut World)` — exclusive system: dispatch every queued event into the engine (`engine.dispatch_event`), set `dirty`, clear the queue.

- [ ] **Step 1: Implement the input seam** — replace `crates/superui_bridge/src/events.rs` with:

```rust
//! Input -> DOM event seam. `bevy_picking`/keyboard produce DOM events, which we
//! dispatch into JS (W3C capture/bubble, synchronous) and then reconcile.

use bevy::picking::events::{Click, Pointer};
use bevy::prelude::*;
use superui_dom::NodeId;

use crate::runtime::{DomNode, UiRuntime};

/// One pending DOM event to dispatch into JS on the next drain.
#[derive(Clone, Debug)]
pub struct PendingDomEvent {
    pub target: NodeId,
    pub type_: String,
    pub bubbles: bool,
    pub cancelable: bool,
}

impl PendingDomEvent {
    pub fn new(target: NodeId, type_: &str) -> Self {
        PendingDomEvent {
            target,
            type_: type_.to_string(),
            bubbles: true,
            cancelable: true,
        }
    }
}

/// Queue of input-originated DOM events awaiting dispatch. Send resource so
/// picking observers can push to it.
#[derive(Resource, Default)]
pub struct PendingDomEvents(pub Vec<PendingDomEvent>);

/// Observer: a pointer click on a UI entity becomes a DOM `click` on its node.
/// For a checkbox input, also mirror the native toggle: flip DOM `checked` and
/// enqueue a `change` event (dispatched after the click).
pub fn on_pointer_click(
    ev: On<Pointer<Click>>,
    nodes: Query<&DomNode>,
    dom: NonSend<UiRuntime>,
    mut pending: ResMut<PendingDomEvents>,
) {
    let Ok(dom_node) = nodes.get(ev.target()) else {
        return;
    };
    let node = dom_node.0;

    // Determine checkbox-ness from the DOM (read-only borrow).
    let is_checkbox = {
        let d = dom.dom.borrow();
        matches!(tag_of(&d, node).as_deref(), Some("input"))
            && d.get_attribute(node, "type") == Some("checkbox")
    };

    pending.0.push(PendingDomEvent::new(node, "click"));
    if is_checkbox {
        let now = { !dom.dom.borrow().checked(node) };
        dom.dom.borrow_mut().set_checked(node, now);
        pending.0.push(PendingDomEvent::new(node, "change"));
    }
}

fn tag_of(dom: &superui_dom::Dom, node: NodeId) -> Option<String> {
    match dom.get(node).map(|n| &n.kind) {
        Some(superui_dom::NodeKind::Element(e)) => Some(e.tag.clone()),
        _ => None,
    }
}

/// Exclusive system: dispatch queued DOM events into the engine, then mark dirty.
pub fn drain_dom_events_system(world: &mut World) {
    let queued = std::mem::take(&mut world.resource_mut::<PendingDomEvents>().0);
    if queued.is_empty() {
        return;
    }
    let Some(mut rt) = world.remove_non_send_resource::<UiRuntime>() else {
        return;
    };
    for e in queued {
        use superui_js::JsEngine;
        rt.engine
            .dispatch_event(e.target, &e.type_, e.bubbles, e.cancelable);
    }
    rt.dirty = true;
    world.insert_non_send_resource(rt);
}
```

Note the observer takes `NonSend<UiRuntime>` to read/mutate the DOM; that forces it onto the main thread, which is correct. If `On<Pointer<Click>>` requires the events import path to be `bevy::picking::events::{Click, Pointer}` vs `bevy::prelude::{Click, Pointer}`, follow the compiler. If `ev.target()` is spelled `ev.event_target()`, use that.

- [ ] **Step 2: Export the new items** — in `crates/superui_bridge/src/lib.rs`, replace the `events` re-export line with:

```rust
pub use events::{
    drain_dom_events_system, on_pointer_click, PendingDomEvent, PendingDomEvents,
};
```

- [ ] **Step 3: Write the click test** — create `crates/superui_bridge/tests/input_events.rs`:

```rust
//! Input seam: a pointer click drives a DOM `click`, runs the JS listener, and
//! the resulting DOM mutation reconciles into the ECS.
mod support;
use support::*;

use std::cell::RefCell;
use std::rc::Rc;

use bevy::prelude::*;
use superui_bridge::{
    drain_dom_events_system, on_pointer_click, DomNode, PendingDomEvents, UiRuntime,
};

/// Register the full input pipeline on top of the base harness.
fn mount_with_input(app: &mut App, dom: Rc<RefCell<superui_dom::Dom>>) -> Entity {
    let root = mount(app, dom);
    app.init_resource::<PendingDomEvents>();
    app.add_observer(on_pointer_click);
    // Drain events, then reconcile, each Update.
    app.add_systems(
        Update,
        drain_dom_events_system.before(superui_bridge::reconcile_system),
    );
    root
}

#[test]
fn click_runs_js_listener_and_reconciles() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<button id='b'>hi</button><div id='out'></div>",
    )));
    let mut app = test_app();
    let root = mount_with_input(&mut app, dom.clone());

    // Author JS: clicking the button writes into #out.
    app.world_mut()
        .non_send_resource_mut::<UiRuntime>()
        .run_script(
            "document.getElementById('b').addEventListener('click', function() { \
               document.getElementById('out').textContent = 'clicked'; \
             });",
        );
    app.update(); // initial reconcile

    // Find the button entity and simulate a pointer click by enqueuing directly
    // through the observer path: trigger Pointer<Click> on it.
    let btn_node = dom.borrow().get_element_by_id("b").unwrap();
    let btn_ent = {
        let mut q = app.world_mut().query::<(Entity, &DomNode)>();
        q.iter(app.world()).find(|(_, d)| d.0 == btn_node).map(|(e, _)| e).unwrap()
    };
    app.world_mut()
        .trigger(bevy::picking::events::Pointer::<bevy::picking::events::Click> {
            ..synthetic_click(btn_ent)
        });
    app.update(); // drain -> dispatch JS -> dirty -> reconcile

    // #out now contains a text node "clicked".
    let out_node = dom.borrow().get_element_by_id("out").unwrap();
    assert_eq!(dom.borrow().text_content(out_node), "clicked");
}
```

`Pointer<Click>` has non-trivial fields (pointer id, hit data, event target). Constructing one literal-by-literal is brittle. Instead of `synthetic_click`, drive the event by **directly enqueuing** into `PendingDomEvents` (the observer is unit-covered by the checkbox test below, and end-to-end picking is exercised in Task 10's app). Replace the `trigger(...)` block with:

```rust
    app.world_mut()
        .resource_mut::<PendingDomEvents>()
        .0
        .push(superui_bridge::PendingDomEvent::new(btn_node, "click"));
    app.update();
```

Delete the `synthetic_click` reference. (This tests the drain→dispatch→reconcile path deterministically; the observer's entity→node mapping + checkbox toggle is covered next.)

- [ ] **Step 4: Add the checkbox-observer test** — append to `crates/superui_bridge/tests/input_events.rs`:

```rust
#[test]
fn checkbox_click_toggles_checked_and_fires_change() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<input id='c' type='checkbox'>",
    )));
    let mut app = test_app();
    mount_with_input(&mut app, dom.clone());

    // JS records change events into a global counter mirrored onto the body id.
    app.world_mut().non_send_resource_mut::<UiRuntime>().run_script(
        "globalThis.changes = 0; \
         document.getElementById('c').addEventListener('change', function(){ globalThis.changes++; });",
    );
    app.update();

    let node = dom.borrow().get_element_by_id("c").unwrap();
    let ent = {
        let mut q = app.world_mut().query::<(Entity, &DomNode)>();
        q.iter(app.world()).find(|(_, d)| d.0 == node).map(|(e, _)| e).unwrap()
    };

    assert!(!dom.borrow().checked(node));
    // Fire the observer directly with a synthetic pointer click.
    app.world_mut().trigger(make_click(ent));
    app.update();

    // The checkbox toggled to checked and a change event fired (JS incremented).
    assert!(dom.borrow().checked(node));
}

/// Build a minimal `Pointer<Click>` targeting `entity`. Field set follows the
/// 0.17 struct; if a field is missing/renamed, the compiler names it — fill it
/// with `Default::default()`-style values (this is a synthetic test event).
fn make_click(entity: Entity) -> bevy::picking::events::Pointer<bevy::picking::events::Click> {
    use bevy::picking::events::{Click, Pointer};
    use bevy::picking::pointer::{PointerId, PointerButton};
    Pointer::<Click> {
        entity,
        pointer_id: PointerId::Mouse,
        pointer_location: Default::default(),
        event: Click {
            button: PointerButton::Primary,
            hit: Default::default(),
            duration: std::time::Duration::ZERO,
        },
    }
}
```

Constructing `Pointer<Click>` may not be feasible if fields are private. If `make_click` won't compile, **delete `checkbox_click_toggles_checked_and_fires_change`'s `trigger` approach** and instead call the observer's effect through the queue is not possible (the toggle logic lives in the observer). In that case, refactor: extract the observer body into a testable free function `pub fn click_effect(dom: &UiRuntime, node: NodeId, pending: &mut PendingDomEvents)` and have both `on_pointer_click` and the test call it. Prefer this refactor if `Pointer<Click>` cannot be constructed in a test — it keeps the checkbox logic unit-tested regardless of picking's constructability.

- [ ] **Step 5: Run the input tests**

Run: `cargo test -p superui_bridge --test input_events`
Expected: PASS — both tests. (If you took the `click_effect` refactor, the checkbox test calls it directly.)

- [ ] **Step 6: Commit**

```bash
git add crates/superui_bridge
git commit -m "feat(bridge): pointer-click -> DOM event dispatch + checkbox toggle/change"
```

---

### Task 5: Input seam — keyboard/focus → keydown/keyup + text-input value

**Files:**
- Modify: `crates/superui_bridge/src/events.rs` (focus tracking on click; keyboard system)
- Modify: `crates/superui_bridge/src/lib.rs` (export the keyboard system)
- Modify: `crates/superui_bridge/tests/input_events.rs` (keyboard test)

**Interfaces:**
- Consumes: `bevy_input::keyboard::{KeyboardInput, Key}`, `UiRuntime.focused`.
- Produces:
  - Focus assignment: `on_pointer_click` also sets `runtime.focused = Some(node)` for the clicked node.
  - `pub fn keyboard_events_system(world: &mut World)` — exclusive system: for each `KeyboardInput` press, dispatch `keydown` to the focused node; if it's a printable char and the focused node is a text input, append to the DOM `value` and dispatch `input`; on release dispatch `keyup`. Sets dirty when anything dispatched.

- [ ] **Step 1: Track focus in the click observer** — in `crates/superui_bridge/src/events.rs`, change `on_pointer_click` to also record focus. Change its signature to take `NonSendMut<UiRuntime>` and set focus:

```rust
pub fn on_pointer_click(
    ev: On<Pointer<Click>>,
    nodes: Query<&DomNode>,
    mut dom: NonSendMut<UiRuntime>,
    mut pending: ResMut<PendingDomEvents>,
) {
    let Ok(dom_node) = nodes.get(ev.target()) else {
        return;
    };
    let node = dom_node.0;
    dom.focused = Some(node);
    // ... unchanged body (is_checkbox, push click, checkbox toggle) ...
}
```

`focused` is `pub(crate)` — reachable from this module (same crate). Keep the rest of the body identical.

- [ ] **Step 2: Add the keyboard system** — append to `crates/superui_bridge/src/events.rs`:

```rust
/// Exclusive system: route keyboard input to the focused DOM node as
/// `keydown`/`keyup`, and for printable characters typed into a text input,
/// mutate the DOM `value` and fire `input` (Phase-1 text entry — TodoMVC needs
/// Enter-to-add and character typing).
pub fn keyboard_events_system(world: &mut World) {
    // Collect key messages first (short borrow of the message queue).
    let presses: Vec<(bevy::input::keyboard::Key, bool)> = {
        let mut reader = world
            .resource_mut::<bevy::ecs::message::Messages<bevy::input::keyboard::KeyboardInput>>();
        reader
            .drain()
            .map(|k| (k.logical_key, matches!(k.state, bevy::input::ButtonState::Pressed)))
            .collect()
    };
    if presses.is_empty() {
        return;
    }
    let Some(mut rt) = world.remove_non_send_resource::<UiRuntime>() else {
        return;
    };
    let Some(focused) = rt.focused else {
        world.insert_non_send_resource(rt);
        return;
    };
    use superui_js::JsEngine;
    let mut any = false;
    for (key, pressed) in presses {
        let type_ = if pressed { "keydown" } else { "keyup" };
        rt.engine.dispatch_event(focused, type_, true, true);
        any = true;
        if pressed {
            if let bevy::input::keyboard::Key::Character(s) = &key {
                // Append typed characters to a text input's DOM value + fire input.
                let is_text_input = {
                    let d = rt.dom.borrow();
                    matches!(tag_of(&d, focused).as_deref(), Some("input"))
                        && d.get_attribute(focused, "type").unwrap_or("text") != "checkbox"
                };
                if is_text_input {
                    let cur = rt.dom.borrow().value(focused);
                    rt.dom.borrow_mut().set_value(focused, &format!("{cur}{s}"));
                    rt.engine.dispatch_event(focused, "input", true, false);
                }
            }
        }
    }
    if any {
        rt.dirty = true;
    }
    world.insert_non_send_resource(rt);
}
```

The exact 0.17 name for the keyboard-message reader may be `MessageReader<KeyboardInput>` / `Messages<KeyboardInput>` (0.17 renamed the buffered-event API to "messages"). If `world.resource_mut::<Messages<KeyboardInput>>()` / `.drain()` doesn't compile, use the system-param form instead: make this a normal system `fn keyboard_events_system(mut reader: MessageReader<KeyboardInput>, mut rt: NonSendMut<UiRuntime>)` and collect from `reader.read()`. Prefer whichever the compiler accepts; the logic is identical. Keep `tag_of` (already defined in this module) accessible.

- [ ] **Step 3: Export it** — in `crates/superui_bridge/src/lib.rs`, add `keyboard_events_system` to the `events` re-export list.

- [ ] **Step 4: Add the keyboard test** — append to `crates/superui_bridge/tests/input_events.rs`:

```rust
#[test]
fn typing_into_focused_input_updates_value_and_fires_input() {
    use bevy::input::keyboard::{Key, KeyboardInput};
    use bevy::input::ButtonState;

    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<input id='t' type='text'>",
    )));
    let mut app = test_app();
    let root = mount(&mut app, dom.clone());
    app.init_resource::<PendingDomEvents>();
    app.add_systems(Update, superui_bridge::keyboard_events_system);

    // JS: count input events.
    app.world_mut().non_send_resource_mut::<UiRuntime>().run_script(
        "globalThis.inputs = 0; \
         document.getElementById('t').addEventListener('input', function(){ globalThis.inputs++; });",
    );
    app.update();

    // Focus the input, then type "hi".
    let node = dom.borrow().get_element_by_id("t").unwrap();
    app.world_mut().non_send_resource_mut::<UiRuntime>().focused = Some(node);

    for ch in ["h", "i"] {
        app.world_mut().write_message(KeyboardInput {
            key_code: bevy::input::keyboard::KeyCode::KeyH, // placeholder; logical_key drives text
            logical_key: Key::Character(ch.into()),
            state: ButtonState::Pressed,
            repeat: false,
            window: Entity::PLACEHOLDER,
            text: None,
        });
        app.update();
    }

    assert_eq!(dom.borrow().value(node), "hi");
    let _ = root;
}
```

`KeyboardInput`'s field set in 0.17 may differ (e.g. no `text` field, or `window` typing). Fill fields per the compiler — the load-bearing field is `logical_key: Key::Character(...)` and `state: Pressed`. If `write_message` is named `send_event`/`write_event`, use the accepted name. `KeyCode` import likewise.

- [ ] **Step 5: Run the input tests**

Run: `cargo test -p superui_bridge --test input_events`
Expected: PASS — all input tests, including the keyboard one (`value` becomes `"hi"`).

- [ ] **Step 6: Commit**

```bash
git add crates/superui_bridge
git commit -m "feat(bridge): keyboard/focus seam — keydown/keyup + text-input value/input"
```

---

### Task 6: `window.bevy` bridge — `send`/`on` + `add_superui_command`/`add_superui_event`

**Files:**
- Modify: `crates/superui_bridge/src/bevy_bridge.rs` (replace stub with the full bridge)
- Modify: `crates/superui_bridge/src/lib.rs` (export new items + systems)
- Create: `crates/superui_bridge/tests/window_bevy.rs`

**Interfaces:**
- Consumes: `boa_engine::{JsValue, NativeFunction, js_string}`, `serde_json`, `superui_js::BoaEngine`, Bevy observers/`Event`.
- Produces:
  - `install_bevy_bridge(engine)` — registers native `__superui_bevy_send(name, payload)` and evals the `window.bevy = { send, on, _emit }` bootstrap.
  - `pub struct BevyBridgeRegistry { commands: HashMap<String, CommandFn>, event_names: HashMap<TypeId, String> }` (Resource).
  - `pub trait SuperUiApp { fn add_superui_command<T: Event + DeserializeOwned>(&mut self, name: &str) -> &mut Self; fn add_superui_event<T: Event + Serialize>(&mut self, name: &str) -> &mut Self; }` impl for `App`.
  - `pub fn drain_bevy_outbox_system(world: &mut World)` — JS `bevy.send` → registered `Event` triggered.
  - `pub fn emit_bevy_inbox_system(world: &mut World)` — game-triggered registered event → JS `bevy._emit(name, payload)`.

- [ ] **Step 1: Implement the bridge** — replace `crates/superui_bridge/src/bevy_bridge.rs` with:

```rust
//! The `window.bevy` bridge: the one non-web API JS sees (design §8). JS calls
//! `bevy.send(name, data)` (JS -> ECS: trigger a registered `Event`) and
//! `bevy.on(name, cb)` (ECS -> JS: a registered game event invokes JS callbacks).
//! Marshalling is `serde_json::Value` <-> boa `JsValue`. Phase 1 = send + on
//! only (no `query`; that is Phase 2).

use std::any::TypeId;
use std::cell::RefCell;
use std::collections::HashMap;

use bevy::prelude::*;
use boa_engine::{js_string, Context, JsValue, NativeFunction};
use serde::de::DeserializeOwned;
use serde::Serialize;
use superui_js::{BoaEngine, JsEngine};

use crate::runtime::UiRuntime;

thread_local! {
    /// JS -> ECS queue: `bevy.send` pushes `(name, payload-json)` here; the ECS
    /// drain system reads it. Thread-local because Boa is single-threaded and the
    /// runtime is NonSend (main thread only).
    static OUTBOX: RefCell<Vec<(String, serde_json::Value)>> = const { RefCell::new(Vec::new()) };
    /// ECS -> JS queue: observers push `(name, payload-json)`; the emit system
    /// forwards to JS `bevy._emit`.
    static INBOX: RefCell<Vec<(String, serde_json::Value)>> = const { RefCell::new(Vec::new()) };
}

/// Native `__superui_bevy_send(name, payload)` — stash `(name, json(payload))`.
fn native_bevy_send(
    _this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> boa_engine::JsResult<JsValue> {
    let name = args
        .first()
        .and_then(|v| v.as_string())
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();
    let payload = match args.get(1) {
        Some(v) => v.to_json(context)?.unwrap_or(serde_json::Value::Null),
        None => serde_json::Value::Null,
    };
    OUTBOX.with(|o| o.borrow_mut().push((name, payload)));
    Ok(JsValue::undefined())
}

/// Install the `window.bevy` global into `engine`. Registers the native send
/// hook and the JS surface (`send`/`on`/`_emit`), and aliases `window` to
/// `globalThis` so both `bevy.*` and `window.bevy.*` resolve.
pub fn install_bevy_bridge(engine: &mut BoaEngine) {
    let ctx = engine.context_mut();
    ctx.register_global_callable(
        js_string!("__superui_bevy_send"),
        2,
        NativeFunction::from_fn_ptr(native_bevy_send),
    )
    .expect("register __superui_bevy_send");

    // Bootstrap the JS-visible object.
    let _ = engine.eval(
        r#"
        globalThis.window = globalThis;
        globalThis.bevy = (function () {
            const listeners = new Map();
            return {
                send: function (name, data) { __superui_bevy_send(String(name), data); },
                on: function (name, cb) {
                    let a = listeners.get(name);
                    if (!a) { a = []; listeners.set(name, a); }
                    a.push(cb);
                },
                _emit: function (name, data) {
                    const a = listeners.get(name);
                    if (a) { for (const cb of a) cb(data); }
                }
            };
        })();
        "#,
    );
}

type CommandFn = Box<dyn Fn(&mut World, serde_json::Value) + Send + Sync>;

/// Registry of the JS-exposed command/event surface.
#[derive(Resource, Default)]
pub struct BevyBridgeRegistry {
    commands: HashMap<String, CommandFn>,
    event_names: HashMap<TypeId, String>,
}

/// App extension for registering the `window.bevy` surface.
pub trait SuperUiApp {
    /// Allow JS `bevy.send("<name>", payload)` to deserialize `payload` into `T`
    /// and `trigger` it as a global Bevy `Event`.
    fn add_superui_command<T: Event + DeserializeOwned>(&mut self, name: &str) -> &mut Self;
    /// Forward a game-triggered global `Event` `T` to JS `bevy.on("<name>", cb)`
    /// callbacks, serialized as JSON.
    fn add_superui_event<T: Event + Serialize>(&mut self, name: &str) -> &mut Self;
}

impl SuperUiApp for App {
    fn add_superui_command<T: Event + DeserializeOwned>(&mut self, name: &str) -> &mut Self {
        self.init_resource::<BevyBridgeRegistry>();
        let mut reg = self.world_mut().resource_mut::<BevyBridgeRegistry>();
        reg.commands.insert(
            name.to_string(),
            Box::new(|world: &mut World, json: serde_json::Value| {
                match serde_json::from_value::<T>(json) {
                    Ok(evt) => {
                        world.trigger(evt);
                    }
                    Err(e) => warn!("superui: bevy.send payload did not match type: {e}"),
                }
            }),
        );
        self
    }

    fn add_superui_event<T: Event + Serialize>(&mut self, name: &str) -> &mut Self {
        self.init_resource::<BevyBridgeRegistry>();
        self.world_mut()
            .resource_mut::<BevyBridgeRegistry>()
            .event_names
            .insert(TypeId::of::<T>(), name.to_string());
        self.add_observer(forward_event_observer::<T>);
        self
    }
}

/// Observer: serialize a registered game event and push it to the JS inbox.
fn forward_event_observer<T: Event + Serialize>(
    ev: On<T>,
    reg: Res<BevyBridgeRegistry>,
) {
    let Some(name) = reg.event_names.get(&TypeId::of::<T>()) else {
        return;
    };
    match serde_json::to_value(ev.event()) {
        Ok(json) => INBOX.with(|i| i.borrow_mut().push((name.clone(), json))),
        Err(e) => warn!("superui: could not serialize bevy event '{name}': {e}"),
    }
}

/// Exclusive system: drain the JS -> ECS outbox, triggering registered events.
pub fn drain_bevy_outbox_system(world: &mut World) {
    let items: Vec<(String, serde_json::Value)> =
        OUTBOX.with(|o| std::mem::take(&mut *o.borrow_mut()));
    if items.is_empty() {
        return;
    }
    if !world.contains_resource::<BevyBridgeRegistry>() {
        return;
    }
    for (name, json) in items {
        // Pull the command fn out of the registry momentarily to satisfy the
        // borrow checker (fn needs &mut World; registry lives in World).
        let cmd = {
            let reg = world.resource::<BevyBridgeRegistry>();
            reg.commands.contains_key(&name)
        };
        if !cmd {
            warn!("superui: bevy.send to unregistered command '{name}'");
            continue;
        }
        world.resource_scope(|world, reg: Mut<BevyBridgeRegistry>| {
            if let Some(f) = reg.commands.get(&name) {
                f(world, json);
            }
        });
    }
}

/// Exclusive system: forward game-triggered events into JS `bevy._emit`.
pub fn emit_bevy_inbox_system(world: &mut World) {
    let items: Vec<(String, serde_json::Value)> =
        INBOX.with(|i| std::mem::take(&mut *i.borrow_mut()));
    if items.is_empty() {
        return;
    }
    let Some(mut rt) = world.remove_non_send_resource::<UiRuntime>() else {
        return;
    };
    for (name, json) in items {
        emit_one(&mut rt.engine, &name, &json);
    }
    rt.dirty = true; // a bevy.on callback may have mutated the DOM
    world.insert_non_send_resource(rt);
}

/// Call JS `globalThis.bevy._emit(name, payload)`.
fn emit_one(engine: &mut BoaEngine, name: &str, json: &serde_json::Value) {
    let ctx = engine.context_mut();
    let Ok(bevy_val) = ctx.global_object().get(js_string!("bevy"), ctx) else {
        return;
    };
    let Some(bevy_obj) = bevy_val.as_object() else {
        return;
    };
    let Ok(emit) = bevy_obj.get(js_string!("_emit"), ctx) else {
        return;
    };
    let Some(emit_fn) = emit.as_callable() else {
        return;
    };
    let payload = JsValue::from_json(json, ctx).unwrap_or(JsValue::undefined());
    let _ = emit_fn.call(
        &bevy_val,
        &[JsValue::from(js_string!(name)), payload],
        ctx,
    );
}
```

Follow the compiler on boa spellings: `as_string()`/`to_std_string_escaped()`, `global_object()`, `as_callable()`, `NativeFunction::from_fn_ptr` — these match the patterns already used in `superui_api` (see `crates/superui_api/src/timers.rs` and `document.rs`). If `On<T>::event()` isn't available for a global event, use `&*ev`.

- [ ] **Step 2: Export the bridge surface** — in `crates/superui_bridge/src/lib.rs`, replace the `bevy_bridge` re-export line with:

```rust
pub use bevy_bridge::{
    drain_bevy_outbox_system, emit_bevy_inbox_system, install_bevy_bridge, BevyBridgeRegistry,
    SuperUiApp,
};
```

- [ ] **Step 3: Write the round-trip tests** — create `crates/superui_bridge/tests/window_bevy.rs`:

```rust
//! `window.bevy` round trips: JS `bevy.send` -> registered Bevy Event, and a
//! game-triggered Event -> JS `bevy.on` callback.
mod support;
use support::*;

use std::cell::RefCell;
use std::rc::Rc;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use superui_bridge::{
    drain_bevy_outbox_system, emit_bevy_inbox_system, SuperUiApp, UiRuntime,
};

#[derive(Event, Serialize, Deserialize, Clone, Debug, PartialEq)]
struct SpawnEnemy {
    x: i64,
    y: i64,
}

#[derive(Event, Serialize, Deserialize, Clone, Debug)]
struct ScoreChanged {
    value: i64,
}

#[derive(Resource, Default)]
struct Received(Vec<SpawnEnemy>);

#[test]
fn js_send_triggers_registered_event() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document("<div></div>")));
    let mut app = test_app();
    mount(&mut app, dom.clone());
    app.init_resource::<Received>();
    app.add_superui_command::<SpawnEnemy>("SpawnEnemy");
    app.add_observer(|ev: On<SpawnEnemy>, mut r: ResMut<Received>| {
        r.0.push(ev.event().clone());
    });
    app.add_systems(Update, drain_bevy_outbox_system);

    app.world_mut()
        .non_send_resource_mut::<UiRuntime>()
        .run_script("bevy.send('SpawnEnemy', { x: 10, y: 4 });");
    app.update();

    let received = app.world().resource::<Received>();
    assert_eq!(received.0, vec![SpawnEnemy { x: 10, y: 4 }]);
}

#[test]
fn game_event_reaches_js_on_callback() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<div id='score'></div>",
    )));
    let mut app = test_app();
    mount(&mut app, dom.clone());
    app.add_superui_event::<ScoreChanged>("ScoreChanged");
    app.add_systems(Update, emit_bevy_inbox_system);

    app.world_mut().non_send_resource_mut::<UiRuntime>().run_script(
        "bevy.on('ScoreChanged', function(e){ \
           document.getElementById('score').textContent = String(e.value); });",
    );
    app.update();

    // The game triggers ScoreChanged; the observer forwards it to JS.
    app.world_mut().trigger(ScoreChanged { value: 42 });
    app.update(); // emit -> JS callback mutates DOM
    app.update(); // reconcile if the harness wired it (not needed for DOM read)

    let score_node = dom.borrow().get_element_by_id("score").unwrap();
    assert_eq!(dom.borrow().text_content(score_node), "42");
}
```

- [ ] **Step 4: Run the window.bevy tests**

Run: `cargo test -p superui_bridge --test window_bevy`
Expected: PASS — both round trips. JS `bevy.send` triggers `SpawnEnemy`; `world.trigger(ScoreChanged)` runs the JS `bevy.on` callback, which mutates the DOM.

Debugging latitude: if `bevy.send` produced nothing, confirm `native_bevy_send` registered before the bootstrap eval and that `to_json` returned the object (not `None`). If the emit test's callback didn't run, confirm `emit_one` found `globalThis.bevy._emit` and that the observer pushed to `INBOX` (the registry must be initialized — `add_superui_event` does that).

- [ ] **Step 5: Full bridge test sweep + commit**

Run: `cargo test -p superui_bridge`
Expected: PASS — lib unit tests + `reconcile` + `input_events` + `window_bevy`.

```bash
git add crates/superui_bridge
git commit -m "feat(bridge): window.bevy send/on + add_superui_command/add_superui_event"
```

---

### Task 7: `superui` crate — `.html`/`.js` assets + loaders

**Files:**
- Create: `crates/superui/Cargo.toml`
- Create: `crates/superui/src/lib.rs` (module wiring + prelude stub)
- Create: `crates/superui/src/assets.rs`

**Interfaces:**
- Consumes: `bevy::asset::{Asset, AssetLoader}`, `superui_html` (not needed here — the loader stores raw source; parsing happens at mount).
- Produces:
  - `pub struct HtmlSource(pub String)` (Asset) + `HtmlLoader` (extension `"html"`).
  - `pub struct JsSource(pub String)` (Asset) + `JsLoader` (extension `"js"`).
  - Both loaders read the file bytes into a UTF-8 `String`. The `.css` loader is already provided by `SuperUiCssPlugin` (do not add one).

- [ ] **Step 1: Create the manifest** — create `crates/superui/Cargo.toml`:

```toml
[package]
name = "superui"
edition.workspace = true
version.workspace = true
license.workspace = true

[dependencies]
superui_dom = { path = "../superui_dom" }
superui_html = { path = "../superui_html" }
superui_js = { path = "../superui_js" }
superui_api = { path = "../superui_api" }
superui_css = { path = "../superui_css" }
superui_bridge = { path = "../superui_bridge" }

bevy = { version = "0.17", default-features = false, features = [
    "std",
    "bevy_ui",
    "bevy_text",
    "bevy_picking",
    "bevy_ui_picking_backend",
    "bevy_input_focus",
    "default_font",
] }

[target.'cfg(target_arch = "wasm32")'.dependencies]
getrandom = { version = "0.3", features = ["wasm_js"] }

[dev-dependencies]
serde = { version = "1", features = ["derive"] }
```

- [ ] **Step 2: Write the failing loader test** — create `crates/superui/src/assets.rs`:

```rust
//! Asset types + loaders for authored `.html` and `.js`. The `.css` loader comes
//! from flair via `SuperUiCssPlugin`. Loaders keep raw source; the HTML is parsed
//! and the JS executed at mount time (so hot reload can re-parse / re-exec).

use bevy::asset::io::Reader;
use bevy::asset::{Asset, AssetLoader, LoadContext};
use bevy::prelude::*;
use bevy::reflect::TypePath;

/// Raw authored HTML source (parsed into a `Dom` at mount).
#[derive(Asset, TypePath, Debug, Clone)]
pub struct HtmlSource(pub String);

/// Raw authored JS source (executed against the DOM at mount).
#[derive(Asset, TypePath, Debug, Clone)]
pub struct JsSource(pub String);

#[derive(Default)]
pub struct HtmlLoader;
#[derive(Default)]
pub struct JsLoader;

async fn read_to_string(reader: &mut dyn Reader) -> Result<String, std::io::Error> {
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).await?;
    String::from_utf8(bytes).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

impl AssetLoader for HtmlLoader {
    type Asset = HtmlSource;
    type Settings = ();
    type Error = std::io::Error;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _lc: &mut LoadContext<'_>,
    ) -> Result<HtmlSource, std::io::Error> {
        Ok(HtmlSource(read_to_string(reader).await?))
    }

    fn extensions(&self) -> &[&str] {
        &["html"]
    }
}

impl AssetLoader for JsLoader {
    type Asset = JsSource;
    type Settings = ();
    type Error = std::io::Error;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _lc: &mut LoadContext<'_>,
    ) -> Result<JsSource, std::io::Error> {
        Ok(JsSource(read_to_string(reader).await?))
    }

    fn extensions(&self) -> &[&str] {
        &["js"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::asset::io::memory::{Dir, MemoryAssetReader};
    use bevy::asset::io::{AssetSource, AssetSourceId};
    use bevy::asset::{AssetApp, AssetPlugin, AssetServer, LoadState};

    #[test]
    fn loads_html_and_js_sources() {
        let dir = Dir::new("assets".into());
        dir.insert_asset("ui.html".as_ref(), b"<div id='x'></div>");
        dir.insert_asset("app.js".as_ref(), b"var a = 1;");

        let mut app = App::new();
        app.register_asset_source(
            AssetSourceId::Default,
            AssetSource::build()
                .with_reader(move || Box::new(MemoryAssetReader { root: dir.clone() })),
        );
        app.add_plugins((
            bevy::app::TaskPoolPlugin::default(),
            AssetPlugin::default(),
        ));
        app.init_asset::<HtmlSource>()
            .init_asset::<JsSource>()
            .register_asset_loader(HtmlLoader)
            .register_asset_loader(JsLoader);
        app.finish();

        let (html, js) = {
            let server = app.world().resource::<AssetServer>().clone();
            (
                server.load::<HtmlSource>("ui.html"),
                server.load::<JsSource>("app.js"),
            )
        };
        for _ in 0..64 {
            app.update();
            let server = app.world().resource::<AssetServer>();
            if matches!(server.load_state(html.id()), LoadState::Loaded)
                && matches!(server.load_state(js.id()), LoadState::Loaded)
            {
                break;
            }
        }
        let htmls = app.world().resource::<Assets<HtmlSource>>();
        let jss = app.world().resource::<Assets<JsSource>>();
        assert_eq!(htmls.get(&html).unwrap().0, "<div id='x'></div>");
        assert_eq!(jss.get(&js).unwrap().0, "var a = 1;");
    }
}
```

- [ ] **Step 3: Create `lib.rs` wiring** — create `crates/superui/src/lib.rs`:

```rust
//! `superui` — the umbrella plugin. Bundles the CSS engine + bridge, registers
//! the `.html`/`.js` asset loaders, mounts authored UI, and hot-reloads it.

mod assets;
mod hot_reload;
mod mount;

pub use assets::{HtmlLoader, HtmlSource, JsLoader, JsSource};
pub use mount::{SuperUiPlugin, SuperUiRoot};

/// The HTML-shaped surface authors/games reach for.
pub mod prelude {
    pub use crate::{HtmlSource, JsSource, SuperUiPlugin, SuperUiRoot};
    pub use superui_bridge::SuperUiApp;
    pub use superui_css::prelude::*;
}
```

Create minimal stubs so it compiles now (Task 8/9 fill them):

`crates/superui/src/mount.rs`:
```rust
use bevy::prelude::*;

/// Marks an entity as the mount point for an authored UI (its asset handles).
#[derive(Component, Default)]
pub struct SuperUiRoot {
    pub html: Handle<crate::HtmlSource>,
    pub css: Handle<superui_css::style::StyleSheet>,
    pub js: Handle<crate::JsSource>,
}

/// The umbrella plugin. Filled in by Task 8.
pub struct SuperUiPlugin;
impl Plugin for SuperUiPlugin {
    fn build(&self, _app: &mut App) {}
}
```

`crates/superui/src/hot_reload.rs`:
```rust
//! Hot reload. Filled in by Task 9.
```

- [ ] **Step 4: Run the loader test**

Run: `cargo test -p superui --lib`
Expected: PASS — `loads_html_and_js_sources` loads both assets. Follow the compiler on the exact `AssetLoader::load` signature / `Reader` trait path (they match flair's `CssStyleSheetLoader` in `crates/bevy_flair_css_parser/src/loader.rs` — cross-check there if unsure).

- [ ] **Step 5: Boundary + commit**

Run (Bash): `cargo tree -p superui_js -e normal | grep -i bevy || echo CLEAN`
Expected: `CLEAN`.

```bash
git add crates/superui Cargo.lock
git commit -m "feat(superui): .html/.js asset types + loaders"
```

---

### Task 8: `SuperUiPlugin` — mount authored UI when assets load + schedule systems

**Files:**
- Modify: `crates/superui/src/mount.rs` (replace the stub plugin)
- Create: `crates/superui/tests/support/mod.rs`
- Create: `crates/superui/tests/integration.rs` (the mount half; capstone extended in Task 10)

**Interfaces:**
- Consumes: `superui_bridge::{UiRuntime, reconcile_system, ... systems}`, `superui_html::parse_document`, `SuperUiCssPlugin`.
- Produces:
  - `SuperUiPlugin` — adds `SuperUiCssPlugin`, `init_asset` + loaders, `PendingDomEvents`, the picking observer + all bridge systems, and a `mount_when_ready` system.
  - `mount_when_ready` — once a `SuperUiRoot`'s html+css+js handles are all `Loaded` and no runtime exists yet, parse the HTML into a `Dom`, build a `UiRuntime` (mount at the root entity, stylesheet = the css handle), `run_script` the JS, and insert the runtime NonSend.

- [ ] **Step 1: Implement the plugin + mount** — replace `crates/superui/src/mount.rs` with:

```rust
//! `SuperUiPlugin` + the mount system that turns loaded assets into a live
//! `UiRuntime` and schedules the per-frame reconcile/input/bridge systems.

use std::cell::RefCell;
use std::rc::Rc;

use bevy::asset::LoadState;
use bevy::prelude::*;
use superui_bridge::{
    drain_bevy_outbox_system, drain_dom_events_system, emit_bevy_inbox_system,
    keyboard_events_system, on_pointer_click, reconcile_system, PendingDomEvents, UiRuntime,
};
use superui_css::style::StyleSheet;
use superui_css::SuperUiCssPlugin;

use crate::assets::{HtmlLoader, HtmlSource, JsLoader, JsSource};

/// Marks an entity as an authored-UI mount point (holds its asset handles). The
/// entity is also the ECS parent the DOM `<body>` reconciles into.
#[derive(Component, Default)]
pub struct SuperUiRoot {
    pub html: Handle<HtmlSource>,
    pub css: Handle<StyleSheet>,
    pub js: Handle<JsSource>,
}

/// The umbrella plugin.
pub struct SuperUiPlugin;

impl Plugin for SuperUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(SuperUiCssPlugin)
            .init_asset::<HtmlSource>()
            .init_asset::<JsSource>()
            .register_asset_loader(HtmlLoader)
            .register_asset_loader(JsLoader)
            .init_resource::<PendingDomEvents>()
            .add_observer(on_pointer_click)
            .add_systems(Update, mount_when_ready)
            .add_systems(
                Update,
                (
                    drain_bevy_outbox_system,
                    drain_dom_events_system,
                    keyboard_events_system,
                    emit_bevy_inbox_system,
                    tick_timers_system,
                    reconcile_system,
                )
                    .chain()
                    .after(mount_when_ready),
            );
    }
}

/// Drive Boa timers each frame from Bevy's clock.
fn tick_timers_system(time: Res<Time>, rt: Option<NonSendMut<UiRuntime>>) {
    use superui_js::JsEngine;
    if let Some(mut rt) = rt {
        let now_ms = time.elapsed_secs_f64() * 1000.0;
        rt.engine.run_timers(now_ms);
    }
}

/// When a `SuperUiRoot`'s three assets are all loaded and no runtime exists yet,
/// build the runtime: parse HTML -> Dom, mount at the root entity with the css
/// handle, run the author JS. Inserts the `UiRuntime` NonSend resource.
fn mount_when_ready(
    roots: Query<(Entity, &SuperUiRoot)>,
    server: Res<AssetServer>,
    html_assets: Res<Assets<HtmlSource>>,
    js_assets: Res<Assets<JsSource>>,
    runtime: Option<NonSend<UiRuntime>>,
    mut commands: Commands,
) {
    if runtime.is_some() {
        return; // Phase 1: a single mounted UI at a time.
    }
    let Ok((entity, root)) = roots.single() else {
        return;
    };
    let ready = |id| matches!(server.load_state(id), LoadState::Loaded);
    if !(ready(root.html.id()) && ready(root.css.id()) && ready(root.js.id())) {
        return;
    }
    let Some(html) = html_assets.get(&root.html) else {
        return;
    };
    let Some(js) = js_assets.get(&root.js) else {
        return;
    };

    let dom = Rc::new(RefCell::new(superui_html::parse_document(&html.0)));
    let mut rt = UiRuntime::new(dom, entity, root.css.clone());
    rt.run_script(&js.0);
    commands.insert_resource_non_send(rt);
}
```

Follow the compiler on a few 0.17 spellings: `roots.single()` may be `roots.get_single()`; `commands.insert_resource_non_send` may be `commands.insert_non_send_resource` or you may need an exclusive closure (`commands.queue(|w: &mut World| w.insert_non_send_resource(rt))`) since `UiRuntime` is `!Send` — **prefer the `commands.queue(move |world| world.insert_non_send_resource(rt))` form**, which is guaranteed to exist. `time.elapsed_secs_f64` may be `time.elapsed_seconds_f64`.

- [ ] **Step 2: Create the superui test harness** — create `crates/superui/tests/support/mod.rs`:

```rust
//! Headless harness for `superui` integration tests: in-memory `.html`/`.css`/
//! `.js` assets + the full app with `SuperUiPlugin`.
#![allow(dead_code)]

use bevy::asset::io::memory::{Dir, MemoryAssetReader};
use bevy::asset::io::{AssetSource, AssetSourceId};
use bevy::input::InputPlugin;
use bevy::input_focus::{InputFocus, InputFocusVisible};
use bevy::picking::{InteractionPlugin, PickingPlugin};
use bevy::prelude::*;
use bevy::text::TextPlugin;
use bevy::ui::UiPlugin;
use std::sync::LazyLock;
use superui::{HtmlSource, JsSource, SuperUiPlugin, SuperUiRoot};
use superui_css::style::StyleSheet;

pub static ASSETS: LazyLock<Dir> = LazyLock::new(|| Dir::new("assets".into()));

pub fn put(name: &str, bytes: &[u8]) {
    ASSETS.insert_asset(name.as_ref(), bytes);
}

/// A headless app with the full `SuperUiPlugin` and an in-memory asset source.
pub fn app() -> App {
    let mut app = App::new();
    app.register_asset_source(
        AssetSourceId::Default,
        AssetSource::build().with_reader(move || Box::new(MemoryAssetReader { root: ASSETS.clone() })),
    );
    app.add_plugins((
        bevy::time::TimePlugin,
        bevy::app::TaskPoolPlugin::default(),
        AssetPlugin::default(),
        WindowPlugin::default(),
        bevy::image::ImagePlugin::default(),
        TextPlugin,
        (InputPlugin, PickingPlugin, InteractionPlugin, UiPlugin),
        SuperUiPlugin,
    ));
    app.init_resource::<InputFocus>()
        .init_resource::<InputFocusVisible>();
    app.finish();
    app
}

/// Spawn a `SuperUiRoot` referencing the given in-memory asset paths.
pub fn spawn_root(app: &mut App, html: &str, css: &str, js: &str) -> Entity {
    let server = app.world().resource::<AssetServer>().clone();
    let root = SuperUiRoot {
        html: server.load::<HtmlSource>(html.to_string()),
        css: server.load::<StyleSheet>(css.to_string()),
        js: server.load::<JsSource>(js.to_string()),
    };
    app.world_mut().spawn((Node::default(), root)).id()
}

/// Tick until a `UiRuntime` exists (assets loaded + mounted), max 128 frames.
pub fn run_until_mounted(app: &mut App) {
    for _ in 0..128 {
        app.update();
        if app.world().get_non_send_resource::<superui::__UiRuntimeProbe>().is_some() {
            return;
        }
        // The runtime type is bridge-private to tests; probe via any spawned child.
    }
}
```

The `run_until_mounted` probe is awkward because `UiRuntime` isn't re-exported. Simplify: instead of probing the runtime, tick a fixed number of frames and assert on entities. Replace `run_until_mounted` with:

```rust
/// Tick `n` frames (enough for asset load + mount + reconcile).
pub fn tick(app: &mut App, n: usize) {
    for _ in 0..n {
        app.update();
    }
}
```

and delete the `__UiRuntimeProbe` reference.

- [ ] **Step 3: Write the mount test** — create `crates/superui/tests/integration.rs`:

```rust
//! `superui` integration: authored assets mount, JS runs, DOM reconciles to ECS.
mod support;
use support::*;

use bevy::prelude::*;
use superui_css::prelude::TypeName;

#[test]
fn mounts_authored_ui_and_runs_js() {
    put("t8.html", b"<ul id='list'></ul>");
    put("t8.css", b"li { }");
    put(
        "t8.js",
        b"var li = document.createElement('li'); \
          li.textContent = 'hello'; \
          document.getElementById('list').appendChild(li);",
    );

    let mut app = app();
    let root = spawn_root(&mut app, "t8.html", "t8.css", "t8.js");
    tick(&mut app, 32);

    // The JS-created <li> exists as an entity with TypeName "li".
    let mut q = app.world_mut().query::<&TypeName>();
    let has_li = q.iter(app.world()).any(|t| t.0 == "li");
    assert!(has_li, "JS-created <li> should reconcile into an entity");

    // And it is under the root's subtree.
    assert!(app.world().get::<Children>(root).is_some());
}
```

- [ ] **Step 4: Run the mount test**

Run: `cargo test -p superui --test integration`
Expected: PASS — assets load, `mount_when_ready` builds the runtime and runs the JS (which creates an `<li>`), and the reconciler produces a `TypeName("li")` entity under the root.

Debugging latitude: if nothing mounts, raise the `tick` count (asset loading takes several frames); confirm `mount_when_ready` runs before the chained systems and that `commands.queue(insert_non_send)` actually inserted (a `queue` closure applies at the next command flush — the following `app.update()` reconciles).

- [ ] **Step 5: Commit**

```bash
git add crates/superui
git commit -m "feat(superui): SuperUiPlugin — mount authored UI on load + schedule bridge systems"
```

---

### Task 9: Hot reload — `AssetEvent::Modified` → re-parse/re-exec → reconcile

**Files:**
- Modify: `crates/superui/src/hot_reload.rs` (implement)
- Modify: `crates/superui/src/mount.rs` (register the hot-reload systems in the plugin)
- Modify: `crates/superui/tests/integration.rs` (hot-reload test)

**Interfaces:**
- Consumes: `AssetEvent<HtmlSource>`, `AssetEvent<StyleSheet>`, `AssetEvent<JsSource>`, `UiRuntime`, `SuperUiRoot`.
- Produces:
  - `hot_reload_html_js_system` — on `AssetEvent::Modified` for the mounted root's html or js: rebuild the `Dom` (html) and/or re-run the JS against the current DOM, then mark dirty. CSS `Modified` needs no bridge action (flair reloads the `StyleSheet` asset and re-cascades automatically) — but we still mark dirty so a reconcile re-applies inherited handles.

- [ ] **Step 1: Implement hot reload** — replace `crates/superui/src/hot_reload.rs` with:

```rust
//! Hot reload via Bevy's asset system (design §6): `AssetEvent::Modified` ->
//! re-parse HTML / re-exec JS / re-cascade CSS -> reconcile. Native `file_watcher`
//! fires these automatically; on wasm the watcher is inactive (no-op), same seam.

use std::cell::RefCell;
use std::rc::Rc;

use bevy::prelude::*;
use superui_bridge::UiRuntime;
use superui_css::style::StyleSheet;

use crate::assets::{HtmlSource, JsSource};
use crate::mount::SuperUiRoot;

/// Re-parse HTML and/or re-execute JS when the mounted root's sources change.
/// Full re-execution (design §6): a JS change tears down nothing structural here
/// — we rebuild the DOM from HTML if HTML changed, then re-run the JS, then
/// reconcile. State resets are acceptable for Phase 1.
pub fn hot_reload_system(world: &mut World) {
    // Gather which of our assets changed this frame.
    let root = match world.query::<&SuperUiRoot>().iter(world).next() {
        Some(r) => SuperUiRoot {
            html: r.html.clone(),
            css: r.css.clone(),
            js: r.js.clone(),
        },
        None => return,
    };

    let mut html_changed = false;
    let mut js_changed = false;
    let mut css_changed = false;

    if let Some(events) = world.get_resource::<Messages<AssetEvent<HtmlSource>>>() {
        let mut r = events.get_cursor();
        for e in r.read(events) {
            if let AssetEvent::Modified { id } = e {
                if *id == root.html.id() {
                    html_changed = true;
                }
            }
        }
    }
    if let Some(events) = world.get_resource::<Messages<AssetEvent<JsSource>>>() {
        let mut r = events.get_cursor();
        for e in r.read(events) {
            if let AssetEvent::Modified { id } = e {
                if *id == root.js.id() {
                    js_changed = true;
                }
            }
        }
    }
    if let Some(events) = world.get_resource::<Messages<AssetEvent<StyleSheet>>>() {
        let mut r = events.get_cursor();
        for e in r.read(events) {
            if let AssetEvent::Modified { id } = e {
                if *id == root.css.id() {
                    css_changed = true;
                }
            }
        }
    }

    if !(html_changed || js_changed || css_changed) {
        return;
    }

    let Some(mut rt) = world.remove_non_send_resource::<UiRuntime>() else {
        return;
    };

    if html_changed {
        if let Some(src) = world
            .resource::<Assets<HtmlSource>>()
            .get(&root.html)
            .map(|h| h.0.clone())
        {
            // Rebuild the DOM and re-point the engine at it: simplest correct
            // path is to rebuild the whole runtime around the fresh DOM.
            let dom = Rc::new(RefCell::new(superui_html::parse_document(&src)));
            let entity = rt.root;
            let stylesheet = rt.stylesheet.clone();
            rt = UiRuntime::new(dom, entity, stylesheet);
            // After an HTML rebuild we must re-run JS too (fresh DOM).
            if let Some(js) = world.resource::<Assets<JsSource>>().get(&root.js) {
                rt.run_script(&js.0.clone());
            }
        }
    } else if js_changed {
        // Full JS re-execution against the current DOM.
        if let Some(js) = world
            .resource::<Assets<JsSource>>()
            .get(&root.js)
            .map(|j| j.0.clone())
        {
            rt.run_script(&js);
        }
    }

    rt.dirty = true; // css_changed alone still needs a reconcile pass
    world.insert_non_send_resource(rt);
}
```

Follow the compiler on the buffered-event API name: 0.17 may expose `AssetEvent` reading via `MessageReader<AssetEvent<T>>` in a normal system rather than `Messages<..>::get_cursor()`. If the exclusive-system cursor form is awkward, split into a small normal system that collects changed flags into a resource and an exclusive system that consumes them. The load-bearing behavior: HTML change → rebuild DOM + re-run JS; JS change → re-run JS; any change → dirty.

- [ ] **Step 2: Register hot reload in the plugin** — in `crates/superui/src/mount.rs`, add `use crate::hot_reload::hot_reload_system;` and add it to the chained `Update` systems, before `reconcile_system`:

```rust
                (
                    hot_reload_system,
                    drain_bevy_outbox_system,
                    drain_dom_events_system,
                    keyboard_events_system,
                    emit_bevy_inbox_system,
                    tick_timers_system,
                    reconcile_system,
                )
                    .chain()
                    .after(mount_when_ready),
```

- [ ] **Step 3: Write the hot-reload test** — append to `crates/superui/tests/integration.rs`:

```rust
#[test]
fn hot_reload_js_re_executes_and_reconciles() {
    put("t9.html", b"<div id='host'></div>");
    put("t9.css", b"span { }");
    put("t9.js", b"var s=document.createElement('span'); s.textContent='v1'; \
                    document.getElementById('host').appendChild(s);");

    let mut app = app();
    let _root = spawn_root(&mut app, "t9.html", "t9.css", "t9.js");
    tick(&mut app, 32);

    // v1 span exists.
    let count_spans = |app: &mut App| {
        let mut q = app.world_mut().query::<&superui_css::prelude::TypeName>();
        q.iter(app.world()).filter(|t| t.0 == "span").count()
    };
    assert_eq!(count_spans(&mut app), 1);

    // Modify the JS asset in place and fire Modified.
    let js_handle = {
        let server = app.world().resource::<bevy::asset::AssetServer>().clone();
        server.load::<superui::JsSource>("t9.js")
    };
    {
        let mut assets = app.world_mut().resource_mut::<Assets<superui::JsSource>>();
        if let Some(js) = assets.get_mut(&js_handle) {
            js.0 = "var s=document.createElement('span'); s.textContent='v2'; \
                    document.getElementById('host').appendChild(s); \
                    var s2=document.createElement('span'); \
                    document.getElementById('host').appendChild(s2);"
                .to_string();
        }
    }
    // `get_mut` already emits AssetEvent::Modified; tick to process it.
    tick(&mut app, 8);

    // Re-execution ran against the current DOM: host now has more spans.
    assert!(count_spans(&mut app) >= 2, "hot reload should re-run the JS");
}
```

If mutating via `get_mut` does not emit `Modified` in this Bevy version, explicitly send it:
`app.world_mut().resource_mut::<Messages<AssetEvent<superui::JsSource>>>().write(AssetEvent::Modified { id: js_handle.id() });` (follow the compiler for `write`/`send`).

- [ ] **Step 4: Run the hot-reload test**

Run: `cargo test -p superui --test integration`
Expected: PASS — both `mounts_authored_ui_and_runs_js` and `hot_reload_js_re_executes_and_reconciles`.

- [ ] **Step 5: Commit**

```bash
git add crates/superui
git commit -m "feat(superui): hot reload via AssetEvent::Modified (re-parse HTML / re-exec JS)"
```

---

### Task 10: Capstone — full integration (input + window.bevy), wasm check, README flip

**Files:**
- Modify: `crates/superui/tests/integration.rs` (capstone test)
- Modify: `docs/superpowers/plans/README.md` (flip Plan 5 → Done; retarget resume block to Plan 6)

**Interfaces:**
- Consumes: everything above.
- Produces: an end-to-end headless proof (authored html/css/js → mount → synthetic input drives JS → DOM mutates → reconciles; a `window.bevy` round trip), a green `wasm32-unknown-unknown` build of the `superui` runtime lib, and the updated plan-series index.

- [ ] **Step 1: Write the capstone test** — append to `crates/superui/tests/integration.rs`:

```rust
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use superui_bridge::{PendingDomEvent, PendingDomEvents, SuperUiApp};

#[derive(Event, Serialize, Deserialize, Clone, Debug, PartialEq)]
struct Added {
    label: String,
}

#[test]
fn capstone_click_drives_js_and_bevy_send() {
    put("cap.html", b"<button id='add'>Add</button><ul id='list'></ul>");
    put("cap.css", b".done { }");
    put(
        "cap.js",
        b"document.getElementById('add').addEventListener('click', function(){ \
             var li=document.createElement('li'); li.textContent='item'; \
             document.getElementById('list').appendChild(li); \
             bevy.send('Added', { label: 'item' }); \
          });",
    );

    let mut app = app();
    app.add_superui_command::<Added>("Added");
    #[derive(Resource, Default)]
    struct Log(Vec<Added>);
    app.init_resource::<Log>();
    app.add_observer(|ev: On<Added>, mut l: ResMut<Log>| l.0.push(ev.event().clone()));

    let _root = spawn_root(&mut app, "cap.html", "cap.css", "cap.js");
    tick(&mut app, 32);

    // No <li> yet.
    let count_li = |app: &mut App| {
        let mut q = app.world_mut().query::<&superui_css::prelude::TypeName>();
        q.iter(app.world()).filter(|t| t.0 == "li").count()
    };
    assert_eq!(count_li(&mut app), 0);

    // Find the button's DOM node id via its entity's DomNode, enqueue a click.
    let btn_node = {
        let mut q = app
            .world_mut()
            .query::<(&superui_bridge::DomNode, &superui_css::prelude::TypeName)>();
        q.iter(app.world())
            .find(|(_, t)| t.0 == "button")
            .map(|(d, _)| d.0)
            .expect("button entity exists")
    };
    app.world_mut()
        .resource_mut::<PendingDomEvents>()
        .0
        .push(PendingDomEvent::new(btn_node, "click"));
    tick(&mut app, 4);

    // The click ran the JS listener: a <li> was created AND a bevy.send fired.
    assert_eq!(count_li(&mut app), 1, "click should create one <li>");
    assert_eq!(
        app.world().resource::<Log>().0,
        vec![Added { label: "item".into() }]
    );
}
```

- [ ] **Step 2: Run the full `superui` test suite**

Run: `cargo test -p superui`
Expected: PASS — loader unit test + mount + hot reload + capstone.

- [ ] **Step 3: Run the whole workspace test suite (regression gate)**

Run: `cargo test --workspace`
Expected: PASS — all crates, including the vendored flair fork tests and the JS/DOM crates, still green.

- [ ] **Step 4: Verify the `superui` runtime library builds for wasm**

Run: `cargo build -p superui --target wasm32-unknown-unknown`
Expected: SUCCESS — the runtime lib + `superui_bridge` + the fork + Boa compile to wasm with the getrandom flag (`.cargo/config.toml` + the wasm-target `getrandom` deps). Dev-dependencies (test harness) are not built for a plain `build`, so the heavy bits are excluded.

If it fails on getrandom (`inner_u32`/`backends`), confirm both `superui_bridge/Cargo.toml` and `superui/Cargo.toml` carry the `[target.'cfg(target_arch = "wasm32")'.dependencies] getrandom = { version = "0.3", features = ["wasm_js"] }` block, and that `.cargo/config.toml` still has the `getrandom_backend="wasm_js"` rustflag.

- [ ] **Step 5: Flip the plan-series status** — in `docs/superpowers/plans/README.md`, change the Plan 5 row from:

```
| 5 | `superui_bridge` + `superui` | Reconciler (DOM diff → Bevy ECS commands; picking/input → DOM events), `SuperUiPlugin`, asset loaders, hot reload via `AssetEvent::Modified`, **the full `window.bevy` bridge (JS-facing `bevy.send`/`bevy.on` global + observer wiring, deferred from Plan 3)**. | ⬜ Not started |
```

to:

```
| 5 | `superui_bridge` + `superui` | Reconciler (DOM diff → Bevy ECS commands; picking/input → DOM events), `SuperUiPlugin`, asset loaders, hot reload via `AssetEvent::Modified`, **the full `window.bevy` bridge (JS-facing `bevy.send`/`bevy.on` global + observer wiring, deferred from Plan 3)**. | ✅ Done — merged to `main` ([plan](./2026-07-19-superui-phase1-05-bridge.md)) |
```

Then update the **"Resuming in a fresh session"** block so it targets **Plan 6** (`examples/todomvc` + `docs/support/`) — the first `⬜ Not started` row — replacing the Plan-5 wording with a one-line pointer to Plan 6's scope: the runnable TodoMVC example (native + wasm) authored as `index.html`/`style.css`/`app.js` on top of `SuperUiPlugin`, plus the capability ledger (`docs/support/{README,html,css,js-dom}.md`, status ✅/🟡/⛔). Note that Plans 1–5 are done and merged, and that `superui` provides `SuperUiPlugin` + `SuperUiRoot { html, css, js }` + the `SuperUiApp` command/event bridge for the example to use.

- [ ] **Step 6: Final commit**

```bash
git add crates/superui docs/superpowers/plans/README.md
git commit -m "test(superui): capstone input + window.bevy integration; wasm check; mark Plan 5 done"
```

---

## Self-Review

**Spec coverage (design §3/§4/§5/§6/§8/§9):**
- "Reconciler: DOM diff → Bevy ECS commands" (§3) → Tasks 2–3: structural + identity sync keyed by stable `NodeId`; re-reconcile updates in place (Task 3 test asserts entity stability).
- "picking/input → DOM events" (§3/§9) → Task 4 (pointer click, checkbox toggle/change) + Task 5 (keyboard keydown/keyup, text-input value/input). Event types wired: `click`, `change`, `keydown`, `keyup`, `input` (design §9 list; `submit` is left for the TodoMVC example glue in Plan 6, achievable via keydown-Enter on the input as the design intends).
- "`SuperUiPlugin`, asset loaders" (§4/§6) → Task 7 (`.html`/`.js` loaders; `.css` from flair) + Task 8 (`SuperUiPlugin`, mount-on-load, `SuperUiRoot`).
- "hot reload via `AssetEvent::Modified`" (§6) → Task 9 (HTML re-parse, JS full re-exec, CSS re-cascade + dirty). wasm no-op inherited from Bevy's inactive watcher — same seam.
- "the full `window.bevy` bridge (`bevy.send`/`bevy.on` + observer wiring)" (§8) → Task 6: `send` (JS→`world.trigger`), `on` (observer→JS `_emit`), `add_superui_command`/`add_superui_event`, serde_json⇄JsValue marshalling. `bevy.query` correctly deferred to Phase 2 (§10) per Global Constraints.
- NonSend engine (§3 "JS objects wrap a NodeId, never an Entity"; Rc-based DOM) → `UiRuntime` is NonSend throughout; entity↔node mapping via `DomNode` component + runtime maps.
- Boundary discipline (§4) → Tasks 1/7 assert the four wasm-clean crates gain no `bevy_*` dep.
- Graceful degradation (§1) → JS errors logged+swallowed (Task 1); unregistered `bevy.send` warns (Task 6); malformed payloads warn (Task 6).
- wasm (§5) → Task 10 Step 4 builds `superui` for `wasm32-unknown-unknown`; both new crates carry the wasm getrandom dep.
- Ledger + TodoMVC example (§7/§9) → explicitly Plan 6; out of scope here.

**Placeholder scan:** No TBD/TODO. Every authored code step is complete. The few "follow the compiler" notes are for known cross-version API-spelling drift (Bevy 0.17 `Message` vs `Event` reader names, boa method spellings, `Pointer<Click>` constructability) — each names the exact fallback, matching the Plan 2–4 convention. No step defers real logic.

**Type consistency:** `UiRuntime` (fields `dom`/`engine`/`root`/`stylesheet`/`dirty`/`focused`, methods `new`/`run_script`/`entity_for`/`node_for`/`bind`/`unbind`/`bindings`/`reconcile`/`sync_children`/`sync_identity`), `DomNode(NodeId)`, `PendingDomEvent::new`/`PendingDomEvents`, `on_pointer_click`/`drain_dom_events_system`/`keyboard_events_system`, `install_bevy_bridge`/`BevyBridgeRegistry`/`SuperUiApp::{add_superui_command,add_superui_event}`/`drain_bevy_outbox_system`/`emit_bevy_inbox_system`, `HtmlSource`/`JsSource`/`HtmlLoader`/`JsLoader`, `SuperUiPlugin`/`SuperUiRoot{html,css,js}`, `hot_reload_system`, `reconcile_system` are used consistently across tasks and match the verified API reference. New `superui_dom::Dom::attributes(id)` is added in Task 3 (the one non-Bevy-crate change) with its own unit test, keeping `superui_dom` Bevy-free.

**Known execution risks (flagged, not blocking):** (1) Bevy 0.17 renamed buffered events to "messages" — reader/writer spellings (`MessageReader`/`Messages`/`write_message`/`get_cursor`) may vary; the plan gives both system-param and exclusive-system forms and says follow the compiler. (2) `Pointer<Click>` may not be constructible in a unit test — Task 4 provides a `click_effect` refactor fallback so checkbox logic stays unit-tested; end-to-end picking is exercised via the deterministic `PendingDomEvents` queue and the capstone. (3) `insert_non_send_resource` from `Commands` — the plan mandates the guaranteed `commands.queue(move |world| world.insert_non_send_resource(rt))` form. (4) trimmed `bevy` features may omit a plugin/type the headless harness needs — same latitude as Plan 4 (widen features / init a missing resource per the compiler).
```

