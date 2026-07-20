use bevy::prelude::*;

pub mod theme;
pub mod widgets;
pub mod interaction;
pub mod project;
pub mod hud;
pub mod screens;

pub struct NativeUiPlugin;

impl Plugin for NativeUiPlugin {
    fn build(&self, app: &mut App) {
        app
            // Projection runs after sim assembly, before UI reads the snapshot.
            .add_systems(Update, project::project_snapshot.after(crate::sim::snapshot::assemble_world_snapshot))
            // Generic button hover/press feedback (runs in every state).
            .add_systems(Update, interaction::button_feedback)
            .add_plugins((hud::HudPlugin, screens::ScreensPlugin));
    }
}
