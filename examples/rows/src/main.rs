//! Windowed rows app. `cargo run -p rows` (vanilla) — set ROWS_BACKEND=supersolid
//! for the TSX build.
use bevy::prelude::*;
use superui::prelude::{SuperUiPlugin, SuperUiRoot};

fn main() {
    let dir = match std::env::var("ROWS_BACKEND").as_deref() {
        Ok("supersolid") => "ui/rows_solid",
        _ => "ui/rows_vanilla",
    };
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(SuperUiPlugin)
        .add_systems(Startup, move |mut c: Commands, a: Res<AssetServer>| {
            c.spawn(Camera2d);
            c.spawn(SuperUiRoot::from_asset_dir(dir, &a));
        })
        .run();
}
