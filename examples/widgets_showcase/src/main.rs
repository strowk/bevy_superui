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

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(SuperUiPlugin)
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    commands.spawn(Camera2d);
    commands.spawn(SuperUiRoot::from_asset_dir("ui/widgets_showcase", &assets));
}
