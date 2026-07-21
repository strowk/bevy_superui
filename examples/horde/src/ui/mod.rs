use bevy::prelude::*;

pub mod project;

#[cfg(feature = "ui-native")]
pub mod native;

#[cfg(not(feature = "ui-native"))]
pub mod supersolid;

pub fn add_ui(app: &mut App) {
    // Shared boundary: fills screen_pos from world_pos, for BOTH backends.
    app.add_systems(
        Update,
        project::project_snapshot.after(crate::sim::snapshot::assemble_world_snapshot),
    );

    #[cfg(feature = "ui-native")]
    app.add_plugins(native::NativeUiPlugin);

    #[cfg(not(feature = "ui-native"))]
    app.add_plugins(supersolid::SupersolidUiPlugin);
}
