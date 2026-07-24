use bevy::prelude::*;
use superui::prelude::{SuperUiPlugin, SuperUiRoot};

use crate::game_state::GameState;
use crate::sim::{Intent, IntentQueue, SimConfig, UiSnapshot};

pub mod bridge;
use bridge::{build_frame, register_bridge, ToggleInventoryFwd};

pub struct SupersolidUiPlugin;

impl Plugin for SupersolidUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(SuperUiPlugin);
        register_bridge(app);
        app.add_systems(Startup, mount_ui);
        // Push the whole frame every update; forwarded to JS bevy.on("frame").
        app.add_systems(Update, (push_ui_frame, forward_toggle_inventory));
    }
}

fn mount_ui(mut commands: Commands, assets: Res<AssetServer>) {
    commands.spawn(SuperUiRoot::from_asset_dir("ui/horde", &assets));
}

fn push_ui_frame(
    snap: Res<UiSnapshot>,
    state: Res<State<GameState>>,
    cfg: Res<SimConfig>,
    mut commands: Commands,
) {
    let frame = build_frame(&snap, state.get(), cfg.arena_half);
    commands.trigger(frame);
}

/// Forward keyboard `ToggleInventory` to the TSX inventory modal (keyboard parity).
fn forward_toggle_inventory(intents: Res<IntentQueue>, mut commands: Commands) {
    if intents.0.iter().any(|i| matches!(i, Intent::ToggleInventory)) {
        commands.trigger(ToggleInventoryFwd);
    }
}
