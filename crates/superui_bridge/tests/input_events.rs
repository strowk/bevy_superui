//! Input seam: a pointer click drives a DOM `click`, runs the JS listener, and
//! the resulting DOM mutation reconciles into the ECS.
mod support;
use support::*;

use std::cell::RefCell;
use std::rc::Rc;

use bevy::prelude::*;
use superui_bridge::{
    click_effect, drain_dom_events_system, on_pointer_click, PendingDomEvent, PendingDomEvents,
    UiRuntime,
};

/// Register the full input pipeline on top of the base harness.
fn mount_with_input(app: &mut App, dom: Rc<RefCell<superui_dom::Dom>>) -> Entity {
    let root = mount(app, dom);
    app.init_resource::<PendingDomEvents>();
    app.add_observer(on_pointer_click);
    // Drain events, then reconcile, each Update.
    app.add_systems(
        Update,
        drain_dom_events_system.before(superui_bridge::reconcile_system),
    );
    root
}

/// Test 1: drain→dispatch JS→dirty→reconcile path.
///
/// We push directly into `PendingDomEvents` rather than constructing a
/// `Pointer<Click>` (whose fields `HitData` and `Location` have no `Default`
/// and reference entities we don't control in a headless test). This exercises
/// the drain system end-to-end deterministically.
#[test]
fn click_runs_js_listener_and_reconciles() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<button id='b'>hi</button><div id='out'></div>",
    )));
    let mut app = test_app();
    let _root = mount_with_input(&mut app, dom.clone());

    // Author JS: clicking the button writes into #out.
    app.world_mut()
        .non_send_resource_mut::<UiRuntime>()
        .run_script(
            "document.getElementById('b').addEventListener('click', function() { \
               document.getElementById('out').textContent = 'clicked'; \
             });",
        );
    app.update(); // initial reconcile

    // Simulate a pointer click by enqueuing directly into PendingDomEvents —
    // the deterministic, harness-friendly path.
    let btn_node = dom.borrow().get_element_by_id("b").unwrap();
    app.world_mut()
        .resource_mut::<PendingDomEvents>()
        .0
        .push(PendingDomEvent::new(btn_node, "click"));
    app.update(); // drain -> dispatch JS -> dirty -> reconcile

    // #out now contains the text "clicked".
    let out_node = dom.borrow().get_element_by_id("out").unwrap();
    assert_eq!(dom.borrow().text_content(out_node), "clicked");
}

/// Test 2: checkbox toggle + change event via `click_effect`.
///
/// `Pointer<Click>` cannot be constructed in a headless test: `HitData` requires
/// a camera `Entity` and `Location` requires a `NormalizedRenderTarget`, neither
/// of which have `Default`. We therefore call `click_effect` directly — the free
/// function extracted from the observer body — which genuinely tests the
/// toggle+change logic without requiring picking machinery.
#[test]
fn checkbox_click_toggles_checked_and_fires_change() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<input id='c' type='checkbox'>",
    )));
    let mut app = test_app();
    mount_with_input(&mut app, dom.clone());

    // JS records change events into a global counter.
    app.world_mut().non_send_resource_mut::<UiRuntime>().run_script(
        "globalThis.changes = 0; \
         document.getElementById('c').addEventListener('change', function(){ globalThis.changes++; });",
    );
    app.update();

    let node = dom.borrow().get_element_by_id("c").unwrap();

    // Before click: unchecked.
    assert!(!dom.borrow().checked(node));

    // Call the observer's core logic directly with world access.
    {
        let rt = app.world().non_send_resource::<UiRuntime>();
        let mut pending = PendingDomEvents::default();
        click_effect(rt, node, &mut pending);
        // Transfer events into the world resource.
        app.world_mut()
            .resource_mut::<PendingDomEvents>()
            .0
            .extend(pending.0);
    }
    app.update(); // drain (dispatches click + change) -> dirty -> reconcile

    // Checkbox toggled to checked.
    assert!(dom.borrow().checked(node));

    // Verify change listener ran: JS wrote the count onto the checkbox's data-changes attr.
    // We use run_script to mirror globalThis.changes into the DOM so Rust can read it back.
    app.world_mut().non_send_resource_mut::<UiRuntime>().run_script(
        "document.getElementById('c').setAttribute('data-changes', String(globalThis.changes));",
    );
    let data_changes = dom.borrow().get_attribute(node, "data-changes").unwrap_or("").to_string();
    assert_eq!(data_changes, "1", "change listener must have fired exactly once");
}
