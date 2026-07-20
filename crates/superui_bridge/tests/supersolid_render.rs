//! End-to-end: a Supersolid counter rendered from JS, incremented by a dispatched
//! DOM click, updates the reconciled ECS `Text`. Locks the sync-scheduler ->
//! DOM-mutation -> reconciler -> ECS path (design §9).

use std::cell::RefCell;
use std::rc::Rc;

use bevy::prelude::*;
use superui_bridge::UiRuntime;

mod support;
use support::{mount, test_app};

#[test]
fn supersolid_click_updates_reconciled_text() {
    // A minimal shell with a mount point; the script renders into #root.
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<div id='root'></div>",
    )));
    let mut app = test_app();
    let _root = mount(&mut app, dom.clone());

    // Render a counter with Supersolid. The button's click handler bumps a signal;
    // the label is a reactive text hole.
    app.world_mut()
        .non_send_resource_mut::<UiRuntime>()
        .run_script(
            r#"
            function Counter() {
                var c = createSignal(0);
                var wrap = $ss.el("div");
                var label = $ss.el("span");
                $ss.insert(label, function () { return c[0](); });
                var btn = $ss.el("button");
                $ss.on(btn, "click", function () { c[1](function (n) { return n + 1; }); });
                $ss.child(btn, $ss.txt("+"));
                $ss.child(wrap, label);
                $ss.child(wrap, btn);
                return wrap;
            }
            render(function () { return $ss.cmp(Counter, {}); },
                   document.getElementById("root"));
            "#,
        );
    app.update(); // reconcile the initial render

    // The label text reconciled to "0".
    let label_text_0 = current_label_text(&mut app, &dom);
    assert_eq!(label_text_0, "0", "initial reactive text should reconcile");

    // Dispatch a click on the button through the engine (as the input system does).
    {
        let mut rt = app.world_mut().non_send_resource_mut::<UiRuntime>();
        let btn = {
            let d = dom.borrow();
            d.query_selector(d.document(), "button").unwrap()
        };
        use superui_js::JsEngine;
        rt.engine.dispatch_event(btn, "click", None, true, true);
        rt.dirty = true; // mirror the input system's post-dispatch dirtying
    }
    app.update(); // reconcile the post-click DOM

    let label_text_1 = current_label_text(&mut app, &dom);
    assert_eq!(label_text_1, "1", "click -> signal -> effect -> DOM -> ECS text");
}

/// Read the reconciled `Text` of the first `<span>`'s text child entity.
fn current_label_text(app: &mut App, dom: &Rc<RefCell<superui_dom::Dom>>) -> String {
    let span = {
        let d = dom.borrow();
        d.query_selector(d.document(), "span").unwrap()
    };
    let span_entity = app
        .world()
        .non_send_resource::<UiRuntime>()
        .entity_for(span)
        .unwrap();
    let text_entity = app.world().get::<Children>(span_entity).unwrap()[0];
    app.world().get::<Text>(text_entity).unwrap().0.clone()
}
