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

impl Dom {
    /// Ordered children of a node (empty slice if node is missing/childless).
    pub fn children(&self, id: NodeId) -> &[NodeId] {
        self.nodes.get(id).map(|n| n.children.as_slice()).unwrap_or(&[])
    }

    /// Parent of a node, if any.
    pub fn parent(&self, id: NodeId) -> Option<NodeId> {
        self.nodes.get(id)?.parent
    }

    /// True if `maybe_ancestor` is `node` or an ancestor of `node`.
    pub(crate) fn is_inclusive_ancestor(&self, maybe_ancestor: NodeId, node: NodeId) -> bool {
        let mut cur = Some(node);
        while let Some(c) = cur {
            if c == maybe_ancestor {
                return true;
            }
            cur = self.nodes.get(c).and_then(|n| n.parent);
        }
        false
    }

    /// Detach `child` from its current parent, if it has one. Internal helper.
    fn detach(&mut self, child: NodeId) {
        let Some(old_parent) = self.nodes.get(child).and_then(|n| n.parent) else { return };
        if let Some(p) = self.nodes.get_mut(old_parent) {
            p.children.retain(|&c| c != child);
        }
        if let Some(n) = self.nodes.get_mut(child) {
            n.parent = None;
        }
    }

    /// Append `child` as the last child of `parent`, reparenting if needed.
    pub fn append_child(&mut self, parent: NodeId, child: NodeId) -> Result<(), DomError> {
        self.insert_before(parent, child, None)
    }

    /// Insert `child` into `parent` immediately before `reference`
    /// (or append when `reference` is `None`). Reparents `child` if attached.
    pub fn insert_before(
        &mut self,
        parent: NodeId,
        child: NodeId,
        reference: Option<NodeId>,
    ) -> Result<(), DomError> {
        if self.nodes.get(parent).is_none() || self.nodes.get(child).is_none() {
            return Err(DomError::NotFound);
        }
        // Inserting an (inclusive) ancestor of `parent` under it would cycle.
        if self.is_inclusive_ancestor(child, parent) {
            return Err(DomError::Hierarchy);
        }
        // Validate `reference` is a current child *before* detaching `child`, so
        // a foreign reference reports NotAChild.
        if let Some(r) = reference {
            if !self.nodes[parent].children.contains(&r) {
                return Err(DomError::NotAChild);
            }
            // Inserting `child` before itself leaves it in place (no-op).
            if r == child {
                return Ok(());
            }
        }
        // Detach first, THEN compute the index against the updated child list, so
        // moving an already-attached earlier sibling lands before `reference`
        // rather than at a stale (pre-detach) position.
        self.detach(child);
        let index = match reference {
            None => self.nodes[parent].children.len(),
            Some(r) => self.nodes[parent]
                .children
                .iter()
                .position(|&c| c == r)
                .unwrap_or_else(|| self.nodes[parent].children.len()),
        };
        self.nodes[parent].children.insert(index, child);
        self.nodes[child].parent = Some(parent);
        Ok(())
    }

    /// Remove `child` from `parent`. Errors if `child` is not a child of `parent`.
    pub fn remove_child(&mut self, parent: NodeId, child: NodeId) -> Result<(), DomError> {
        let siblings = &self.nodes.get(parent).ok_or(DomError::NotFound)?.children;
        if !siblings.contains(&child) {
            return Err(DomError::NotAChild);
        }
        self.nodes[parent].children.retain(|&c| c != child);
        self.nodes[child].parent = None;
        Ok(())
    }

    /// Replace `old` (a child of `parent`) with `new`, preserving position.
    pub fn replace_child(
        &mut self,
        parent: NodeId,
        new: NodeId,
        old: NodeId,
    ) -> Result<(), DomError> {
        let index = self
            .nodes
            .get(parent)
            .ok_or(DomError::NotFound)?
            .children
            .iter()
            .position(|&c| c == old)
            .ok_or(DomError::NotAChild)?;
        if self.nodes.get(new).is_none() {
            return Err(DomError::NotFound);
        }
        if self.is_inclusive_ancestor(new, parent) {
            return Err(DomError::Hierarchy);
        }
        self.detach(new);
        // `old` may have shifted if `new` was an earlier sibling that got detached.
        let index = self.nodes[parent].children.iter().position(|&c| c == old).unwrap_or(index);
        self.nodes[parent].children[index] = new;
        self.nodes[new].parent = Some(parent);
        self.nodes[old].parent = None;
        Ok(())
    }

    /// Next sibling of `id` within its parent, if any.
    pub fn next_sibling(&self, id: NodeId) -> Option<NodeId> {
        let parent = self.parent(id)?;
        let kids = &self.nodes.get(parent)?.children;
        let i = kids.iter().position(|&c| c == id)?;
        kids.get(i + 1).copied()
    }

    /// Previous sibling of `id` within its parent, if any.
    pub fn previous_sibling(&self, id: NodeId) -> Option<NodeId> {
        let parent = self.parent(id)?;
        let kids = &self.nodes.get(parent)?.children;
        let i = kids.iter().position(|&c| c == id)?;
        if i == 0 { None } else { kids.get(i - 1).copied() }
    }
}

#[cfg(test)]
mod mutation_tests {
    use super::*;

    #[test]
    fn append_child_sets_parent_and_order() {
        let mut dom = Dom::new();
        let root = dom.document();
        let a = dom.create_element("a");
        let b = dom.create_element("b");
        dom.append_child(root, a).unwrap();
        dom.append_child(root, b).unwrap();
        assert_eq!(dom.children(root), &[a, b]);
        assert_eq!(dom.parent(a), Some(root));
        assert_eq!(dom.parent(b), Some(root));
    }

    #[test]
    fn append_child_reparents_detaching_from_old_parent() {
        let mut dom = Dom::new();
        let p1 = dom.create_element("p1");
        let p2 = dom.create_element("p2");
        let c = dom.create_element("c");
        dom.append_child(p1, c).unwrap();
        dom.append_child(p2, c).unwrap();
        assert_eq!(dom.children(p1), &[]);
        assert_eq!(dom.children(p2), &[c]);
        assert_eq!(dom.parent(c), Some(p2));
    }

    #[test]
    fn insert_before_places_child_ahead_of_reference() {
        let mut dom = Dom::new();
        let root = dom.document();
        let a = dom.create_element("a");
        let b = dom.create_element("b");
        let c = dom.create_element("c");
        dom.append_child(root, a).unwrap();
        dom.append_child(root, c).unwrap();
        dom.insert_before(root, b, Some(c)).unwrap();
        assert_eq!(dom.children(root), &[a, b, c]);
    }

    #[test]
    fn insert_before_none_reference_appends() {
        let mut dom = Dom::new();
        let root = dom.document();
        let a = dom.create_element("a");
        dom.insert_before(root, a, None).unwrap();
        assert_eq!(dom.children(root), &[a]);
    }

    #[test]
    fn insert_before_moving_earlier_sibling_lands_before_reference() {
        let mut dom = Dom::new();
        let root = dom.document();
        let a = dom.create_element("a");
        let b = dom.create_element("b");
        let c = dom.create_element("c");
        dom.append_child(root, a).unwrap();
        dom.append_child(root, b).unwrap();
        dom.append_child(root, c).unwrap();
        // Move `a` (currently first) to just before `c`; must land before `c`.
        dom.insert_before(root, a, Some(c)).unwrap();
        assert_eq!(dom.children(root), &[b, a, c]);
    }

    #[test]
    fn insert_before_self_is_noop() {
        let mut dom = Dom::new();
        let root = dom.document();
        let a = dom.create_element("a");
        let b = dom.create_element("b");
        dom.append_child(root, a).unwrap();
        dom.append_child(root, b).unwrap();
        dom.insert_before(root, a, Some(a)).unwrap();
        assert_eq!(dom.children(root), &[a, b]);
    }

    #[test]
    fn insert_before_foreign_reference_errors() {
        let mut dom = Dom::new();
        let root = dom.document();
        let a = dom.create_element("a");
        let stranger = dom.create_element("s");
        let x = dom.create_element("x");
        dom.append_child(root, a).unwrap();
        assert_eq!(dom.insert_before(root, x, Some(stranger)), Err(DomError::NotAChild));
    }

    #[test]
    fn remove_child_detaches() {
        let mut dom = Dom::new();
        let root = dom.document();
        let a = dom.create_element("a");
        dom.append_child(root, a).unwrap();
        dom.remove_child(root, a).unwrap();
        assert_eq!(dom.children(root), &[]);
        assert_eq!(dom.parent(a), None);
    }

    #[test]
    fn remove_child_of_wrong_parent_errors() {
        let mut dom = Dom::new();
        let root = dom.document();
        let a = dom.create_element("a");
        let stranger = dom.create_element("s");
        dom.append_child(root, a).unwrap();
        assert_eq!(dom.remove_child(a, stranger), Err(DomError::NotAChild));
    }

    #[test]
    fn replace_child_swaps_in_place() {
        let mut dom = Dom::new();
        let root = dom.document();
        let a = dom.create_element("a");
        let b = dom.create_element("b");
        let n = dom.create_element("n");
        dom.append_child(root, a).unwrap();
        dom.append_child(root, b).unwrap();
        dom.replace_child(root, n, a).unwrap();
        assert_eq!(dom.children(root), &[n, b]);
        assert_eq!(dom.parent(a), None);
        assert_eq!(dom.parent(n), Some(root));
    }

    #[test]
    fn append_cycle_is_rejected() {
        let mut dom = Dom::new();
        let a = dom.create_element("a");
        let b = dom.create_element("b");
        dom.append_child(a, b).unwrap();
        // Making `a` a child of its own descendant `b` must fail.
        assert_eq!(dom.append_child(b, a), Err(DomError::Hierarchy));
    }

    #[test]
    fn siblings_report_neighbors() {
        let mut dom = Dom::new();
        let root = dom.document();
        let a = dom.create_element("a");
        let b = dom.create_element("b");
        dom.append_child(root, a).unwrap();
        dom.append_child(root, b).unwrap();
        assert_eq!(dom.next_sibling(a), Some(b));
        assert_eq!(dom.previous_sibling(b), Some(a));
        assert_eq!(dom.next_sibling(b), None);
        assert_eq!(dom.previous_sibling(a), None);
    }
}
