//! Loads a bestiary of monsters from a JSON asset and pushes it to the UI
//! over the Bevy bridge, once the UI announces itself ready.
//!
//! - `cargo run -p bestiary --features hmr` — native, live `.tsx` via the
//!   transpiling asset loader, state-preserving hot reload.
//! - `cargo run -p bestiary` — native, loads the pre-transpiled
//!   `.superui/build/app.js` (build.rs output); no HMR.
//! - `cargo build -p bestiary --target wasm32-unknown-unknown` — web build.

use bevy::prelude::*;
use superui::prelude::{SuperUiPlugin, SuperUiRoot};

use bestiary::{Bestiary, BestiaryHandle, BestiaryLoader, BridgeState, push_bestiary, register_bridge};

/// On the web, bind the primary window to the host page's canvas. Identity on native.
fn web_window(window: bevy::window::Window) -> bevy::window::Window {
    #[cfg(target_arch = "wasm32")]
    let window = bevy::window::Window {
        canvas: Some("#superui-canvas".into()),
        fit_canvas_to_parent: true,
        ..window
    };
    window
}

/// Bevy probes for a `<asset>.meta` sidecar next to every asset it loads. Those
/// files are not shipped, which on native is a silent miss but on the web is a
/// 404 per asset in the browser console. Skip the probe on wasm. Identity on native.
fn web_asset_plugin(plugin: AssetPlugin) -> AssetPlugin {
    #[cfg(target_arch = "wasm32")]
    let plugin = AssetPlugin {
        meta_check: bevy::asset::AssetMetaCheck::Never,
        ..plugin
    };
    plugin
}

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(web_asset_plugin(default())).set(WindowPlugin {
        primary_window: Some(web_window(Window::default())),
        ..default()
    }));
    app.init_asset::<Bestiary>().register_asset_loader(BestiaryLoader);
    app.add_plugins(SuperUiPlugin);
    register_bridge(&mut app);
    app.init_resource::<BridgeState>();
    app.add_systems(Startup, setup);
    app.add_systems(Update, push_bestiary);
    app.run();
}

fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    commands.spawn(Camera2d);
    commands.insert_resource(BestiaryHandle(assets.load("data/bestiary.json")));
    commands.spawn(SuperUiRoot::from_asset_dir("ui/bestiary", &assets));
}
