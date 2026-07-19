//! Best-effort ledger↔impl sync (design §11): a handful of ✅ DOM rows must have
//! a live binding — executing a snippet that uses them must not throw.
mod support;
use support::*;
use superui_bridge::UiRuntime;

#[test]
fn sampled_supported_dom_apis_have_live_bindings() {
    let mut app = app();
    let _root = mount_todomvc(&mut app);

    // Exercise a representative slice of the ✅ js-dom.md surface. If any of
    // these were secretly unimplemented, the eval would throw and `run_script`
    // would warn+swallow — so we assert an observable side effect instead.
    let mut rt = app.world_mut().non_send_resource_mut::<UiRuntime>();
    rt.run_script(
        "var d = document.createElement('div'); \
         d.id = 'ledger-probe'; d.className = 'x'; \
         d.setAttribute('data-k', 'v'); \
         d.classList.add('y'); \
         var t = document.createTextNode('hi'); d.appendChild(t); \
         document.getElementById('app').appendChild(d);",
    );
    drop(rt);
    tick(&mut app, 2);

    // The probe node materialised with its attributes -> the sampled APIs work.
    let probe = node_by_selector(&app, "#ledger-probe");
    let (has_class, attr, text) = {
        let rt = app.world().non_send_resource::<UiRuntime>();
        let d = rt.dom.borrow();
        (
            d.classes(probe).iter().any(|c| c == "y"),
            d.get_attribute(probe, "data-k").map(|s| s.to_string()),
            d.text_content(probe),
        )
    };
    assert!(has_class);
    assert_eq!(attr.as_deref(), Some("v"));
    assert_eq!(text, "hi");
}
