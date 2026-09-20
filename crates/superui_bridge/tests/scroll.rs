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
use superui_bridge::{clamp_scroll_position_system, wheel_scroll_system};

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

#[test]
fn overscroll_is_clamped_by_layout() {
    use std::cell::RefCell;
    use std::rc::Rc;

    use bevy::ecs::reflect::AppTypeRegistry;
    use superui_css::core::PropertyRegistry;
    use superui_css::prelude::Styled;
    use superui_css::style::asset_loader::StyleAssetLoader;
    use superui_css::style::{StyleSheet, StyleSheetBuilder};

    // A short scroll box with content taller than it: max_scroll_y > 0.
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<div style='width:40px;height:40px;overflow-y:scroll'>\
           <div style='width:40px;height:400px'></div>\
         </div>",
    )));
    let mut app = test_app();
    let root = mount(&mut app, dom.clone());
    app.add_systems(Update, wheel_scroll_system);
    // Real clamp, not the system under test: the whole point of this test is
    // that layout, not `wheel_scroll_system`, is what bounds the offset.
    app.add_systems(
        PostUpdate,
        clamp_scroll_position_system.in_set(bevy::ui::UiSystems::PostLayout),
    );
    app.update(); // reconcile + first layout

    // `mount()` gives root a `Handle::default()` stylesheet, which never
    // resolves in `Assets<StyleSheet>`; flair's cascade requires the
    // stylesheet to be loaded before it applies anything, inline styles
    // included. Swap in a real (empty) one so the `style=` attributes above
    // actually reach `Node`.
    let stylesheet = {
        let world = app.world();
        let type_registry = world.resource::<AppTypeRegistry>().0.read();
        let loader = StyleAssetLoader::from_asset_server(world.resource::<AssetServer>());
        StyleSheetBuilder::new()
            .build(&type_registry, world.resource::<PropertyRegistry>(), loader)
            .expect("empty stylesheet should build")
    };
    let stylesheet = app.world_mut().resource_mut::<Assets<StyleSheet>>().add(stylesheet);
    app.world_mut().entity_mut(root).insert(Styled::new(stylesheet));
    app.update(); // let flair's cascade turn the inline styles into Node fields
    app.update(); // and one more so that layout picks up the new Node sizes

    let scroller = app.world_mut().get::<Children>(root).unwrap()[0];

    // Hover the scroller and scroll far past the bottom. The HoverMap has to
    // be (re-)inserted from a system ordered right before `wheel_scroll_system`:
    // `PickingPlugin` recomputes it from scratch every frame in `PreUpdate`,
    // which runs ahead of `Update` and would otherwise wipe out a HoverMap
    // inserted from outside the schedule before `app.update()` is called.
    app.add_systems(
        Update,
        (move |mut hover_map: ResMut<HoverMap>| {
            let mut inner = bevy::ecs::entity::EntityHashMap::default();
            inner.insert(scroller, HitData::new(Entity::PLACEHOLDER, 0.0, None, None));
            let mut map = HashMap::default();
            map.insert(PointerId::Mouse, inner);
            *hover_map = HoverMap(map);
        })
        .before(wheel_scroll_system),
    );
    app.world_mut().write_message(MouseWheel {
        unit: MouseScrollUnit::Pixel,
        x: 0.0,
        y: -100000.0, // wheel down, absurdly far
        window: Entity::PLACEHOLDER,
        phase: bevy::input::touch::TouchPhase::Moved,
    });
    app.update(); // wheel system runs, then layout clamps

    let y = app.world().get::<ScrollPosition>(scroller).unwrap().0.y;
    assert!(y > 0.0, "should have scrolled down some");
    assert!(
        y <= 400.0 - 40.0 + 1.0,
        "layout must clamp within content bounds, got {y}"
    );
}
