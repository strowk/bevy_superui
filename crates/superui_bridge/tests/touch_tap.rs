//! Touch-tap mouse-event synthesis (Task 1): a `PointerPress` from the primary
//! active touch finger becomes the browser compatibility mouse sequence
//! (`mousemove`, `mouseover`, `mouseenter`, `mousedown`) enqueued into
//! `PendingDomEvents`. Mirrors `input_events.rs`'s real-`PointerClick` harness.
mod support;
use support::*;

use std::cell::RefCell;
use std::rc::Rc;

use bevy::camera::NormalizedRenderTarget;
use bevy::picking::backend::HitData;
use bevy::picking::events::{Pointer, PointerPress};
use bevy::picking::pointer::{Location, PointerButton, PointerId};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowRef};
use superui_bridge::{on_touch_press, Modifiers, PendingDomEvents, PrimaryTouch, UiRuntime};

/// The ECS entity a DOM id resolves to (must already be reconciled).
fn entity_of(app: &App, dom: &Rc<RefCell<superui_dom::Dom>>, id: &str) -> Entity {
    let node = dom.borrow().get_element_by_id(id).unwrap();
    app.world().non_send::<UiRuntime>().entity_for(node).unwrap()
}

/// Register `on_touch_press` + its resources on top of the base harness.
fn mount_touch(app: &mut App, dom: Rc<RefCell<superui_dom::Dom>>) -> Entity {
    let root = mount(app, dom);
    app.init_resource::<PendingDomEvents>();
    app.init_resource::<PrimaryTouch>();
    app.add_observer(on_touch_press);
    root
}

/// Trigger a real `PointerPress` from pointer `id` on `entity` at `pos`.
/// Builds `target`/`camera` the way `input_events.rs`'s foreign-entity test does.
fn press(app: &mut App, entity: Entity, id: PointerId, pos: Vec2) {
    let camera = app.world_mut().spawn(Camera2d).id();
    let window = app
        .world_mut()
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(app.world())
        .expect("primary window");
    let target = WindowRef::Entity(window)
        .normalize(Some(window))
        .expect("normalized window");
    app.world_mut().trigger(PointerPress {
        entity,
        pointer: Pointer::new(
            id,
            Location { target: NormalizedRenderTarget::Window(target), position: pos },
        ),
        button: PointerButton::Primary,
        hit: HitData::new(camera, 0.0, None, None),
        count: 1,
    });
}

#[test]
fn primary_tap_press_enqueues_the_compat_sequence() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document("<div id='a'></div>")));
    let mut app = test_app();
    mount_touch(&mut app, dom.clone());
    app.update(); // mount + first reconcile: #a now has an ECS entity

    let a = entity_of(&app, &dom, "a");
    press(&mut app, a, PointerId::Touch(1), Vec2::new(12.0, 34.0));

    let events = &app.world().resource::<PendingDomEvents>().0;
    let types: Vec<&str> = events.iter().map(|e| e.type_.as_str()).collect();
    assert_eq!(types, vec!["mousemove", "mouseover", "mouseenter", "mousedown"]);

    let expected_button_buttons = [(0u8, 0u8), (0, 0), (0, 0), (0, 1)];
    let expected_bubbles = [true, true, false, true];
    for (i, e) in events.iter().enumerate() {
        let m = e.mouse.unwrap_or_else(|| panic!("event {i} ({}) missing mouse payload", e.type_));
        assert_eq!(m.client_x, 12.0, "event {i} ({})", e.type_);
        assert_eq!(m.client_y, 34.0, "event {i} ({})", e.type_);
        assert_eq!((m.button, m.buttons), expected_button_buttons[i], "event {i} ({})", e.type_);
        assert_eq!(e.bubbles, expected_bubbles[i], "event {i} ({})", e.type_);
        assert_eq!(e.modifiers, Some(Modifiers::default()), "event {i} ({})", e.type_);
    }
}

#[test]
fn secondary_concurrent_touch_press_enqueues_nothing() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document("<div id='a'></div>")));
    let mut app = test_app();
    mount_touch(&mut app, dom.clone());
    app.update();

    let a = entity_of(&app, &dom, "a");
    press(&mut app, a, PointerId::Touch(1), Vec2::new(1.0, 1.0));
    assert_eq!(app.world().resource::<PendingDomEvents>().0.len(), 4);

    // A second finger landing while the first is still down (no release in
    // between) must not synthesize a second compat sequence.
    press(&mut app, a, PointerId::Touch(2), Vec2::new(2.0, 2.0));
    assert_eq!(
        app.world().resource::<PendingDomEvents>().0.len(),
        4,
        "a second concurrent finger must not enqueue its own compat sequence"
    );
}

#[test]
fn mouse_press_enqueues_nothing_from_touch_observer() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document("<div id='a'></div>")));
    let mut app = test_app();
    mount_touch(&mut app, dom.clone());
    app.update();

    let a = entity_of(&app, &dom, "a");
    press(&mut app, a, PointerId::Mouse, Vec2::new(1.0, 1.0));

    assert!(
        app.world().resource::<PendingDomEvents>().0.is_empty(),
        "the touch observer must ignore mouse pointers; mouse stays the sole mouse source"
    );
}

#[test]
fn touch_press_on_a_foreign_entity_enqueues_nothing_and_leaves_primary_free() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document("<div id='a'></div>")));
    let mut app = test_app();
    mount_touch(&mut app, dom.clone());
    app.update();

    // A host-game entity with no DomNode — nothing to do with superui.
    let foreign = app.world_mut().spawn(Node::default()).id();
    press(&mut app, foreign, PointerId::Touch(1), Vec2::new(1.0, 1.0));

    assert!(
        app.world().resource::<PendingDomEvents>().0.is_empty(),
        "a foreign entity must not enqueue any compat sequence"
    );
    assert!(
        app.world().resource::<PrimaryTouch>().0.is_none(),
        "a foreign entity's touch must not consume the primary-finger slot"
    );

    // The primary slot being free means a following real tap still synthesizes.
    let a = entity_of(&app, &dom, "a");
    press(&mut app, a, PointerId::Touch(2), Vec2::new(3.0, 3.0));
    assert_eq!(app.world().resource::<PendingDomEvents>().0.len(), 4);
}
