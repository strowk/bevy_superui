//! Regression tests for the browser-like input/checkbox/click polish that was
//! previously only verified live via BRP: placeholder-vs-value color, the
//! blinking caret while focused, single-line (no-wrap) input rendering, the
//! checked-checkbox mark, and click-propagation stopping at the deepest DOM node.
mod support;
use support::*;

use std::cell::RefCell;
use std::rc::Rc;

use bevy::prelude::*;
use superui_bridge::{DomNode, InputValueText, PendingDomEvents, UiRuntime};
use superui_dom::NodeId;

/// Resolve the entity carrying a given DOM node.
fn entity_for(app: &mut App, node: NodeId) -> Entity {
    let mut q = app.world_mut().query::<(Entity, &DomNode)>();
    q.iter(app.world())
        .find(|(_, d)| d.0 == node)
        .map(|(e, _)| e)
        .expect("no entity for node")
}

/// The managed `InputValueText` child of an input/checkbox element, if present.
fn managed_child(app: &mut App, element: Entity) -> Option<Entity> {
    let kids = app.world().get::<Children>(element)?.to_vec();
    kids.into_iter()
        .find(|&k| app.world().get::<InputValueText>(k).is_some())
}

fn child_text(app: &mut App, element: Entity) -> String {
    let child = managed_child(app, element).expect("element has managed text child");
    app.world().get::<Text>(child).unwrap().0.clone()
}

fn set_dirty(app: &mut App) {
    app.world_mut().non_send_mut::<UiRuntime>().dirty = true;
}

/// A text input's placeholder renders dimmer (grey) than a typed value (dark),
/// mirroring `::placeholder`. The color lives on the managed text child.
#[test]
fn placeholder_and_value_use_distinct_colors() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<input id='t' type='text' placeholder='hint'>",
    )));
    let mut app = test_app();
    let _root = mount(&mut app, dom.clone());
    app.update();

    let node = dom.borrow().get_element_by_id("t").unwrap();
    let input = entity_for(&mut app, node);

    // Empty (placeholder shown) -> dim grey.
    let child = managed_child(&mut app, input).expect("managed child");
    let placeholder_color = app.world().get::<TextColor>(child).unwrap().0;
    assert_eq!(
        placeholder_color,
        Color::srgb(0.6, 0.6, 0.6),
        "empty input should render its placeholder in dim grey"
    );

    // Type a value -> darker text color (distinct from the placeholder grey).
    dom.borrow_mut().set_value(node, "typed");
    set_dirty(&mut app);
    app.update();

    let child = managed_child(&mut app, input).expect("managed child");
    let value_color = app.world().get::<TextColor>(child).unwrap().0;
    assert_ne!(
        value_color, placeholder_color,
        "a typed value must not use the placeholder color"
    );
    assert_eq!(
        value_color,
        Color::srgb(0.2, 0.2, 0.2),
        "typed value should fall back to the dark default color"
    );
}

/// While focused, the field shows a caret bar after its value; blinking flips it
/// to a same-width space. Unfocused fields show no caret. (Assertions target the
/// caret at the tail — where it sits for a non-empty value — so they're robust to
/// the single-line tail-truncation that the live layout may apply here.)
#[test]
fn focused_input_shows_and_blinks_a_caret() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<input id='t' type='text'>",
    )));
    let mut app = test_app();
    let _root = mount(&mut app, dom.clone());
    app.update();

    let node = dom.borrow().get_element_by_id("t").unwrap();
    dom.borrow_mut().set_value(node, "hi");
    let input = entity_for(&mut app, node);

    // Unfocused: no caret bar.
    set_dirty(&mut app);
    app.update();
    let unfocused = child_text(&mut app, input);
    assert!(
        !unfocused.ends_with('|'),
        "an unfocused field shows no caret bar, got {unfocused:?}"
    );

    // Focus it and re-render: caret is on immediately (set by focus).
    app.world_mut()
        .non_send_mut::<UiRuntime>()
        .set_focus(Some(node));
    set_dirty(&mut app);
    app.update();
    let focused_on = child_text(&mut app, input);
    assert!(
        focused_on.ends_with('|'),
        "a focused field shows the caret bar after its value, got {focused_on:?}"
    );

    // Advance past the blink interval: caret toggles to a same-width space so the
    // field doesn't jitter. `advance_caret` returns true when the glyph flipped.
    let flipped = app
        .world_mut()
        .non_send_mut::<UiRuntime>()
        .advance_caret(1.0);
    assert!(flipped, "a full second must cross the ~2Hz blink boundary");
    set_dirty(&mut app);
    app.update();
    let focused_off = child_text(&mut app, input);
    assert!(
        !focused_off.ends_with('|'),
        "the blinked-off caret is a space, not a bar, got {focused_off:?}"
    );
}

/// The managed text child never wraps — a text field is a single line (the fix
/// for the input growing taller like a textarea as you type).
#[test]
fn input_text_child_is_single_line_no_wrap() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<input id='t' type='text'>",
    )));
    let mut app = test_app();
    let _root = mount(&mut app, dom.clone());
    app.update();

    let node = dom.borrow().get_element_by_id("t").unwrap();
    let input = entity_for(&mut app, node);
    let child = managed_child(&mut app, input).expect("managed child");

    let layout = app
        .world()
        .get::<TextLayout>(child)
        .expect("managed text child has a TextLayout");
    assert_eq!(
        layout.linebreak,
        bevy::text::LineBreak::NoWrap,
        "input text must not wrap (single-line field)"
    );
}

/// A checked checkbox shows a mark as a managed child; unchecking removes it.
#[test]
fn checked_checkbox_shows_a_mark_and_unchecking_removes_it() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<input id='c' type='checkbox'>",
    )));
    let mut app = test_app();
    let _root = mount(&mut app, dom.clone());
    app.update();

    let node = dom.borrow().get_element_by_id("c").unwrap();
    let checkbox = entity_for(&mut app, node);

    // Unchecked: no managed mark child.
    assert!(
        managed_child(&mut app, checkbox).is_none(),
        "an unchecked checkbox has no mark"
    );

    // Check it: a non-empty mark child appears.
    dom.borrow_mut().set_checked(node, true);
    set_dirty(&mut app);
    app.update();
    let mark = child_text(&mut app, checkbox);
    assert!(
        !mark.trim().is_empty(),
        "a checked checkbox shows a visible mark, got {mark:?}"
    );

    // Uncheck it: the mark child goes away again.
    dom.borrow_mut().set_checked(node, false);
    set_dirty(&mut app);
    app.update();
    assert!(
        managed_child(&mut app, checkbox).is_none(),
        "unchecking removes the mark"
    );
}

/// A real `Pointer<Click>` bubbles up the entity hierarchy, firing the observer
/// once per ancestor. Without `propagate(false)` a single physical click would
/// be handled repeatedly — enqueuing duplicate DOM `click` events (and, for a
/// checkbox, double-toggling it). This drives a genuine propagating click on a
/// deeply-nested leaf and asserts it is handled exactly once, resolving to the
/// nearest DOM element (the ancestor walk) rather than the internal text child.
#[test]
fn click_stops_propagation_and_focuses_the_deepest_dom_node() {
    use bevy::picking::backend::HitData;
    use bevy::picking::events::{Click, Pointer};
    use bevy::picking::pointer::{Location, PointerButton, PointerId};
    use bevy::window::{PrimaryWindow, WindowRef};

    // A text `<input>` wrapped in a div. The input's managed value text is a
    // reconciler-internal child with NO DomNode — exactly the entity a real
    // click lands on. It must resolve up to the input (the ancestor walk).
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<div id='wrap'><input id='field' type='text'></div>",
    )));
    let mut app = test_app();
    let _root = mount(&mut app, dom.clone());
    app.init_resource::<PendingDomEvents>();
    app.add_observer(superui_bridge::on_pointer_click);
    app.update(); // reconcile: build the entity tree + DomNode bindings

    let (field_node, body_node) = {
        let d = dom.borrow();
        (
            d.get_element_by_id("field").unwrap(),
            d.query_selector(d.document(), "body").unwrap(),
        )
    };
    let field = entity_for(&mut app, field_node);
    // The managed InputValueText child has no DomNode — the walk must climb to
    // the input. (A plain text node WOULD carry its own DomNode and resolve to
    // itself, which is why we use the managed child here.)
    let text_child = managed_child(&mut app, field).expect("input has a managed text child");

    // Build a real propagating Pointer<Click> (same shape as the picking backend
    // and the example's BRP click injector). HitData/Location aren't read by the
    // observer, but must be constructed to trigger the event.
    let win = app
        .world_mut()
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(app.world())
        .expect("headless WindowPlugin provides a primary window");
    let target = WindowRef::Entity(win)
        .normalize(Some(win))
        .expect("normalize window ref");
    let location = Location {
        target: bevy::camera::NormalizedRenderTarget::Window(target),
        position: Vec2::ZERO,
    };
    // 0.19: `Pointer` fields (`propagate`) are private — use `Pointer::new`
    // (id, location, event, entity); `Click` gained a `count` field.
    app.world_mut().trigger(Pointer::new(
        PointerId::Mouse,
        location,
        Click {
            button: PointerButton::Primary,
            hit: HitData::new(Entity::PLACEHOLDER, 0.0, None, None),
            duration: std::time::Duration::ZERO,
            count: 1,
        },
        text_child,
    ));

    // Handled exactly once (propagation stopped): a single queued click event,
    // targeting the input — not duplicated once per ancestor up to <body>.
    let pending = app.world().resource::<PendingDomEvents>();
    assert_eq!(
        pending.0.len(),
        1,
        "propagation must stop after the first handler (one click enqueued, not one per ancestor)"
    );
    assert_eq!(
        pending.0[0].target, field_node,
        "the click resolves up to the nearest DOM element (the input), not its internal child"
    );

    // Focus landed on the input, never bubbling up to <body>.
    let focused = app.world().non_send::<UiRuntime>().focused();
    assert_eq!(focused, Some(field_node), "focus is the clicked input");
    assert_ne!(focused, Some(body_node), "focus did not bubble to <body>");
}
