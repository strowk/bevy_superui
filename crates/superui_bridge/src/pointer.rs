//! Per-frame geometry snapshot feeding `document.elementFromPoint`: the JS
//! shadow DOM has no layout of its own, so `sync_hit_rects_system` reads
//! `bevy_ui`'s computed layout each frame and pushes an AABB-per-node snapshot
//! into the engine for the `__ss_element_from_point` host binding to hit-test.

use bevy::prelude::*;
use superui_js::HitRect;

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
