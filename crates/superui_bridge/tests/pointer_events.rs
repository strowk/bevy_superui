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

// Task 3: document.elementFromPoint, backed by a host-binding AABB hit-test
// over a per-frame geometry snapshot (`sync_hit_rects_system`/`HitRect`). The
// pure hit-test itself (`element_from_point`) is unit-tested in
// `superui_js::lib`; these tests exercise the host binding + `dom.js` surface
// end-to-end, against a snapshot pushed directly via `UiRuntime::set_hit_rects`
// rather than real headless layout (deliberately — see task-3-brief.md step 7;
// real-geometry correctness is Task 6's job).
#[test]
fn element_from_point_resolves_a_known_snapshot_and_misses_outside_it() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document("<div id='a'></div>")));
    let mut app = test_app();
    let _root = mount_with_input(&mut app, dom.clone());
    app.update(); // initial reconcile: #a now has a js binding

    let node = dom.borrow().get_element_by_id("a").unwrap();
    let js_id = app
        .world()
        .non_send::<UiRuntime>()
        .applier
        .js(node)
        .expect("#a must have a js binding");

    app.world_mut().non_send_mut::<UiRuntime>().set_hit_rects(vec![superui_js::HitRect {
        js_id,
        min: (0.0, 0.0),
        max: (10.0, 10.0),
        depth: 0,
    }]);

    app.world_mut().non_send_mut::<UiRuntime>().run_script(
        "globalThis.hit = (function () { \
           var n = document.elementFromPoint(5, 5); \
           return n ? n.id : null; \
         })(); \
         globalThis.miss = (function () { \
           var n = document.elementFromPoint(500, 500); \
           return n === null; \
         })();",
    );

    app.world_mut().non_send_mut::<UiRuntime>().run_script(
        "var b = document.getElementById('a'); \
         b.setAttribute('data-hit', String(globalThis.hit)); \
         b.setAttribute('data-miss', String(globalThis.miss));",
    );
    assert_eq!(dom.borrow().get_attribute(node, "data-hit"), Some("a"));
    assert_eq!(dom.borrow().get_attribute(node, "data-miss"), Some("true"));
}

#[test]
fn element_from_point_is_null_when_snapshot_is_empty() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document("<div id='a'></div>")));
    let mut app = test_app();
    let _root = mount_with_input(&mut app, dom.clone());
    app.update();

    // No set_hit_rects call yet this run: the engine starts with an empty
    // snapshot, so every probe misses.
    app.world_mut()
        .non_send_mut::<UiRuntime>()
        .run_script("globalThis.hit = document.elementFromPoint(1, 1);");
    app.world_mut().non_send_mut::<UiRuntime>().run_script(
        "var b = document.getElementById('a'); \
         b.setAttribute('data-hit', String(globalThis.hit === null));",
    );
    let node = dom.borrow().get_element_by_id("a").unwrap();
    assert_eq!(dom.borrow().get_attribute(node, "data-hit"), Some("true"));
}

// Task 2: uncaptured pointer-event synthesis observers. Six global picking
// observers (`on_pointer_{over,out,press,release,move,cancel}`) map picking
// events to the W3C pointer-event family, for every pointer type, through
// the same `PendingDomEvents` FIFO. Mirrors `input_events.rs`'s real-event
// construction and `touch_tap.rs`'s press/release triggering.
mod pointer_observers {
    use std::cell::RefCell;
    use std::rc::Rc;

    use bevy::camera::NormalizedRenderTarget;
    use bevy::picking::backend::HitData;
    use bevy::picking::events::{
        Pointer, PointerCancel, PointerMove, PointerOut, PointerOver, PointerPress,
        PointerRelease,
    };
    use bevy::picking::pointer::{Location, PointerButton, PointerId};
    use bevy::prelude::*;
    use bevy::window::{PrimaryWindow, WindowRef};
    use superui_bridge::{
        js_pointer_id, on_pointer_cancel, on_pointer_move, on_pointer_out, on_pointer_over,
        on_pointer_press, on_pointer_release, PendingDomEvents, PointerCaptures, PrimaryTouch,
        UiRuntime,
    };

    use crate::{mount, test_app};

    /// The ECS entity a DOM id resolves to (must already be reconciled).
    fn entity_of(app: &App, dom: &Rc<RefCell<superui_dom::Dom>>, id: &str) -> Entity {
        let node = dom.borrow().get_element_by_id(id).unwrap();
        app.world().non_send::<UiRuntime>().entity_for(node).unwrap()
    }

    /// Register the six Task-2 observers + their resources on top of the base
    /// harness.
    fn mount_pointer(app: &mut App, dom: Rc<RefCell<superui_dom::Dom>>) -> Entity {
        let root = mount(app, dom);
        app.init_resource::<PendingDomEvents>();
        app.init_resource::<PrimaryTouch>();
        app.init_resource::<PointerCaptures>();
        app.add_observer(on_pointer_over);
        app.add_observer(on_pointer_out);
        app.add_observer(on_pointer_press);
        app.add_observer(on_pointer_release);
        app.add_observer(on_pointer_move);
        app.add_observer(on_pointer_cancel);
        root
    }

    /// A `Camera2d` + the normalized `PrimaryWindow` target, built fresh each
    /// call (mirrors `input_events.rs`/`touch_tap.rs`).
    fn ctx(app: &mut App) -> (Entity, NormalizedRenderTarget) {
        let camera = app.world_mut().spawn(Camera2d).id();
        let window = app
            .world_mut()
            .query_filtered::<Entity, With<PrimaryWindow>>()
            .single(app.world())
            .expect("primary window");
        let target = WindowRef::Entity(window)
            .normalize(Some(window))
            .expect("normalized window");
        (camera, NormalizedRenderTarget::Window(target))
    }

    fn trigger_press(app: &mut App, entity: Entity, id: PointerId, pos: Vec2) {
        let (camera, target) = ctx(app);
        app.world_mut().trigger(PointerPress {
            entity,
            pointer: Pointer::new(id, Location { target, position: pos }),
            button: PointerButton::Primary,
            hit: HitData::new(camera, 0.0, None, None),
            count: 1,
        });
    }

    fn trigger_release(app: &mut App, entity: Entity, id: PointerId, pos: Vec2) {
        let (camera, target) = ctx(app);
        app.world_mut().trigger(PointerRelease {
            entity,
            pointer: Pointer::new(id, Location { target, position: pos }),
            button: PointerButton::Primary,
            hit: HitData::new(camera, 0.0, None, None),
        });
    }

    fn trigger_over(app: &mut App, entity: Entity, id: PointerId, pos: Vec2) {
        let (camera, target) = ctx(app);
        app.world_mut().trigger(PointerOver {
            entity,
            pointer: Pointer::new(id, Location { target, position: pos }),
            hit: HitData::new(camera, 0.0, None, None),
        });
    }

    fn trigger_out(app: &mut App, entity: Entity, id: PointerId, pos: Vec2) {
        let (camera, target) = ctx(app);
        app.world_mut().trigger(PointerOut {
            entity,
            pointer: Pointer::new(id, Location { target, position: pos }),
            hit: HitData::new(camera, 0.0, None, None),
        });
    }

    fn trigger_move(app: &mut App, entity: Entity, id: PointerId, pos: Vec2) {
        let (camera, target) = ctx(app);
        app.world_mut().trigger(PointerMove {
            entity,
            pointer: Pointer::new(id, Location { target, position: pos }),
            hit: HitData::new(camera, 0.0, None, None),
            delta: Vec2::ZERO,
        });
    }

    fn trigger_cancel(app: &mut App, entity: Entity, id: PointerId, pos: Vec2) {
        let (camera, target) = ctx(app);
        app.world_mut().trigger(PointerCancel {
            entity,
            pointer: Pointer::new(id, Location { target, position: pos }),
            hit: HitData::new(camera, 0.0, None, None),
        });
    }

    #[test]
    fn mouse_press_enqueues_pointerdown_primary_mouse() {
        let dom = Rc::new(RefCell::new(superui_html::parse_document("<div id='a'></div>")));
        let mut app = test_app();
        mount_pointer(&mut app, dom.clone());
        app.update(); // mount + first reconcile: #a now has an ECS entity

        let a = entity_of(&app, &dom, "a");
        trigger_press(&mut app, a, PointerId::Mouse, Vec2::new(3.0, 4.0));

        let events = &app.world().resource::<PendingDomEvents>().0;
        let types: Vec<&str> = events.iter().map(|e| e.type_.as_str()).collect();
        assert_eq!(types, vec!["pointerdown"]);

        let e = &events[0];
        assert!(e.bubbles, "pointerdown must bubble");
        let m = e.mouse.expect("pointerdown missing mouse payload");
        assert_eq!(m.pointer_type, superui_js::PointerKind::Mouse);
        assert!(m.is_primary, "a mouse pointer is always primary");
        assert_eq!(m.pointer_id, 1);
        assert_eq!((m.client_x, m.client_y), (3.0, 4.0));
        assert_eq!((m.button, m.buttons), (0, 1));
    }

    #[test]
    fn mouse_over_enqueues_pointerover_then_pointerenter() {
        let dom = Rc::new(RefCell::new(superui_html::parse_document("<div id='a'></div>")));
        let mut app = test_app();
        mount_pointer(&mut app, dom.clone());
        app.update();

        let a = entity_of(&app, &dom, "a");
        trigger_over(&mut app, a, PointerId::Mouse, Vec2::new(1.0, 2.0));
        {
            let events = &app.world().resource::<PendingDomEvents>().0;
            let types: Vec<&str> = events.iter().map(|e| e.type_.as_str()).collect();
            assert_eq!(types, vec!["pointerover", "pointerenter"]);
            assert!(events[0].bubbles, "pointerover must bubble");
            assert!(!events[1].bubbles, "pointerenter must not bubble");
        }

        trigger_out(&mut app, a, PointerId::Mouse, Vec2::new(1.0, 2.0));
        let events = &app.world().resource::<PendingDomEvents>().0;
        let types: Vec<&str> = events.iter().map(|e| e.type_.as_str()).collect();
        assert_eq!(types, vec!["pointerover", "pointerenter", "pointerout", "pointerleave"]);
        assert!(events[2].bubbles, "pointerout must bubble");
        assert!(!events[3].bubbles, "pointerleave must not bubble");
    }

    #[test]
    fn touch_press_is_primary_for_the_first_finger() {
        let dom = Rc::new(RefCell::new(superui_html::parse_document("<div id='a'></div>")));
        let mut app = test_app();
        mount_pointer(&mut app, dom.clone());
        app.update();

        assert!(app.world().resource::<PrimaryTouch>().0.is_none());
        let a = entity_of(&app, &dom, "a");
        trigger_press(&mut app, a, PointerId::Touch(5), Vec2::new(1.0, 1.0));

        let events = &app.world().resource::<PendingDomEvents>().0;
        assert_eq!(events.len(), 1);
        let m = events[0].mouse.expect("pointerdown missing mouse payload");
        assert_eq!(m.pointer_type, superui_js::PointerKind::Touch);
        assert_eq!(m.pointer_id, js_pointer_id(PointerId::Touch(5)));
        assert_eq!(m.pointer_id, 7);
        assert!(m.is_primary, "the first concurrent finger is primary");
    }

    #[test]
    fn secondary_touch_is_not_primary() {
        let dom = Rc::new(RefCell::new(superui_html::parse_document("<div id='a'></div>")));
        let mut app = test_app();
        mount_pointer(&mut app, dom.clone());
        app.update();

        app.world_mut().resource_mut::<PrimaryTouch>().0 = Some(PointerId::Touch(5));
        let a = entity_of(&app, &dom, "a");
        trigger_press(&mut app, a, PointerId::Touch(6), Vec2::new(1.0, 1.0));

        let events = &app.world().resource::<PendingDomEvents>().0;
        assert_eq!(events.len(), 1);
        let m = events[0].mouse.expect("pointerdown missing mouse payload");
        assert!(
            !m.is_primary,
            "a second finger still fires pointerdown, but is not primary"
        );
    }

    #[test]
    fn pointer_release_and_cancel_map_through() {
        let dom = Rc::new(RefCell::new(superui_html::parse_document("<div id='a'></div>")));
        let mut app = test_app();
        mount_pointer(&mut app, dom.clone());
        app.update();

        let a = entity_of(&app, &dom, "a");
        trigger_release(&mut app, a, PointerId::Mouse, Vec2::new(1.0, 1.0));
        trigger_cancel(&mut app, a, PointerId::Touch(5), Vec2::new(2.0, 2.0));

        let events = &app.world().resource::<PendingDomEvents>().0;
        let types: Vec<&str> = events.iter().map(|e| e.type_.as_str()).collect();
        assert_eq!(types, vec!["pointerup", "pointercancel"]);

        let up = &events[0];
        assert!(up.bubbles, "pointerup must bubble");
        let m = up.mouse.expect("pointerup missing mouse payload");
        assert_eq!((m.button, m.buttons), (0, 0));

        let cancel = &events[1];
        assert!(cancel.bubbles, "pointercancel must bubble");
        let m = cancel.mouse.expect("pointercancel missing mouse payload");
        assert_eq!((m.button, m.buttons), (0, 0));
    }

    #[test]
    fn captured_pointer_is_skipped_by_the_observers() {
        let dom = Rc::new(RefCell::new(superui_html::parse_document("<div id='a'></div>")));
        let mut app = test_app();
        mount_pointer(&mut app, dom.clone());
        app.update();

        let a = entity_of(&app, &dom, "a");
        let node = dom.borrow().get_element_by_id("a").unwrap();
        app.world_mut()
            .resource_mut::<PointerCaptures>()
            .0
            .insert(js_pointer_id(PointerId::Mouse), node);

        trigger_move(&mut app, a, PointerId::Mouse, Vec2::new(1.0, 1.0));

        assert!(
            app.world().resource::<PendingDomEvents>().0.is_empty(),
            "a captured pointer's move must be left to the raw-location path"
        );
    }
}
