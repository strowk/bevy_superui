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
        .non_send_mut::<UiRuntime>()
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

/// Test 3: keyboard events update a text input's DOM value and fire `input`.
///
/// Drives real Bevy editing through `InputFocus`: focuses the input entity, then
/// writes `KeyboardInput` messages and ticks the app. Asserts:
///   - `value == "hi"` (two characters accumulated in the input),
///   - the JS `input` listener fired at least once, verified by mirroring
///     `globalThis.inputs` into a DOM attribute and reading it back.
#[test]
fn typing_into_focused_input_updates_value_and_fires_input() {
    use bevy::input::keyboard::{Key, KeyCode, KeyboardInput};
    use bevy::input::ButtonState;
    use bevy::input_focus::{FocusCause, InputFocus};

    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<input id='t' type='text'>",
    )));
    let mut app = test_app();
    let _root = mount(&mut app, dom.clone());
    app.init_resource::<PendingDomEvents>();
    // Full input pipeline: emit input events from EditableText edits, then drain.
    app.add_systems(
        Update,
        (
            superui_bridge::editable_input_events_system,
            superui_bridge::keyboard_events_system,
            drain_dom_events_system,
        )
            .chain()
            .before(superui_bridge::reconcile_system),
    );

    app.world_mut().non_send_mut::<UiRuntime>().run_script(
        "globalThis.inputs = 0; \
         document.getElementById('t').addEventListener('input', function(){ globalThis.inputs++; });",
    );
    app.update();

    // Focus the input entity via InputFocus (the source of truth).
    let node = dom.borrow().get_element_by_id("t").unwrap();
    let input_ent = {
        let mut q = app.world_mut().query::<(Entity, &superui_bridge::DomNode)>();
        q.iter(app.world()).find(|(_, d)| d.0 == node).map(|(e, _)| e).unwrap()
    };
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(input_ent, FocusCause::Pressed);
    app.update();

    for ch in ["h", "i"] {
        // bevy_ui_widgets 0.19 inserts from KeyboardInput.text, not logical_key.
        app.world_mut().write_message(KeyboardInput {
            key_code: KeyCode::KeyH,
            logical_key: Key::Character(ch.into()),
            state: ButtonState::Pressed,
            repeat: false,
            window: Entity::PLACEHOLDER,
            text: Some(ch.into()),
        });
        app.update();
    }
    app.update(); // settle: apply_text_edits -> Changed -> input event -> drain

    assert_eq!(dom.borrow().value(node), "hi", "typed value reaches the DOM");

    app.world_mut().non_send_mut::<UiRuntime>().run_script(
        "document.getElementById('t').setAttribute('data-inputs', String(globalThis.inputs));",
    );
    let n: i32 = dom
        .borrow()
        .get_attribute(node, "data-inputs")
        .unwrap_or("0")
        .parse()
        .unwrap_or(0);
    assert!(n >= 1, "the input listener fired at least once, got {n}");
}

/// A JSX-style controlled input sets `.value` from JS every render. Pushing that
/// into the EditableText buffer must NOT re-emit `input` (which would loop).
#[test]
fn js_value_set_does_not_re_emit_input() {
    use bevy::input_focus::InputFocus;
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<input id='t' type='text'>",
    )));
    let mut app = test_app();
    let _root = mount(&mut app, dom.clone());
    app.init_resource::<PendingDomEvents>();
    app.add_systems(
        Update,
        (superui_bridge::editable_input_events_system, drain_dom_events_system)
            .chain()
            .before(superui_bridge::reconcile_system),
    );
    app.world_mut().non_send_mut::<UiRuntime>().run_script(
        "globalThis.inputs = 0; \
         document.getElementById('t').addEventListener('input', function(){ globalThis.inputs++; });",
    );
    app.update();

    // JS sets the value (controlled input).
    app.world_mut()
        .non_send_mut::<UiRuntime>()
        .run_script("document.getElementById('t').value = 'abc';");
    app.update(); // reconcile pushes into buffer
    app.update(); // apply_text_edits -> Changed -> compare -> (no input)
    app.update();

    let node = dom.borrow().get_element_by_id("t").unwrap();
    app.world_mut().non_send_mut::<UiRuntime>().run_script(
        "document.getElementById('t').setAttribute('data-inputs', String(globalThis.inputs));",
    );
    assert_eq!(
        dom.borrow().get_attribute(node, "data-inputs").unwrap_or("0"),
        "0",
        "setting .value from JS must not fire input"
    );
}

/// An input with an initial value seeds the buffer without firing `input` on mount.
#[test]
fn initial_value_does_not_fire_input() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<input id='t' type='text' value='seed'>",
    )));
    let mut app = test_app();
    let _root = mount(&mut app, dom.clone());
    app.init_resource::<PendingDomEvents>();
    app.add_systems(
        Update,
        (superui_bridge::editable_input_events_system, drain_dom_events_system)
            .chain()
            .before(superui_bridge::reconcile_system),
    );
    app.world_mut().non_send_mut::<UiRuntime>().run_script(
        "globalThis.inputs = 0; \
         document.getElementById('t').addEventListener('input', function(){ globalThis.inputs++; });",
    );
    for _ in 0..4 { app.update(); }
    let node = dom.borrow().get_element_by_id("t").unwrap();
    app.world_mut().non_send_mut::<UiRuntime>().run_script(
        "document.getElementById('t').setAttribute('data-inputs', String(globalThis.inputs));",
    );
    assert_eq!(
        dom.borrow().get_attribute(node, "data-inputs").unwrap_or("0"),
        "0",
        "seeding an initial value must not fire input"
    );
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
    app.world_mut().non_send_mut::<UiRuntime>().run_script(
        "globalThis.changes = 0; \
         document.getElementById('c').addEventListener('change', function(){ globalThis.changes++; });",
    );
    app.update();

    let node = dom.borrow().get_element_by_id("c").unwrap();

    // Before click: unchecked.
    assert!(!dom.borrow().checked(node));

    // Call the observer's core logic directly with world access.
    {
        let rt = app.world().non_send::<UiRuntime>();
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
    app.world_mut().non_send_mut::<UiRuntime>().run_script(
        "document.getElementById('c').setAttribute('data-changes', String(globalThis.changes));",
    );
    let data_changes = dom.borrow().get_attribute(node, "data-changes").unwrap_or("").to_string();
    assert_eq!(data_changes, "1", "change listener must have fired exactly once");
}

/// Regression: superui's click observer is global — it runs for every
/// `Pointer<Click>` in the app, including entities that belong to the host game.
/// Claiming those (stopping propagation) cancels bubbling to the handler on their
/// ancestor, which is where a Bevy UI puts it: the pick lands on the `Text` child
/// and only reaches the button by propagation. So while a UI is mounted, every
/// button in the host app goes dead.
#[test]
fn a_click_on_a_foreign_entity_still_reaches_its_ancestors_handler() {
    use bevy::camera::NormalizedRenderTarget;
    use bevy::picking::backend::HitData;
    use bevy::picking::events::{Click, Pointer};
    use bevy::picking::pointer::{Location, PointerButton, PointerId};
    use bevy::window::{PrimaryWindow, WindowRef};

    #[derive(Resource, Default)]
    struct AncestorRan(bool);

    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<button id='b'>superui</button>",
    )));
    let mut app = test_app();
    mount_with_input(&mut app, dom.clone());
    app.init_resource::<AncestorRan>();
    app.update(); // mount + first reconcile

    // A host-app widget with nothing to do with superui: handler on the parent,
    // pickable child underneath it.
    let child = app.world_mut().spawn(Node::default()).id();
    app.world_mut()
        .spawn(Node::default())
        .add_child(child)
        .observe(|_: On<Pointer<Click>>, mut ran: ResMut<AncestorRan>| ran.0 = true);

    // A real `Pointer<Click>` on the child — the entity-only path can't reproduce
    // a propagation bug.
    let camera = app.world_mut().spawn(Camera2d).id();
    let window = app
        .world_mut()
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(app.world())
        .expect("primary window");
    let target = WindowRef::Entity(window)
        .normalize(Some(window))
        .expect("normalized window");
    app.world_mut().trigger(Pointer::new(
        PointerId::Mouse,
        Location {
            target: NormalizedRenderTarget::Window(target),
            position: Vec2::ZERO,
        },
        Click {
            button: PointerButton::Primary,
            hit: HitData::new(camera, 0.0, None, None),
            duration: std::time::Duration::ZERO,
            count: 1,
        },
        child,
    ));

    assert!(
        app.world().resource::<AncestorRan>().0,
        "superui must leave clicks it does not own alone so they keep bubbling"
    );
}
