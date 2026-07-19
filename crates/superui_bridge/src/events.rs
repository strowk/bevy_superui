//! Input -> DOM event seam. `bevy_picking`/keyboard produce DOM events, which we
//! dispatch into JS (W3C capture/bubble, synchronous) and then reconcile.

use bevy::picking::events::{Click, Pointer};
use bevy::prelude::*;
use superui_dom::NodeId;

use crate::runtime::{DomNode, UiRuntime};

/// One pending DOM event to dispatch into JS on the next drain.
#[derive(Clone, Debug)]
pub struct PendingDomEvent {
    pub target: NodeId,
    pub type_: String,
    pub bubbles: bool,
    pub cancelable: bool,
}

impl PendingDomEvent {
    pub fn new(target: NodeId, type_: &str) -> Self {
        PendingDomEvent {
            target,
            type_: type_.to_string(),
            bubbles: true,
            cancelable: true,
        }
    }
}

/// Queue of input-originated DOM events awaiting dispatch. Send resource so
/// picking observers can push to it.
#[derive(Resource, Default)]
pub struct PendingDomEvents(pub Vec<PendingDomEvent>);

/// Core logic for a click on a DOM node: enqueue a `"click"` event and,
/// for a checkbox `<input>`, also mirror the native toggle (flip `checked`)
/// and enqueue a subsequent `"change"` event.
///
/// Extracted as a free function so both the observer and the test harness can
/// call it without needing to construct a `Pointer<Click>` event.
pub fn click_effect(rt: &UiRuntime, node: NodeId, pending: &mut PendingDomEvents) {
    let is_checkbox = {
        let d = rt.dom.borrow();
        matches!(tag_of(&d, node).as_deref(), Some("input"))
            && d.get_attribute(node, "type") == Some("checkbox")
    };

    pending.0.push(PendingDomEvent::new(node, "click"));
    if is_checkbox {
        let now = !rt.dom.borrow().checked(node);
        rt.dom.borrow_mut().set_checked(node, now);
        pending.0.push(PendingDomEvent::new(node, "change"));
    }
}

/// Observer: a pointer click on a UI entity becomes a DOM `click` on its node.
/// For a checkbox input, also mirror the native toggle: flip DOM `checked` and
/// enqueue a `change` event (dispatched after the click).
pub fn on_pointer_click(
    ev: On<Pointer<Click>>,
    nodes: Query<&DomNode>,
    rt: NonSend<UiRuntime>,
    mut pending: ResMut<PendingDomEvents>,
) {
    let Ok(dom_node) = nodes.get(ev.event().entity) else {
        return;
    };
    let node = dom_node.0;
    click_effect(&rt, node, &mut pending);
}

fn tag_of(dom: &superui_dom::Dom, node: NodeId) -> Option<String> {
    match dom.get(node).map(|n| &n.kind) {
        Some(superui_dom::NodeKind::Element(e)) => Some(e.tag.clone()),
        _ => None,
    }
}

/// Exclusive system: dispatch queued DOM events into the engine, then mark dirty.
pub fn drain_dom_events_system(world: &mut World) {
    let queued = std::mem::take(&mut world.resource_mut::<PendingDomEvents>().0);
    if queued.is_empty() {
        return;
    }
    let Some(mut rt) = world.remove_non_send_resource::<UiRuntime>() else {
        return;
    };
    for e in queued {
        use superui_js::JsEngine;
        rt.engine
            .dispatch_event(e.target, &e.type_, e.bubbles, e.cancelable);
    }
    rt.dirty = true;
    world.insert_non_send_resource(rt);
}
