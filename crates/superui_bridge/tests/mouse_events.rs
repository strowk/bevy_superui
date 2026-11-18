//! Picking -> DOM mouse events: `mouse_events_system` reads `HoverMap` + the
//! mouse pointer's `PointerLocation` + `ButtonInput<MouseButton>` and enqueues
//! `mouseover`/`mouseout`/`mouseenter`/`mouseleave`/`mousedown`/`mouseup`/
//! `mousemove`, gated per-family by `UiRuntime::mouse_listener_counts`.
mod support;
use support::*;

use std::cell::RefCell;
use std::rc::Rc;

use bevy::app::App;
use bevy::asset::AssetPlugin;
use bevy::camera::NormalizedRenderTarget;
use bevy::ecs::entity::EntityHashMap;
use bevy::image::{ImagePlugin, TextureAtlasPlugin};
use bevy::input::keyboard::KeyCode;
use bevy::input::mouse::MouseButton;
use bevy::input::{ButtonInput, InputPlugin};
use bevy::math::Vec2;
use bevy::picking::backend::HitData;
use bevy::picking::hover::HoverMap;
use bevy::picking::pointer::{Location, PointerId, PointerLocation};
use bevy::picking::PickingPlugin;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::text::TextPlugin;
use bevy::ui::UiPlugin;
use bevy::window::{PrimaryWindow, WindowPlugin, WindowRef};
use superui_bridge::{
    drain_dom_events_system, mouse_events_system, mouse_listeners_active, reconcile_system,
    sync_mouse_listener_index, MouseListenerIndex, PendingDomEvents, PointerState, UiRuntime,
};
use superui_css::SuperUiCssPlugin;

/// Same stack as `support::test_app()`, minus `InteractionPlugin`. That plugin's
/// `generate_hovermap` recomputes `HoverMap` from real backend hits every
/// `PreUpdate` — including clearing the entries of any `PointerId` with no
/// spawned pointer entity — which would stomp the hand-set `HoverMap` these
/// tests drive the system with. `PickingPlugin` is kept (it registers
/// `PointerMap`/`PointerHits`, needed to safely spawn a `PointerId::Mouse`
/// entity); `HoverMap` itself is normally init'd by the dropped plugin, so it's
/// init'd here instead.
fn mouse_test_app() -> App {
    let mut app = App::new();
    app.add_plugins((
        bevy::time::TimePlugin,
        bevy::app::TaskPoolPlugin::default(),
        AssetPlugin::default(),
        WindowPlugin::default(),
        ImagePlugin::default(),
        TextureAtlasPlugin,
        TextPlugin,
        (InputPlugin, PickingPlugin, UiPlugin),
        SuperUiCssPlugin,
    ));
    app.init_resource::<HoverMap>();
    // `bevy_ui`'s `viewport_picking` (added unconditionally by `UiPlugin`) reads
    // `bevy_picking::events::PointerState` (unrelated to our own `PointerState`
    // resource) — normally init'd by the dropped `InteractionPlugin`.
    app.init_resource::<bevy::picking::events::PointerState>();
    app.finish();
    app
}

/// Register `mouse_events_system` (gated by its run condition) -> `drain_dom_events_system`
/// -> `reconcile_system`, plus the `PendingDomEvents`/`PointerState` resources.
fn mount_mouse(app: &mut App, dom: Rc<RefCell<superui_dom::Dom>>) -> Entity {
    let root = mount(app, dom);
    app.init_resource::<PendingDomEvents>();
    app.init_resource::<PointerState>();
    app.init_resource::<MouseListenerIndex>();
    app.add_systems(
        Update,
        (
            sync_mouse_listener_index,
            mouse_events_system.run_if(mouse_listeners_active),
            drain_dom_events_system,
        )
            .chain()
            .before(reconcile_system),
    );
    root
}

/// The `NormalizedRenderTarget` for this app's primary window, for building a
/// pointer `Location`.
fn window_target(app: &mut App) -> NormalizedRenderTarget {
    let window = app
        .world_mut()
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(app.world())
        .expect("primary window");
    let target = WindowRef::Entity(window)
        .normalize(Some(window))
        .expect("normalized window");
    NormalizedRenderTarget::Window(target)
}

/// Spawn the `PointerId::Mouse` entity at `pos`, overriding the component's
/// required-component default (`PointerLocation` with no location).
fn spawn_mouse_pointer(app: &mut App, pos: Vec2) -> Entity {
    let target = window_target(app);
    app.world_mut()
        .spawn((
            PointerId::Mouse,
            PointerLocation::new(Location { target, position: pos }),
        ))
        .id()
}

/// Move the already-spawned mouse pointer entity to `pos`.
fn move_mouse_pointer(app: &mut App, pointer: Entity, pos: Vec2) {
    let mut loc = app.world_mut().get_mut::<PointerLocation>(pointer).unwrap();
    if let Some(l) = loc.location.as_mut() {
        l.position = pos;
    }
}

/// Replace `HoverMap`'s `PointerId::Mouse` entry with exactly `entities`.
fn set_hover(app: &mut App, entities: &[Entity]) {
    let mut inner = EntityHashMap::<HitData>::default();
    for &e in entities {
        inner.insert(e, HitData::new(Entity::PLACEHOLDER, 0.0, None, None));
    }
    let mut map: HashMap<PointerId, EntityHashMap<HitData>> = HashMap::default();
    map.insert(PointerId::Mouse, inner);
    app.world_mut().insert_resource(HoverMap(map));
}

/// Replace `HoverMap`'s `PointerId::Mouse` entry with `entries` (entity, depth)
/// pairs — for simulating pass-through picking, where several ancestors are
/// hovered at once at different depths.
fn set_hover_with_depth(app: &mut App, entries: &[(Entity, f32)]) {
    let mut inner = EntityHashMap::<HitData>::default();
    for &(e, depth) in entries {
        inner.insert(e, HitData::new(Entity::PLACEHOLDER, depth, None, None));
    }
    let mut map: HashMap<PointerId, EntityHashMap<HitData>> = HashMap::default();
    map.insert(PointerId::Mouse, inner);
    app.world_mut().insert_resource(HoverMap(map));
}

/// The ECS entity a DOM id resolves to (must already be reconciled).
fn entity_of(app: &App, dom: &Rc<RefCell<superui_dom::Dom>>, id: &str) -> Entity {
    let node = dom.borrow().get_element_by_id(id).unwrap();
    app.world().non_send::<UiRuntime>().entity_for(node).unwrap()
}

/// Mirror a `globalThis` value into a DOM attribute so Rust can read it back
/// (the harness has no other way to observe JS state).
fn mirror(app: &mut App, expr: &str, attr: &str, on_id: &str) {
    app.world_mut().non_send_mut::<UiRuntime>().run_script(&format!(
        "document.getElementById('{on_id}').setAttribute('{attr}', String({expr}));"
    ));
}

#[test]
fn activation_seeds_without_firing() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<div id='a'></div>",
    )));
    let mut app = mouse_test_app();
    mount_mouse(&mut app, dom.clone());
    app.update(); // mount + first reconcile: #a now has an ECS entity

    app.world_mut().non_send_mut::<UiRuntime>().run_script(
        "globalThis.overs = 0; \
         document.getElementById('a').addEventListener('mouseover', function(){ globalThis.overs++; });",
    );

    let a = entity_of(&app, &dom, "a");
    spawn_mouse_pointer(&mut app, Vec2::new(5.0, 5.0));
    set_hover(&mut app, &[a]);

    // First run after the listener exists: the cursor is already over #a, but
    // this is the seed frame — it must record state, not fire.
    app.update();

    mirror(&mut app, "globalThis.overs", "data-overs", "a");
    let node = dom.borrow().get_element_by_id("a").unwrap();
    assert_eq!(
        dom.borrow().get_attribute(node, "data-overs"),
        Some("0"),
        "the seed frame must not fire mouseover for a cursor already in place"
    );
}

#[test]
fn enter_then_leave_sequence() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<div id='a'></div><div id='b'></div>",
    )));
    let mut app = mouse_test_app();
    mount_mouse(&mut app, dom.clone());
    app.update();

    app.world_mut().non_send_mut::<UiRuntime>().run_script(
        "globalThis.order = ''; \
         document.getElementById('a').addEventListener('mouseout', function(){ globalThis.order += 'outA;'; }); \
         document.getElementById('a').addEventListener('mouseleave', function(){ globalThis.order += 'leaveA;'; }); \
         document.getElementById('b').addEventListener('mouseover', function(){ globalThis.order += 'overB;'; }); \
         document.getElementById('b').addEventListener('mouseenter', function(){ globalThis.order += 'enterB;'; });",
    );

    let a = entity_of(&app, &dom, "a");
    let b = entity_of(&app, &dom, "b");
    spawn_mouse_pointer(&mut app, Vec2::new(1.0, 1.0));
    set_hover(&mut app, &[a]);
    app.update(); // seed: hover = [a]

    set_hover(&mut app, &[b]);
    app.update(); // diff: out(a), leave(a), over(b), enter(b)

    mirror(&mut app, "globalThis.order", "data-order", "b");
    let node = dom.borrow().get_element_by_id("b").unwrap();
    assert_eq!(
        dom.borrow().get_attribute(node, "data-order"),
        Some("outA;leaveA;overB;enterB;"),
        "outs/leaves must dispatch before overs/enters, in that family order"
    );
}

#[test]
fn mousedown_up_dispatch() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<div id='a'></div>",
    )));
    let mut app = mouse_test_app();
    mount_mouse(&mut app, dom.clone());
    app.update();

    app.world_mut().non_send_mut::<UiRuntime>().run_script(
        "globalThis.downs = 0; globalThis.ups = 0; \
         globalThis.downButton = -1; globalThis.upButton = -1; \
         document.getElementById('a').addEventListener('mousedown', function(e){ \
           globalThis.downs++; globalThis.downButton = e.button; }); \
         document.getElementById('a').addEventListener('mouseup', function(e){ \
           globalThis.ups++; globalThis.upButton = e.button; });",
    );

    let a = entity_of(&app, &dom, "a");
    spawn_mouse_pointer(&mut app, Vec2::new(2.0, 2.0));
    set_hover(&mut app, &[a]);
    app.update(); // seed

    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.update(); // button edge: mousedown

    mirror(&mut app, "globalThis.downs", "data-downs", "a");
    mirror(&mut app, "globalThis.downButton", "data-down-button", "a");
    let node = dom.borrow().get_element_by_id("a").unwrap();
    assert_eq!(dom.borrow().get_attribute(node, "data-downs"), Some("1"));
    assert_eq!(dom.borrow().get_attribute(node, "data-down-button"), Some("0"));

    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .release(MouseButton::Left);
    app.update(); // button edge: mouseup

    mirror(&mut app, "globalThis.ups", "data-ups", "a");
    mirror(&mut app, "globalThis.upButton", "data-up-button", "a");
    assert_eq!(dom.borrow().get_attribute(node, "data-ups"), Some("1"));
    assert_eq!(dom.borrow().get_attribute(node, "data-up-button"), Some("0"));
}

/// `mouse_events_system` snapshots the live `ButtonInput<KeyCode>` at enqueue
/// time, so a `mousedown` fired while Shift is held carries `shiftKey` through
/// to the JS listener, and one fired with no modifier held does not.
#[test]
fn mousedown_carries_modifier_state() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<div id='a'></div>",
    )));
    let mut app = mouse_test_app();
    mount_mouse(&mut app, dom.clone());
    app.update();

    app.world_mut().non_send_mut::<UiRuntime>().run_script(
        "globalThis.shift = null; \
         document.getElementById('a').addEventListener('mousedown', function(e){ \
           globalThis.shift = e.shiftKey; });",
    );

    let a = entity_of(&app, &dom, "a");
    spawn_mouse_pointer(&mut app, Vec2::new(2.0, 2.0));
    set_hover(&mut app, &[a]);
    app.update(); // seed

    // Unmodified press: shiftKey must read false.
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.update(); // button edge: mousedown (no modifier)

    mirror(&mut app, "globalThis.shift", "data-shift", "a");
    let node = dom.borrow().get_element_by_id("a").unwrap();
    assert_eq!(
        dom.borrow().get_attribute(node, "data-shift"),
        Some("false"),
        "an unmodified mousedown must carry shiftKey=false"
    );

    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .release(MouseButton::Left);
    app.update(); // button edge: mouseup

    // Hold Shift, then press again: shiftKey must read true.
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::ShiftLeft);
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.update(); // button edge: mousedown (Shift held)

    mirror(&mut app, "globalThis.shift", "data-shift", "a");
    assert_eq!(
        dom.borrow().get_attribute(node, "data-shift"),
        Some("true"),
        "a mousedown fired while Shift is held must carry shiftKey=true"
    );
}

#[test]
fn mousemove_over_root_reaches_document() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<div id='x'></div>",
    )));
    let mut app = mouse_test_app();
    let root = mount_mouse(&mut app, dom.clone());
    app.update(); // root entity now carries DomNode(body)

    app.world_mut().non_send_mut::<UiRuntime>().run_script(
        "globalThis.moves = 0; globalThis.lastX = -1; \
         document.addEventListener('mousemove', function(e){ \
           globalThis.moves++; globalThis.lastX = e.clientX; });",
    );

    spawn_mouse_pointer(&mut app, Vec2::new(0.0, 0.0));
    // Hovered only on the root (the full-viewport node) — i.e. the cursor is
    // over empty background, not any specific element.
    set_hover(&mut app, &[root]);
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.update(); // seed

    let pointer = app
        .world_mut()
        .query::<(Entity, &PointerId)>()
        .iter(app.world())
        .find(|(_, id)| matches!(id, PointerId::Mouse))
        .map(|(e, _)| e)
        .unwrap();
    move_mouse_pointer(&mut app, pointer, Vec2::new(42.0, 7.0));
    app.update(); // move, while the left button is held, bubbles to document

    mirror(&mut app, "globalThis.moves", "data-moves", "x");
    mirror(&mut app, "globalThis.lastX", "data-last-x", "x");
    let node = dom.borrow().get_element_by_id("x").unwrap();
    assert_eq!(
        dom.borrow().get_attribute(node, "data-moves"),
        Some("1"),
        "a mousemove hovering only the root must still reach a document listener"
    );
    assert_eq!(dom.borrow().get_attribute(node, "data-last-x"), Some("42"));
}

#[test]
fn mousedown_only_does_no_move_or_hover_work() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<div id='a'></div><div id='b'></div>",
    )));
    let mut app = mouse_test_app();
    // No drain here: inspect `PendingDomEvents` directly before anything
    // drains it, so an accidental enqueue is observable even though dispatching
    // an event type with no listener would otherwise be a silent no-op.
    let root_dom = dom.clone();
    let root = mount(&mut app, root_dom);
    let _ = root;
    app.init_resource::<PendingDomEvents>();
    app.init_resource::<PointerState>();
    app.init_resource::<MouseListenerIndex>();
    app.add_systems(
        Update,
        (
            sync_mouse_listener_index,
            mouse_events_system.run_if(mouse_listeners_active),
        )
            .chain()
            .before(reconcile_system),
    );
    app.update();

    app.world_mut().non_send_mut::<UiRuntime>().run_script(
        "document.getElementById('a').addEventListener('mousedown', function(){});",
    );

    let a = entity_of(&app, &dom, "a");
    let b = entity_of(&app, &dom, "b");
    spawn_mouse_pointer(&mut app, Vec2::new(1.0, 1.0));
    set_hover(&mut app, &[a]);
    app.update(); // seed

    let pointer = app
        .world_mut()
        .query::<(Entity, &PointerId)>()
        .iter(app.world())
        .find(|(_, id)| matches!(id, PointerId::Mouse))
        .map(|(e, _)| e)
        .unwrap();
    // Change both hover (a -> b) and position in the same frame; with only a
    // mousedown listener registered, neither hover-diff nor move work must run.
    set_hover(&mut app, &[b]);
    move_mouse_pointer(&mut app, pointer, Vec2::new(99.0, 99.0));
    app.update();

    assert!(
        app.world().resource::<PendingDomEvents>().0.is_empty(),
        "no hover-diff or move events may be enqueued while only mousedown is listened for"
    );
}

/// `HoverMap`'s entry is an unordered map, and pass-through picking puts a
/// whole non-blocking ancestor chain in it at once — so the button/move target
/// has to be chosen by lowest `HitData.depth` (topmost), not "the first entry".
#[test]
fn mousedown_targets_the_topmost_hovered_node() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<div id='a'></div><div id='b'></div>",
    )));
    let mut app = mouse_test_app();
    mount_mouse(&mut app, dom.clone());
    app.update();

    // A single document-level listener, so dispatch to the wrong target is
    // still observable via `e.target.id` rather than silently matching
    // whichever node happens to carry a listener.
    app.world_mut().non_send_mut::<UiRuntime>().run_script(
        "globalThis.downTarget = ''; \
         document.addEventListener('mousedown', function(e){ globalThis.downTarget = e.target.id; });",
    );

    let a = entity_of(&app, &dom, "a");
    let b = entity_of(&app, &dom, "b");
    spawn_mouse_pointer(&mut app, Vec2::new(3.0, 3.0));
    // Both hovered at once (as pass-through picking reports for a non-blocking
    // ancestor chain); #b is the frontmost hit (lowest depth), #a is not.
    set_hover_with_depth(&mut app, &[(a, 5.0), (b, 0.0)]);
    app.update(); // seed

    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.update(); // button edge: mousedown must target #b

    mirror(&mut app, "globalThis.downTarget", "data-down-target", "a");
    let node = dom.borrow().get_element_by_id("a").unwrap();
    assert_eq!(
        dom.borrow().get_attribute(node, "data-down-target"),
        Some("b"),
        "mousedown must target the lowest-depth (topmost) hovered node"
    );
}
