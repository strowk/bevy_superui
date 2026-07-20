// Under incremental construction: sim/UI items are wired up across later tasks.
// TODO(task-32): remove this once all modules are consumed.
#![allow(dead_code)]

use bevy::prelude::*;

mod ui;
mod sim;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(AssetPlugin {
            watch_for_changes_override: Some(true),
            ..default()
        }))
        .add_systems(Startup, setup_camera)
        .run();
}

fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}
