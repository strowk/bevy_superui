//! Headless macro-benchmark harness for the horde game.
//! See docs/superpowers/specs/2026-07-21-horde-benchmark-harness-design.md.

use std::time::{Duration, Instant};

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

/// Per-frame timing statistics from a benchmark run.
#[derive(Clone, Copy, Debug)]
pub struct Stats {
    pub mean_ms: f64,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub fps: f64,
}

/// Nearest-rank percentile over sorted samples using index `round(p*(n-1))`.
pub fn stats_from(mut samples: Vec<f64>) -> Stats {
    assert!(!samples.is_empty(), "stats_from: empty samples");
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = samples.len();
    let pct = |p: f64| {
        let idx = ((n - 1) as f64 * p).round() as usize;
        samples[idx]
    };
    let mean = samples.iter().sum::<f64>() / n as f64;
    Stats {
        mean_ms: mean,
        p50_ms: pct(0.50),
        p95_ms: pct(0.95),
        p99_ms: pct(0.99),
        fps: if mean > 0.0 { 1000.0 / mean } else { f64::INFINITY },
    }
}

/// Drive `frames` measured updates (after `warmup`), returning per-frame ms.
pub fn time_backend(backend: Backend, sim: SimConfig, frames: usize, warmup: usize) -> Vec<f64> {
    let mut app = build_bench_app(backend, sim);
    for _ in 0..warmup {
        app.update();
    }
    let mut out = Vec::with_capacity(frames);
    for _ in 0..frames {
        let t = Instant::now();
        app.update();
        out.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    out
}

use crate::ui::supersolid::bridge::build_frame;

/// Mean isolated cost of building the FrameDto (the JSON marshal) in ms.
/// Runs a supersolid app (real snapshot churn) but times only `build_frame`.
pub fn probe_marshal(sim: SimConfig, frames: usize, warmup: usize) -> f64 {
    let mut app = build_bench_app(Backend::Supersolid, sim);
    for _ in 0..warmup {
        app.update();
    }
    let mut total = 0.0;
    for _ in 0..frames {
        app.update();
        // Bind all borrows before the timer so only build_frame is measured.
        let w = app.world();
        let snap = w.resource::<UiSnapshot>();
        let state = w.resource::<State<GameState>>();
        let cfg = w.resource::<SimConfig>();
        let t = Instant::now();
        let dto = build_frame(snap, state.get(), cfg.arena_half);
        total += t.elapsed().as_secs_f64() * 1000.0;
        std::hint::black_box(&dto);
    }
    total / frames as f64
}

#[derive(Clone, Debug)]
pub struct Report {
    pub backend: Backend,
    pub cap: usize,
    pub frames: usize,
    pub total: Stats,
    /// Shared floor: mean total of the null backend (sim + snapshot).
    pub shared_ms: f64,
    /// UI-backend cost = total.mean − shared.
    pub ui_ms: f64,
    /// Supersolid only: isolated marshal cost (subset of ui_ms).
    pub marshal_ms: Option<f64>,
    /// Supersolid only: native backend mean total, for the gap.
    pub native_total_ms: Option<f64>,
}

/// Run the full differential: the chosen backend + the null floor (+ marshal &
/// native gap for supersolid).
pub fn run_report(backend: Backend, sim: SimConfig, frames: usize, warmup: usize) -> Report {
    let total = stats_from(time_backend(backend, sim.clone(), frames, warmup));
    let shared = stats_from(time_backend(Backend::Null, sim.clone(), frames, warmup)).mean_ms;

    let (marshal_ms, native_total_ms) = if backend == Backend::Supersolid {
        let m = probe_marshal(sim.clone(), frames, warmup);
        let n = stats_from(time_backend(Backend::Native, sim.clone(), frames, warmup)).mean_ms;
        (Some(m), Some(n))
    } else {
        (None, None)
    };

    Report {
        backend,
        cap: sim.enemy_cap,
        frames,
        ui_ms: total.mean_ms - shared,
        total,
        shared_ms: shared,
        marshal_ms,
        native_total_ms,
    }
}

/// Human-readable attribution table.
pub fn report_table(r: &Report) -> String {
    use std::fmt::Write as _;
    let mut s = String::new();
    let _ = writeln!(
        s,
        "backend={} enemy_cap={} frames={}",
        r.backend.label(),
        r.cap,
        r.frames
    );
    let _ = writeln!(
        s,
        "  total : mean {:.3} ms | p50 {:.3} | p95 {:.3} | p99 {:.3} | {:.1} fps",
        r.total.mean_ms, r.total.p50_ms, r.total.p95_ms, r.total.p99_ms, r.total.fps
    );
    let _ = writeln!(s, "  attribution (mean-based):");
    let pct = |x: f64| if r.total.mean_ms > 0.0 { 100.0 * x / r.total.mean_ms } else { 0.0 };
    let _ = writeln!(s, "    shared (sim+snapshot) : {:.3} ms ({:.1}%)", r.shared_ms, pct(r.shared_ms));
    let _ = writeln!(s, "    ui_backend            : {:.3} ms ({:.1}%)  [optimization ceiling]", r.ui_ms, pct(r.ui_ms));
    if let Some(m) = r.marshal_ms {
        let _ = writeln!(s, "      of which marshal    : {:.3} ms ({:.1}%)", m, pct(m));
        let _ = writeln!(s, "      reconcile+layout    : {:.3} ms ({:.1}%)  [finer split = Tracy follow-up]", r.ui_ms - m, pct(r.ui_ms - m));
    }
    if let Some(nt) = r.native_total_ms {
        let gap = if nt > 0.0 { r.total.mean_ms / nt } else { f64::INFINITY };
        let _ = writeln!(s, "  vs native floor: {:.3} ms  → gap {:.1}x  (closing to native saves up to {:.3} ms)", nt, gap, r.total.mean_ms - nt);
    }
    s
}

#[cfg(test)]
mod report_tests {
    use super::*;

    #[test]
    fn report_computes_ui_cost_as_total_minus_shared() {
        let r = run_report(Backend::Supersolid, SimConfig::play(), 40, 10);
        assert!((r.ui_ms - (r.total.mean_ms - r.shared_ms)).abs() < 1e-9);
        assert!(r.ui_ms >= 0.0, "ui_ms must be non-negative (supersolid UI cannot be cheaper than the null floor)");
        assert!(r.ui_ms <= r.total.mean_ms + 1e-9, "ui_ms cannot exceed total");
        assert!(r.marshal_ms.is_some(), "supersolid report must include marshal");
        assert!(r.native_total_ms.is_some(), "supersolid report must include native gap");
    }

    #[test]
    fn native_report_has_no_marshal() {
        let r = run_report(Backend::Native, SimConfig::play(), 40, 10);
        assert!(r.marshal_ms.is_none());
        assert!(r.native_total_ms.is_none());
    }

    #[test]
    fn table_renders_key_fields() {
        let r = run_report(Backend::Supersolid, SimConfig::play(), 40, 10);
        let t = report_table(&r);
        assert!(t.contains("supersolid"));
        assert!(t.contains("shared"));
        assert!(t.contains("ui_backend"));
        assert!(t.contains("marshal"));
    }
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
mod stats_tests {
    use super::*;

    #[test]
    fn percentiles_on_known_data() {
        // 1..=100 ms
        let samples: Vec<f64> = (1..=100).map(|n| n as f64).collect();
        let s = stats_from(samples);
        assert!((s.mean_ms - 50.5).abs() < 1e-9);
        assert_eq!(s.p50_ms, 51.0); // index round(0.5*99)=50 -> value 51
        assert_eq!(s.p95_ms, 95.0);
        assert_eq!(s.p99_ms, 99.0);
        assert!((s.fps - 1000.0 / 50.5).abs() < 1e-9);
    }

    #[test]
    fn timing_run_produces_frames() {
        let v = time_backend(Backend::Null, SimConfig::play(), 30, 5);
        assert_eq!(v.len(), 30);
        assert!(v.iter().all(|&ms| ms >= 0.0));
    }
}

/// Dependency-free JSON serialization of a report.
pub fn report_json(r: &Report) -> String {
    let opt = |v: Option<f64>| match v {
        Some(x) => format!("{:.6}", x),
        None => "null".to_string(),
    };
    format!(
        "{{\"backend\":\"{}\",\"enemy_cap\":{},\"frames\":{},\
         \"total_mean_ms\":{:.6},\"p50_ms\":{:.6},\"p95_ms\":{:.6},\"p99_ms\":{:.6},\"fps\":{:.3},\
         \"shared_ms\":{:.6},\"ui_ms\":{:.6},\"marshal_ms\":{},\"native_total_ms\":{}}}",
        r.backend.label(),
        r.cap,
        r.frames,
        r.total.mean_ms,
        r.total.p50_ms,
        r.total.p95_ms,
        r.total.p99_ms,
        r.total.fps,
        r.shared_ms,
        r.ui_ms,
        opt(r.marshal_ms),
        opt(r.native_total_ms),
    )
}

#[derive(Clone, Debug)]
pub struct BenchArgs {
    pub backend: Backend,
    pub preset: String,
    pub caps: Vec<usize>,
    pub frames: usize,
    pub warmup: usize,
    pub seed: u64,
    pub json: bool,
    pub dhat: bool,
}

/// Build a SimConfig from a preset name, overriding enemy_cap and seed.
pub fn sim_for(preset: &str, cap: usize, seed: u64) -> SimConfig {
    let mut cfg = match preset {
        "stress" => SimConfig::stress(),
        _ => SimConfig::play(),
    };
    cfg.enemy_cap = cap;
    if seed != 0 {
        cfg.seed = seed;
    }
    cfg
}

/// Minimal `--key value` / `--flag` parser. `backend` is required.
pub fn parse_args(argv: &[String]) -> Result<BenchArgs, String> {
    let mut backend: Option<Backend> = None;
    let mut preset = "play".to_string();
    let mut caps: Vec<usize> = Vec::new();
    let mut frames = 1000usize;
    let mut warmup = 100usize;
    let mut seed = 0u64;
    let mut json = false;
    let mut dhat = false;

    let mut i = 0;
    while i < argv.len() {
        let key = argv[i].as_str();
        // Advance i and return the next token, or an error.
        let advance = |i: &mut usize| -> Result<&str, String> {
            *i += 1;
            argv.get(*i).map(|s| s.as_str()).ok_or_else(|| format!("missing value for {key}"))
        };
        match key {
            "--backend" => {
                backend = Some(match advance(&mut i)? {
                    "null" => Backend::Null,
                    "native" => Backend::Native,
                    "supersolid" => Backend::Supersolid,
                    other => return Err(format!("unknown backend '{other}'")),
                });
            }
            "--preset" => preset = advance(&mut i)?.to_string(),
            "--enemy-cap" => {
                let v = advance(&mut i)?.parse().map_err(|_| "bad --enemy-cap".to_string())?;
                caps = vec![v];
            }
            "--sweep" => {
                caps = advance(&mut i)?
                    .split(',')
                    .map(|s| s.trim().parse::<usize>().map_err(|_| "bad --sweep list".to_string()))
                    .collect::<Result<_, _>>()?;
            }
            "--frames" => frames = advance(&mut i)?.parse().map_err(|_| "bad --frames".to_string())?,
            "--warmup" => warmup = advance(&mut i)?.parse().map_err(|_| "bad --warmup".to_string())?,
            "--seed" => seed = advance(&mut i)?.parse().map_err(|_| "bad --seed".to_string())?,
            "--format" => json = advance(&mut i)? == "json",
            "--dhat" => dhat = true,
            other => return Err(format!("unknown arg '{other}'")),
        }
        i += 1;
    }

    let backend = backend.ok_or("--backend is required (null|native|supersolid)")?;
    if caps.is_empty() {
        caps = vec![match preset.as_str() {
            "stress" => SimConfig::stress().enemy_cap,
            _ => SimConfig::play().enemy_cap,
        }];
    }
    Ok(BenchArgs { backend, preset, caps, frames, warmup, seed, json, dhat })
}

#[cfg(test)]
mod cli_tests {
    use super::*;

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parses_backend_and_flags() {
        let a = parse_args(&args(&[
            "--backend", "supersolid", "--preset", "stress",
            "--sweep", "60,400", "--frames", "500", "--warmup", "50",
            "--seed", "7", "--format", "json",
        ]))
        .unwrap();
        assert_eq!(a.backend, Backend::Supersolid);
        assert_eq!(a.preset, "stress");
        assert_eq!(a.caps, vec![60, 400]);
        assert_eq!(a.frames, 500);
        assert_eq!(a.warmup, 50);
        assert_eq!(a.seed, 7);
        assert!(a.json);
    }

    #[test]
    fn backend_is_required() {
        assert!(parse_args(&args(&["--frames", "10"])).is_err());
    }

    #[test]
    fn json_contains_fields() {
        let r = run_report(Backend::Native, sim_for("play", 60, 1), 20, 5);
        let j = report_json(&r);
        assert!(j.contains("\"backend\":\"native\""));
        assert!(j.contains("\"total_mean_ms\""));
        assert!(j.contains("\"shared_ms\""));
        assert!(j.contains("\"ui_ms\""));
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
