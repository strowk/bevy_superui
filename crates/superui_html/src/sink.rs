use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashSet;

use html5ever::driver::{parse_document as parse_html_document, ParseOpts};
use html5ever::interface::{ElementFlags, NodeOrText, QuirksMode, TreeSink};
use html5ever::tendril::{StrTendril, TendrilSink};
use html5ever::{Attribute, LocalName, Namespace, QualName};

use superui_dom::{Dom, NodeId, NodeKind};

/// A handle to a node in the DOM being built. Carries the arena id plus the
/// element's qualified name, so `elem_name` can hand html5ever a borrowed
/// `&QualName` (which implements `ElemName`) without borrowing through the
/// `RefCell`. Non-element handles (document/text/comment) get a placeholder name.
#[derive(Clone)]
pub(crate) struct Handle {
    pub(crate) id: NodeId,
    name: QualName,
}

/// A `QualName` for nodes that are not elements (never meaningfully queried).
fn placeholder_name() -> QualName {
    QualName::new(None, Namespace::from(""), LocalName::from(""))
}

/// html5ever tree sink that builds a [`superui_dom::Dom`].
///
/// All `TreeSink` methods take `&self`, so the mutable DOM lives behind a
/// `RefCell`. Comment / doctype / processing-instruction nodes are outside our
/// DOM subset: they get a throwaway detached node whose id is recorded in
/// `ignored`, and append operations skip ignored children so they never enter
/// the rendered tree.
pub(crate) struct HtmlSink {
    dom: RefCell<Dom>,
    ignored: RefCell<HashSet<NodeId>>,
}

impl HtmlSink {
    pub(crate) fn new() -> Self {
        HtmlSink {
            dom: RefCell::new(Dom::new()),
            ignored: RefCell::new(HashSet::new()),
        }
    }

    fn is_ignored(&self, id: NodeId) -> bool {
        self.ignored.borrow().contains(&id)
    }

    /// Allocate a detached placeholder node for a dropped comment/PI and record
    /// it as ignored. Returns a handle html5ever can keep referencing safely.
    fn ignored_handle(&self) -> Handle {
        let id = self.dom.borrow_mut().create_text("");
        self.ignored.borrow_mut().insert(id);
        Handle { id, name: placeholder_name() }
    }

    /// Append `text` as a child of `parent`, merging into a trailing text
    /// sibling if present (html5ever expects adjacent text to coalesce).
    fn append_text(&self, parent: NodeId, text: &str) {
        let mut dom = self.dom.borrow_mut();
        if let Some(&last) = dom.children(parent).last() {
            if let Some(node) = dom.get_mut(last) {
                if let NodeKind::Text(s) = &mut node.kind {
                    s.push_str(text);
                    return;
                }
            }
        }
        let t = dom.create_text(text);
        let _ = dom.append_child(parent, t);
    }
}

impl TreeSink for HtmlSink {
    type Handle = Handle;
    type Output = Dom;
    type ElemName<'a> = &'a QualName where Self: 'a;

    fn finish(self) -> Dom {
        self.dom.into_inner()
    }

    fn parse_error(&self, _msg: Cow<'static, str>) {}

    fn get_document(&self) -> Handle {
        let id = self.dom.borrow().document();
        Handle { id, name: placeholder_name() }
    }

    fn elem_name<'a>(&'a self, target: &'a Handle) -> &'a QualName {
        &target.name
    }

    fn create_element(&self, name: QualName, attrs: Vec<Attribute>, _flags: ElementFlags) -> Handle {
        let mut dom = self.dom.borrow_mut();
        let id = dom.create_element(&name.local);
        for attr in attrs {
            let _ = dom.set_attribute(id, &attr.name.local, &attr.value);
        }
        Handle { id, name }
    }

    fn create_comment(&self, _text: StrTendril) -> Handle {
        self.ignored_handle()
    }

    fn create_pi(&self, _target: StrTendril, _data: StrTendril) -> Handle {
        self.ignored_handle()
    }

    fn append(&self, parent: &Handle, child: NodeOrText<Handle>) {
        match child {
            NodeOrText::AppendNode(node) => {
                if self.is_ignored(node.id) {
                    return;
                }
                let _ = self.dom.borrow_mut().append_child(parent.id, node.id);
            }
            NodeOrText::AppendText(text) => self.append_text(parent.id, &text),
        }
    }

    fn append_based_on_parent_node(
        &self,
        element: &Handle,
        prev_element: &Handle,
        child: NodeOrText<Handle>,
    ) {
        let has_parent = self.dom.borrow().parent(element.id).is_some();
        if has_parent {
            self.append_before_sibling(element, child);
        } else {
            self.append(prev_element, child);
        }
    }

    fn append_doctype_to_document(&self, _name: StrTendril, _public: StrTendril, _system: StrTendril) {}

    fn get_template_contents(&self, target: &Handle) -> Handle {
        // Our subset has no separate template document fragment; template
        // children just live under the element itself (graceful degradation).
        target.clone()
    }

    fn same_node(&self, x: &Handle, y: &Handle) -> bool {
        x.id == y.id
    }

    fn set_quirks_mode(&self, _mode: QuirksMode) {}

    fn append_before_sibling(&self, sibling: &Handle, new_node: NodeOrText<Handle>) {
        // Copy the parent out first so the immutable borrow is dropped before we
        // take a mutable borrow in the arms below.
        let parent = self.dom.borrow().parent(sibling.id);
        let parent = match parent {
            Some(p) => p,
            None => return, // detached sibling; nothing to do
        };
        match new_node {
            NodeOrText::AppendNode(node) => {
                if self.is_ignored(node.id) {
                    return;
                }
                let _ = self
                    .dom
                    .borrow_mut()
                    .insert_before(parent, node.id, Some(sibling.id));
            }
            NodeOrText::AppendText(text) => {
                let mut dom = self.dom.borrow_mut();
                if let Some(prev) = dom.previous_sibling(sibling.id) {
                    if let Some(node) = dom.get_mut(prev) {
                        if let NodeKind::Text(s) = &mut node.kind {
                            s.push_str(&text);
                            return;
                        }
                    }
                }
                let t = dom.create_text(&text);
                let _ = dom.insert_before(parent, t, Some(sibling.id));
            }
        }
    }

    fn add_attrs_if_missing(&self, target: &Handle, attrs: Vec<Attribute>) {
        let mut dom = self.dom.borrow_mut();
        for attr in attrs {
            if dom.get_attribute(target.id, &attr.name.local).is_none() {
                let _ = dom.set_attribute(target.id, &attr.name.local, &attr.value);
            }
        }
    }

    fn remove_from_parent(&self, target: &Handle) {
        let mut dom = self.dom.borrow_mut();
        if let Some(parent) = dom.parent(target.id) {
            let _ = dom.remove_child(parent, target.id);
        }
    }

    fn reparent_children(&self, node: &Handle, new_parent: &Handle) {
        let mut dom = self.dom.borrow_mut();
        let kids: Vec<NodeId> = dom.children(node.id).to_vec();
        for child in kids {
            // append_child detaches from the old parent, preserving order.
            let _ = dom.append_child(new_parent.id, child);
        }
    }
}

/// Parse a full HTML document into a fresh [`superui_dom::Dom`].
///
/// Produces the implied `html > head + body` structure like a browser. Unknown
/// tags become plain elements; comments, the doctype, and processing
/// instructions are dropped (outside our DOM subset). Never panics on malformed
/// input.
pub fn parse_document(html: &str) -> Dom {
    let sink = HtmlSink::new();
    parse_html_document(sink, ParseOpts::default()).one(html)
}
