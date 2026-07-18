use slotmap::new_key_type;

new_key_type! {
    /// Stable, generational handle to a node. Cheap `Copy`. A handle to a
    /// removed node becomes permanently invalid (accessors return `None`).
    pub struct NodeId;
}

use slotmap::Key;

impl NodeId {
    /// Encode this handle as a stable `u64` (for marshalling to the JS layer).
    pub fn to_ffi(self) -> u64 {
        self.data().as_ffi()
    }

    /// Reconstruct a handle from [`NodeId::to_ffi`]. The result is only valid if
    /// the original node still exists; accessors return `None` otherwise.
    pub fn from_ffi(v: u64) -> Self {
        slotmap::KeyData::from_ffi(v).into()
    }
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
    /// `.value` IDL property once set by JS (`None` = derive from attribute).
    pub(crate) value: Option<String>,
    /// `.checked` IDL property once set by JS (`None` = derive from attribute).
    pub(crate) checked: Option<bool>,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::Dom;

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

#[cfg(test)]
mod ffi_tests {
    use crate::Dom;

    #[test]
    fn node_id_round_trips_through_ffi() {
        let mut dom = Dom::new();
        let el = dom.create_element("div");
        let raw = el.to_ffi();
        assert_eq!(crate::NodeId::from_ffi(raw), el);
    }
}
