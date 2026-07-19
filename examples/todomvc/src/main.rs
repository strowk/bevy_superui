//! Runnable TodoMVC — authored in plain HTML/CSS/JS under `assets/ui/todomvc/`,
//! mounted on `SuperUiPlugin`. `cargo run -p todomvc` (native, hot-reloading);
//! `cargo build -p todomvc --target wasm32-unknown-unknown` (web build).
//!
//! The only non-web wiring is the `window.bevy` demo: `app.js` fires
//! `bevy.send("TodoAdded", { label })` when a todo is added, which this binary
//! registers as a Bevy command and logs — proving the ECS seam (design §9).

use bevy::prelude::*;
use serde::Deserialize;
use superui::prelude::{SuperUiPlugin, SuperUiRoot};
use superui_css::style::StyleSheet;

/// Fired from JS via `bevy.send("TodoAdded", { label })`.
#[derive(Event, Deserialize, Debug, Clone)]
struct TodoAdded {
    label: String,
}

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(AssetPlugin {
        // Enable native hot reload (design §6). Inert on wasm.
        watch_for_changes_override: Some(true),
        ..default()
    }))
    .add_plugins(SuperUiPlugin);

    // Register the one demo command so `bevy.send("TodoAdded", ...)` reaches ECS.
    use superui::prelude::SuperUiApp;
    app.add_superui_command::<TodoAdded>("TodoAdded");
    app.add_observer(|ev: On<TodoAdded>| info!("todo added: {}", ev.event().label));

    app.add_systems(Startup, setup);
    app.run();
}

fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    commands.spawn(Camera2d);
    commands.spawn(SuperUiRoot {
        html: assets.load("ui/todomvc/index.html"),
        css: assets.load::<StyleSheet>("ui/todomvc/style.css"),
        js: assets.load("ui/todomvc/app.js"),
    });
}
