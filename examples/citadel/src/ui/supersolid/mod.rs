use bevy::prelude::*;
use superui::prelude::{SuperUiPlugin, SuperUiRoot};
use superui::JsSource;
use superui_css::style::StyleSheet;

use crate::sim::UiSnapshot;

pub mod bridge;
use bridge::{build_frame, register_bridge};

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
        app.add_systems(Update, push_ui_frame);
    }
}

fn mount_ui(mut commands: Commands, assets: Res<AssetServer>) {
    let js: Handle<JsSource> = if USE_LIVE_TSX {
        assets.load("ui/citadel/app.tsx")
    } else {
        assets.load("ui/citadel/app.generated.js")
    };
    // The SuperUiRoot entity is the bevy_ui root the authored `<body>` reconciles
    // under. It must fill the window so the `#root`/`#hud`/`.screen` `100%` children
    // resolve against the full viewport (otherwise a default auto-sized node collapses
    // to content and `.screen`'s centering pivots around x=0, clipping the left half).
    commands.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        SuperUiRoot {
            html: assets.load("ui/citadel/index.html"),
            css: assets.load::<StyleSheet>("ui/citadel/theme.css"),
            js,
        },
    ));
}

fn push_ui_frame(snap: Res<UiSnapshot>, mut commands: Commands) {
    let frame = build_frame(&snap);
    commands.trigger(frame);
}
