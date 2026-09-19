//! The op-wire per-frame loop end to end: author JS mutates the shadow DOM,
//! `run_script`'s pump replays it onto the render mirror, a dispatched click runs
//! the listener (which calls `bevy.send`), and the engine outbox carries it out.

use std::cell::RefCell;
use std::rc::Rc;

use bevy::prelude::*;
use serde_json::json;
use superui_bridge::UiRuntime;

#[test]
fn run_script_pumps_dom_and_dispatch_delivers_outbox() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<div id='root'></div>",
    )));
    let mut rt = UiRuntime::new(dom.clone(), Entity::PLACEHOLDER, Handle::default(), false);

    // Author JS: create a child under #root and register a click listener that
    // sends a `clicked` command back to the ECS.
    rt.run_script(
        r#"
        var root = document.getElementById("root");
        var box = document.createElement("div");
        box.setAttribute("id", "box");
        box.addEventListener("click", function () { bevy.send("clicked", {}); });
        root.appendChild(box);
        "#,
    );

    // The render mirror gained the node (proves flush_ops -> applier.apply ran).
    let box_node = dom
        .borrow()
        .get_element_by_id("box")
        .expect("appended node reached the render mirror through the op-wire");

    // Nothing sent yet.
    assert!(rt.engine.drain_outbox().is_empty());

    // Dispatch a click at the mirror node; the runtime routes it to the shadow
    // DOM's jsId and runs the listener, whose bevy.send lands in the outbox.
    rt.dispatch_dom_event(box_node, "click", None, true, true);
    assert_eq!(rt.engine.drain_outbox(), vec![("clicked".to_string(), json!({}))]);
}
