//! Picking -> DOM mouse-event seam. Reads `bevy_picking`'s per-frame
//! [`HoverMap`] + the mouse pointer's [`PointerLocation`] + `ButtonInput<MouseButton>`,
//! diffs them against last frame's [`PointerState`], and enqueues `mouseover`/
//! `mouseout`/`mouseenter`/`mouseleave`/`mousedown`/`mouseup`/`mousemove` as
//! [`PendingDomEvent`]s (drained into JS the same way clicks/keys are).
//!
//! Gated by [`mouse_listeners_active`] (a run condition) so a UI with no mouse
//! listener anywhere pays nothing per frame — not even the seed.
//!
//! ## Per-frame algorithm
//! 1. Resolve the current hovered [`NodeId`] set: walk every entity in
//!    `HoverMap`'s `PointerId::Mouse` entry up to its nearest [`DomNode`]
//!    ancestor (`resolve_dom_node`, same as clicks), deduping (set semantics
//!    for the hover diff — order doesn't matter there). Separately track
//!    `top`: the resolved node whose `HitData.depth` is lowest. `HoverMap`'s
//!    entry is an unordered map, and under pass-through picking a whole
//!    non-blocking ancestor chain can be in it at once, so "first entry" would
//!    be an arbitrary node, not the one under the cursor; `bevy_ui`'s picking
//!    backend gives the frontmost hit `depth = 0.0` and increments going
//!    back, so lowest depth is topmost.
//! 2. **Seed**: the first time this system runs (`!state.seeded`), record the
//!    current hover set/position/button mask and return — no events. Because
//!    the system only runs while a listener exists (the run condition), this
//!    is "the first activation after a listener was registered", not "frame
//!    zero of the app" — so a cursor already sitting over a node when its
//!    first listener is added does not fire a spurious `mouseover`.
//! 3. **Hover diff** (only if `counts.any_hover()`): nodes that left `current`
//!    get `mouseout` (bubbles) then `mouseleave` (doesn't); nodes that joined
//!    it get `mouseover` (bubbles) then `mouseenter` (doesn't). Outs/leaves are
//!    enqueued before overs/enters (so a move from A to B reads out(A),
//!    leave(A), over(B), enter(B)).
//! 4. **Buttons** (only if `counts.any_button()`): compute this frame's W3C
//!    `buttons` bitmask (`left=1, right=2, middle=4`) from `ButtonInput`; for
//!    each bit that flipped since `state.buttons`, enqueue `mousedown` (press)
//!    or `mouseup` (release) at `top`. No `top` (cursor outside the UI
//!    entirely) means skip — a known, narrow gap: a drag that started inside
//!    the UI but is released with the cursor past its edge loses the
//!    `mouseup`. The brief accepts this; the common case (held button,
//!    pointer still over the full-viewport root) is covered by the hover
//!    resolution itself.
//! 5. **Move** (only if `counts.any_move()`): if `position` changed, enqueue
//!    one `mousemove` at `top`. This is also how a `document`-level drag-move
//!    listener gets reached: the root DOM node is a full-viewport hoverable
//!    node (see `reconcile.rs`), so hovering empty background still resolves
//!    to it as `top`, and the root's shadow-DOM node has `document` as its
//!    bubble-chain parent (Task 5) — no document sentinel `NodeId` is ever
//!    constructed here.
//! 6. Update `state` to this frame's hover/position/buttons.

use bevy::ecs::hierarchy::ChildOf;
use bevy::input::keyboard::KeyCode;
use bevy::input::mouse::MouseButton;
use bevy::input::ButtonInput;
use bevy::math::Vec2;
use bevy::picking::hover::HoverMap;
use bevy::picking::pointer::{PointerId, PointerLocation};
use bevy::prelude::*;
use superui_dom::NodeId;
use superui_js::opwire::MouseListenerCounts;
use superui_js::MousePayload;

use crate::events::{modifiers_now, resolve_dom_node, PendingDomEvent, PendingDomEvents};
use crate::runtime::{DomNode, UiRuntime};

/// W3C `MouseEvent.buttons` bit for the left/primary button.
const BTN_LEFT: u8 = 1 << 0;
/// W3C `MouseEvent.buttons` bit for the right/secondary button.
const BTN_RIGHT: u8 = 1 << 1;
/// W3C `MouseEvent.buttons` bit for the middle/auxiliary button.
const BTN_MIDDLE: u8 = 1 << 2;

/// Last frame's hover set / pointer position / button mask, diffed each frame
/// by [`mouse_events_system`] to decide which DOM events to enqueue.
#[derive(Resource, Default)]
pub struct PointerState {
    /// Nodes the pointer was over last frame (set semantics — order unused).
    pub hover: Vec<NodeId>,
    /// The mouse pointer's last known `client` position, or `None` before the
    /// first observed location.
    pub position: Option<Vec2>,
    /// W3C `buttons` bitmask last frame (`BTN_LEFT | BTN_RIGHT | BTN_MIDDLE`).
    pub buttons: u8,
    /// Whether the first frame has already seeded `hover`/`position`/`buttons`
    /// without firing events (see module doc, "Seed").
    pub seeded: bool,
}

/// Send mirror of [`UiRuntime`]'s (`NonSend`) mouse-listener counts, kept
/// fresh each frame by [`sync_mouse_listener_index`] (a main-thread-only
/// system) so that [`mouse_listeners_active`] — a run condition, which Bevy's
/// multithreaded executor may evaluate on any worker thread — never has to
/// touch `NonSend` data itself. Touching `NonSend` off the main thread aborts
/// the process (`bevy_ecs` enforces this); reading this plain `Resource`
/// instead is always safe regardless of which thread the scheduler runs it on.
#[derive(Resource, Clone, Copy, Default)]
pub struct MouseListenerIndex(pub MouseListenerCounts);

impl MouseListenerIndex {
    /// Any mouse listener at all is registered anywhere.
    pub fn any(&self) -> bool {
        self.0.any()
    }

    /// Any of the hover-family listeners (mouseover/out/enter/leave).
    pub fn any_hover(&self) -> bool {
        self.0.any_hover()
    }

    /// Any of the button-family listeners (mousedown/up).
    pub fn any_button(&self) -> bool {
        self.0.any_button()
    }

    /// A mousemove listener is registered anywhere.
    pub fn any_move(&self) -> bool {
        self.0.any_move()
    }
}

/// Main-thread-only system that copies `UiRuntime`'s live mouse-listener
/// counts into the Send [`MouseListenerIndex`] resource. Taking `NonSend<UiRuntime>`
/// forces Bevy to schedule this on the main thread (the only thread allowed to
/// touch it), which is exactly why this system exists: it is the single place
/// that bridges the NonSend runtime to the Send index the rest of the mouse
/// pipeline — including the [`mouse_listeners_active`] run condition, which the
/// executor may run on a worker thread — reads instead. The copy is a plain
/// `[u32; 7]` copy, O(1) regardless of listener count, so running it every
/// frame (even with zero listeners) doesn't reintroduce the per-frame cost the
/// gate exists to avoid; the expensive hover-diff/picking work downstream
/// stays gated on the index's `any()`/`any_hover()`/etc.
pub fn sync_mouse_listener_index(rt: NonSend<UiRuntime>, mut idx: ResMut<MouseListenerIndex>) {
    idx.0 = rt.mouse_listener_counts();
}

/// Run condition: [`mouse_events_system`] (and its seeding) only matters once
/// at least one mouse listener of any family exists anywhere in the mounted
/// DOM. Reads the Send [`MouseListenerIndex`] (kept fresh by
/// [`sync_mouse_listener_index`]) rather than the `NonSend` `UiRuntime`, so this
/// run condition is safe to evaluate from any thread — see the type's doc.
/// Absent index (no UI ever mounted) also reads as "inactive".
pub fn mouse_listeners_active(idx: Option<Res<MouseListenerIndex>>) -> bool {
    idx.is_some_and(|idx| idx.any())
}

/// This frame's W3C `buttons` bitmask from `ButtonInput<MouseButton>`.
fn current_mask(buttons: &ButtonInput<MouseButton>) -> u8 {
    let mut mask = 0u8;
    if buttons.pressed(MouseButton::Left) {
        mask |= BTN_LEFT;
    }
    if buttons.pressed(MouseButton::Right) {
        mask |= BTN_RIGHT;
    }
    if buttons.pressed(MouseButton::Middle) {
        mask |= BTN_MIDDLE;
    }
    mask
}

/// The mouse pointer's current `client` position, read off the `PointerId::Mouse`
/// entity's `PointerLocation` (`None` while the pointer has never been located,
/// e.g. before the first `CursorMoved`).
fn mouse_position(pointers: &Query<(&PointerId, &PointerLocation)>) -> Option<Vec2> {
    pointers
        .iter()
        .find(|(id, _)| matches!(id, PointerId::Mouse))
        .and_then(|(_, loc)| loc.location())
        .map(|l| l.position)
}

/// Build the `MousePayload` for this frame: `client_x`/`client_y` from
/// `position` (0.0 when unknown — no resolved node means this payload is
/// unused anyway), `button` as given, `buttons` as the live mask.
fn payload(position: Option<Vec2>, mask: u8, button: u8) -> MousePayload {
    MousePayload {
        client_x: position.map(|p| p.x).unwrap_or(0.0),
        client_y: position.map(|p| p.y).unwrap_or(0.0),
        button,
        buttons: mask,
        ..Default::default()
    }
}

/// See the module doc for the full per-frame algorithm. Gated by
/// [`mouse_listeners_active`] — only scheduled while a mouse listener exists.
pub fn mouse_events_system(
    hover_map: Res<HoverMap>,
    pointers: Query<(&PointerId, &PointerLocation)>,
    buttons: Res<ButtonInput<MouseButton>>,
    nodes: Query<&DomNode>,
    parents: Query<&ChildOf>,
    mut pending: ResMut<PendingDomEvents>,
    mut state: ResMut<PointerState>,
    idx: Res<MouseListenerIndex>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    let counts = idx.0;

    // Current hovered node set (deduped), plus `top`: the resolved node with
    // the lowest `HitData.depth` — see module doc point 1 for why this can't
    // just be "the first entry".
    let mut current: Vec<NodeId> = Vec::new();
    let mut top: Option<NodeId> = None;
    let mut top_depth = f32::INFINITY;
    if let Some(hovered) = hover_map.get(&PointerId::Mouse) {
        for (&entity, hit) in hovered.iter() {
            if let Some(node) = resolve_dom_node(entity, &nodes, &parents) {
                if !current.contains(&node) {
                    current.push(node);
                }
                if hit.depth < top_depth {
                    top_depth = hit.depth;
                    top = Some(node);
                }
            }
        }
    }

    let position = mouse_position(&pointers);
    let mask = current_mask(&buttons);
    let mods = modifiers_now(&keys);

    if !state.seeded {
        state.hover = current;
        state.position = position;
        state.buttons = mask;
        state.seeded = true;
        return;
    }

    if counts.any_hover() {
        for &node in &state.hover {
            if !current.contains(&node) {
                let mut out = PendingDomEvent::new(node, "mouseout")
                    .with_mouse(payload(position, mask, 0))
                    .with_modifiers(mods);
                out.bubbles = true;
                pending.0.push(out);
                let mut leave = PendingDomEvent::new(node, "mouseleave")
                    .with_mouse(payload(position, mask, 0))
                    .with_modifiers(mods);
                leave.bubbles = false;
                pending.0.push(leave);
            }
        }
        for &node in &current {
            if !state.hover.contains(&node) {
                let mut over = PendingDomEvent::new(node, "mouseover")
                    .with_mouse(payload(position, mask, 0))
                    .with_modifiers(mods);
                over.bubbles = true;
                pending.0.push(over);
                let mut enter = PendingDomEvent::new(node, "mouseenter")
                    .with_mouse(payload(position, mask, 0))
                    .with_modifiers(mods);
                enter.bubbles = false;
                pending.0.push(enter);
            }
        }
    }

    if counts.any_button() {
        let changed = mask ^ state.buttons;
        if changed != 0 {
            if let Some(top) = top {
                // (bit, W3C MouseEvent.button code: 0=left, 1=middle, 2=right)
                for (bit, code) in [(BTN_LEFT, 0u8), (BTN_RIGHT, 2u8), (BTN_MIDDLE, 1u8)] {
                    if changed & bit != 0 {
                        let pressed = mask & bit != 0;
                        let ty = if pressed { "mousedown" } else { "mouseup" };
                        pending.0.push(
                            PendingDomEvent::new(top, ty)
                                .with_mouse(payload(position, mask, code))
                                .with_modifiers(mods),
                        );
                    }
                }
            }
        }
    }

    if counts.any_move() && position != state.position {
        if let Some(top) = top {
            pending.0.push(
                PendingDomEvent::new(top, "mousemove")
                    .with_mouse(payload(position, mask, 0))
                    .with_modifiers(mods),
            );
        }
    }

    state.hover = current;
    state.position = position;
    state.buttons = mask;
}
