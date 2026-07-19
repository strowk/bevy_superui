use bevy::prelude::*;

/// Marks an entity as the mount point for an authored UI (its asset handles).
#[derive(Component, Default)]
pub struct SuperUiRoot {
    pub html: Handle<crate::HtmlSource>,
    pub css: Handle<superui_css::style::StyleSheet>,
    pub js: Handle<crate::JsSource>,
}

/// The umbrella plugin. Filled in by Task 8.
pub struct SuperUiPlugin;
impl Plugin for SuperUiPlugin {
    fn build(&self, _app: &mut App) {}
}
