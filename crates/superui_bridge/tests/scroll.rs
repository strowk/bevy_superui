//! Wheel-scroll system: maps MouseWheel deltas onto hovered scrollable nodes.
mod support;
use support::*;

use bevy::ecs::system::RunSystemOnce;
use bevy::input::keyboard::KeyCode;
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::input::ButtonInput;
use bevy::picking::backend::HitData;
use bevy::picking::hover::HoverMap;
use bevy::picking::pointer::PointerId;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::ui::{Node, Overflow, ScrollPosition};
use superui_bridge::wheel_scroll_system;

/// Spawn one node with the given overflow + a zero ScrollPosition, mark it the
/// single hovered entity, and queue one MouseWheel message. Returns the entity.
fn setup(app: &mut App, overflow: Overflow, wheel: MouseWheel) -> Entity {
    let e = app
        .world_mut()
        .spawn((
            Node {
                overflow,
                ..default()
            },
            ScrollPosition::default(),
        ))
        .id();

    let mut inner = bevy::ecs::entity::EntityHashMap::default();
    inner.insert(e, HitData::new(Entity::PLACEHOLDER, 0.0, None, None));
    let mut map = HashMap::default();
    map.insert(PointerId::Mouse, inner);
    app.world_mut().insert_resource(HoverMap(map));

    app.world_mut().write_message(wheel);
    e
}

fn scroll_of(app: &App, e: Entity) -> Vec2 {
    app.world().get::<ScrollPosition>(e).unwrap().0
}

#[test]
fn wheel_scrolls_a_vertical_scroll_node() {
    let mut app = test_app();
    let e = setup(
        &mut app,
        Overflow::scroll_y(),
        MouseWheel {
            unit: MouseScrollUnit::Line,
            x: 0.0,
            y: 1.0, // wheel up
            window: Entity::PLACEHOLDER,
            phase: bevy::input::touch::TouchPhase::Moved,
        },
    );
    app.world_mut().run_system_once(wheel_scroll_system).unwrap();
    // Wheel-up subtracts: offset goes negative by one line before layout clamps.
    assert_eq!(scroll_of(&app, e), Vec2::new(0.0, -20.0));
}

#[test]
fn wheel_over_non_scroll_node_does_nothing() {
    let mut app = test_app();
    let e = setup(
        &mut app,
        Overflow::visible(),
        MouseWheel {
            unit: MouseScrollUnit::Line,
            x: 0.0,
            y: 1.0,
            window: Entity::PLACEHOLDER,
            phase: bevy::input::touch::TouchPhase::Moved,
        },
    );
    app.world_mut().run_system_once(wheel_scroll_system).unwrap();
    assert_eq!(scroll_of(&app, e), Vec2::ZERO);
}

#[test]
fn pixel_unit_applies_raw_delta() {
    let mut app = test_app();
    let e = setup(
        &mut app,
        Overflow::scroll_y(),
        MouseWheel {
            unit: MouseScrollUnit::Pixel,
            x: 0.0,
            y: 13.0,
            window: Entity::PLACEHOLDER,
            phase: bevy::input::touch::TouchPhase::Moved,
        },
    );
    app.world_mut().run_system_once(wheel_scroll_system).unwrap();
    assert_eq!(scroll_of(&app, e), Vec2::new(0.0, -13.0));
}

#[test]
fn shift_wheel_scrolls_horizontally() {
    let mut app = test_app();
    let e = setup(
        &mut app,
        Overflow::scroll_x(),
        MouseWheel {
            unit: MouseScrollUnit::Pixel,
            x: 0.0,
            y: 10.0,
            window: Entity::PLACEHOLDER,
            phase: bevy::input::touch::TouchPhase::Moved,
        },
    );
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::ShiftLeft);
    app.world_mut().run_system_once(wheel_scroll_system).unwrap();
    // Shift maps the vertical delta onto x; y stays put.
    assert_eq!(scroll_of(&app, e), Vec2::new(-10.0, 0.0));
}

#[test]
fn wheel_with_no_hover_target_does_nothing() {
    let mut app = test_app();
    let e = app
        .world_mut()
        .spawn((
            Node {
                overflow: Overflow::scroll_y(),
                ..default()
            },
            ScrollPosition::default(),
        ))
        .id();
    // Empty hover map: nothing under the cursor.
    app.world_mut().insert_resource(HoverMap(HashMap::default()));
    app.world_mut().write_message(MouseWheel {
        unit: MouseScrollUnit::Line,
        x: 0.0,
        y: 1.0,
        window: Entity::PLACEHOLDER,
        phase: bevy::input::touch::TouchPhase::Moved,
    });
    app.world_mut().run_system_once(wheel_scroll_system).unwrap();
    assert_eq!(scroll_of(&app, e), Vec2::ZERO);
}
