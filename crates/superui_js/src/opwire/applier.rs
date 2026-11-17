//! Applies a decoded [`OpBatch`] to a render-mirror [`superui_dom::Dom`],
//! translating [`JsNodeId`]s through an [`IdMap`].

use std::collections::{HashMap, HashSet};

use superui_dom::{Dom, ListenerId, NodeId};

use crate::opwire::{IdMap, JsNodeId, Op, OpBatch};

/// Global counts of live mouse listeners, by [`ListenerKind`] wire code
/// (1..=7), tracked by the [`OpApplier`] from [`Op::SetListener`] traffic.
///
/// Counted at the op level, not per-node: a document/window sentinel id (used
/// by a later task for e.g. a document-wide `mousemove`) has no render-mirror
/// node to key a per-node set by, but still has to count here.
#[derive(Clone, Copy, Default, Debug)]
pub struct MouseListenerCounts {
    counts: [u32; 7],
}

impl MouseListenerCounts {
    /// Any mouse listener at all is registered anywhere.
    pub fn any(&self) -> bool {
        self.counts.iter().any(|&c| c > 0)
    }

    /// Any of the hover-family listeners (mouseover/out/enter/leave, codes
    /// 1..=4) is registered anywhere.
    pub fn any_hover(&self) -> bool {
        self.counts[0..4].iter().any(|&c| c > 0)
    }

    /// Any of the button-family listeners (mousedown/up, codes 5..=6) is
    /// registered anywhere.
    pub fn any_button(&self) -> bool {
        self.counts[4..6].iter().any(|&c| c > 0)
    }

    /// A mousemove listener (code 7) is registered anywhere.
    pub fn any_move(&self) -> bool {
        self.counts[6] > 0
    }
}

/// Replays an [`OpBatch`]'s recorded mutations onto a [`Dom`], resolving
/// each op's [`JsNodeId`]s through an owned [`IdMap`].
///
/// Ops referencing an id with no (or a stale) binding are skipped silently —
/// the JS side and the render-mirror `Dom` can transiently disagree (e.g. a
/// node removed and then referenced by a queued-but-now-stale op), and this
/// is not a bug worth panicking over.
pub struct OpApplier {
    map: IdMap,
    /// Synthetic marker listeners keyed by node, driven by [`Op::SetListener`].
    /// A node carries one exactly while its JS shadow node has at least one
    /// listener (of any kind — see `kinds_per_node`), so `reconcile.rs`'s
    /// `dom.listeners(node).is_empty()` picking check tracks interactivity
    /// without the reconciler knowing about the op-wire.
    listeners: HashMap<NodeId, ListenerId>,
    /// Union of live listener kind codes per node, used to decide whether the
    /// `"__ss"` marker should still be present: a node with two different
    /// listener kinds must keep reading as interactive when only one of them
    /// is removed.
    kinds_per_node: HashMap<NodeId, HashSet<u8>>,
    /// Global per-kind mouse listener counts; see [`MouseListenerCounts`].
    mouse_counts: MouseListenerCounts,
}

impl OpApplier {
    /// Creates an applier with the root pre-bound: jsId `1 -> root_node`.
    pub fn new(root_node: NodeId) -> Self {
        OpApplier {
            map: IdMap::new(root_node),
            listeners: HashMap::new(),
            kinds_per_node: HashMap::new(),
            mouse_counts: MouseListenerCounts::default(),
        }
    }

    /// A copy of the current global mouse-listener counts.
    pub fn mouse_listener_counts(&self) -> MouseListenerCounts {
        self.mouse_counts
    }

    /// The [`NodeId`] currently bound to `js`, if any.
    pub fn node(&self, js: JsNodeId) -> Option<NodeId> {
        self.map.node(js)
    }

    /// The [`JsNodeId`] bound to a render-mirror `node`, if any (reverse of
    /// [`OpApplier::node`]). The bridge uses it to route an input hit on a
    /// mirror node back to the shadow-DOM node the engine dispatches against.
    pub fn js(&self, node: NodeId) -> Option<JsNodeId> {
        self.map.js(node)
    }

    /// Applies every op in `batch`, in order.
    pub fn apply(&mut self, dom: &mut Dom, batch: &OpBatch) {
        // Ids detached by a `RemoveChild` this batch; unbinding is deferred (below).
        let mut removed: Vec<JsNodeId> = Vec::new();
        for op in &batch.ops {
            self.apply_op(dom, batch, op, &mut removed);
        }
        // A node a `RemoveChild` detached may be re-attached by a later
        // `InsertBefore` in the SAME batch: the identity-keyed list reconcile
        // (`render.js` `reconcileArrays`) moves a node forward via
        // `replaceChild(new, old)`, detaching `old` even though it reappears later
        // in the new order. Unbinding on `RemoveChild` stranded that re-insert (its
        // id no longer resolved, so the op was dropped and the node vanished from
        // the render mirror). Defer the unbind here and apply it only to nodes left
        // detached.
        for js in removed {
            if let Some(node) = self.map.node(js) {
                if dom.parent(node).is_none() {
                    self.map.unbind(js);
                    self.listeners.remove(&node);
                    self.kinds_per_node.remove(&node);
                }
            }
        }
    }

    fn apply_op(&mut self, dom: &mut Dom, batch: &OpBatch, op: &Op, removed: &mut Vec<JsNodeId>) {
        match *op {
            Op::CreateElement { id, tag } => {
                let node = dom.create_element(batch.resolve(tag));
                self.map.bind(id, node);
            }
            Op::CreateText { id, data } => {
                let node = dom.create_text(batch.resolve(data));
                self.map.bind(id, node);
            }
            Op::SetAttribute { id, name, value } => {
                let Some(node) = self.map.node(id) else { return };
                let _ = dom.set_attribute(node, batch.resolve(name), batch.resolve(value));
            }
            Op::RemoveAttribute { id, name } => {
                let Some(node) = self.map.node(id) else { return };
                dom.remove_attribute(node, batch.resolve(name));
            }
            Op::SetProperty { id, name, value } => {
                let Some(node) = self.map.node(id) else { return };
                let value = batch.resolve(value);
                match batch.resolve(name) {
                    "value" => dom.set_value(node, value),
                    "checked" => dom.set_checked(node, value == "true"),
                    // Only `value`/`checked` are real IDL props here; anything
                    // else has no live-property counterpart on this Dom.
                    _ => {}
                }
            }
            Op::SetStyle { id, prop, value } => {
                let Some(node) = self.map.node(id) else { return };
                let merged =
                    merge_style(dom.get_attribute(node, "style"), batch.resolve(prop), batch.resolve(value));
                let _ = dom.set_attribute(node, "style", &merged);
            }
            Op::SetText { id, data } => {
                let Some(node) = self.map.node(id) else { return };
                dom.set_text_content(node, batch.resolve(data));
            }
            Op::InsertBefore { parent, node, reference } => {
                let Some(parent) = self.map.node(parent) else { return };
                let Some(child) = self.map.node(node) else { return };
                let reference = match reference {
                    0 => None,
                    r => match self.map.node(r) {
                        Some(r) => Some(r),
                        None => return,
                    },
                };
                let _ = dom.insert_before(parent, child, reference);
            }
            Op::RemoveChild { parent, node } => {
                let Some(parent) = self.map.node(parent) else { return };
                let Some(child) = self.map.node(node) else { return };
                if dom.remove_child(parent, child).is_ok() {
                    // Unbind is deferred to batch end so a re-insert this batch survives.
                    removed.push(node);
                }
            }
            Op::SetListener { id, kind, has_listener } => {
                let code = kind.code();
                // Global mouse counts update regardless of whether `id` maps to a
                // render-mirror node: document/window sentinel ids don't, but
                // still have to count (a later task dispatches off these).
                if (1..=7).contains(&code) {
                    let slot = &mut self.mouse_counts.counts[(code - 1) as usize];
                    if has_listener {
                        *slot += 1;
                    } else {
                        *slot = slot.saturating_sub(1);
                    }
                }
                let Some(node) = self.map.node(id) else { return };
                // Track the union of kinds live on this node, so removing one
                // kind doesn't tear down the marker while another remains.
                let set = self.kinds_per_node.entry(node).or_default();
                if has_listener {
                    set.insert(code);
                } else {
                    set.remove(&code);
                }
                let interactive = !set.is_empty();
                if set.is_empty() {
                    self.kinds_per_node.remove(&node);
                }
                // Keep exactly one synthetic "__ss" marker while the node has
                // any listener, driving `reconcile.rs`'s picking check.
                let has_marker = self.listeners.contains_key(&node);
                if interactive && !has_marker {
                    if let Some(lid) = dom.add_event_listener(node, "__ss", false) {
                        self.listeners.insert(node, lid);
                    }
                } else if !interactive && has_marker {
                    if let Some(lid) = self.listeners.remove(&node) {
                        dom.remove_event_listener(node, lid);
                    }
                }
            }
        }
    }
}

/// Merges a single `prop:value` declaration into an inline `style` attribute
/// string, overwriting `prop` if already present and preserving the order of
/// the other declarations.
fn merge_style(existing: Option<&str>, prop: &str, value: &str) -> String {
    let mut decls: Vec<(&str, &str)> = existing
        .unwrap_or("")
        .split(';')
        .filter_map(|part| part.split_once(':'))
        .map(|(k, v)| (k.trim(), v.trim()))
        .filter(|(k, _)| !k.is_empty())
        .collect();
    match decls.iter_mut().find(|(k, _)| *k == prop) {
        Some(slot) => slot.1 = value,
        None => decls.push((prop, value)),
    }
    decls.iter().map(|(k, v)| format!("{k}:{v}")).collect::<Vec<_>>().join(";")
}

#[cfg(test)]
mod tests {
    use superui_dom::Dom;

    use crate::opwire::{ListenerKind, Op, OpBatch};

    use super::super::OpApplier;
    use super::merge_style;

    #[test]
    fn merge_style_overwrites_one_prop_among_several_preserving_order() {
        let merged = merge_style(Some("color:red;font-size:12px"), "color", "blue");
        assert_eq!(merged, "color:blue;font-size:12px");
    }

    #[test]
    fn applies_create_and_insert_into_dom() {
        let mut dom = Dom::new();
        let root = dom.document();
        let mut applier = OpApplier::new(root);
        let mut b = OpBatch::default();
        let div = b.intern("div");
        let id = b.intern("id");
        let v = b.intern("main");
        b.ops.push(Op::CreateElement { id: 2, tag: div });
        b.ops.push(Op::SetAttribute { id: 2, name: id, value: v });
        b.ops.push(Op::InsertBefore { parent: 1, node: 2, reference: 0 });
        applier.apply(&mut dom, &b);
        let node = applier.node(2).unwrap();
        assert_eq!(dom.children(root), vec![node]);
        assert_eq!(dom.get_attribute(node, "id").as_deref(), Some("main"));
    }

    #[test]
    fn stale_id_is_skipped_not_panicked() {
        let mut dom = Dom::new();
        let mut applier = OpApplier::new(dom.document());
        let mut b = OpBatch::default();
        b.ops.push(Op::RemoveChild { parent: 1, node: 999 }); // never created
        applier.apply(&mut dom, &b); // must not panic
    }

    #[test]
    fn applies_every_op_kind() {
        let mut dom = Dom::new();
        let root = dom.document();
        let mut applier = OpApplier::new(root);
        let mut b = OpBatch::default();
        let div = b.intern("div");
        let span = b.intern("span");
        let cls = b.intern("class");
        let row = b.intern("row");
        let value_name = b.intern("value");
        let val = b.intern("typed");
        let checked_name = b.intern("checked");
        let truth = b.intern("true");
        let color = b.intern("color");
        let red = b.intern("red");
        let hello = b.intern("hello");
        let world = b.intern("world");

        b.ops.push(Op::CreateElement { id: 2, tag: div });
        b.ops.push(Op::InsertBefore { parent: 1, node: 2, reference: 0 });
        b.ops.push(Op::SetAttribute { id: 2, name: cls, value: row });
        b.ops.push(Op::RemoveAttribute { id: 2, name: cls });
        b.ops.push(Op::SetProperty { id: 2, name: value_name, value: val });
        b.ops.push(Op::SetProperty { id: 2, name: checked_name, value: truth });
        b.ops.push(Op::SetStyle { id: 2, prop: color, value: red });

        b.ops.push(Op::CreateText { id: 3, data: hello });
        b.ops.push(Op::InsertBefore { parent: 2, node: 3, reference: 0 });
        b.ops.push(Op::SetText { id: 3, data: world });

        b.ops.push(Op::CreateElement { id: 4, tag: span });
        // Insert the span before the text node -> children become [span, text].
        b.ops.push(Op::InsertBefore { parent: 2, node: 4, reference: 3 });

        applier.apply(&mut dom, &b);

        let div_node = applier.node(2).unwrap();
        let text_node = applier.node(3).unwrap();
        let span_node = applier.node(4).unwrap();

        assert_eq!(dom.children(root), vec![div_node]);
        assert_eq!(dom.children(div_node), vec![span_node, text_node]);
        assert_eq!(dom.get_attribute(div_node, "class"), None);
        assert_eq!(dom.get_attribute(div_node, "style"), Some("color:red"));
        assert_eq!(dom.value(div_node), "typed");
        assert!(dom.checked(div_node));
        assert_eq!(dom.text_content(text_node), "world");

        let mut b2 = OpBatch::default();
        b2.ops.push(Op::RemoveChild { parent: 1, node: 2 });
        applier.apply(&mut dom, &b2);
        assert_eq!(dom.children(root).len(), 0);
        assert_eq!(applier.node(2), None);
    }

    #[test]
    fn applier_counts_mouse_kinds_and_keeps_interactive_marker() {
        let mut dom = Dom::new();
        let root = dom.document();
        let mut applier = OpApplier::new(root);
        let mut b = OpBatch::default();
        let div = b.intern("div");
        b.ops.push(Op::CreateElement { id: 2, tag: div });
        b.ops.push(Op::InsertBefore { parent: 1, node: 2, reference: 0 });
        applier.apply(&mut dom, &b);
        let node2 = applier.node(2).unwrap();

        // Register a mousemove listener on the element.
        let mut b2 = OpBatch::default();
        b2.ops.push(Op::SetListener { id: 2, kind: ListenerKind::MouseMove, has_listener: true });
        applier.apply(&mut dom, &b2);
        assert!(applier.mouse_listener_counts().any_move());
        assert!(applier.mouse_listener_counts().any());
        // The node must read as interactive (has a listener) for picking.
        assert!(!dom.listeners(node2).is_empty());

        // Add a second, different kind on the same node.
        let mut b3 = OpBatch::default();
        b3.ops.push(Op::SetListener { id: 2, kind: ListenerKind::MouseOver, has_listener: true });
        applier.apply(&mut dom, &b3);
        assert!(applier.mouse_listener_counts().any_hover());

        // Removing one kind must not tear down the marker while the other remains.
        let mut b4 = OpBatch::default();
        b4.ops.push(Op::SetListener { id: 2, kind: ListenerKind::MouseMove, has_listener: false });
        applier.apply(&mut dom, &b4);
        assert!(!applier.mouse_listener_counts().any_move());
        assert!(
            !dom.listeners(node2).is_empty(),
            "node still has a mouseover listener, so it must stay interactive"
        );

        // Removing the last kind tears the marker down.
        let mut b5 = OpBatch::default();
        b5.ops.push(Op::SetListener { id: 2, kind: ListenerKind::MouseOver, has_listener: false });
        applier.apply(&mut dom, &b5);
        assert!(!applier.mouse_listener_counts().any());
        assert!(dom.listeners(node2).is_empty());
    }
}
