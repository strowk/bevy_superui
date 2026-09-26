//! `widgets_showcase` — the basic HTML form controls: a text `<input>`, an
//! `<input type=checkbox>`, and an `<input type=range>` slider with a
//! JS-bound value readout. `assets/ui/widgets_showcase/style.css` overrides
//! `::slider-thumb`, proving author CSS wins over the framework's default
//! slider look (`docs/fork-patches.md#slider-default-layer`).
//!
//! `cargo run -p widgets_showcase`. Driven end-to-end by
//! `tests/widgets_showcase.spec.ts` via the `superui_test` CLI.

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
    App::new()
        .add_plugins(DefaultPlugins.set(web_asset_plugin(default())).set(WindowPlugin {
            primary_window: Some(web_window(Window::default())),
            ..default()
        }))
        .add_plugins(SuperUiPlugin)
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    commands.spawn(Camera2d);
    commands.spawn(SuperUiRoot::from_asset_dir("ui/widgets_showcase", &assets));
}
