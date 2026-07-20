use bevy::prelude::*;
use super::theme;

/// A styled container node bundle.
pub fn panel(width: Val, padding: f32) -> impl Bundle {
    (
        Node {
            width,
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(padding)),
            row_gap: Val::Px(theme::SPACE),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(theme::PANEL),
        BorderColor::all(theme::PANEL_BORDER),
        BorderRadius::all(Val::Px(theme::RADIUS)),
    )
}

/// A text bundle at a given size/color.
pub fn label(text: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(text),
        TextFont::from_font_size(size),
        TextColor(color),
    )
}

/// A horizontal progress bar: outer track + inner fill sized by `frac`.
/// Returns the outer bundle; caller spawns the fill child via `bar_fill`.
pub fn bar_track(height: f32) -> impl Bundle {
    (
        Node {
            width: Val::Percent(100.0),
            height: Val::Px(height),
            ..default()
        },
        BackgroundColor(Color::srgb(0.18, 0.19, 0.26)),
        BorderRadius::all(Val::Px(3.0)),
    )
}

pub fn bar_fill(frac: f32, color: Color) -> impl Bundle {
    (
        Node {
            width: Val::Percent((frac.clamp(0.0, 1.0)) * 100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        BackgroundColor(color),
        BorderRadius::all(Val::Px(3.0)),
    )
}

/// A menu button with label. Add your own marker component alongside for click routing.
pub fn menu_button() -> impl Bundle {
    (
        Button,
        Node {
            padding: UiRect::axes(Val::Px(18.0), Val::Px(10.0)),
            border: UiRect::all(Val::Px(1.0)),
            justify_content: JustifyContent::Center,
            ..default()
        },
        BackgroundColor(Color::srgb(0.16, 0.18, 0.26)),
        BorderColor::all(theme::PANEL_BORDER),
        BorderRadius::all(Val::Px(theme::RADIUS)),
    )
}
