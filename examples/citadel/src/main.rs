use bevy::prelude::*;

use citadel::sim::{build_economy, CitadelConfig, Economy, SimPlugin};
use citadel::ui::supersolid::SupersolidUiPlugin;

// ── Load "crank" ────────────────────────────────────────────────────────────
// The windowed example ships a comfortable node count so it renders smoothly, and
// lets you dial the load up/down live to find the frame-rate breaking point (the
// same knob the headless benchmark drives with `--building-count`/`--sweep`).
// `building_count` is the dominant node driver (~16 UI nodes per building card).
//
//   [  or  -   → fewer buildings (lighter, higher FPS)
//   ]  or  =   → more buildings (heavier, stress)
const WINDOWED_BUILDINGS: usize = 40; // ~900 nodes — smooth showcase default
const CRANK_STEP: usize = 16; // one grid row of 4 × a few
const CRANK_MIN: usize = 8;
const CRANK_MAX: usize = 400; // matches the benchmark's stress ceiling

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
    App::new()
        // Insert BEFORE SimPlugin so it keeps our (lighter) windowed config instead
        // of the 120-building default the benchmark uses. Only `building_count` is
        // cranked live; units/techs stay fixed and modest.
        .insert_resource(CitadelConfig {
            building_count: WINDOWED_BUILDINGS,
            unit_count: 24,
            tech_count: 32,
            seed: 1,
        })
        .add_plugins(DefaultPlugins.set(web_asset_plugin(default())).set(WindowPlugin {
            primary_window: Some(web_window(Window {
                title: "Citadel".into(),
                resolution: (1600u32, 900u32).into(),
                ..default()
            })),
            ..default()
        }))
        .add_plugins((SimPlugin, SupersolidUiPlugin))
        // FPS debug overlay in the reserved top-left corner (opt-in via `debug-ui`).
        .add_plugins({
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
        })
        .add_systems(Startup, (setup_camera, announce_controls))
        // Read the crank, then rebuild the economy if the count changed.
        .add_systems(Update, (crank_load, apply_crank).chain())
        .run();
}

fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}

fn announce_controls() {
    info!(
        "citadel: load crank — press '[' / '-' for fewer buildings, ']' / '=' for more \
         (start = {WINDOWED_BUILDINGS}, range {CRANK_MIN}..{CRANK_MAX})."
    );
}

/// Adjust the live load in response to the keyboard.
fn crank_load(keys: Res<ButtonInput<KeyCode>>, mut cfg: ResMut<CitadelConfig>) {
    let up = keys.just_pressed(KeyCode::BracketRight)
        || keys.just_pressed(KeyCode::Equal)
        || keys.just_pressed(KeyCode::NumpadAdd);
    let down = keys.just_pressed(KeyCode::BracketLeft)
        || keys.just_pressed(KeyCode::Minus)
        || keys.just_pressed(KeyCode::NumpadSubtract);
    if !up && !down {
        return;
    }
    let next = if up {
        (cfg.building_count + CRANK_STEP).min(CRANK_MAX)
    } else {
        cfg.building_count.saturating_sub(CRANK_STEP).max(CRANK_MIN)
    };
    if next != cfg.building_count {
        set_load(&mut cfg, next);
        info!(
            "citadel load: {} buildings, {} units, {} techs",
            cfg.building_count, cfg.unit_count, cfg.tech_count
        );
    }
}

/// One knob scales the whole screen: buildings dominate node count, and units/techs
/// scale proportionally so dialing down actually lightens the frame.
fn set_load(cfg: &mut CitadelConfig, buildings: usize) {
    cfg.building_count = buildings;
    cfg.unit_count = (buildings * 3 / 5).max(4);
    cfg.tech_count = (buildings * 4 / 5).max(4);
}

/// Rebuild the economy when the cranked `building_count` no longer matches. The
/// sim's own tick systems keep running on the resized world; the `<Keyed>` UI diffs
/// the new list (rows added/removed) without a full re-render. Clock/tick are
/// preserved so the mission timer doesn't jump on a crank.
fn apply_crank(cfg: Res<CitadelConfig>, econ: Option<ResMut<Economy>>) {
    let Some(mut econ) = econ else {
        return;
    };
    if econ.buildings.len() != cfg.building_count {
        let (clock, tick) = (econ.clock, econ.tick);
        *econ = build_economy(&cfg);
        econ.clock = clock;
        econ.tick = tick;
    }
}
