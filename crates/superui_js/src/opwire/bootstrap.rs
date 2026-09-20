//! Generates JS that rebuilds an initial [`superui_dom::Dom`] tree through
//! the shadow DOM's standard API (`js/dom.js`).
//!
//! Apps mount via `render(() => <App/>, document.getElementById("root"))`
//! against the document's initial HTML (e.g. `index.html`'s
//! `<div id="root"></div>`). Since the DOM lives JS-side, that HTML has to
//! exist in the JS shadow DOM before author JS runs. [`bootstrap_js`] emits a
//! script that recreates it via `document.createElement`/`createTextNode`,
//! `el.setAttribute`, and `appendChild` — the same calls author code makes —
//! so the shadow DOM records an ordinary op batch that the host flushes and
//! applies like any other frame.

use superui_dom::{Dom, NodeId, NodeKind};

/// Emits JS that recreates every descendant of `dom.document()` under the
/// global `__ss_root` (already bound to jsId `1`), using only
/// `document.createElement`/`createTextNode`, `el.setAttribute`, and
/// `appendChild`. The document node itself is never created, only its
/// children.
///
/// Returns a no-op script (safe to `eval`) when the document has no children.
pub fn bootstrap_js(dom: &Dom) -> String {
    let root = dom.document();
    let children = dom.children(root);
    if children.is_empty() {
        return "// bootstrap: empty document, nothing to hydrate\n".to_string();
    }
    let mut out = String::new();
    let mut next_var = 0u32;
    for &child in children {
        let var = emit_node(dom, child, &mut out, &mut next_var);
        out.push_str(&format!("__ss_root.appendChild({var});\n"));
    }
    out
}

/// Emits `id`'s creation — attributes and recursively-created children for an
/// element, or its data for a text node — into `out`. Returns the local var
/// name holding the created node; the caller appends it to its own parent.
fn emit_node(dom: &Dom, id: NodeId, out: &mut String, next_var: &mut u32) -> String {
    let var = format!("_n{}", *next_var);
    *next_var += 1;
    match dom.get(id).map(|n| &n.kind) {
        Some(NodeKind::Element(_)) => {
            let tag = dom.tag(id).unwrap_or("div");
            out.push_str(&format!("var {var} = document.createElement({});\n", js_string(tag)));
            for (name, value) in dom.attributes(id) {
                out.push_str(&format!(
                    "{var}.setAttribute({}, {});\n",
                    js_string(&name),
                    js_string(&value)
                ));
            }
            for &child in dom.children(id) {
                let child_var = emit_node(dom, child, out, next_var);
                out.push_str(&format!("{var}.appendChild({child_var});\n"));
            }
        }
        Some(NodeKind::Text(data)) => {
            out.push_str(&format!("var {var} = document.createTextNode({});\n", js_string(data)));
        }
        // Document or a stale handle can't occur as a child in practice; emit
        // an empty text node rather than invalid JS.
        _ => {
            out.push_str(&format!("var {var} = document.createTextNode({});\n", js_string("")));
        }
    }
    var
}

/// Encodes `s` as a JS string literal, reusing JSON's escaping rules (a valid
/// subset of JS string syntax).
fn js_string(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".to_string())
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use serde_json::json;
    use superui_dom::Dom;
    use superui_html::parse_document;

    use crate::opwire::OpApplier;
    use crate::{JsEngine, V8Engine};

    use super::bootstrap_js;

    #[test]
    fn bootstrap_rebuilds_tree_through_the_shadow_dom() {
        let src = parse_document("<div id=\"root\"><span class=\"x\">hi</span></div>");
        let script = bootstrap_js(&src);

        let shared = Rc::new(RefCell::new(Dom::new()));
        let mut engine = V8Engine::new(shared.clone());
        engine.eval(&script).expect("bootstrap script evaluates cleanly");

        // Readback via the same outbox the host drains every frame.
        engine
            .eval("__superui_bevy_send('ok', document.getElementById('root') !== null);")
            .expect("readback eval ok");
        assert_eq!(
            engine.drain_outbox(),
            vec![("ok".to_string(), json!(true))],
            "document.getElementById('root') should resolve after bootstrap"
        );

        // Flush the recorded ops and apply them to the render-mirror Dom, then
        // assert the reconstructed tree for real.
        let batch = engine.flush_ops();
        let root_node = shared.borrow().document();
        let mut applier = OpApplier::new(root_node);
        applier.apply(&mut shared.borrow_mut(), &batch);

        let dom = shared.borrow();
        let div = dom.get_element_by_id("root").expect("div#root exists in the render-mirror Dom");
        assert_eq!(dom.tag(div), Some("div"));

        let div_children = dom.children(div);
        assert_eq!(div_children.len(), 1, "div#root should have exactly one child (the span)");
        let span = div_children[0];
        assert_eq!(dom.tag(span), Some("span"));
        assert_eq!(dom.get_attribute(span, "class"), Some("x"));

        let span_children = dom.children(span);
        assert_eq!(span_children.len(), 1, "span should have exactly one child (the text node)");
        assert_eq!(dom.text_content(span_children[0]), "hi");
    }

    #[test]
    fn empty_document_bootstrap_is_noop() {
        let empty = Dom::new();
        let script = bootstrap_js(&empty);

        let shared = Rc::new(RefCell::new(Dom::new()));
        let mut engine = V8Engine::new(shared.clone());
        engine.eval(&script).expect("no-op script evaluates cleanly");

        let batch = engine.flush_ops();
        assert!(batch.ops.is_empty(), "empty Dom must yield no ops, got {:?}", batch.ops);
    }
}
