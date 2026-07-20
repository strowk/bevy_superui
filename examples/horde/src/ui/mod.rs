use bevy::prelude::*;

#[cfg(feature = "ui-native")]
pub mod native;

#[cfg(feature = "ui-native")]
pub fn add_ui(app: &mut App) {
    app.add_plugins(native::NativeUiPlugin);
}

#[cfg(not(feature = "ui-native"))]
pub fn add_ui(_app: &mut App) {
    panic!(
        "Supersolid UI backend not yet implemented — build with the default \
         `ui-native` feature. TODO(supersolid-runtime)."
    );
}
