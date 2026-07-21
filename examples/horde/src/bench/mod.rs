//! Headless macro-benchmark harness for the horde game.
//! See docs/superpowers/specs/2026-07-21-horde-benchmark-harness-design.md.

use std::time::Duration;

use bevy::asset::io::memory::{Dir, MemoryAssetReader};
use bevy::asset::io::{AssetSource, AssetSourceId};
use bevy::asset::AssetPlugin;
use bevy::image::TextureAtlasPlugin;
use bevy::input::InputPlugin;
use bevy::input_focus::{InputFocus, InputFocusVisible};
use bevy::picking::{InteractionPlugin, PickingPlugin};
use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use bevy::text::TextPlugin;
use bevy::time::TimeUpdateStrategy;
use bevy::ui::UiPlugin;

use crate::game_state::{apply_menu_intents, GameState};
use crate::sim::enemy::Enemy;
use crate::sim::snapshot::assemble_world_snapshot;
use crate::sim::{Intent, IntentQueue, Player, SimConfig, UiSnapshot};

/// Which UI backend the bench app assembles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backend {
    /// sim + snapshot + synthetic projection only — the shared floor.
    Null,
    /// native `bevy_ui` UI.
    Native,
    /// supersolid TSX UI.
    Supersolid,
}

impl Backend {
    pub fn label(self) -> &'static str {
        match self {
            Backend::Null => "null",
            Backend::Native => "native",
            Backend::Supersolid => "supersolid",
        }
    }
}

/// Fixed per-update time step: exactly one FixedUpdate tick per app.update().
pub const DT: f64 = 1.0 / 60.0;
/// Synthetic viewport used by the bench projection (no camera dependency).
pub const VIEWPORT: Vec2 = Vec2::new(1280.0, 720.0);

/// Frame counter that drives the deterministic scripted auto-player.
/// Frame 0 is the first update (MainMenu → sends StartGame); Playing typically begins at frame 1.
#[derive(Resource, Default)]
pub struct BenchFrame(pub u64);

const HTML: &str = include_str!("../../assets/ui/horde/index.html");
const CSS: &str = include_str!("../../assets/ui/horde/theme.css");
const TSX: &str = include_str!("../../assets/ui/horde/app.tsx");

/// Deterministic scripted player: frame-indexed movement/aim/fire + periodic
/// weapon-switch and inventory-toggle. No wall-clock, no randomness of its own.
pub fn auto_player(
    frame: Res<BenchFrame>,
    state: Res<State<GameState>>,
    players: Query<&Transform, With<Player>>,
    enemies: Query<&Transform, With<Enemy>>,
    mut intents: ResMut<IntentQueue>,
) {
    if *state.get() == GameState::MainMenu {
        intents.push(Intent::StartGame);
        return;
    }
    if *state.get() != GameState::Playing {
        return;
    }
    let f = frame.0;
    let a = f as f32 * 0.03;
    intents.push(Intent::Move(Vec2::new(a.cos(), a.sin())));

    let aim = players
        .single()
        .ok()
        .map(|p| p.translation.truncate())
        .map(|pp| {
            enemies
                .iter()
                .map(|t| t.translation.truncate())
                .min_by(|x, y| {
                    x.distance_squared(pp)
                        .partial_cmp(&y.distance_squared(pp))
                        .unwrap()
                })
                .map(|nearest| (nearest - pp).normalize_or_zero())
                .filter(|v| *v != Vec2::ZERO)
                .unwrap_or(Vec2::new((a * 1.7).cos(), (a * 1.7).sin()))
        })
        .unwrap_or(Vec2::Y);
    intents.push(Intent::Aim(aim));
    intents.push(Intent::Shoot(true));

    if f > 0 && f % 300 == 0 {
        intents.push(Intent::SwitchWeapon(((f / 300) % 4) as usize));
    }
    if f > 0 && f % 500 == 0 {
        intents.push(Intent::ToggleInventory);
    }
}

fn advance_frame(mut frame: ResMut<BenchFrame>) {
    frame.0 += 1;
}

/// World→screen projection with a fixed viewport; camera-free replacement for
/// `ui::project::project_snapshot` so the bench never depends on a render camera.
pub fn synthetic_project(mut snap: ResMut<UiSnapshot>, cfg: Res<SimConfig>) {
    let half = cfg.arena_half.max(1.0);
    let map = |w: Vec2| {
        Vec2::new(
            (w.x / half * 0.5 + 0.5) * VIEWPORT.x,
            (0.5 - w.y / half * 0.5) * VIEWPORT.y,
        )
    };
    for n in snap.enemies.iter_mut() {
        n.screen_pos = map(n.world_pos);
    }
    for d in snap.damage_numbers.iter_mut() {
        d.screen_pos = map(d.world_pos);
    }
    for b in snap.blips.iter_mut() {
        b.screen_pos = map(b.world_pos);
    }
}

fn memory_asset_dir() -> Dir {
    let dir = Dir::new("assets".into());
    dir.insert_asset("ui/horde/index.html".as_ref(), HTML.as_bytes());
    dir.insert_asset("ui/horde/theme.css".as_ref(), CSS.as_bytes());
    dir.insert_asset("ui/horde/app.tsx".as_ref(), TSX.as_bytes());
    dir
}

/// Build a finished, headless, deterministic bench app for `backend`.
pub fn build_bench_app(backend: Backend, sim: SimConfig) -> App {
    let mut app = App::new();

    // Memory asset source so supersolid loads the real authored assets headlessly.
    let dir = memory_asset_dir();
    app.register_asset_source(
        AssetSourceId::Default,
        AssetSource::build().with_reader(move || Box::new(MemoryAssetReader { root: dir.clone() })),
    );

    // Identical base plugin recipe for every backend (proven in tests/support).
    app.add_plugins((
        bevy::time::TimePlugin,
        bevy::app::TaskPoolPlugin::default(),
        AssetPlugin::default(),
        WindowPlugin::default(),
        bevy::image::ImagePlugin::default(),
        TextureAtlasPlugin,
        TextPlugin,
        (InputPlugin, PickingPlugin, InteractionPlugin, UiPlugin),
        StatesPlugin,
    ));
    app.init_resource::<InputFocus>()
        .init_resource::<InputFocusVisible>();

    // Deterministic clock: one FixedUpdate tick per update; no wall-clock.
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(DT)));
    app.insert_resource(Time::<Fixed>::from_seconds(DT));

    // Seed the sim from the passed config (SimPlugin re-reads env otherwise).
    app.init_state::<GameState>();
    app.insert_resource(sim.clone());
    app.insert_resource(crate::sim::Rng::new(sim.seed));

    app.add_plugins(crate::sim::SimPlugin);
    // SimPlugin::build re-inserts SimConfig::from_env(); overwrite with ours.
    app.insert_resource(sim);

    // Menu-intent → state transitions (added by main.rs in the real game).
    app.add_systems(PostUpdate, apply_menu_intents);

    // Bench driver systems.
    app.init_resource::<BenchFrame>();
    app.add_systems(PreUpdate, auto_player);
    app.add_systems(Last, advance_frame);
    app.add_systems(
        Update,
        synthetic_project
            .after(assemble_world_snapshot)
            .run_if(in_state(GameState::Playing)),
    );

    // Chosen UI backend (null adds nothing).
    match backend {
        Backend::Null => {}
        Backend::Native => {
            app.add_plugins(crate::ui::native::NativeUiPlugin);
        }
        Backend::Supersolid => {
            app.add_plugins(crate::ui::supersolid::SupersolidUiPlugin);
        }
    }

    app.finish();
    app
}

/// Cheap determinism fingerprint read from the current snapshot.
pub fn trajectory_signature(app: &App) -> (u32, u32, usize) {
    let s = app.world().resource::<UiSnapshot>();
    (s.kills, s.wave, s.enemies.len())
}

#[cfg(test)]
mod determinism_tests {
    use super::*;

    fn run_to(frame: usize) -> (u32, u32, usize) {
        let mut app = build_bench_app(Backend::Null, SimConfig::play());
        for _ in 0..frame {
            app.update();
        }
        trajectory_signature(&app)
    }

    #[test]
    fn identical_runs_produce_identical_trajectory() {
        let a = run_to(120);
        let b = run_to(120);
        assert_eq!(a, b, "same seed + script must be bit-identical");
    }

    #[test]
    fn trajectory_is_nontrivial() {
        let (_kills, wave, enemies) = run_to(120);
        assert!(enemies > 0, "swarm should be populated by frame 120");
        assert!(wave >= 1, "at least wave 1 should have started");
    }
}

#[cfg(test)]
mod app_tests {
    use super::*;
    use crate::game_state::GameState;
    use bevy::prelude::State;

    fn boots_and_plays(backend: Backend) {
        let mut app = build_bench_app(backend, crate::sim::SimConfig::play());
        for _ in 0..40 { app.update(); }
        let state = app.world().resource::<State<GameState>>().get().clone();
        assert_eq!(state, GameState::Playing, "{:?} should reach Playing", backend);
        // The auto-player + sim must have produced enemies by frame 40.
        let snap = app.world().resource::<crate::sim::UiSnapshot>();
        assert!(!snap.enemies.is_empty(), "{:?}: enemies should have spawned", backend);
    }

    #[test]
    fn null_backend_boots() { boots_and_plays(Backend::Null); }
    #[test]
    fn native_backend_boots() { boots_and_plays(Backend::Native); }
    #[test]
    fn supersolid_backend_boots() { boots_and_plays(Backend::Supersolid); }
}
