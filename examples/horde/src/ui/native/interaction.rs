use bevy::prelude::*;
use super::theme;

/// Marker for buttons that should receive generic hover/press visual feedback.
/// (Weapon-bar slots manage their own active-highlight, so they do NOT carry this.)
#[derive(Component)]
pub struct HoverButton;

pub const BTN_IDLE_BG: Color = Color::srgb(0.13, 0.16, 0.25);
pub const BTN_HOVER_BG: Color = Color::srgb(0.21, 0.27, 0.42);
pub const BTN_PRESSED_BG: Color = Color::srgb(0.10, 0.13, 0.20);

/// Mirrors CSS `:hover` / `:active` feedback for menu buttons: brighten + accent
/// border on hover, darken on press, restore on release.
pub fn button_feedback(
    mut q: Query<
        (&Interaction, &mut BackgroundColor, &mut BorderColor),
        (Changed<Interaction>, With<HoverButton>),
    >,
) {
    for (interaction, mut bg, mut border) in q.iter_mut() {
        match *interaction {
            Interaction::Hovered => {
                bg.0 = BTN_HOVER_BG;
                *border = BorderColor::all(theme::ACCENT);
            }
            Interaction::Pressed => {
                bg.0 = BTN_PRESSED_BG;
                *border = BorderColor::all(theme::ACCENT);
            }
            Interaction::None => {
                bg.0 = BTN_IDLE_BG;
                *border = BorderColor::all(theme::PANEL_BORDER);
            }
        }
    }
}
