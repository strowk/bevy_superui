mod support;
use support::*;

use bevy::prelude::*;
use bevy::ui::{Node, Val};
use bevy::ui_widgets::{SliderThumb, SliderValue};
use superui_css::SliderPart;

#[test]
fn value_positioning_wins_over_cascade_left_width() {
    // Unlayered author rules => beat the `superui-defaults` layer, which
    // deliberately leaves thumb `left` / fill `width` unset. These px values
    // are what ApplyComputedProperties writes; position_slider_parts must override.
    put(
        "slider_pos.css",
        b"input::slider-thumb { left: 11px; } input::slider-fill { width: 11px; }",
    );
    put("slider_pos.js", b"");

    let mut app = app();
    let _root = spawn_root(&mut app, "<input type=\"range\" value=\"40\">", "slider_pos.css", "slider_pos.js");
    tick(&mut app, 32);

    // value 40 of the default 0..100 range => 40%. Thumb left and fill width
    // must be Percent(40), NOT the CSS Px(11): only true if position_slider_parts
    // ran AFTER ApplyComputedProperties on the settle frame.
    let (thumb_left, fill_width) = slider_part_geometry(&mut app);
    assert_eq!(thumb_left, Val::Percent(40.0), "thumb left should track value, not CSS px");
    assert_eq!(fill_width, Val::Percent(40.0), "fill width should track value, not CSS px");

    // Explicit value change still tracks (spec: set SliderValue, update, assert).
    let host = {
        let mut q = app.world_mut().query_filtered::<Entity, With<SliderValue>>();
        q.single(app.world()).unwrap()
    };
    app.world_mut().entity_mut(host).insert(SliderValue(70.0));
    app.update();
    let (thumb_left, _) = slider_part_geometry(&mut app);
    assert_eq!(thumb_left, Val::Percent(70.0));
}

// Helper: read the thumb's `Node.left` and the fill's `Node.width` from the
// single reconciled slider subtree.
fn slider_part_geometry(app: &mut App) -> (Val, Val) {
    let thumb_left = {
        let mut q = app.world_mut().query_filtered::<&Node, With<SliderThumb>>();
        q.single(app.world()).unwrap().left
    };
    let fill_width = {
        let mut q = app.world_mut().query::<(&Node, &SliderPart)>();
        q.iter(app.world())
            .find(|(_, p)| matches!(p, SliderPart::Fill))
            .unwrap()
            .0
            .width
    };
    (thumb_left, fill_width)
}
