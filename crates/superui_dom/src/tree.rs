use slotmap::SlotMap;

use crate::node::{ElementData, NodeData, NodeId, NodeKind};

/// Errors from structural mutations. Returned instead of panicking so that
/// AI-generated / framework code hitting an illegal operation degrades.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DomError {
    /// A handle referred to a removed or foreign node.
    NotFound,
    /// The operation requires an element but the node was Document/Text.
    NotAnElement,
    /// The reference/old child is not actually a child of the parent.
    NotAChild,
    /// The operation would create a cycle (inserting an ancestor into a descendant).
    Hierarchy,
}

/// The DOM: an arena of nodes plus the document root and a listener counter.
pub struct Dom {
    nodes: SlotMap<NodeId, NodeData>,
    document: NodeId,
    pub(crate) next_listener: u64,
}

impl Dom {
    /// Create an empty document (a `Document` root with no children).
    pub fn new() -> Self {
        let mut nodes = SlotMap::with_key();
        let document = nodes.insert(NodeData::new(NodeKind::Document));
        Dom { nodes, document, next_listener: 0 }
    }

    /// The document root handle.
    pub fn document(&self) -> NodeId {
        self.document
    }

    /// Create a detached element node. Tag name is lowercased (HTML-insensitive).
    pub fn create_element(&mut self, tag: &str) -> NodeId {
        let data = ElementData { tag: tag.to_ascii_lowercase(), ..Default::default() };
        self.nodes.insert(NodeData::new(NodeKind::Element(data)))
    }

    /// Create a detached text node.
    pub fn create_text(&mut self, data: &str) -> NodeId {
        self.nodes.insert(NodeData::new(NodeKind::Text(data.to_string())))
    }

    /// Node data by handle, or `None` if the handle is stale/foreign.
    pub fn get(&self, id: NodeId) -> Option<&NodeData> {
        self.nodes.get(id)
    }

    /// Mutable node data by handle.
    pub fn get_mut(&mut self, id: NodeId) -> Option<&mut NodeData> {
        self.nodes.get_mut(id)
    }

    /// Tag name of an element node, else `None`.
    pub fn tag(&self, id: NodeId) -> Option<&str> {
        match &self.nodes.get(id)?.kind {
            NodeKind::Element(e) => Some(e.tag.as_str()),
            _ => None,
        }
    }

    /// Whether the node is an element.
    pub fn is_element(&self, id: NodeId) -> bool {
        matches!(self.nodes.get(id).map(|n| &n.kind), Some(NodeKind::Element(_)))
    }
}

impl Default for Dom {
    fn default() -> Self {
        Self::new()
    }
}
