use bevy::prelude::*;
use super::interaction::{HoverButton, BTN_IDLE_BG};
use super::theme;

/// A styled container node bundle: gradient fill, accent-tinted border, rounded
/// corners, and a soft drop shadow.
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
        theme::panel_gradient(),
        BorderColor::all(theme::PANEL_BORDER),
        BorderRadius::all(Val::Px(theme::RADIUS)),
        theme::panel_shadow(),
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

/// A horizontal progress bar track: outer element the caller fills via `bar_fill`.
pub fn bar_track(height: f32) -> impl Bundle {
    (
        Node {
            width: Val::Percent(100.0),
            height: Val::Px(height),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.10, 0.12, 0.18)),
        BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.06)),
        BorderRadius::all(Val::Px(4.0)),
    )
}

/// A solid bar fill. Kept solid (not a gradient) because several callers recolor
/// it per-frame from a live value (HP fraction).
pub fn bar_fill(frac: f32, color: Color) -> impl Bundle {
    (
        Node {
            width: Val::Percent((frac.clamp(0.0, 1.0)) * 100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        BackgroundColor(color),
        BorderRadius::all(Val::Px(4.0)),
    )
}

/// A menu button with hover/press feedback (see `interaction::button_feedback`).
/// Add your own marker component alongside for click routing.
pub fn menu_button() -> impl Bundle {
    (
        Button,
        HoverButton,
        Node {
            padding: UiRect::axes(Val::Px(20.0), Val::Px(11.0)),
            border: UiRect::all(Val::Px(1.0)),
            justify_content: JustifyContent::Center,
            ..default()
        },
        BackgroundColor(BTN_IDLE_BG),
        BorderColor::all(theme::PANEL_BORDER),
        BorderRadius::all(Val::Px(theme::RADIUS)),
    )
}
