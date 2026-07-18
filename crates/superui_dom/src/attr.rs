use crate::node::{NodeId, NodeKind};
use crate::tree::{Dom, DomError};

impl Dom {
    /// Set an attribute (overwriting) on an element. `class`/`id` are ordinary
    /// attributes here; `classList` reads/writes the `class` attribute.
    pub fn set_attribute(&mut self, id: NodeId, name: &str, value: &str) -> Result<(), DomError> {
        let node = self.get_mut(id).ok_or(DomError::NotFound)?;
        let NodeKind::Element(el) = &mut node.kind else { return Err(DomError::NotAnElement) };
        let name = name.to_ascii_lowercase();
        match el.attrs.iter_mut().find(|(n, _)| *n == name) {
            Some(slot) => slot.1 = value.to_string(),
            None => el.attrs.push((name, value.to_string())),
        }
        Ok(())
    }

    /// Attribute value, if present on an element.
    pub fn get_attribute(&self, id: NodeId, name: &str) -> Option<&str> {
        let NodeKind::Element(el) = &self.get(id)?.kind else { return None };
        let name = name.to_ascii_lowercase();
        el.attrs.iter().find(|(n, _)| *n == name).map(|(_, v)| v.as_str())
    }

    /// Whether an element carries the attribute.
    pub fn has_attribute(&self, id: NodeId, name: &str) -> bool {
        self.get_attribute(id, name).is_some()
    }

    /// Remove an attribute (no-op if absent or not an element).
    pub fn remove_attribute(&mut self, id: NodeId, name: &str) {
        if let Some(node) = self.get_mut(id) {
            if let NodeKind::Element(el) = &mut node.kind {
                let name = name.to_ascii_lowercase();
                el.attrs.retain(|(n, _)| *n != name);
            }
        }
    }

    /// Classes parsed from the `class` attribute (whitespace-separated).
    pub fn classes(&self, id: NodeId) -> Vec<String> {
        self.get_attribute(id, "class")
            .map(|c| c.split_whitespace().map(str::to_string).collect())
            .unwrap_or_default()
    }

    /// Whether `class` is present.
    pub fn class_contains(&self, id: NodeId, class: &str) -> bool {
        self.classes(id).iter().any(|c| c == class)
    }

    /// Add a class (idempotent). Rewrites the `class` attribute.
    pub fn class_add(&mut self, id: NodeId, class: &str) {
        let mut classes = self.classes(id);
        if !classes.iter().any(|c| c == class) {
            classes.push(class.to_string());
            let _ = self.set_attribute(id, "class", &classes.join(" "));
        }
    }

    /// Remove a class if present. Rewrites the `class` attribute.
    pub fn class_remove(&mut self, id: NodeId, class: &str) {
        let mut classes = self.classes(id);
        let before = classes.len();
        classes.retain(|c| c != class);
        if classes.len() != before {
            let _ = self.set_attribute(id, "class", &classes.join(" "));
        }
    }

    /// Toggle a class. Returns `true` if the class is present afterwards.
    pub fn class_toggle(&mut self, id: NodeId, class: &str) -> bool {
        if self.class_contains(id, class) {
            self.class_remove(id, class);
            false
        } else {
            self.class_add(id, class);
            true
        }
    }

    /// Concatenated text of a node and its descendants (document order).
    pub fn text_content(&self, id: NodeId) -> String {
        let Some(node) = self.get(id) else { return String::new() };
        match &node.kind {
            NodeKind::Text(s) => s.clone(),
            _ => {
                let mut out = String::new();
                for &child in node.children() {
                    out.push_str(&self.text_content(child));
                }
                out
            }
        }
    }

    /// Replace a node's children with a single text node (or clear if empty).
    /// On a text node, sets its data directly.
    pub fn set_text_content(&mut self, id: NodeId, text: &str) {
        let Some(node) = self.get(id) else { return };
        if let NodeKind::Text(_) = node.kind {
            if let Some(n) = self.get_mut(id) {
                n.kind = NodeKind::Text(text.to_string());
            }
            return;
        }
        let existing: Vec<NodeId> = self.children(id).to_vec();
        for c in existing {
            let _ = self.remove_child(id, c);
        }
        if !text.is_empty() {
            let t = self.create_text(text);
            let _ = self.append_child(id, t);
        }
    }

    /// First element (document order) whose `id` attribute equals `id_value`.
    pub fn get_element_by_id(&self, id_value: &str) -> Option<NodeId> {
        fn walk(dom: &Dom, node: NodeId, target: &str) -> Option<NodeId> {
            if dom.get_attribute(node, "id") == Some(target) {
                return Some(node);
            }
            for &child in dom.children(node) {
                if let Some(found) = walk(dom, child, target) {
                    return Some(found);
                }
            }
            None
        }
        walk(self, self.document(), id_value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_get_remove_attribute() {
        let mut dom = Dom::new();
        let el = dom.create_element("input");
        dom.set_attribute(el, "type", "checkbox").unwrap();
        assert_eq!(dom.get_attribute(el, "type"), Some("checkbox"));
        assert!(dom.has_attribute(el, "type"));
        dom.remove_attribute(el, "type");
        assert_eq!(dom.get_attribute(el, "type"), None);
        assert!(!dom.has_attribute(el, "type"));
    }

    #[test]
    fn set_attribute_overwrites() {
        let mut dom = Dom::new();
        let el = dom.create_element("div");
        dom.set_attribute(el, "id", "a").unwrap();
        dom.set_attribute(el, "id", "b").unwrap();
        assert_eq!(dom.get_attribute(el, "id"), Some("b"));
    }

    #[test]
    fn set_attribute_on_non_element_errors() {
        let mut dom = Dom::new();
        let t = dom.create_text("x");
        assert_eq!(dom.set_attribute(t, "id", "a"), Err(DomError::NotAnElement));
    }

    #[test]
    fn classlist_derives_from_class_attribute() {
        let mut dom = Dom::new();
        let el = dom.create_element("div");
        dom.set_attribute(el, "class", "  foo   bar ").unwrap();
        assert_eq!(dom.classes(el), vec!["foo".to_string(), "bar".to_string()]);
        assert!(dom.class_contains(el, "foo"));
        assert!(!dom.class_contains(el, "baz"));
    }

    #[test]
    fn classlist_mutations_rewrite_class_attribute() {
        let mut dom = Dom::new();
        let el = dom.create_element("div");
        dom.class_add(el, "a");
        dom.class_add(el, "b");
        dom.class_add(el, "a"); // idempotent
        assert_eq!(dom.get_attribute(el, "class"), Some("a b"));
        dom.class_remove(el, "a");
        assert_eq!(dom.get_attribute(el, "class"), Some("b"));
        assert_eq!(dom.class_toggle(el, "b"), false); // was present -> removed
        assert_eq!(dom.class_toggle(el, "c"), true); // was absent -> added
        assert_eq!(dom.classes(el), vec!["c".to_string()]);
    }

    #[test]
    fn text_content_reads_and_concatenates_descendants() {
        let mut dom = Dom::new();
        let root = dom.document();
        let p = dom.create_element("p");
        let t1 = dom.create_text("Hello, ");
        let span = dom.create_element("span");
        let t2 = dom.create_text("world");
        dom.append_child(root, p).unwrap();
        dom.append_child(p, t1).unwrap();
        dom.append_child(p, span).unwrap();
        dom.append_child(span, t2).unwrap();
        assert_eq!(dom.text_content(p), "Hello, world");
    }

    #[test]
    fn set_text_content_replaces_children_with_one_text_node() {
        let mut dom = Dom::new();
        let p = dom.create_element("p");
        let old = dom.create_element("span");
        dom.append_child(p, old).unwrap();
        dom.set_text_content(p, "just text");
        assert_eq!(dom.text_content(p), "just text");
        assert_eq!(dom.children(p).len(), 1);
        assert_eq!(dom.parent(old), None);
    }

    #[test]
    fn set_text_content_empty_clears_children() {
        let mut dom = Dom::new();
        let p = dom.create_element("p");
        let c = dom.create_text("x");
        dom.append_child(p, c).unwrap();
        dom.set_text_content(p, "");
        assert_eq!(dom.children(p).len(), 0);
        assert_eq!(dom.text_content(p), "");
    }

    #[test]
    fn get_element_by_id_finds_first_match() {
        let mut dom = Dom::new();
        let root = dom.document();
        let a = dom.create_element("div");
        let b = dom.create_element("div");
        dom.set_attribute(b, "id", "target").unwrap();
        dom.append_child(root, a).unwrap();
        dom.append_child(a, b).unwrap();
        assert_eq!(dom.get_element_by_id("target"), Some(b));
        assert_eq!(dom.get_element_by_id("missing"), None);
    }
}
