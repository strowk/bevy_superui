use crate::node::{NodeId, NodeKind};
use crate::tree::Dom;

impl Dom {
    /// The `.value` IDL property: the live value, defaulting to the `value`
    /// attribute until JS explicitly sets it. Empty string for non-elements.
    pub fn value(&self, id: NodeId) -> String {
        let Some(NodeKind::Element(el)) = self.get(id).map(|n| &n.kind) else {
            return String::new();
        };
        if let Some(v) = &el.value {
            return v.clone();
        }
        self.get_attribute(id, "value").unwrap_or("").to_string()
    }

    /// Set the `.value` IDL property (does not change the `value` attribute,
    /// mirroring browser input-value semantics; a Phase-1 simplification).
    pub fn set_value(&mut self, id: NodeId, value: &str) {
        if let Some(node) = self.get_mut(id) {
            if let NodeKind::Element(el) = &mut node.kind {
                el.value = Some(value.to_string());
            }
        }
    }

    /// The `.checked` IDL property: defaults to presence of the `checked`
    /// attribute until JS explicitly sets it.
    pub fn checked(&self, id: NodeId) -> bool {
        let Some(NodeKind::Element(el)) = self.get(id).map(|n| &n.kind) else {
            return false;
        };
        if let Some(c) = el.checked {
            return c;
        }
        self.has_attribute(id, "checked")
    }

    /// Set the `.checked` IDL property.
    pub fn set_checked(&mut self, id: NodeId, checked: bool) {
        if let Some(node) = self.get_mut(id) {
            if let NodeKind::Element(el) = &mut node.kind {
                el.checked = Some(checked);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::Dom;

    #[test]
    fn value_defaults_to_attribute_then_reflects_set() {
        let mut dom = Dom::new();
        let input = dom.create_element("input");
        assert_eq!(dom.value(input), "");
        dom.set_attribute(input, "value", "default").unwrap();
        assert_eq!(dom.value(input), "default");
        dom.set_value(input, "typed");
        assert_eq!(dom.value(input), "typed");
        // Setting the property does not rewrite the attribute.
        assert_eq!(dom.get_attribute(input, "value"), Some("default"));
    }

    #[test]
    fn checked_defaults_to_attribute_presence_then_reflects_set() {
        let mut dom = Dom::new();
        let input = dom.create_element("input");
        assert!(!dom.checked(input));
        dom.set_attribute(input, "checked", "").unwrap();
        assert!(dom.checked(input));
        dom.set_checked(input, false);
        assert!(!dom.checked(input));
        dom.set_checked(input, true);
        assert!(dom.checked(input));
    }

    #[test]
    fn value_and_checked_on_non_element_are_defaults() {
        let mut dom = Dom::new();
        let t = dom.create_text("x");
        assert_eq!(dom.value(t), "");
        assert!(!dom.checked(t));
    }
}
