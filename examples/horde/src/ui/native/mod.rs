use bevy::prelude::*;

pub mod theme;
pub mod widgets;
pub mod interaction;
pub mod hud;
pub mod screens;

pub struct NativeUiPlugin;

impl Plugin for NativeUiPlugin {
    fn build(&self, app: &mut App) {
        app
            // Generic button hover/press feedback (runs in every state).
            .add_systems(Update, interaction::button_feedback)
            .add_plugins((hud::HudPlugin, screens::ScreensPlugin));
    }
}
