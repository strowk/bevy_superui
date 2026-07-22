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
//!   `app.generated.js` (build.rs output); no HMR.
//! - `cargo build -p game_menu --target wasm32-unknown-unknown` — web build,
//!   loads `app.generated.js` (the transpiler never enters wasm).

use bevy::prelude::*;
use superui::prelude::{SuperUiPlugin, SuperUiRoot};
use superui::JsSource;
use superui_css::style::StyleSheet;

/// Live `.tsx` (transpiled at load, hot-reloadable) is used only on native
/// builds with the `hmr` feature; every other build loads the pre-transpiled JS.
const USE_LIVE_TSX: bool = cfg!(all(not(target_arch = "wasm32"), feature = "hmr"));

fn main() {
    let mut app = App::new();

    let asset_plugin = AssetPlugin {
        // Only meaningful with `bevy/file_watcher` (pulled by the `hmr` feature).
        watch_for_changes_override: Some(USE_LIVE_TSX),
        ..default()
    };
    app.add_plugins(DefaultPlugins.set(asset_plugin))
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

    app.run();
}

fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    commands.spawn(Camera2d);

    // Native+hmr loads `app.tsx` through the transpiling TsxLoader (live HMR);
    // wasm / no-hmr loads the build.rs-generated `.js`. Both yield Handle<JsSource>.
    let js: Handle<JsSource> = if USE_LIVE_TSX {
        assets.load("ui/game_menu/app.tsx")
    } else {
        assets.load("ui/game_menu/app.generated.js")
    };

    // The SuperUiRoot entity is the bevy_ui root the authored markup reconciles
    // under. It must fill the window so the `#root`/`.stage` `100%`/`inset:0`
    // children resolve against the full viewport — a default auto-sized node
    // collapses to zero (its only children are `position:absolute`), leaving a
    // blank screen.
    commands.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        SuperUiRoot {
            html: assets.load("ui/game_menu/index.html"),
            css: assets.load::<StyleSheet>("ui/game_menu/style.css"),
            js,
        },
    ));
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
