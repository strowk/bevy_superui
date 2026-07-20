//! [`UiRuntime`]: the NonSend holder for the JS engine + shared DOM + the stable
//! `NodeId <-> Entity` map that the reconciler maintains. One per mounted UI.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use bevy::log::warn;
use bevy::prelude::*;
use superui_css::style::StyleSheet;
use superui_dom::{Dom, NodeId};
use superui_js::{BoaEngine, JsEngine};

/// Stamped by the reconciler on every entity it owns, so observers and systems
/// can resolve `Entity -> NodeId` via a normal query (the runtime is NonSend and
/// awkward to reach from observers).
#[derive(Component, Clone, Copy, Debug)]
pub struct DomNode(pub NodeId);

/// Marks the reconciler-managed child that renders a text `<input>`'s value or
/// placeholder. It's kept non-pickable so clicks fall through to the input
/// (container) itself, which is what receives keyboard focus.
#[derive(Component, Clone, Copy, Debug)]
pub struct InputValueText;

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
    /// Whether the text caret is currently drawn (blinks; see `blink_caret_system`).
    pub(crate) caret_visible: bool,
    /// Time accumulator driving the caret blink.
    pub(crate) caret_accum: f32,
    /// Text-`<input>` node -> its managed [`InputValueText`] child entity.
    pub(crate) input_texts: HashMap<NodeId, Entity>,
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
        supersolid_runtime::install(&mut engine);
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
            caret_visible: true,
            caret_accum: 0.0,
            input_texts: HashMap::new(),
        }
    }

    /// Advance the caret-blink clock by `dt` seconds. Returns `true` if the caret
    /// visibility flipped (so the caller can mark the runtime dirty to re-render).
    pub fn advance_caret(&mut self, dt: f32) -> bool {
        if self.focused.is_none() {
            // No focus: keep the caret "on" so it shows immediately next focus.
            self.caret_accum = 0.0;
            let was_off = !self.caret_visible;
            self.caret_visible = true;
            return was_off;
        }
        self.caret_accum += dt;
        if self.caret_accum >= 0.53 {
            self.caret_accum = 0.0;
            self.caret_visible = !self.caret_visible;
            return true;
        }
        false
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

    #[test]
    fn supersolid_runtime_globals_are_available_in_the_ui_runtime() {
        let dom = Rc::new(RefCell::new(superui_html::parse_document(
            "<div id='a'></div>",
        )));
        let mut rt = UiRuntime::new(dom, Entity::PLACEHOLDER, Handle::default());
        // The reactive globals the Plan 2 transpiler emits imports for must resolve.
        rt.run_script(
            r#"
            var n = createSignal(1);
            globalThis.captured = 0;
            createEffect(function () { globalThis.captured = n[0](); });
            n[1](42);
            "#,
        );
        let got = rt
            .engine
            .context_mut()
            .eval(boa_engine::Source::from_bytes("globalThis.captured"))
            .unwrap()
            .as_number()
            .unwrap();
        assert_eq!(got, 42.0);
    }
}
