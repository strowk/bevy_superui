use bevy::prelude::*;
use superui::prelude::{SuperUiPlugin, SuperUiRoot};
use superui::JsSource;
use superui_css::style::StyleSheet;

use crate::game_state::GameState;
use crate::sim::{Intent, IntentQueue, SimConfig, UiSnapshot};

pub mod bridge;
use bridge::{build_frame, register_bridge, ToggleInventoryFwd};

/// Live `.tsx` (transpiled at load, hot-reloadable) only on native + `hmr`;
/// every other build loads the pre-transpiled `.js`.
const USE_LIVE_TSX: bool = cfg!(all(not(target_arch = "wasm32"), feature = "hmr"));

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
    let js: Handle<JsSource> = if USE_LIVE_TSX {
        assets.load("ui/horde/app.tsx")
    } else {
        assets.load("ui/horde/app.generated.js")
    };
    commands.spawn((
        Node::default(),
        SuperUiRoot {
            html: assets.load("ui/horde/index.html"),
            css: assets.load::<StyleSheet>("ui/horde/theme.css"),
            js,
        },
    ));
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
