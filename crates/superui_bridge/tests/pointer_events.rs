//! PointerEvent shape: `PendingDomEvent`'s `MousePayload` carries pointer
//! fields (`pointer_id`/`pointer_type`/`is_primary`) through to the JS event
//! object, but only for pointer event types — a plain `MouseEvent` must not
//! gain `pointerId`/`pointerType`.
mod support;
use support::*;

use std::cell::RefCell;
use std::rc::Rc;

use bevy::prelude::*;
use superui_bridge::{drain_dom_events_system, PendingDomEvent, PendingDomEvents, UiRuntime};
use superui_js::{MousePayload, PointerKind};

/// Register the input-drain pipeline on top of the base harness (mirrors
/// `input_events.rs::mount_with_input`, minus the picking observer this suite
/// doesn't need since every test pushes directly into `PendingDomEvents`).
fn mount_with_input(app: &mut App, dom: Rc<RefCell<superui_dom::Dom>>) -> Entity {
    let root = mount(app, dom);
    app.init_resource::<PendingDomEvents>();
    app.add_systems(
        Update,
        drain_dom_events_system.before(superui_bridge::reconcile_system),
    );
    root
}

/// A `pointerdown` carrying a `MousePayload` with pointer fields set exposes
/// `pointerId`/`pointerType`/`isPrimary`/`width`/`height`/`pressure` on the
/// listener's event object, alongside the existing `clientX`.
#[test]
fn pointerdown_exposes_pointer_props_to_the_listener() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document("<div id='b'></div>")));
    let mut app = test_app();
    let _root = mount_with_input(&mut app, dom.clone());

    app.world_mut().non_send_mut::<UiRuntime>().run_script(
        "document.getElementById('b').addEventListener('pointerdown', function(e) { \
           globalThis.seen = { \
             pointerId: e.pointerId, pointerType: e.pointerType, isPrimary: e.isPrimary, \
             width: e.width, height: e.height, pressure: e.pressure, clientX: e.clientX, \
           }; \
         });",
    );
    app.update(); // initial reconcile

    let node = dom.borrow().get_element_by_id("b").unwrap();
    app.world_mut().resource_mut::<PendingDomEvents>().0.push(
        PendingDomEvent::new(node, "pointerdown").with_mouse(MousePayload {
            client_x: 7.0,
            client_y: 9.0,
            button: 0,
            buttons: 1,
            pointer_id: 1,
            pointer_type: PointerKind::Mouse,
            is_primary: true,
        }),
    );
    app.update(); // drain -> dispatch JS (with pointer payload) -> reconcile

    app.world_mut().non_send_mut::<UiRuntime>().run_script(
        "var b = document.getElementById('b'); \
         b.setAttribute('data-pointer-id', String(globalThis.seen.pointerId)); \
         b.setAttribute('data-pointer-type', String(globalThis.seen.pointerType)); \
         b.setAttribute('data-is-primary', String(globalThis.seen.isPrimary)); \
         b.setAttribute('data-width', String(globalThis.seen.width)); \
         b.setAttribute('data-height', String(globalThis.seen.height)); \
         b.setAttribute('data-pressure', String(globalThis.seen.pressure)); \
         b.setAttribute('data-client-x', String(globalThis.seen.clientX));",
    );
    assert_eq!(dom.borrow().get_attribute(node, "data-pointer-id"), Some("1"));
    assert_eq!(dom.borrow().get_attribute(node, "data-pointer-type"), Some("mouse"));
    assert_eq!(dom.borrow().get_attribute(node, "data-is-primary"), Some("true"));
    assert_eq!(dom.borrow().get_attribute(node, "data-width"), Some("1"));
    assert_eq!(dom.borrow().get_attribute(node, "data-height"), Some("1"));
    assert_eq!(dom.borrow().get_attribute(node, "data-pressure"), Some("0.5"));
    assert_eq!(dom.borrow().get_attribute(node, "data-client-x"), Some("7"));
}

/// A touch-sourced, non-primary `pointermove` with no buttons held reports
/// `pointerType == "touch"`, `isPrimary == false`, and `pressure == 0`.
#[test]
fn touch_pointermove_reports_touch_type_and_zero_pressure() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document("<div id='b'></div>")));
    let mut app = test_app();
    let _root = mount_with_input(&mut app, dom.clone());

    app.world_mut().non_send_mut::<UiRuntime>().run_script(
        "document.getElementById('b').addEventListener('pointermove', function(e) { \
           globalThis.seen = { pointerType: e.pointerType, isPrimary: e.isPrimary, pressure: e.pressure }; \
         });",
    );
    app.update();

    let node = dom.borrow().get_element_by_id("b").unwrap();
    app.world_mut().resource_mut::<PendingDomEvents>().0.push(
        PendingDomEvent::new(node, "pointermove").with_mouse(MousePayload {
            client_x: 3.0,
            client_y: 4.0,
            button: 0,
            buttons: 0,
            pointer_id: 2,
            pointer_type: PointerKind::Touch,
            is_primary: false,
        }),
    );
    app.update();

    app.world_mut().non_send_mut::<UiRuntime>().run_script(
        "var b = document.getElementById('b'); \
         b.setAttribute('data-pointer-type', String(globalThis.seen.pointerType)); \
         b.setAttribute('data-is-primary', String(globalThis.seen.isPrimary)); \
         b.setAttribute('data-pressure', String(globalThis.seen.pressure));",
    );
    assert_eq!(dom.borrow().get_attribute(node, "data-pointer-type"), Some("touch"));
    assert_eq!(dom.borrow().get_attribute(node, "data-is-primary"), Some("false"));
    assert_eq!(dom.borrow().get_attribute(node, "data-pressure"), Some("0"));
}

/// Pointer fields must not leak onto a plain `MouseEvent` — a `mousedown`
/// dispatched with a `MousePayload` that happens to carry pointer fields
/// still reports `pointerId`/`pointerType` as `undefined`.
#[test]
fn mouse_event_has_no_pointer_props() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document("<div id='b'></div>")));
    let mut app = test_app();
    let _root = mount_with_input(&mut app, dom.clone());

    app.world_mut().non_send_mut::<UiRuntime>().run_script(
        "document.getElementById('b').addEventListener('mousedown', function(e) { \
           globalThis.seen = { pointerId: typeof e.pointerId, pointerType: typeof e.pointerType }; \
         });",
    );
    app.update();

    let node = dom.borrow().get_element_by_id("b").unwrap();
    app.world_mut().resource_mut::<PendingDomEvents>().0.push(
        PendingDomEvent::new(node, "mousedown").with_mouse(MousePayload {
            client_x: 1.0,
            client_y: 2.0,
            button: 0,
            buttons: 1,
            pointer_id: 1,
            pointer_type: PointerKind::Mouse,
            is_primary: true,
        }),
    );
    app.update();

    app.world_mut().non_send_mut::<UiRuntime>().run_script(
        "var b = document.getElementById('b'); \
         b.setAttribute('data-pointer-id-type', String(globalThis.seen.pointerId)); \
         b.setAttribute('data-pointer-type-type', String(globalThis.seen.pointerType));",
    );
    assert_eq!(dom.borrow().get_attribute(node, "data-pointer-id-type"), Some("undefined"));
    assert_eq!(dom.borrow().get_attribute(node, "data-pointer-type-type"), Some("undefined"));
}
