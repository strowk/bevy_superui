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

// Regression for a `value=0` reconcile: `Slider`'s `#[require(SliderValue)]`
// auto-inserts the matching `SliderValue(0.0)` default in the same `insert`
// call that adds `Slider`, so this exercises the path where the component
// already reads 0.0 before `sync_range_input`'s own `SliderValue` write would
// run. `range_synced` (the bookkeeping this guards) is `pub(crate)` and not
// reachable from this integration test; this only asserts the observable
// component value does not regress.
#[test]
fn range_value_zero_reconciles_without_panicking() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<input id='r' type='range' min='0' max='40' value='0'>",
    )));
    let mut app = test_app();
    let _root = mount(&mut app, dom.clone());
    app.update();
    let node = dom.borrow().get_element_by_id("r").unwrap();
    let e = slider_entity(&mut app, node);
    assert_eq!(app.world().get::<SliderValue>(e).copied(), Some(SliderValue(0.0)));
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
