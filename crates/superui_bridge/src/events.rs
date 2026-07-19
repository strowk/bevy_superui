//! Input -> DOM event seam. Filled in by Task 4/5.
use bevy::prelude::*;
use superui_dom::NodeId;

/// One pending DOM event to dispatch into JS on the next drain.
#[derive(Clone, Debug)]
pub struct PendingDomEvent {
    pub target: NodeId,
    pub type_: String,
    pub bubbles: bool,
    pub cancelable: bool,
}

/// Queue of input-originated DOM events awaiting dispatch (a Send resource, so
/// picking observers can push to it).
#[derive(Resource, Default)]
pub struct PendingDomEvents(pub Vec<PendingDomEvent>);
