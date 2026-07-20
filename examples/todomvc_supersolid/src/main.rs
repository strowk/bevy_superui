//! Runnable Supersolid TodoMVC — authored in Solid-style `.tsx` under
//! `assets/ui/todomvc_supersolid/`, mounted on `SuperUiPlugin`.
//!
//! - `cargo run -p todomvc_supersolid --features hmr` — native, live `.tsx` via
//!   the transpiling asset loader, state-preserving hot reload.
//! - `cargo run -p todomvc_supersolid` — native, loads the pre-transpiled
//!   `app.generated.js` (build.rs output); no HMR.
//! - `cargo build -p todomvc_supersolid --target wasm32-unknown-unknown` — web
//!   build, loads `app.generated.js` (the transpiler never enters wasm).

use bevy::prelude::*;
use superui::prelude::{SuperUiPlugin, SuperUiRoot};
use superui::JsSource;
use superui_css::style::StyleSheet;

/// Live `.tsx` (transpiled at load, hot-reloadable) is used only on native builds
/// with the `hmr` feature; every other build loads the pre-transpiled `.js`.
const USE_LIVE_TSX: bool = cfg!(all(not(target_arch = "wasm32"), feature = "hmr"));

fn main() {
    let mut app = App::new();

    let asset_plugin = AssetPlugin {
        // Only meaningful with `bevy/file_watcher` (pulled by the `hmr` feature).
        watch_for_changes_override: Some(USE_LIVE_TSX),
        ..default()
    };
    app.add_plugins(DefaultPlugins.set(asset_plugin))
        .add_plugins(SuperUiPlugin);

    #[cfg(feature = "mcp_debug")]
    {
        app.add_plugins(bevy_brp_extras::BrpExtrasPlugin)
            .register_type::<mcp_debug::DebugClick>()
            .init_resource::<mcp_debug::DebugClick>()
            .add_systems(Update, mcp_debug::debug_click_system);
    }

    app.add_systems(Startup, setup);
    app.run();
}

fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    commands.spawn(Camera2d);

    // Native+hmr loads `app.tsx` through the transpiling TsxLoader (live HMR);
    // wasm / no-hmr loads the build.rs-generated `.js`. Both yield Handle<JsSource>.
    let js: Handle<JsSource> = if USE_LIVE_TSX {
        assets.load("ui/todomvc_supersolid/app.tsx")
    } else {
        assets.load("ui/todomvc_supersolid/app.generated.js")
    };

    commands.spawn((
        Node::default(),
        SuperUiRoot {
            html: assets.load("ui/todomvc_supersolid/index.html"),
            css: assets.load::<StyleSheet>("ui/todomvc_supersolid/style.css"),
            js,
        },
    ));
}
