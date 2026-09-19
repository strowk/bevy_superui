//! Applies a decoded [`OpBatch`] to a render-mirror [`superui_dom::Dom`],
//! translating [`JsNodeId`]s through an [`IdMap`].

use superui_dom::{Dom, NodeId};

use crate::opwire::{IdMap, JsNodeId, Op, OpBatch};

/// Replays an [`OpBatch`]'s recorded mutations onto a [`Dom`], resolving
/// each op's [`JsNodeId`]s through an owned [`IdMap`].
///
/// Ops referencing an id with no (or a stale) binding are skipped silently —
/// the JS side and the render-mirror `Dom` can transiently disagree (e.g. a
/// node removed and then referenced by a queued-but-now-stale op), and this
/// is not a bug worth panicking over.
pub struct OpApplier {
    map: IdMap,
}

impl OpApplier {
    /// Creates an applier with the root pre-bound: jsId `1 -> root_node`.
    pub fn new(root_node: NodeId) -> Self {
        OpApplier { map: IdMap::new(root_node) }
    }

    /// The [`NodeId`] currently bound to `js`, if any.
    pub fn node(&self, js: JsNodeId) -> Option<NodeId> {
        self.map.node(js)
    }

    /// Applies every op in `batch`, in order.
    pub fn apply(&mut self, dom: &mut Dom, batch: &OpBatch) {
        for op in &batch.ops {
            self.apply_op(dom, batch, op);
        }
    }

    fn apply_op(&mut self, dom: &mut Dom, batch: &OpBatch, op: &Op) {
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
                    self.map.unbind(node);
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

    use crate::opwire::{Op, OpBatch};

    use super::super::OpApplier;

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
}
