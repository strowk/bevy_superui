use crate::node::NodeId;
use crate::tree::Dom;

/// A single compound selector: an optional type, an optional id, and zero or
/// more required classes (e.g. `input.toggle` → tag=input, classes=[toggle]).
struct Compound {
    tag: Option<String>,
    id: Option<String>,
    classes: Vec<String>,
}

/// Parse one compound selector (no combinators). Returns `None` if malformed.
fn parse_compound(sel: &str) -> Option<Compound> {
    let mut tag = None;
    let mut id = None;
    let mut classes = Vec::new();

    let mut rest = sel;
    // Optional leading type selector (runs until the first '.' or '#').
    let type_end = rest.find(['.', '#']).unwrap_or(rest.len());
    if type_end > 0 {
        let t = &rest[..type_end];
        if t != "*" {
            tag = Some(t.to_ascii_lowercase());
        }
        rest = &rest[type_end..];
    }
    // Then a sequence of `.class` / `#id` tokens.
    while !rest.is_empty() {
        let marker = rest.as_bytes()[0];
        let after = &rest[1..];
        let end = after.find(['.', '#']).unwrap_or(after.len());
        let name = &after[..end];
        if name.is_empty() {
            return None;
        }
        match marker {
            b'.' => classes.push(name.to_string()),
            b'#' => id = Some(name.to_string()),
            _ => return None,
        }
        rest = &after[end..];
    }
    Some(Compound { tag, id, classes })
}

/// Parse a full selector into its whitespace-separated compound sequence.
fn parse_selector(selector: &str) -> Option<Vec<Compound>> {
    let compounds: Option<Vec<Compound>> =
        selector.split_whitespace().map(parse_compound).collect();
    match compounds {
        Some(v) if !v.is_empty() => Some(v),
        _ => None,
    }
}

/// Whether `node` (an element) matches a single compound selector.
fn matches_compound(dom: &Dom, node: NodeId, c: &Compound) -> bool {
    if !dom.is_element(node) {
        return false;
    }
    if let Some(t) = &c.tag {
        if dom.tag(node) != Some(t.as_str()) {
            return false;
        }
    }
    if let Some(id) = &c.id {
        if dom.get_attribute(node, "id") != Some(id.as_str()) {
            return false;
        }
    }
    for class in &c.classes {
        if !dom.class_contains(node, class) {
            return false;
        }
    }
    true
}

/// Whether `node` matches the full descendant-combinator selector: it must match
/// the rightmost compound, and the preceding compounds must match ancestors in
/// order (not necessarily contiguous).
fn matches_selector(dom: &Dom, node: NodeId, compounds: &[Compound]) -> bool {
    let last = compounds.len() - 1;
    if !matches_compound(dom, node, &compounds[last]) {
        return false;
    }
    if last == 0 {
        return true;
    }
    // Match compounds[last-1..=0] up the ancestor chain, right to left.
    let mut ci = last - 1;
    let mut cur = dom.parent(node);
    while let Some(a) = cur {
        if matches_compound(dom, a, &compounds[ci]) {
            if ci == 0 {
                return true;
            }
            ci -= 1;
        }
        cur = dom.parent(a);
    }
    false
}

impl Dom {
    /// All elements in `root`'s subtree (excluding `root`) matching `selector`,
    /// in document order. Empty for an unparseable selector.
    pub fn query_selector_all(&self, root: NodeId, selector: &str) -> Vec<NodeId> {
        let Some(compounds) = parse_selector(selector) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        fn walk(dom: &Dom, node: NodeId, compounds: &[Compound], out: &mut Vec<NodeId>) {
            for &child in dom.children(node) {
                if matches_selector(dom, child, compounds) {
                    out.push(child);
                }
                walk(dom, child, compounds, out);
            }
        }
        walk(self, root, &compounds, &mut out);
        out
    }

    /// The first element (document order) in `root`'s subtree matching
    /// `selector`, or `None`.
    pub fn query_selector(&self, root: NodeId, selector: &str) -> Option<NodeId> {
        self.query_selector_all(root, selector).into_iter().next()
    }
}

#[cfg(test)]
mod tests {
    use crate::Dom;

    /// Build: document > section.todoapp > ul.todo-list > (li.completed > label, li > label)
    fn fixture() -> (Dom, super::NodeId, super::NodeId, super::NodeId) {
        let mut dom = Dom::new();
        let doc = dom.document();
        let section = dom.create_element("section");
        dom.set_attribute(section, "class", "todoapp").unwrap();
        let ul = dom.create_element("ul");
        dom.set_attribute(ul, "class", "todo-list").unwrap();
        let li1 = dom.create_element("li");
        dom.set_attribute(li1, "class", "completed").unwrap();
        let label1 = dom.create_element("label");
        let li2 = dom.create_element("li");
        let label2 = dom.create_element("label");
        dom.append_child(doc, section).unwrap();
        dom.append_child(section, ul).unwrap();
        dom.append_child(ul, li1).unwrap();
        dom.append_child(li1, label1).unwrap();
        dom.append_child(ul, li2).unwrap();
        dom.append_child(li2, label2).unwrap();
        (dom, section, li1, li2)
    }

    #[test]
    fn type_class_id_selectors() {
        let (mut dom, section, _, _) = fixture();
        dom.set_attribute(section, "id", "app").unwrap();
        let root = dom.document();
        assert_eq!(dom.query_selector(root, "section"), Some(section));
        assert_eq!(dom.query_selector(root, ".todoapp"), Some(section));
        assert_eq!(dom.query_selector(root, "#app"), Some(section));
        assert_eq!(dom.query_selector(root, "section.todoapp"), Some(section));
        assert_eq!(dom.query_selector(root, ".nope"), None);
    }

    #[test]
    fn descendant_combinator_and_query_all() {
        let (dom, _, li1, li2) = fixture();
        let root = dom.document();
        let lis = dom.query_selector_all(root, ".todo-list li");
        assert_eq!(lis, vec![li1, li2]);
        assert_eq!(dom.query_selector(root, "li.completed"), Some(li1));
        assert_eq!(dom.query_selector_all(root, "label").len(), 2);
    }

    #[test]
    fn scoped_to_root_excludes_root_itself() {
        let (dom, section, _, _) = fixture();
        // Searching within `section` never returns `section`.
        assert_eq!(dom.query_selector(section, "section"), None);
        assert!(dom.query_selector(section, ".todo-list").is_some());
    }

    #[test]
    fn malformed_selector_yields_nothing() {
        let (dom, _, _, _) = fixture();
        let root = dom.document();
        assert_eq!(dom.query_selector_all(root, ".").len(), 0);
        assert_eq!(dom.query_selector(root, ""), None);
    }
}
