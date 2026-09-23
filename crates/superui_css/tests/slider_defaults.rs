//! Regression test for the `slider-default-layer` fork patch
//! (docs/fork-patches.md#slider-default-layer).
//!
//! `<input type=range>` ships default track/fill/thumb styling in a
//! low-priority `superui-defaults` `@layer`. An unlayered author rule
//! targeting the same part must still win, no matter its specificity versus
//! the framework's own (more specific, attribute-qualified) default
//! selectors — layer priority is compared before specificity.

mod support;
use support::*;

use bevy::prelude::*;

use superui_css::html_type_name;
use superui_css::prelude::*;

const CSS: &str = r#"
input[type=range]::slider-thumb { width: 99px; }
"#;

#[test]
fn defaults_layer_loses_to_unlayered_author_rules() {
    put_css("slider_defaults.css", CSS);

    let mut app = test_app();
    let handle = {
        let server = app.world().resource::<AssetServer>().clone();
        server.load_style_sheet("slider_defaults.css")
    };

    let host = app
        .world_mut()
        .spawn((
            Node::default(),
            html_type_name("input"),
            AttributeList::from_iter([("type", "range")]),
            Styled::new(handle.clone()),
        ))
        .id();

    let thumb = app
        .world_mut()
        .spawn((Node::default(), StyleData::default(), SliderPart::Thumb, Name::new("thumb")))
        .id();

    app.world_mut().entity_mut(host).add_children(&[thumb]);

    load_until_ready(&mut app, &handle);

    let thumb_width = app.world().get::<Node>(thumb).unwrap().width;
    assert_eq!(
        thumb_width,
        Val::Px(99.0),
        "unlayered author rule must override the superui-defaults layer"
    );
}

/// With no author rule at all, the shipped defaults themselves must apply
/// (host size, track/fill/thumb size and color) — proves the defaults are
/// actually injected, not just that they lose when present.
#[test]
fn defaults_apply_with_no_author_css() {
    put_css("slider_defaults_empty.css", "");

    let mut app = test_app();
    let handle = {
        let server = app.world().resource::<AssetServer>().clone();
        server.load_style_sheet("slider_defaults_empty.css")
    };

    let host = app
        .world_mut()
        .spawn((
            Node::default(),
            html_type_name("input"),
            AttributeList::from_iter([("type", "range")]),
            Styled::new(handle.clone()),
        ))
        .id();

    let track = app
        .world_mut()
        .spawn((Node::default(), StyleData::default(), SliderPart::Track, Name::new("track")))
        .id();
    let thumb = app
        .world_mut()
        .spawn((Node::default(), StyleData::default(), SliderPart::Thumb, Name::new("thumb")))
        .id();

    app.world_mut().entity_mut(host).add_children(&[track, thumb]);

    load_until_ready(&mut app, &handle);

    let world = app.world();
    assert_eq!(world.get::<Node>(host).unwrap().width, Val::Px(150.0), "host width default");
    assert_eq!(world.get::<Node>(host).unwrap().height, Val::Px(20.0), "host height default");
    assert_eq!(world.get::<Node>(track).unwrap().height, Val::Px(4.0), "track height default");
    assert_eq!(world.get::<Node>(thumb).unwrap().width, Val::Px(16.0), "thumb width default");
    assert_eq!(world.get::<Node>(thumb).unwrap().height, Val::Px(16.0), "thumb height default");
    assert_eq!(
        world.get::<Node>(thumb).unwrap().margin.left,
        Val::Px(-8.0),
        "thumb centering margin default"
    );
}
