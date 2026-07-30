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
/// Sets focus directly on the runtime (no pointer click needed in the test harness),
/// then writes `KeyboardInput` messages and ticks the app. Asserts:
///   - `value == "hi"` (two characters accumulated in the input),
///   - the JS `input` listener fired exactly twice (once per character), verified
///     by mirroring `globalThis.inputs` into a DOM attribute and reading it back.
#[test]
fn typing_into_focused_input_updates_value_and_fires_input() {
    use bevy::input::keyboard::{Key, KeyboardInput};
    use bevy::input::ButtonState;

    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<input id='t' type='text'>",
    )));
    let mut app = test_app();
    let root = mount(&mut app, dom.clone());
    app.init_resource::<PendingDomEvents>();
    app.add_systems(Update, superui_bridge::keyboard_events_system);

    // JS: count input events.
    app.world_mut().non_send_mut::<UiRuntime>().run_script(
        "globalThis.inputs = 0; \
         document.getElementById('t').addEventListener('input', function(){ globalThis.inputs++; });",
    );
    app.update();

    // Focus the input, then type "hi".
    let node = dom.borrow().get_element_by_id("t").unwrap();
    app.world_mut().non_send_mut::<UiRuntime>().set_focus(Some(node));

    for ch in ["h", "i"] {
        app.world_mut().write_message(KeyboardInput {
            key_code: bevy::input::keyboard::KeyCode::KeyH, // placeholder; logical_key drives text
            logical_key: Key::Character(ch.into()),
            state: ButtonState::Pressed,
            repeat: false,
            window: Entity::PLACEHOLDER,
            text: None,
        });
        app.update();
    }

    assert_eq!(dom.borrow().value(node), "hi");

    // Settle one reconcile so the input entity's Text reflects the final value
    // (keyboard_events_system and reconcile_system are unordered in this harness).
    app.update();

    // The typed value renders in the input's managed `InputValueText` child (the
    // input element itself is a container so it can draw a border). The child is
    // focused here, so it shows the value plus a caret glyph ("hi" + "|" or " ").
    let input_ent = {
        let mut q = app.world_mut().query::<(Entity, &superui_bridge::DomNode)>();
        q.iter(app.world())
            .find(|(_, d)| d.0 == node)
            .map(|(e, _)| e)
            .unwrap()
    };
    let child_text = {
        let kids = app.world().get::<Children>(input_ent).unwrap().to_vec();
        kids.into_iter()
            .find(|&k| app.world().get::<superui_bridge::InputValueText>(k).is_some())
            .map(|k| app.world().get::<Text>(k).unwrap().0.clone())
            .expect("managed input text child")
    };
    assert!(
        child_text.starts_with("hi"),
        "expected the child text to start with the typed value, got {child_text:?}"
    );

    // Mirror globalThis.inputs into the DOM so Rust can read it back
    // (same pattern as checkbox_click_toggles_checked_and_fires_change).
    app.world_mut().non_send_mut::<UiRuntime>().run_script(
        "document.getElementById('t').setAttribute('data-inputs', String(globalThis.inputs));",
    );
    let data_inputs = dom.borrow().get_attribute(node, "data-inputs").unwrap_or("").to_string();
    assert_eq!(data_inputs, "2", "input listener must have fired once per character (twice total)");

    let _ = root;
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
