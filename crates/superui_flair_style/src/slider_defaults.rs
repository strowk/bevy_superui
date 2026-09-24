//! Default browser-like `<input type=range>` appearance, shipped in a
//! low-priority `superui-defaults` `@layer` so any unlayered author rule
//! overrides it (layer priority beats specificity in `StyleSheetBuilder::build`,
//! and the anonymous layer is always highest-priority).
//!
//! `superui_flair_css_parser`'s internal loader injects this into every
//! stylesheet before parsing author rules, so `superui-defaults` is defined
//! first and no author-declared named layer can rank below it.
//!
// >>> SUPERUI-FORK-PATCH: slider-default-layer  (docs/fork-patches.md#slider-default-layer)
//! `::slider-fill`'s `width` and `::slider-thumb`'s `left` are deliberately
//! unset: `position_slider_parts` drives them from `SliderValue` every frame,
//! and a default here would race it.

use crate::css_selector::CssSelector;
use crate::{BlockBuilder, StyleBuilderProperty, StyleSheetBuilder};
use bevy_color::Color;
use bevy_reflect::{FromReflect, TypePath};
use bevy_ui::{BackgroundColor, Node, PositionType, Val};
use superui_flair_core::{PropertyCanonicalName, PropertyPath, ReflectValue};

/// Name of the low-priority layer the defaults are shipped under.
pub const SLIDER_DEFAULTS_LAYER: &str = "superui-defaults";

fn node_property(path: &str) -> PropertyCanonicalName {
    PropertyCanonicalName::new(<Node as TypePath>::type_path(), PropertyPath::parse(path))
}

fn background_color_property() -> PropertyCanonicalName {
    PropertyCanonicalName::new(
        <BackgroundColor as TypePath>::type_path(),
        PropertyPath::parse(".0"),
    )
}

/// Builds a [`StyleBuilderProperty`] setting `name` to `value`.
fn prop(name: PropertyCanonicalName, value: impl FromReflect) -> StyleBuilderProperty {
    StyleBuilderProperty::new(name, ReflectValue::new(value))
}

/// Parses `selector`, tagging it with [`SLIDER_DEFAULTS_LAYER`].
///
/// Panics on an invalid selector: these are fixed constants owned by this
/// module, so a parse failure is a programmer error, not a caller concern.
fn defaults_selector(selector: &str) -> CssSelector {
    CssSelector::parse_single(selector)
        .unwrap_or_else(|error| panic!("invalid default slider selector {selector:?}: {error:?}"))
        .with_layer(SLIDER_DEFAULTS_LAYER.into())
}

/// Adds the default browser-like `<input type=range>` look to `builder`
/// under the low-priority [`SLIDER_DEFAULTS_LAYER`] layer: host
/// size/positioning, `::slider-track`, `::slider-fill`, `::slider-thumb`.
///
/// Does not set `::slider-fill`'s `width` or `::slider-thumb`'s `left` — see
/// the module docs.
pub fn add_slider_defaults(builder: &mut StyleSheetBuilder) {
    builder.define_layers(&[SLIDER_DEFAULTS_LAYER]);

    // Host: `input[type=range]`.
    builder
        .new_ruleset()
        .with_css_selector(defaults_selector("input[type=range]"))
        .with_property(prop(node_property(".position_type"), PositionType::Relative))
        .with_property(prop(node_property(".width"), Val::Px(150.0)))
        .with_property(prop(node_property(".height"), Val::Px(20.0)));

    // `::slider-track`
    builder
        .new_ruleset()
        .with_css_selector(defaults_selector("input[type=range]::slider-track"))
        .with_property(prop(node_property(".position_type"), PositionType::Absolute))
        .with_property(prop(node_property(".left"), Val::Px(0.0)))
        .with_property(prop(node_property(".top"), Val::Px(8.0)))
        .with_property(prop(node_property(".width"), Val::Percent(100.0)))
        .with_property(prop(node_property(".height"), Val::Px(4.0)))
        .with_property(prop(
            background_color_property(),
            Color::srgb_u8(0xc8, 0xc8, 0xc8),
        ))
        .with_property(prop(node_property(".border_radius.top_left"), Val::Px(2.0)))
        .with_property(prop(node_property(".border_radius.top_right"), Val::Px(2.0)))
        .with_property(prop(node_property(".border_radius.bottom_left"), Val::Px(2.0)))
        .with_property(prop(
            node_property(".border_radius.bottom_right"),
            Val::Px(2.0),
        ));

    // `::slider-fill` — no `width`: `position_slider_parts` owns it.
    builder
        .new_ruleset()
        .with_css_selector(defaults_selector("input[type=range]::slider-fill"))
        .with_property(prop(node_property(".position_type"), PositionType::Absolute))
        .with_property(prop(node_property(".left"), Val::Px(0.0)))
        .with_property(prop(node_property(".top"), Val::Px(8.0)))
        .with_property(prop(node_property(".height"), Val::Px(4.0)))
        .with_property(prop(
            background_color_property(),
            Color::srgb_u8(0x4a, 0x90, 0xd2),
        ))
        .with_property(prop(node_property(".border_radius.top_left"), Val::Px(2.0)))
        .with_property(prop(node_property(".border_radius.top_right"), Val::Px(2.0)))
        .with_property(prop(node_property(".border_radius.bottom_left"), Val::Px(2.0)))
        .with_property(prop(
            node_property(".border_radius.bottom_right"),
            Val::Px(2.0),
        ));

    // `::slider-thumb` — no `left`: `position_slider_parts` owns it.
    builder
        .new_ruleset()
        .with_css_selector(defaults_selector("input[type=range]::slider-thumb"))
        .with_property(prop(node_property(".position_type"), PositionType::Absolute))
        .with_property(prop(node_property(".top"), Val::Percent(50.0)))
        .with_property(prop(node_property(".width"), Val::Px(16.0)))
        .with_property(prop(node_property(".height"), Val::Px(16.0)))
        .with_property(prop(node_property(".margin.left"), Val::Px(-8.0)))
        .with_property(prop(node_property(".margin.top"), Val::Px(-8.0)))
        .with_property(prop(
            background_color_property(),
            Color::srgb_u8(0x1e, 0x90, 0xff), // dodgerblue
        ))
        .with_property(prop(node_property(".border_radius.top_left"), Val::Px(8.0)))
        .with_property(prop(node_property(".border_radius.top_right"), Val::Px(8.0)))
        .with_property(prop(node_property(".border_radius.bottom_left"), Val::Px(8.0)))
        .with_property(prop(
            node_property(".border_radius.bottom_right"),
            Val::Px(8.0),
        ));
}
// <<< SUPERUI-FORK-PATCH: slider-default-layer
