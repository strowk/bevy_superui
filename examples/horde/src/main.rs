// This example/benchmark vehicle retains some intentionally-unused scaffolding:
// sim API surface (Rng::unit_vec, Intent::Quit, IntentQueue::drain, Nameplate::kind)
// exists for future Supersolid backend parity and the benchmark harness — it mirrors
// the full public contract even where the current native-UI path doesn't consume every field.
#![allow(dead_code)]

use bevy::prelude::*;

mod game_state;
mod input;
mod sim;
mod ui;
mod world_render;

use game_state::GameState;

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(AssetPlugin {
        watch_for_changes_override: Some(cfg!(not(target_arch = "wasm32"))),
        ..default()
    }))
    .init_state::<GameState>()
    .add_plugins(sim::SimPlugin)
    .add_systems(Startup, (setup_camera, world_render::spawn_arena))
    .add_systems(Update, (game_state::apply_menu_intents, world_render::sync_sprites))
    .add_systems(PreUpdate, input::gather_input);

    // FPS debug overlay in the reserved top-left corner (opt-in via `debug-ui`).
    #[cfg(feature = "debug-ui")]
    app.add_plugins((
        bevy::diagnostic::FrameTimeDiagnosticsPlugin::default(),
        bevy::dev_tools::fps_overlay::FpsOverlayPlugin::default(),
    ));

    // BRP + extras for MCP-driven screenshots/key-injection (opt-in via `mcp_debug`).
    #[cfg(feature = "mcp_debug")]
    app.add_plugins(bevy_brp_extras::BrpExtrasPlugin);

    ui::add_ui(&mut app);

    app.run();
}

fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}
