use bevy::prelude::*;

#[cfg(feature = "ui-native")]
#[allow(dead_code)] // wired in Task 5/18
pub fn add_ui(_app: &mut App) { /* NativeUiPlugin wired in Task 18 */ }

#[cfg(not(feature = "ui-native"))]
#[allow(dead_code)] // wired in Task 5/18
pub fn add_ui(_app: &mut App) {
    panic!("Supersolid UI backend not yet implemented — build with the default \
            `ui-native` feature. TODO(supersolid-runtime).");
}
