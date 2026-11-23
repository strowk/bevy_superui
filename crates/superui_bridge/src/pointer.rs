//! Per-frame geometry snapshot feeding `document.elementFromPoint`: the JS
//! shadow DOM has no layout of its own, so `sync_hit_rects_system` reads
//! `bevy_ui`'s computed layout each frame and pushes an AABB-per-node snapshot
//! into the engine for the `__ss_element_from_point` host binding to hit-test.

use std::collections::{HashMap, HashSet};

use bevy::input::mouse::MouseButton;
use bevy::input::touch::Touches;
use bevy::input::ButtonInput;
use bevy::picking::pointer::{PointerId, PointerLocation};
use bevy::prelude::*;
use superui_js::{HitRect, Modifiers, MousePayload};

use crate::events::{
    bevy_pointer_id, js_pointer_id, pointer_is_primary, pointer_kind, PendingDomEvent,
    PendingDomEvents, PointerCaptures, PrimaryTouch,
};
use crate::mouse::mouse_position;
use crate::runtime::{DomNode, UiRuntime};

/// Snapshot every reconciled node's screen-space AABB into the engine, so
/// `document.elementFromPoint(x, y)` (no layout on the JS side) can hit-test
/// against the latest `bevy_ui` geometry.
///
/// Coordinates are logical window pixels — the same space `MouseEvent.clientX`/
/// `clientY` use (see `mouse.rs`'s `mouse_position`) — so a probe and a real
/// pointer event agree on where `(x, y)` lands. `ComputedNode`/`UiGlobalTransform`
/// are in *physical* pixels (per their own docs); `ComputedNode.inverse_scale_factor()`
/// converts back to logical, which is why it's used here rather than the
/// generic (non-UI) `Transform`/`GlobalTransform` — `bevy_ui` 0.20 doesn't
/// position nodes through those at all, only through `UiGlobalTransform`.
///
/// `depth` is the node's distance from the document root, walked via
/// `rt.dom`'s `parent()` chain — it drives `element_from_point`'s "topmost
/// wins" tie-break, independent of `bevy_ui`'s render/paint order.
pub fn sync_hit_rects_system(
    mut rt: NonSendMut<UiRuntime>,
    nodes: Query<(&DomNode, &ComputedNode, &UiGlobalTransform)>,
) {
    let mut rects: Vec<HitRect> = Vec::new();
    for (dom_node, computed, transform) in &nodes {
        let Some(js_id) = rt.applier.js(dom_node.0) else {
            continue;
        };
        let inv_scale = computed.inverse_scale_factor();
        let center = transform.translation * inv_scale;
        let half = computed.size() * inv_scale * 0.5;
        let min = center - half;
        let max = center + half;

        let depth = {
            let dom = rt.dom.borrow();
            let mut d = 0u32;
            let mut cur = dom.parent(dom_node.0);
            while let Some(p) = cur {
                d += 1;
                cur = dom.parent(p);
            }
            d
        };

        rects.push(HitRect { js_id, min: (min.x, min.y), max: (max.x, max.y), depth });
    }
    rt.set_hit_rects(rects);
}

/// Last-delivered position per captured JS pointerId — [`captured_delivery`]'s
/// move-dedup state, kept fresh across frames by
/// [`captured_pointer_delivery_system`]. Forgets a pointer on release (not just
/// on an explicit `releasePointerCapture`): once gone, there's nothing left to
/// dedup against if the same JS pointerId is captured again later.
#[derive(Resource, Default)]
pub struct CapturedPointerState(pub HashMap<u32, Vec2>);

/// A pointer-event `MousePayload` with pointer-identity fields derived from
/// `js_id` (mirrors `events.rs`'s private `pointer_payload`, which isn't
/// reachable from here).
fn captured_payload(pos: Vec2, buttons: u8, js_id: u32, primary: &PrimaryTouch) -> MousePayload {
    let id = bevy_pointer_id(js_id);
    MousePayload {
        client_x: pos.x,
        client_y: pos.y,
        button: 0,
        buttons,
        pointer_id: js_id,
        pointer_type: pointer_kind(id),
        is_primary: pointer_is_primary(id, primary),
    }
}

/// The core of off-UI captured delivery: for every pointer `PointerCaptures`
/// currently holds, synthesize the events bevy_picking can't — it has nothing
/// to say about a pointer that isn't over any UI, which is exactly the case a
/// drag that's left the UI's bounds hits. Pure (no ECS access) so the test
/// suite can drive it directly; [`captured_pointer_delivery_system`] is the
/// thin adapter that gathers `positions`/`released`/`canceled` from the raw
/// input resources and calls this each frame.
///
/// Per captured pointer: a changed position (vs. `last`) delivers one
/// `pointermove` (`buttons = 1` — captured delivery only ever fires mid-drag,
/// button held); a canceled pointer then also gets `pointercancel`; a released
/// pointer then also gets `pointerup` (`buttons = 0`). Either a cancel or a
/// release ends the pointer's gesture (W3C: both implicitly release capture,
/// mirrored on the JS side by `dom.js`'s `dispatch`), so either one drops the
/// pointer from `last` — nothing left to dedup against. A still-held pointer's
/// `last` is updated to the current position instead. Order within a pointer:
/// move, then cancel/up (matches a real browser's event order for a single
/// frame).
pub fn captured_delivery(
    captures: &PointerCaptures,
    positions: &HashMap<u32, Vec2>,
    released: &HashSet<u32>,
    canceled: &HashSet<u32>,
    last: &mut HashMap<u32, Vec2>,
    primary: &PrimaryTouch,
) -> Vec<PendingDomEvent> {
    let mut out = Vec::new();
    for (&js_id, &node) in captures.0.iter() {
        let Some(&pos) = positions.get(&js_id) else {
            continue;
        };
        let moved = last.get(&js_id) != Some(&pos);
        if moved {
            let m = captured_payload(pos, 1, js_id, primary);
            out.push(PendingDomEvent::new(node, "pointermove").with_mouse(m).with_modifiers(Modifiers::default()));
        }
        let is_canceled = canceled.contains(&js_id);
        if is_canceled {
            let m = captured_payload(pos, 0, js_id, primary);
            out.push(PendingDomEvent::new(node, "pointercancel").with_mouse(m).with_modifiers(Modifiers::default()));
        }
        let is_released = released.contains(&js_id);
        if is_released {
            let m = captured_payload(pos, 0, js_id, primary);
            out.push(PendingDomEvent::new(node, "pointerup").with_mouse(m).with_modifiers(Modifiers::default()));
        }
        // A canceled OR released pointer's gesture is over: W3C has both
        // implicitly release capture (pointer.rs's caller mirrors this on the
        // JS side in dom.js's dispatch), so there's nothing left to dedup
        // against — forget it, same as a plain release.
        if is_canceled || is_released {
            last.remove(&js_id);
        } else {
            last.insert(js_id, pos);
        }
    }
    out
}

/// Normal system: the thin adapter around [`captured_delivery`]. Gathers this
/// frame's raw positions/released/canceled sets — mouse from the
/// `PointerId::Mouse` entity's `PointerLocation` + `ButtonInput<MouseButton>`
/// (mirrors `mouse.rs`'s `mouse_position`), touch from `Touches` — and
/// extends `PendingDomEvents` with whatever `captured_delivery` yields. Only
/// pointers present in `PointerCaptures` are ever considered (an uncaptured
/// pointer's events are already covered by the Task-2 picking observers);
/// cheap to call every frame even with no capture active since
/// `captured_delivery` itself is a no-op on an empty `captures` map.
///
/// `Touches::iter()` only yields currently-*pressed* touches — a touch that
/// just ended/was canceled this frame has already left `pressed` (see
/// `bevy_input::touch::Touches::process_touch_event`), so its position is
/// read from `iter_just_released`/`iter_just_canceled` instead, which still
/// carry the touch's last known position.
///
/// Registered `.before(drain_dom_events_system)` so a raw `pointerup` this
/// system enqueues dispatches — and implicitly releases capture — the same
/// frame (mirrors `mouse_events_system`'s ordering rationale).
pub fn captured_pointer_delivery_system(
    captures: Res<PointerCaptures>,
    mouse_pointers: Query<(&PointerId, &PointerLocation)>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    touches: Res<Touches>,
    mut state: ResMut<CapturedPointerState>,
    primary: Res<PrimaryTouch>,
    mut pending: ResMut<PendingDomEvents>,
) {
    // Prune any `last` entry whose pointerId is no longer captured — an explicit
    // `releasePointerCapture` removes the pointer from `PointerCaptures` (via
    // `drain_pointer_captures_system`'s `None`-op branch) but never touches this
    // resource, which would otherwise leave a stale position behind to dedupe
    // away the first `pointermove` of a future re-capture of the same pointerId.
    state.0.retain(|id, _| captures.0.contains_key(id));

    if captures.0.is_empty() {
        return;
    }

    let mut positions: HashMap<u32, Vec2> = HashMap::new();
    let mut released: HashSet<u32> = HashSet::new();
    let mut canceled: HashSet<u32> = HashSet::new();

    if let Some(pos) = mouse_position(&mouse_pointers) {
        positions.insert(js_pointer_id(PointerId::Mouse), pos);
    }
    if mouse_buttons.just_released(MouseButton::Left) {
        released.insert(js_pointer_id(PointerId::Mouse));
    }

    for touch in touches.iter() {
        positions.insert(js_pointer_id(PointerId::Touch(touch.id())), touch.position());
    }
    for touch in touches.iter_just_released() {
        let js_id = js_pointer_id(PointerId::Touch(touch.id()));
        positions.insert(js_id, touch.position());
        released.insert(js_id);
    }
    for touch in touches.iter_just_canceled() {
        let js_id = js_pointer_id(PointerId::Touch(touch.id()));
        positions.insert(js_id, touch.position());
        canceled.insert(js_id);
    }

    let events =
        captured_delivery(&captures, &positions, &released, &canceled, &mut state.0, &primary);
    pending.0.extend(events);
}
