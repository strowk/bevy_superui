use bevy::prelude::*;

pub mod main_menu;
pub mod pause;
pub mod game_over;
pub mod inventory;
pub mod settings;

pub struct ScreensPlugin;
impl Plugin for ScreensPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            main_menu::MainMenuPlugin,
            pause::PausePlugin,
            game_over::GameOverPlugin,
            inventory::InventoryPlugin,
            settings::SettingsPlugin,
        ));
    }
}

/// Shared: a fullscreen centered overlay container bundle.
pub fn overlay(dim: bool) -> impl Bundle {
    (
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            row_gap: Val::Px(16.0),
            ..default()
        },
        BackgroundColor(if dim { Color::srgba(0.0, 0.0, 0.0, 0.6) } else { super::theme::BG }),
    )
}
