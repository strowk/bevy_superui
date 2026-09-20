//! Focus is unified on InputFocus: focus/blur DOM events fire, change fires only
//! on a real edit at blur, and the runtime focus mirror tracks it.
mod support;
use support::*;

use std::cell::RefCell;
use std::rc::Rc;

use bevy::input_focus::{FocusCause, InputFocus};
use bevy::prelude::*;
use superui_bridge::{DomNode, PendingDomEvents, UiRuntime};

fn entity_for(app: &mut App, node: superui_dom::NodeId) -> Entity {
    let mut q = app.world_mut().query::<(Entity, &DomNode)>();
    q.iter(app.world()).find(|(_, d)| d.0 == node).map(|(e, _)| e).unwrap()
}

fn mount_with_focus(app: &mut App, dom: Rc<RefCell<superui_dom::Dom>>) -> Entity {
    let root = mount(app, dom);
    app.init_resource::<PendingDomEvents>();
    app.add_observer(superui_bridge::on_focus_gained);
    app.add_observer(superui_bridge::on_focus_lost);
    app.add_systems(
        Update,
        superui_bridge::drain_dom_events_system.before(superui_bridge::reconcile_system),
    );
    root
}

#[test]
fn focus_and_blur_dispatch_dom_events() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<input id='a' type='text'><input id='b' type='text'>",
    )));
    let mut app = test_app();
    let _root = mount_with_focus(&mut app, dom.clone());
    app.world_mut().non_send_mut::<UiRuntime>().run_script(
        "globalThis.log = ''; \
         document.getElementById('a').addEventListener('focus', function(){ globalThis.log += 'fa'; }); \
         document.getElementById('a').addEventListener('blur', function(){ globalThis.log += 'ba'; }); \
         document.getElementById('b').addEventListener('focus', function(){ globalThis.log += 'fb'; });",
    );
    app.update();

    let (a, b) = {
        let d = dom.borrow();
        (d.get_element_by_id("a").unwrap(), d.get_element_by_id("b").unwrap())
    };
    let (ea, eb) = (entity_for(&mut app, a), entity_for(&mut app, b));

    app.world_mut().resource_mut::<InputFocus>().set(ea, FocusCause::Pressed);
    app.update();
    app.update();
    app.world_mut().resource_mut::<InputFocus>().set(eb, FocusCause::Pressed);
    app.update();
    app.update();

    app.world_mut().non_send_mut::<UiRuntime>().run_script(
        "document.getElementById('a').setAttribute('data-log', globalThis.log);",
    );
    let log = dom.borrow().get_attribute(a, "data-log").unwrap_or("").to_string();
    assert!(log.contains("fa"), "focus fired on a: {log}");
    assert!(log.contains("ba"), "blur fired on a when focus moved: {log}");
    assert!(log.contains("fb"), "focus fired on b: {log}");
}

#[test]
fn blur_without_edit_fires_no_change() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<input id='a' type='text'><input id='b' type='text'>",
    )));
    let mut app = test_app();
    let _root = mount_with_focus(&mut app, dom.clone());
    app.world_mut().non_send_mut::<UiRuntime>().run_script(
        "globalThis.changes = 0; \
         document.getElementById('a').addEventListener('change', function(){ globalThis.changes++; });",
    );
    app.update();
    let (a, b) = {
        let d = dom.borrow();
        (d.get_element_by_id("a").unwrap(), d.get_element_by_id("b").unwrap())
    };
    let (ea, eb) = (entity_for(&mut app, a), entity_for(&mut app, b));

    app.world_mut().resource_mut::<InputFocus>().set(ea, FocusCause::Pressed);
    app.update(); app.update();
    // Move focus away without editing.
    app.world_mut().resource_mut::<InputFocus>().set(eb, FocusCause::Pressed);
    app.update(); app.update();

    app.world_mut().non_send_mut::<UiRuntime>().run_script(
        "document.getElementById('a').setAttribute('data-changes', String(globalThis.changes));",
    );
    assert_eq!(
        dom.borrow().get_attribute(a, "data-changes").unwrap_or("0"),
        "0",
        "no edit means no change event"
    );
}

#[test]
fn blur_clears_focus_mirror() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<input id='a' type='text'>",
    )));
    let mut app = test_app();
    let _root = mount_with_focus(&mut app, dom.clone());
    app.update();
    let a = dom.borrow().get_element_by_id("a").unwrap();
    let ea = entity_for(&mut app, a);
    app.world_mut().resource_mut::<InputFocus>().set(ea, FocusCause::Pressed);
    app.update(); app.update();
    assert_eq!(app.world().non_send::<UiRuntime>().focused(), Some(a));
    app.world_mut().resource_mut::<InputFocus>().clear();
    app.update(); app.update();
    assert_eq!(app.world().non_send::<UiRuntime>().focused(), None, "blur clears the mirror");
}
