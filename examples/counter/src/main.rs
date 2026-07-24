//! The smallest Supersolid app: a single button that counts its own clicks,
//! authored in Solid-style `.tsx` and mounted from a web-like `index.html`.
//!
//! - `cargo run -p counter --features hmr` — native, live `.tsx` via the
//!   transpiling asset loader, state-preserving hot reload.
//! - `cargo run -p counter` — native, loads the pre-transpiled
//!   `.superui/build/app.js` (build.rs output); no HMR.
//! - `cargo build -p counter --target wasm32-unknown-unknown` — web build.

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

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(web_window(Window::default())),
            ..default()
        }))
        .add_plugins(SuperUiPlugin)
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    commands.spawn(Camera2d);
    commands.spawn(SuperUiRoot::from_asset_dir("ui/counter", &assets));
}
