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
