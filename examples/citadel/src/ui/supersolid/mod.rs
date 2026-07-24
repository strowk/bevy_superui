use bevy::prelude::*;
use superui::prelude::{SuperUiPlugin, SuperUiRoot};

use crate::sim::UiSnapshot;

pub mod bridge;
use bridge::{build_frame, register_bridge};

pub struct SupersolidUiPlugin;

impl Plugin for SupersolidUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(SuperUiPlugin);
        register_bridge(app);
        app.add_systems(Startup, mount_ui);
        // Push the whole frame every update; forwarded to JS bevy.on("frame").
        app.add_systems(Update, push_ui_frame);
    }
}

fn mount_ui(mut commands: Commands, assets: Res<AssetServer>) {
    commands.spawn(SuperUiRoot::from_asset_dir("ui/citadel", &assets));
}

fn push_ui_frame(snap: Res<UiSnapshot>, mut commands: Commands) {
    let frame = build_frame(&snap);
    commands.trigger(frame);
}
