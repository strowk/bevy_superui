use bevy::prelude::*;

pub mod project;

#[cfg(feature = "ui-native")]
pub mod native;

pub fn add_ui(app: &mut App) {
    // Shared boundary: fills screen_pos from world_pos, for BOTH backends.
    app.add_systems(
        Update,
        project::project_snapshot.after(crate::sim::snapshot::assemble_world_snapshot),
    );

    #[cfg(feature = "ui-native")]
    app.add_plugins(native::NativeUiPlugin);

    #[cfg(not(feature = "ui-native"))]
    panic!(
        "Supersolid UI backend not yet wired — see Task A4. TODO(supersolid-runtime)."
    );
}
