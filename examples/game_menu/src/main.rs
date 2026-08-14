//! Runnable Supersolid game menu — a sci-fi "Flight OS" front-end authored in
//! Solid-style `.tsx` under `assets/ui/game_menu/`, mounted on `SuperUiPlugin`.
//!
//! It reproduces (within superui's bevy_ui/flair limits) a sci-fi game-menu
//! design: a main menu, pause overlay, systems-config screen and a game-over
//! screen, plus a bottom-right "preview" tab bar that switches between them —
//! the interactive part that exercises supersolid reactivity.
//!
//! - `cargo run -p game_menu --features hmr` — native, live `.tsx` via the
//!   transpiling asset loader, state-preserving hot reload.
//! - `cargo run -p game_menu` — native, loads the pre-transpiled
//!   `.superui/build/app.js` (build.rs output); no HMR.
//! - `cargo build -p game_menu --target wasm32-unknown-unknown` — web build,
//!   loads the pre-transpiled `.superui/build/app.js` (the transpiler never enters wasm).

use bevy::prelude::*;
use superui::prelude::{SuperUiPlugin, SuperUiRoot};

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
    }))
        .add_plugins(SuperUiPlugin)
        .add_systems(Startup, setup);

    // Opt-in: expose the app over BRP so bevy_brp_mcp can screenshot/inspect it.
    #[cfg(feature = "mcp_debug")]
    {
        app.add_plugins(bevy_brp_extras::BrpExtrasPlugin)
            .register_type::<mcp_debug::DebugClick>()
            .init_resource::<mcp_debug::DebugClick>()
            .add_systems(Update, mcp_debug::debug_click_system);
    }

    
    // FPS debug overlay in the reserved top-left corner (opt-in via `debug-ui`).
    app.add_plugins({
        #[cfg(feature = "debug-ui")]
        {
            (
                bevy::diagnostic::FrameTimeDiagnosticsPlugin::default(),
                bevy::dev_tools::fps_overlay::FpsOverlayPlugin::default(),
            )
        }
        #[cfg(not(feature = "debug-ui"))]
        {
            ()
        }
    });

    app.run();
}

fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    commands.spawn(Camera2d);
    commands.spawn(SuperUiRoot::from_asset_dir("ui/game_menu", &assets));
}

/// Opt-in click injector (feature `mcp_debug`): lets a BRP client drive a real
/// `Pointer<Click>` through the whole picking/observer path (bevy_brp injects
/// keys, not mouse). Set `DebugClick(Some(entity))` via `world_insert_resources`;
/// the system fires the click and clears it.
#[cfg(feature = "mcp_debug")]
mod mcp_debug {
    use bevy::camera::NormalizedRenderTarget;
    use bevy::picking::backend::HitData;
    use bevy::picking::events::{Click, Pointer};
    use bevy::picking::pointer::{Location, PointerId};
    use bevy::prelude::*;
    use bevy::window::{PrimaryWindow, WindowRef};

    #[derive(Resource, Default, Reflect)]
    #[reflect(Resource)]
    pub struct DebugClick(pub Option<Entity>);

    pub fn debug_click_system(
        mut click: ResMut<DebugClick>,
        mut commands: Commands,
        cameras: Query<Entity, With<Camera>>,
        window: Query<Entity, With<PrimaryWindow>>,
    ) {
        let Some(entity) = click.0.take() else {
            return;
        };
        let (Some(camera), Ok(win)) = (cameras.iter().next(), window.single()) else {
            return;
        };
        let Some(target) = WindowRef::Entity(win).normalize(Some(win)) else {
            return;
        };
        let location = Location {
            target: NormalizedRenderTarget::Window(target),
            position: Vec2::ZERO,
        };
        commands.trigger(Pointer {
            pointer_id: PointerId::Mouse,
            pointer_location: location,
            entity,
            event: Click {
                button: bevy::picking::pointer::PointerButton::Primary,
                hit: HitData::new(camera, 0.0, None, None),
                duration: std::time::Duration::from_millis(0),
            },
        });
    }
}
