//! [`UiRuntime`]: the NonSend holder for the JS engine + shared DOM + the stable
//! `NodeId <-> Entity` map that the reconciler maintains. One per mounted UI.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use bevy::log::warn;
use bevy::prelude::*;
use superui_css::style::StyleSheet;
use superui_dom::{Dom, NodeId};
use superui_js::opwire::{bootstrap_js, OpApplier};
use superui_js::JsEngine;

/// Stamped by the reconciler on every entity it owns, so observers and systems
/// can resolve `Entity -> NodeId` via a normal query (the runtime is NonSend and
/// awkward to reach from observers).
#[derive(Component, Clone, Copy, Debug)]
pub struct DomNode(pub NodeId);

/// Marks a reconciler-managed overlay child: a checkbox's checkmark, or a
/// text `<input>`'s dim placeholder (shown only while empty; `EditableText`
/// renders the actual value). Non-pickable so clicks fall through to the
/// input itself, which holds keyboard focus.
#[derive(Component, Clone, Copy, Debug)]
pub struct InputValueText;

/// Marks the placeholder overlay specifically (a subset of [`InputValueText`],
/// which also tags checkbox marks). `dim_placeholder_text_system` recolors these
/// to a faded version of the input's resolved text color each frame.
#[derive(Component, Clone, Copy, Debug)]
pub struct PlaceholderText;

/// How a mounted UI's nodes take part in `bevy_picking`. Put it on the root
/// entity next to `SuperUiRoot`; the reconciler reads it once per pass.
///
/// Bevy's UI picking backend treats a node without a `Pickable` component as
/// blocking, and it runs at camera order +0.5 — so it outranks the sprite and
/// mesh backends. Left alone, a mounted UI therefore hides the whole world from
/// the pointer, which is wrong for anything drawn over live gameplay.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PickingPolicy {
    /// Nodes block the layers below only where the DOM is actually interactive:
    /// a node blocks if it or one of its ancestors has an event listener. HUDs,
    /// overlays and anything mounted over a live world want this.
    ///
    /// The trade-off is that clicks land on the world through non-interactive
    /// chrome — a panel background, padding, a decorative card. Note this is
    /// per-node and unrelated to entity-hierarchy bubbling: a listener on an
    /// element still receives clicks on its children, because the reconciler
    /// resolves a pick to the nearest DOM ancestor and the DOM dispatch bubbles.
    #[default]
    PassThrough,
    /// Every element blocks, like a page in a browser. For full-screen menus and
    /// modals that should swallow everything behind them.
    ///
    /// Text nodes stay ignored either way — a pick resolves to its nearest
    /// element ancestor, which blocks under this policy, so the layers below are
    /// covered just the same.
    Solid,
}

/// NonSend because the engine holds `Rc<RefCell<Dom>>` and `!Send` JS handles.
pub struct UiRuntime {
    /// The render-mirror DOM the reconciler reads: the [`OpApplier`]'s output,
    /// rebuilt from the JS shadow DOM's op batches. Shared with the engine.
    pub dom: Rc<RefCell<Dom>>,
    /// The JS engine, running the shadow DOM + reactive runtime + `window.bevy`.
    pub engine: Box<dyn JsEngine>,
    /// Replays each flushed op batch onto `dom` and owns the `jsId <-> NodeId` map.
    pub applier: OpApplier,
    /// The ECS entity the DOM `<body>` reconciles into (its children mount here).
    pub root: Entity,
    /// The stylesheet handle the root carries (children inherit it in flair).
    pub stylesheet: Handle<StyleSheet>,
    /// Set whenever the DOM may have changed; cleared after a reconcile.
    pub dirty: bool,
    /// Completed reconcile passes, monotonic. `dirty` is set and cleared within a
    /// single schedule run, so it cannot be observed from outside; this counter is
    /// what lets an external driver ask "did a reconcile happen during that
    /// `app.update()`?" after the fact. Used by the rows benchmark's quiescence loop.
    pub reconciles: u64,
    node_to_entity: HashMap<NodeId, Entity>,
    entity_to_node: HashMap<Entity, NodeId>,
    /// The DOM node that currently has keyboard focus (Task 5).
    pub(crate) focused: Option<NodeId>,
    /// The focused node's value at focus-gain, for the on_focus_lost change check.
    pub(crate) focus_snapshot: Option<(NodeId, String)>,
    /// Text-`<input>` node -> its managed [`InputValueText`] child entity.
    pub(crate) input_texts: HashMap<NodeId, Entity>,
    /// EditableText `<input>` node -> the value the DOM and the `EditableText`
    /// buffer last agreed on. `editable_input_events_system` compares against
    /// this (not a live re-read of the DOM) to tell a real user edit from an
    /// echo of the reconciler's own DOM->buffer push: Bevy's `Changed<T>` fires
    /// on the frame *after* a mutation is observed, by which point the live DOM
    /// value may have moved again (e.g. a controlled input's next JS write),
    /// making a stale-vs-live comparison unreliable.
    pub(crate) editable_synced: HashMap<NodeId, String>,
    /// Range `<input>` node -> the `SliderValue` the DOM and the slider component
    /// last agreed on. Mirrors `editable_synced`'s role: tells an external DOM
    /// write (JS/attribute change) from an echo of the reconciler's own push.
    pub(crate) range_synced: HashMap<NodeId, f32>,
}

impl UiRuntime {
    /// Build a runtime around a parsed `dom`, mounting at `root` with `stylesheet`.
    ///
    /// The parsed tree is taken out as a hydration source and `dom` is left empty
    /// to serve as the render mirror the applier rebuilds into — so the shared
    /// handle the reconciler reads is the op-wire's output, while
    /// `document.getElementById(...)` in JS resolves against the shadow DOM. The
    /// initial HTML is replayed through the op-wire, then the reactive runtime and
    /// `window.bevy` are installed. Does NOT run author JS yet (callers
    /// `run_script` after, so hot reload can re-exec independently). Starts `dirty`.
    pub fn new(
        dom: Rc<RefCell<Dom>>,
        root: Entity,
        stylesheet: Handle<StyleSheet>,
        hmr: bool,
    ) -> Self {
        // Repurpose the parsed document as the mirror: take its tree out as the
        // hydration source, leaving an empty Dom for the applier to rebuild into.
        let source = std::mem::replace(&mut *dom.borrow_mut(), Dom::new());

        let mut engine = superui_js::new_engine(dom.clone());
        let mut applier = OpApplier::new(dom.borrow().document());

        // Hydrate the initial HTML through the op-wire: bootstrap_js rebuilds the
        // source tree via the shadow DOM's own createElement/appendChild, recording
        // an ordinary op batch the applier replays onto the (empty) mirror. This
        // makes getElementById work in JS AND populates the render mirror.
        let boot = bootstrap_js(&source);
        let _ = engine.eval(&boot);
        let batch = engine.flush_ops();
        if !batch.ops.is_empty() {
            applier.apply(&mut dom.borrow_mut(), &batch);
        }

        supersolid_runtime::install(engine.as_mut());
        // Plan 5: enable state-preserving HMR collection in render.js. Must run
        // after install (so the runtime exists) and before any run_script (so the
        // first render already collects). Gate decided by the caller (feature +
        // asset watcher); off => render.js takes the Plan-4 fast paths.
        if hmr {
            let _ = engine.eval("globalThis.__ssHmr = true;");
        }
        crate::bevy_bridge::install_bevy_bridge(engine.as_mut());

        UiRuntime {
            dom,
            engine,
            applier,
            root,
            stylesheet,
            dirty: true,
            reconciles: 0,
            node_to_entity: HashMap::new(),
            entity_to_node: HashMap::new(),
            focused: None,
            focus_snapshot: None,
            input_texts: HashMap::new(),
            editable_synced: HashMap::new(),
            range_synced: HashMap::new(),
        }
    }

    /// Flush the JS shadow DOM's queued mutations onto the render mirror. Call
    /// after any JS runs (author script, event dispatch, timers, ECS→JS emit) so
    /// its DOM changes — including listener-presence markers (`SetListener`) that
    /// drive picking — reach the reconciler.
    pub fn pump(&mut self) {
        let batch = self.engine.flush_ops();
        if !batch.ops.is_empty() {
            self.applier.apply(&mut self.dom.borrow_mut(), &batch);
            self.dirty = true;
        }
    }

    /// Evaluate an author script against the current DOM, then flush its DOM
    /// mutations onto the render mirror. Errors are logged and swallowed (graceful
    /// degradation, design §1). Marks the runtime dirty.
    ///
    /// The script is wrapped in an IIFE so its top-level `const`/`let`/`class`
    /// bindings are function-scoped rather than landing in the global lexical
    /// environment. That environment persists across `eval` calls on the same
    /// Context, so an unwrapped re-exec — which is exactly what hot reload does —
    /// would throw "duplicate lexical declaration" and silently abort. Wrapping
    /// makes re-execution idempotent; author code shares state across reloads via
    /// `globalThis` / the runtime, not via top-level lexical names, so nothing is
    /// lost. Runtime globals (`render`, `$ss`, `createSignal`, …) are installed on
    /// the global object and stay reachable from inside the IIFE.
    pub fn run_script(&mut self, src: &str) {
        // Leading/trailing newlines guard against a `//` line comment or a missing
        // final semicolon in `src` swallowing or fusing with the wrapper syntax.
        let wrapped = format!("(function () {{\n{src}\n}})();");
        if let Err(e) = self.engine.eval(&wrapped) {
            warn!("superui: JS error: {e}");
        }
        self.dirty = true;
        self.pump();
    }

    /// Push host-owned live IDL state (a text field's `value`, a checkbox's
    /// `checked`) from the render mirror into the JS shadow.
    ///
    /// Keyboard and pointer input mutate the mirror, not the shadow, so author
    /// listeners would read stale `input.value`/`checked` unless the shadow is
    /// realigned first. Emits no op — the mirror already holds these values, so
    /// this is a one-way host->shadow sync, not a shadow mutation.
    fn sync_live_props_to_shadow(&mut self) {
        let mut entries: Vec<(u32, String, bool)> = Vec::new();
        {
            let d = self.dom.borrow();
            let mut stack = vec![d.document()];
            while let Some(n) = stack.pop() {
                for &c in d.children(n) {
                    stack.push(c);
                }
                if d.tag(n) == Some("input") {
                    if let Some(js) = self.applier.js(n) {
                        entries.push((js, d.value(n), d.checked(n)));
                    }
                }
            }
        }
        if entries.is_empty() {
            return;
        }
        if let Ok(json) = serde_json::to_string(&entries) {
            let _ = self.engine.eval(&format!("__ss_set_live({json});"));
        }
    }

    /// Dispatch a DOM event at the render-mirror `node` by routing it to the
    /// shadow DOM's `jsId`, then flush any DOM mutations the listeners made.
    /// Returns whether `preventDefault()` was called. A node with no shadow
    /// binding is skipped (returns `false`).
    pub fn dispatch_dom_event(
        &mut self,
        node: NodeId,
        ty: &str,
        key: Option<&str>,
        bubbles: bool,
        cancelable: bool,
    ) -> bool {
        let Some(js) = self.applier.js(node) else {
            return false;
        };
        self.sync_live_props_to_shadow();
        let prevented = self.engine.dispatch_event(js, ty, key, bubbles, cancelable);
        self.pump();
        prevented
    }

    pub fn entity_for(&self, node: NodeId) -> Option<Entity> {
        self.node_to_entity.get(&node).copied()
    }

    pub fn node_for(&self, entity: Entity) -> Option<NodeId> {
        self.entity_to_node.get(&entity).copied()
    }

    /// Insert/refresh the bidirectional map entry (used by the reconciler).
    #[allow(dead_code)]
    pub(crate) fn bind(&mut self, node: NodeId, entity: Entity) {
        self.node_to_entity.insert(node, entity);
        self.entity_to_node.insert(entity, node);
    }

    /// Drop a mapping (used when the reconciler despawns a vanished node).
    #[allow(dead_code)]
    pub(crate) fn unbind(&mut self, node: NodeId, entity: Entity) {
        self.node_to_entity.remove(&node);
        self.entity_to_node.remove(&entity);
    }

    /// Read-only view of the current node->entity bindings (for the reconciler).
    #[allow(dead_code)]
    pub(crate) fn bindings(&self) -> &HashMap<NodeId, Entity> {
        &self.node_to_entity
    }

    /// Returns all bound entities except `root` — useful for hot-reload cleanup.
    pub fn bound_non_root_entities(&self) -> Vec<Entity> {
        self.node_to_entity
            .values()
            .copied()
            .filter(|&e| e != self.root)
            .collect()
    }

    /// Set the keyboard-focused DOM node (Task 5). Public so integration tests and
    /// the bevy_bridge can assign focus without going through the observer.
    pub fn set_focus(&mut self, node: Option<NodeId>) {
        self.focused = node;
    }

    /// The currently keyboard-focused DOM node, if any. Public so integration
    /// tests (and automation) can assert where a click/Tab landed focus.
    pub fn focused(&self) -> Option<NodeId> {
        self.focused
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Read a JS-side value back over the engine boundary via the outbox — the
    /// op-wire has no direct value-read, so the assertions route through
    /// `__superui_bevy_send`, exactly the path the bevy bridge drains each frame.
    fn read_back(rt: &mut UiRuntime, expr: &str) -> serde_json::Value {
        rt.engine
            .eval(&format!("__superui_bevy_send('__t', ({expr}));"))
            .unwrap();
        let out = rt.engine.drain_outbox();
        out.into_iter()
            .find(|(n, _)| n == "__t")
            .map(|(_, v)| v)
            .unwrap_or(serde_json::Value::Null)
    }

    #[test]
    fn new_runtime_is_dirty_and_reflects_script_on_the_mirror() {
        let dom = Rc::new(RefCell::new(superui_html::parse_document(
            "<div id='a'></div>",
        )));
        let mut rt = UiRuntime::new(dom.clone(), Entity::PLACEHOLDER, Handle::default(), false);
        assert!(rt.dirty, "a fresh runtime must reconcile on the first frame");

        // A script that mutates the DOM reaches the render mirror and re-dirties.
        rt.dirty = false;
        rt.run_script("document.getElementById('a').setAttribute('data-x','1');");
        assert!(rt.dirty);
        let node = dom.borrow().get_element_by_id("a").unwrap();
        assert_eq!(dom.borrow().get_attribute(node, "data-x"), Some("1"));

        // A broken script is swallowed, not panicked, and still marks dirty.
        rt.run_script("this is not valid js @@@");
        assert!(rt.dirty);
    }

    #[test]
    fn supersolid_runtime_globals_are_available_in_the_ui_runtime() {
        let dom = Rc::new(RefCell::new(superui_html::parse_document(
            "<div id='a'></div>",
        )));
        let mut rt = UiRuntime::new(dom, Entity::PLACEHOLDER, Handle::default(), false);
        // The reactive globals the Plan 2 transpiler emits imports for must resolve.
        rt.run_script(
            r#"
            var n = createSignal(1);
            globalThis.captured = 0;
            createEffect(function () { globalThis.captured = n[0](); });
            n[1](42);
            "#,
        );
        assert_eq!(read_back(&mut rt, "globalThis.captured").as_f64(), Some(42.0));
    }

    #[test]
    fn hmr_flag_set_when_enabled() {
        let dom = Rc::new(RefCell::new(superui_html::parse_document("<div id='a'></div>")));
        let mut rt = UiRuntime::new(dom, Entity::PLACEHOLDER, Handle::default(), true);
        assert_eq!(read_back(&mut rt, "globalThis.__ssHmr === true"), serde_json::json!(true));
    }

    #[test]
    fn hmr_flag_absent_when_disabled() {
        let dom = Rc::new(RefCell::new(superui_html::parse_document("<div id='a'></div>")));
        let mut rt = UiRuntime::new(dom, Entity::PLACEHOLDER, Handle::default(), false);
        assert_eq!(read_back(&mut rt, "globalThis.__ssHmr === true"), serde_json::json!(false));
    }

    #[test]
    fn top_level_const_can_be_reexecuted_for_hot_reload() {
        // Regression: a top-level `const`/`let` lands in the global lexical
        // environment, which persists across `eval` calls in the same Context.
        // Re-running the same author script (as hot reload does) must NOT throw
        // "duplicate lexical declaration" and silently abort the re-exec.
        let dom = Rc::new(RefCell::new(superui_html::parse_document("<div id='a'></div>")));
        let mut rt = UiRuntime::new(dom, Entity::PLACEHOLDER, Handle::default(), false);
        let script = r#"
            const CONTROL_ROWS = [
                { id: "move_up", field: "up" },
                { id: "move_down", field: "down" },
            ];
            globalThis.runs = (globalThis.runs || 0) + 1;
            globalThis.rowCount = CONTROL_ROWS.length;
        "#;

        rt.run_script(script);
        rt.run_script(script); // simulate a hot reload re-exec

        assert_eq!(
            read_back(&mut rt, "globalThis.runs").as_f64(),
            Some(2.0),
            "re-exec must run fully, not abort on a duplicate const"
        );
    }
}
