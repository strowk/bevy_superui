//! Bidirectional map between JS-side shadow DOM node ids ([`JsNodeId`]) and
//! the render-mirror [`superui_dom::Dom`]'s [`NodeId`] handles.

use std::collections::HashMap;

use superui_dom::NodeId;

use crate::opwire::JsNodeId;

/// Bidirectional `JsNodeId <-> NodeId` binding. The root is pre-bound: jsId
/// `1` always maps to the [`NodeId`] passed to [`IdMap::new`].
pub struct IdMap {
    by_js: HashMap<JsNodeId, NodeId>,
    by_node: HashMap<u64, JsNodeId>,
}

impl IdMap {
    /// Creates a map with the root pre-bound: jsId `1 -> root_node`.
    pub fn new(root_node: NodeId) -> Self {
        let mut map = IdMap { by_js: HashMap::new(), by_node: HashMap::new() };
        map.bind(1, root_node);
        map
    }

    /// Binds `js <-> node` in both directions, overwriting any prior binding
    /// for either side. Clears the stale entry a prior binding of `js` or
    /// `node` left on the other axis, so both maps stay mutual inverses.
    pub fn bind(&mut self, js: JsNodeId, node: NodeId) {
        if let Some(old_node) = self.by_js.get(&js).copied() {
            self.by_node.remove(&old_node.to_ffi());
        }
        if let Some(old_js) = self.by_node.get(&node.to_ffi()).copied() {
            self.by_js.remove(&old_js);
        }
        self.by_js.insert(js, node);
        self.by_node.insert(node.to_ffi(), js);
    }

    /// The [`NodeId`] bound to `js`, if any.
    pub fn node(&self, js: JsNodeId) -> Option<NodeId> {
        self.by_js.get(&js).copied()
    }

    /// The [`JsNodeId`] bound to `node`, if any.
    pub fn js(&self, node: NodeId) -> Option<JsNodeId> {
        self.by_node.get(&node.to_ffi()).copied()
    }

    /// Removes `js`'s binding in both directions.
    pub fn unbind(&mut self, js: JsNodeId) {
        if let Some(node) = self.by_js.remove(&js) {
            self.by_node.remove(&node.to_ffi());
        }
    }
}

#[cfg(test)]
mod tests {
    use superui_dom::NodeId;

    use super::super::IdMap;

    #[test]
    fn binds_both_directions_and_unbinds() {
        let root = NodeId::from_ffi(10);
        let mut m = IdMap::new(root);
        assert_eq!(m.node(1), Some(root));
        let n = NodeId::from_ffi(42);
        m.bind(7, n);
        assert_eq!(m.node(7), Some(n));
        assert_eq!(m.js(n), Some(7));
        m.unbind(7);
        assert_eq!(m.node(7), None);
        assert_eq!(m.js(n), None);
    }

    #[test]
    fn rebinding_js_to_a_new_node_clears_the_old_reverse_entry() {
        let root = NodeId::from_ffi(1);
        let mut m = IdMap::new(root);
        let old_node = NodeId::from_ffi(20);
        let new_node = NodeId::from_ffi(21);
        m.bind(7, old_node);
        m.bind(7, new_node);
        assert_eq!(m.node(7), Some(new_node));
        assert_eq!(m.js(new_node), Some(7));
        assert_eq!(m.js(old_node), None);
    }

    #[test]
    fn rebinding_node_to_a_new_js_clears_the_old_forward_entry() {
        let root = NodeId::from_ffi(1);
        let mut m = IdMap::new(root);
        let node = NodeId::from_ffi(30);
        m.bind(7, node);
        m.bind(8, node);
        assert_eq!(m.js(node), Some(8));
        assert_eq!(m.node(8), Some(node));
        assert_eq!(m.node(7), None);
        // Old js entry must be gone, not dangling: otherwise unbind(7) below
        // would delete the still-valid 8<->node reverse entry.
        m.unbind(7);
        assert_eq!(m.node(8), Some(node));
        assert_eq!(m.js(node), Some(8));
    }
}
