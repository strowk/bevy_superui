//! End-to-end tests for [`BoaEngine`] running on the JS shadow DOM (`dom.js`)
//! and the op-wire. Author JS mutates `document`; the engine flushes a decoded
//! [`OpBatch`], dispatches events back into JS, and drains the Bevy outbox.

use std::cell::RefCell;
use std::rc::Rc;

use serde_json::json;
use superui_dom::Dom;
use superui_js::opwire::{JsNodeId, Op, OpBatch};
use superui_js::{BoaEngine, JsEngine};

/// The `JsNodeId` of the first `CreateElement` op whose tag resolves to `tag`.
fn created_id(batch: &OpBatch, tag: &str) -> JsNodeId {
    for op in &batch.ops {
        if let Op::CreateElement { id, tag: t } = op {
            if batch.resolve(*t) == tag {
                return *id;
            }
        }
    }
    panic!("no CreateElement for tag {tag:?} in {:?}", batch.ops);
}

fn engine() -> BoaEngine {
    BoaEngine::new(Rc::new(RefCell::new(Dom::new())))
}

#[test]
fn author_mutations_flush_as_a_decoded_batch() {
    let mut e = engine();
    e.eval(
        "const d = document.createElement('div');\
         d.setAttribute('class','row');\
         __ss_root.appendChild(d);",
    )
    .expect("eval ok");

    let batch = e.flush_ops();

    // CreateElement('div')
    let div = created_id(&batch, "div");
    assert!(div >= 2, "child id should be past the pre-bound root");

    // SetAttribute(class=row) on the div.
    let has_attr = batch.ops.iter().any(|op| {
        matches!(op, Op::SetAttribute { id, name, value }
            if *id == div && batch.resolve(*name) == "class" && batch.resolve(*value) == "row")
    });
    assert!(has_attr, "expected SetAttribute class=row, got {:?}", batch.ops);

    // InsertBefore(root, div, append).
    let inserted = batch.ops.iter().any(|op| {
        matches!(op, Op::InsertBefore { parent, node, reference }
            if *parent == 1 && *node == div && *reference == 0)
    });
    assert!(inserted, "expected InsertBefore under root, got {:?}", batch.ops);
}

#[test]
fn dispatch_reports_prevent_default_and_fills_outbox() {
    let mut e = engine();
    e.eval(
        "const d = document.createElement('div');\
         __ss_root.appendChild(d);\
         d.addEventListener('click', e => { e.preventDefault(); __superui_bevy_send('clicked', {id: 1}); });",
    )
    .expect("eval ok");

    let div = created_id(&e.flush_ops(), "div");

    // preventDefault() was called and the event is cancelable → true.
    assert!(e.dispatch_event(div, "click", None, true, true));

    assert_eq!(e.drain_outbox(), vec![("clicked".to_string(), json!({"id": 1}))]);
    // Draining leaves the outbox empty.
    assert!(e.drain_outbox().is_empty());
}

#[test]
fn run_timers_fires_due_callbacks() {
    let mut e = engine();
    e.eval("setTimeout(() => __superui_bevy_send('t', {}), 5);").expect("eval ok");

    // Not yet due.
    e.run_timers(1.0);
    assert!(e.drain_outbox().is_empty());

    // Past the 5ms deadline → fires.
    e.run_timers(10.0);
    assert_eq!(e.drain_outbox(), vec![("t".to_string(), json!({}))]);
}

#[test]
fn eval_reports_syntax_errors_without_panicking() {
    let mut e = engine();
    assert!(e.eval("this is not valid )(").is_err());
}
