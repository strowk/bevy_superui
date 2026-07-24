//! The smallest Supersolid app: a single button that counts its own clicks,
//! authored in Solid-style `.tsx` and run by superui's supersolid runtime on
//! top of bevy_ui.
//!
//! - `cargo run -p counter --features hmr` — native, live `.tsx` via the
//!   transpiling asset loader, state-preserving hot reload.
//! - `cargo run -p counter` — native, loads the pre-transpiled
//!   `app.generated.js` (build.rs output); no HMR.
//! - `cargo build -p counter --target wasm32-unknown-unknown` — web build.

use bevy::prelude::*;
use superui::prelude::{SuperUiPlugin, SuperUiRoot};
use superui::JsSource;
use superui_css::style::StyleSheet;

/// Live `.tsx` (transpiled at load, hot-reloadable) is used only on native
/// builds with the `hmr` feature; every other build loads the pre-transpiled JS.
const USE_LIVE_TSX: bool = cfg!(all(not(target_arch = "wasm32"), feature = "hmr"));

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
    let asset_plugin = AssetPlugin {
        // Only meaningful with `bevy/file_watcher` (pulled by the `hmr` feature).
        watch_for_changes_override: Some(USE_LIVE_TSX),
        ..default()
    };
    App::new()
        .add_plugins(DefaultPlugins.set(asset_plugin).set(WindowPlugin {
            primary_window: Some(web_window(Window::default())),
            ..default()
        }))
        .add_plugins(SuperUiPlugin)
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    commands.spawn(Camera2d);

    // Native+hmr loads `app.tsx` through the transpiling TsxLoader (live HMR);
    // wasm / no-hmr loads the build.rs-generated `.js`. Both yield Handle<JsSource>.
    let js: Handle<JsSource> = if USE_LIVE_TSX {
        assets.load("ui/counter/app.tsx")
    } else {
        assets.load("ui/counter/app.generated.js")
    };

    // The SuperUiRoot entity is the bevy_ui root the authored markup reconciles
    // under; fill the window so the centered layout resolves against the viewport.
    commands.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        SuperUiRoot {
            html: assets.load("ui/counter/index.html"),
            css: assets.load::<StyleSheet>("ui/counter/style.css"),
            js,
        },
    ));
}
