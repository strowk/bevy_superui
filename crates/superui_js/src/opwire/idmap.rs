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
    /// for either side.
    pub fn bind(&mut self, js: JsNodeId, node: NodeId) {
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
}
