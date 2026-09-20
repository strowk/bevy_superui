//! Mouse-wheel scrolling for `overflow: scroll` nodes. Bevy's layout offsets a
//! node's children by its `ScrollPosition` and auto-clamps that offset to the
//! scrollable range; this system is the only thing that *moves* it. Native-only:
//! the wheel never reaches JS, and no DOM event is dispatched.

use bevy::ecs::message::MessageReader;
use bevy::input::keyboard::KeyCode;
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::input::ButtonInput;
use bevy::picking::hover::HoverMap;
use bevy::picking::pointer::PointerId;
use bevy::prelude::*;
use bevy::ui::{OverflowAxis, ScrollPosition};

/// Logical pixels scrolled per `MouseScrollUnit::Line` notch.
const LINE_HEIGHT: f32 = 20.0;

/// Read this frame's wheel deltas and move the `ScrollPosition` of every
/// hovered node whose overflow is `Scroll` on the relevant axis. Shift swaps a
/// vertical wheel to horizontal (browser convention). The offset is subtracted
/// so wheel-up reveals content above, matching browsers; Bevy's layout clamps
/// the result to the valid range next frame.
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
