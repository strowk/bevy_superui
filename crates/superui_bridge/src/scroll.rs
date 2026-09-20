//! Mouse-wheel scrolling for `overflow: scroll` nodes. Bevy's layout offsets a
//! node's children by `ScrollPosition`, but — unlike the render-only offset it
//! keeps on `ComputedNode` — it never clamps the `ScrollPosition` component
//! itself back into range (see `bevy_ui_widgets`' own `ScrollArea`, which does
//! the same clamp by hand for the same reason). `wheel_scroll_system` applies
//! raw wheel deltas; `clamp_scroll_position_system` is what keeps the result
//! from drifting past the scrollable range. Native-only: the wheel never
//! reaches JS, and no DOM event is dispatched.

use bevy::ecs::message::MessageReader;
use bevy::input::keyboard::KeyCode;
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::input::ButtonInput;
use bevy::picking::hover::HoverMap;
use bevy::picking::pointer::PointerId;
use bevy::prelude::*;
use bevy::ui::{ComputedNode, OverflowAxis, ScrollPosition};

/// Logical pixels scrolled per `MouseScrollUnit::Line` notch.
const LINE_HEIGHT: f32 = 20.0;

/// Read this frame's wheel deltas and move the `ScrollPosition` of every
/// hovered node whose overflow is `Scroll` on the relevant axis. Shift swaps a
/// vertical wheel to horizontal (browser convention). The offset is subtracted
/// so wheel-up reveals content above, matching browsers. Unclamped by design —
/// `clamp_scroll_position_system` brings the result back into range once this
/// frame's layout is known.
pub fn wheel_scroll_system(
    mut wheel: MessageReader<MouseWheel>,
    hover_map: Res<HoverMap>,
    keys: Res<ButtonInput<KeyCode>>,
    mut scrollables: Query<(&Node, &mut ScrollPosition)>,
) {
    // Sum the frame's deltas into logical pixels.
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let (mut dx, mut dy) = (0.0f32, 0.0f32);
    for ev in wheel.read() {
        let mult = match ev.unit {
            MouseScrollUnit::Line => LINE_HEIGHT,
            MouseScrollUnit::Pixel => 1.0,
        };
        let (mut ex, mut ey) = (ev.x * mult, ev.y * mult);
        // Shift turns a purely vertical wheel into horizontal movement.
        if shift && ex == 0.0 {
            ex = ey;
            ey = 0.0;
        }
        dx += ex;
        dy += ey;
    }
    if dx == 0.0 && dy == 0.0 {
        return;
    }

    let Some(hovered) = hover_map.get(&PointerId::Mouse) else {
        return;
    };
    for (&entity, _hit) in hovered.iter() {
        if let Ok((node, mut scroll)) = scrollables.get_mut(entity) {
            if node.overflow.y == OverflowAxis::Scroll {
                scroll.0.y -= dy;
            }
            if node.overflow.x == OverflowAxis::Scroll {
                scroll.0.x -= dx;
            }
        }
    }
}

/// Clamp every `overflow: scroll` node's `ScrollPosition` to `[0, content_size
/// - visible_size]` on the scrollable axes. Bevy's `ui_layout_system` computes
/// this same range each frame but only applies it to `ComputedNode`'s
/// render-time offset, not to the `ScrollPosition` component — so run this
/// after layout (`UiSystems::PostLayout`) to correct the component itself,
/// otherwise `wheel_scroll_system` drifts it arbitrarily far past the content.
pub fn clamp_scroll_position_system(
    mut scrollables: Query<(&Node, &ComputedNode, &mut ScrollPosition)>,
) {
    for (node, computed, mut scroll) in &mut scrollables {
        let visible_size = computed.size() * computed.inverse_scale_factor();
        let content_size = computed.content_size() * computed.inverse_scale_factor();
        let max_range = (content_size - visible_size).max(Vec2::ZERO);

        let mut clamped = scroll.0;
        if node.overflow.y == OverflowAxis::Scroll {
            clamped.y = clamped.y.clamp(0.0, max_range.y);
        }
        if node.overflow.x == OverflowAxis::Scroll {
            clamped.x = clamped.x.clamp(0.0, max_range.x);
        }
        if clamped != scroll.0 {
            scroll.0 = clamped;
        }
    }
}
