//! Chest <-> player-inventory drag-and-drop demo: two panels of item tiles you
//! drag between with the mouse, authored in Solid-style `.tsx`. The worked
//! example for superui's browser-faithful mouse events + document-level drag
//! capture: `startDrag` registers `document`-level `mousemove`/`mouseup`
//! listeners, and the drop slot resolves from `event.target` walking up
//! `parentNode` for a `data-slot` ancestor (see `assets/ui/inventory/app.tsx`).
//!
//! - `cargo run -p inventory --features hmr` — native, live `.tsx` via the
//!   transpiling asset loader, state-preserving hot reload.
//! - `cargo run -p inventory` — native, loads the pre-transpiled
//!   `.superui/build/app.js` (build.rs output); no HMR.
//! - `cargo build -p inventory --target wasm32-unknown-unknown` — web build.

use bevy::prelude::*;
use superui::prelude::{SuperUiPlugin, SuperUiRoot};

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
    app.add_plugins(SuperUiPlugin);
    app.add_systems(Startup, setup);
    app.run();
}

fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    commands.spawn(Camera2d);
    commands.spawn(SuperUiRoot::from_asset_dir("ui/inventory", &assets));
}
