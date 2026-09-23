mod support;
use support::*;
use std::cell::RefCell;
use std::rc::Rc;
use bevy::prelude::*;
use bevy::ui_widgets::{SliderRange, SliderStep, SliderValue};
use superui_bridge::{DomNode, UiRuntime};

fn slider_entity(app: &mut App, node: superui_dom::NodeId) -> Entity {
    let mut q = app.world_mut().query::<(Entity, &DomNode)>();
    q.iter(app.world()).find(|(_, d)| d.0 == node).map(|(e, _)| e).unwrap()
}

#[test]
fn range_input_gets_slider_components_with_parsed_attrs() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<input id='r' type='range' min='0' max='200' step='10' value='50'>",
    )));
    let mut app = test_app();
    let _root = mount(&mut app, dom.clone());
    app.update();

    let node = dom.borrow().get_element_by_id("r").unwrap();
    let e = slider_entity(&mut app, node);
    let w = app.world();
    assert_eq!(w.get::<SliderValue>(e).copied(), Some(SliderValue(50.0)));
    let range = w.get::<SliderRange>(e).copied().unwrap();
    assert_eq!((range.start(), range.end()), (0.0, 200.0));
    assert_eq!(w.get::<SliderStep>(e).cloned(), Some(SliderStep(10.0)));
    // Not an EditableText (it must not fall into the text-input path).
    assert!(w.get::<bevy::text::EditableText>(e).is_none());
}

#[test]
fn range_defaults_apply_and_value_clamps() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<input id='r' type='range' value='999'>",
    )));
    let mut app = test_app();
    let _root = mount(&mut app, dom.clone());
    app.update();

    let node = dom.borrow().get_element_by_id("r").unwrap();
    let e = slider_entity(&mut app, node);
    let range = app.world().get::<SliderRange>(e).copied().unwrap();
    assert_eq!((range.start(), range.end()), (0.0, 100.0)); // HTML defaults
    // value clamped to max
    assert_eq!(app.world().get::<SliderValue>(e).copied(), Some(SliderValue(100.0)));
}

#[test]
fn range_value_absent_defaults_to_midpoint() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<input id='r' type='range' min='0' max='40'>",
    )));
    let mut app = test_app();
    let _root = mount(&mut app, dom.clone());
    app.update();
    let node = dom.borrow().get_element_by_id("r").unwrap();
    let e = slider_entity(&mut app, node);
    assert_eq!(app.world().get::<SliderValue>(e).copied(), Some(SliderValue(20.0)));
}

// Regression for a `value=0` mount: `Slider`'s `#[require(SliderValue)]`
// auto-inserts the matching `SliderValue(0.0)` default in the same `insert`
// call that adds `Slider`, so `sync_range_input` sees the component already
// reading 0.0 before its own write would run — the masking case Task 2 fixed.
// Pinned directly via `range_synced_value`: the old AND-guard wrote
// `range_synced` only inside the same `if` that wrote `SliderValue`, so the
// already-matching default skipped that `if` and left `range_synced` at
// `None` (an external `value=0` change would then misread as its own echo).
#[test]
fn range_value_zero_reconciles_and_records_range_synced() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<input id='r' type='range' min='0' max='40' value='0'>",
    )));
    let mut app = test_app();
    let _root = mount(&mut app, dom.clone());
    app.update();
    let node = dom.borrow().get_element_by_id("r").unwrap();
    let e = slider_entity(&mut app, node);
    assert_eq!(app.world().get::<SliderValue>(e).copied(), Some(SliderValue(0.0)));
    assert_eq!(
        app.world().non_send::<UiRuntime>().range_synced_value(node),
        Some(0.0),
        "range_synced must record 0.0 even though SliderValue's require-default already matched"
    );
}

// A malformed `min > max` must not produce an inverted `SliderRange`; both
// bounds are normalized before construction.
#[test]
fn range_min_greater_than_max_normalizes() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<input id='r' type='range' min='100' max='0' value='50'>",
    )));
    let mut app = test_app();
    let _root = mount(&mut app, dom.clone());
    app.update();
    let node = dom.borrow().get_element_by_id("r").unwrap();
    let e = slider_entity(&mut app, node);
    let range = app.world().get::<SliderRange>(e).copied().unwrap();
    assert_eq!((range.start(), range.end()), (0.0, 100.0));
}

#[test]
fn range_spawns_three_parts_absent_from_dom_map_and_persisting() {
    use bevy::ui_widgets::SliderThumb;
    use superui_css::SliderPart;

    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<input id='r' type='range'>",
    )));
    let mut app = test_app();
    let _root = mount(&mut app, dom.clone());
    app.update();

    let node = dom.borrow().get_element_by_id("r").unwrap();
    let host = slider_entity(&mut app, node);

    // Exactly three children, one of each part; thumb also carries SliderThumb.
    let children: Vec<Entity> =
        app.world().get::<Children>(host).map(|c| c.iter().collect()).unwrap_or_default();
    assert_eq!(children.len(), 3);
    let parts: Vec<SliderPart> =
        children.iter().filter_map(|&c| app.world().get::<SliderPart>(c).copied()).collect();
    assert!(parts.contains(&SliderPart::Track));
    assert!(parts.contains(&SliderPart::Fill));
    assert!(parts.contains(&SliderPart::Thumb));
    let thumb = children.iter().copied()
        .find(|&c| app.world().get::<SliderPart>(c) == Some(&SliderPart::Thumb)).unwrap();
    assert!(app.world().get::<SliderThumb>(thumb).is_some());

    // Parts are not registered as DOM nodes.
    let rt = app.world().non_send::<UiRuntime>();
    for &c in &children {
        assert!(rt.node_for(c).is_none(), "part entity leaked into the DOM map");
    }

    // A second reconcile reuses the same part entities (no respawn). `dirty`
    // is `pub` and this is the pattern other bridge tests use to force a pass
    // without a real DOM mutation.
    app.world_mut().non_send_mut::<UiRuntime>().dirty = true;
    app.update();
    let children2: Vec<Entity> =
        app.world().get::<Children>(host).map(|c| c.iter().collect()).unwrap_or_default();
    assert_eq!(children2, children, "parts must persist across reconciles");
}

#[test]
fn value_change_mirrors_value_and_emits_input_then_change() {
    use bevy::ui_widgets::{SliderValue, ValueChange};
    use superui_bridge::{drain_dom_events_system, on_slider_value_change, PendingDomEvents};

    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<input id='r' type='range' min='0' max='100' step='1' value='0'>",
    )));
    let mut app = test_app();
    let _root = mount(&mut app, dom.clone());
    app.init_resource::<PendingDomEvents>();
    app.add_observer(on_slider_value_change);
    app.add_systems(Update, drain_dom_events_system.before(superui_bridge::reconcile_system));
    app.update();

    let node = dom.borrow().get_element_by_id("r").unwrap();
    let e = slider_entity(&mut app, node);

    app.world_mut().non_send_mut::<UiRuntime>().run_script(
        "globalThis.log=[]; \
         let el=document.getElementById('r'); \
         el.addEventListener('input', (e)=>globalThis.log.push('input:'+el.value+':'+e.cancelable)); \
         el.addEventListener('change', ()=>globalThis.log.push('change:'+el.value));",
    );
    app.update();

    // Mid-drag (not final): input only.
    app.world_mut().trigger(ValueChange::<f32> { source: e, value: 30.0, is_final: false });
    app.update();
    // Commit (final): input + change.
    app.world_mut().trigger(ValueChange::<f32> { source: e, value: 42.0, is_final: true });
    app.update();

    assert_eq!(dom.borrow().value(node), "42");
    assert_eq!(app.world().get::<SliderValue>(e).copied(), Some(SliderValue(42.0)));

    app.world_mut().non_send_mut::<UiRuntime>().run_script(
        "document.getElementById('r').setAttribute('data-log', globalThis.log.join(','));",
    );
    let log = dom.borrow().get_attribute(node, "data-log").unwrap_or("").to_string();
    // `:false` on both `input`s confirms the non-cancelable requirement.
    assert_eq!(log, "input:30:false,input:42:false,change:42");
}

// `range_synced` (the echo-guard bookkeeping `on_slider_value_change` also writes)
// is `pub(crate)` and unreachable from this integration test, so this only asserts
// the observable DOM value rounds correctly for a fractional step. The fix this
// guards — recording the reparsed/formatted value in `range_synced`, not the raw
// float — is a code-level correctness fix (same situation as Task 2's finding-1):
// this test exercises the same formatting path but can't distinguish "stored raw
// 0.30000004" from "stored reparsed 0.3" without a reconcile round-trip.
#[test]
fn value_change_rounds_fractional_step_in_dom() {
    use bevy::ui_widgets::ValueChange;
    use superui_bridge::{drain_dom_events_system, on_slider_value_change, PendingDomEvents};

    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<input id='r' type='range' min='0' max='1' step='0.1' value='0'>",
    )));
    let mut app = test_app();
    let _root = mount(&mut app, dom.clone());
    app.init_resource::<PendingDomEvents>();
    app.add_observer(on_slider_value_change);
    app.add_systems(Update, drain_dom_events_system.before(superui_bridge::reconcile_system));
    app.update();

    let node = dom.borrow().get_element_by_id("r").unwrap();
    let e = slider_entity(&mut app, node);

    app.world_mut().trigger(ValueChange::<f32> { source: e, value: 0.30000004, is_final: true });
    app.update();

    assert_eq!(dom.borrow().value(node), "0.3");
}

// Pins the full two-way loop: a JS-side `.value=` write must move the ECS
// `SliderValue` (thumb) on the following reconcile, but must NOT be mistaken
// for a widget-driven change and re-emit `input`/`change` — that would be an
// infinite echo (JS write -> DOM event -> JS write -> ...).
#[test]
fn js_set_value_moves_thumb_without_reemitting() {
    use bevy::ui_widgets::SliderValue;
    use superui_bridge::{drain_dom_events_system, on_slider_value_change, PendingDomEvents};

    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<input id='r' type='range' min='0' max='100' value='0'>",
    )));
    let mut app = test_app();
    let _root = mount(&mut app, dom.clone());
    app.init_resource::<PendingDomEvents>();
    app.add_observer(on_slider_value_change);
    app.add_systems(Update, drain_dom_events_system.before(superui_bridge::reconcile_system));
    app.update();
    let node = dom.borrow().get_element_by_id("r").unwrap();
    let e = slider_entity(&mut app, node);

    app.world_mut().non_send_mut::<UiRuntime>().run_script(
        "globalThis.n=0; let el=document.getElementById('r'); \
         el.addEventListener('input', ()=>globalThis.n++); \
         el.addEventListener('change', ()=>globalThis.n++); \
         el.value='30';",
    );
    app.update(); // reconcile picks up the JS value change
    app.update();

    assert_eq!(app.world().get::<SliderValue>(e).copied(), Some(SliderValue(30.0)));
    assert_eq!(
        app.world().non_send::<UiRuntime>().range_synced_value(node),
        Some(30.0),
        "range_synced must track the JS-driven value so the next reconcile reads it as an echo, not a new external change"
    );
    app.world_mut().non_send_mut::<UiRuntime>().run_script(
        "document.getElementById('r').setAttribute('data-n', String(globalThis.n));",
    );
    assert_eq!(dom.borrow().get_attribute(node, "data-n").as_deref(), Some("0"),
        "a JS-driven value must not re-emit input/change");
}

// Same loop as above, but landing on exactly 0.0: `SliderValue`'s zero is also
// `Slider`'s `#[require]` default, so this pins that the echo guard still
// treats a real JS write as a real write (updates `SliderValue`/`range_synced`,
// no re-emit) rather than special-casing 0.0. It does NOT exercise the
// mount-time masking case (`Slider` inserted for the first time with an
// already-zero DOM value) — see `range_value_zero_reconciles_without_panicking`
// for that.
#[test]
fn js_set_value_zero_moves_thumb_without_reemitting() {
    use bevy::ui_widgets::SliderValue;
    use superui_bridge::{drain_dom_events_system, on_slider_value_change, PendingDomEvents};

    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<input id='r' type='range' min='0' max='100' value='50'>",
    )));
    let mut app = test_app();
    let _root = mount(&mut app, dom.clone());
    app.init_resource::<PendingDomEvents>();
    app.add_observer(on_slider_value_change);
    app.add_systems(Update, drain_dom_events_system.before(superui_bridge::reconcile_system));
    app.update();
    let node = dom.borrow().get_element_by_id("r").unwrap();
    let e = slider_entity(&mut app, node);
    assert_eq!(app.world().get::<SliderValue>(e).copied(), Some(SliderValue(50.0)));

    app.world_mut().non_send_mut::<UiRuntime>().run_script(
        "globalThis.n=0; let el=document.getElementById('r'); \
         el.addEventListener('input', ()=>globalThis.n++); \
         el.addEventListener('change', ()=>globalThis.n++); \
         el.value='0';",
    );
    app.update(); // reconcile picks up the JS value change
    app.update();

    assert_eq!(app.world().get::<SliderValue>(e).copied(), Some(SliderValue(0.0)));
    app.world_mut().non_send_mut::<UiRuntime>().run_script(
        "document.getElementById('r').setAttribute('data-n', String(globalThis.n));",
    );
    assert_eq!(dom.borrow().get_attribute(node, "data-n").as_deref(), Some("0"),
        "a JS-driven value of 0 must not re-emit input/change");
}

#[test]
fn fractional_step_rounds_value() {
    assert_eq!(superui_bridge::format_slider_value(0.30000004, 0.1), "0.3");
    assert_eq!(superui_bridge::format_slider_value(42.0, 1.0), "42");
    assert_eq!(superui_bridge::format_slider_value(0.126, 0.01), "0.13");
}
