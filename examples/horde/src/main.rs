use bevy::prelude::*;

use horde::game_state::{self, GameState};
use horde::{input, sim, ui, world_render};

/// On the web, bind the primary window to the host page's `<canvas id="superui-canvas">`
/// and size it to that element. Identity on native (default OS window).
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
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(AssetPlugin {
        watch_for_changes_override: Some(cfg!(not(target_arch = "wasm32"))),
        ..default()
    }).set(WindowPlugin {
        primary_window: Some(web_window(Window::default())),
        ..default()
    }))
    .init_state::<GameState>()
    .add_plugins(sim::SimPlugin)
    .add_systems(Startup, (setup_camera, world_render::spawn_arena))
    .add_systems(Update, (world_render::sync_sprites, world_render::render_explosions))
    // apply_menu_intents runs in PostUpdate so it sees intents pushed by UI button
    // handlers (Update) as well as keyboard input (PreUpdate), before clear_intents (Last).
    .add_systems(PostUpdate, game_state::apply_menu_intents)
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
