use slotmap::new_key_type;

new_key_type! {
    /// Stable, generational handle to a node. Cheap `Copy`. A handle to a
    /// removed node becomes permanently invalid (accessors return `None`).
    pub struct NodeId;
}

/// Monotonic identifier for a registered event listener on a node.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ListenerId(pub u64);

/// A registered event listener. The actual callback lives in the JS layer;
/// the DOM only records that a listener of a given type/phase exists.
#[derive(Clone, Debug)]
pub struct Listener {
    pub id: ListenerId,
    pub event_type: String,
    pub capture: bool,
}

/// Element-specific data: tag name, ordered attributes, and listeners.
#[derive(Clone, Debug, Default)]
pub struct ElementData {
    pub tag: String,
    pub(crate) attrs: Vec<(String, String)>,
    pub(crate) listeners: Vec<Listener>,
}

/// The variant of a node.
#[derive(Clone, Debug)]
pub enum NodeKind {
    Document,
    Element(ElementData),
    Text(String),
}

/// A single node in the arena. Structural links are private; mutate them only
/// through `Dom` so invariants (single parent, no cycles) hold.
#[derive(Clone, Debug)]
pub struct NodeData {
    pub kind: NodeKind,
    pub(crate) parent: Option<NodeId>,
    pub(crate) children: Vec<NodeId>,
}

impl NodeData {
    pub(crate) fn new(kind: NodeKind) -> Self {
        NodeData { kind, parent: None, children: Vec::new() }
    }

    /// Parent node, or `None` for the document root or a detached node.
    pub fn parent(&self) -> Option<NodeId> {
        self.parent
    }

    /// Ordered child handles.
    pub fn children(&self) -> &[NodeId] {
        &self.children
    }
}

use crate::tree::Dom;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::NodeKind;

    #[test]
    fn new_dom_has_a_document_root() {
        let dom = Dom::new();
        let root = dom.document();
        assert!(matches!(dom.get(root).unwrap().kind, NodeKind::Document));
        assert_eq!(dom.get(root).unwrap().parent(), None);
        assert_eq!(dom.get(root).unwrap().children().len(), 0);
    }

    #[test]
    fn create_element_lowercases_tag_and_stores_it() {
        let mut dom = Dom::new();
        let el = dom.create_element("DIV");
        assert_eq!(dom.tag(el), Some("div"));
        assert!(dom.is_element(el));
    }

    #[test]
    fn create_text_stores_data() {
        let mut dom = Dom::new();
        let t = dom.create_text("hello");
        assert!(matches!(&dom.get(t).unwrap().kind, NodeKind::Text(s) if s == "hello"));
        assert!(!dom.is_element(t));
    }

    #[test]
    fn distinct_nodes_get_distinct_handles_and_resolve() {
        let mut dom = Dom::new();
        let a = dom.create_element("a");
        let b = dom.create_element("b");
        assert_ne!(a, b);
        assert_ne!(a, dom.document());
        assert!(dom.get(a).is_some());
        assert!(dom.get(b).is_some());
    }
}
