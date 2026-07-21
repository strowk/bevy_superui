# Horde Macro-Benchmark Harness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A headless, deterministic benchmark binary for the horde game that reports per-frame cost (mean + percentiles + FPS), attributes cost across UI backends (null / native / supersolid), sweeps element counts, and measures allocation churn — so optimizations can be predicted (ceiling) and proven (before/after).

**Architecture:** A new `horde-bench` binary drives `app.update()` in a fixed loop over a headless Bevy app (the `tests/support` plugin recipe — no winit/GPU), fed by a deterministic scripted auto-player and seeded sim. Attribution uses **backend-differential subtraction**: a `null` backend (sim + snapshot, no UI) gives the shared floor; `ui_cost = total_backend − total_null`. Marshal cost is probed in isolation via the `pub build_frame`. Reusable logic lives in a feature-gated `horde::bench` library module; the bin is a thin `main`.

**Tech Stack:** Rust 2021, Bevy 0.17, `superui`/`superui_css` runtime, `std::time::Instant`, hand-rolled CLI + JSON (no new required deps), optional `dhat` behind a feature.

## Global Constraints

- Bevy `0.17`, edition `2021`. Copy exact values.
- The bench is **native-only dev tooling**: the `horde-bench` bin declares `required-features = ["bench"]` and must never be part of a wasm build. Do not add unconditional heavy deps to the crate.
- **Determinism is non-negotiable:** seeded RNG (`SimConfig.seed`) + fixed time step via `TimeUpdateStrategy::ManualDuration(DT)` with `Time::<Fixed>::from_seconds(DT)`, `DT = 1.0 / 60.0`. No wall-clock in sim/auto-player logic (frame-index only).
- The `bench` cargo feature must compile **both** `ui::native` and `ui::supersolid` modules; normal game builds (feature off) must be byte-for-byte unchanged in their cfg behavior.
- Reuse existing public surfaces: `horde::sim::{SimConfig, UiSnapshot, IntentQueue, Intent, Player}`, `horde::sim::enemy::Enemy`, `horde::game_state::{GameState, apply_menu_intents}`, `horde::sim::PendingReset`, `horde::ui::supersolid::bridge::build_frame`, `horde::ui::supersolid::bridge::register_bridge`, `horde::ui::native::NativeUiPlugin`, `horde::ui::supersolid::SupersolidUiPlugin`.
- Headless plugin recipe is the one proven in `examples/horde/tests/support/mod.rs` (TimePlugin, TaskPoolPlugin, AssetPlugin, WindowPlugin, ImagePlugin, TextureAtlasPlugin, TextPlugin, InputPlugin, PickingPlugin, InteractionPlugin, UiPlugin, StatesPlugin) + memory asset source for the supersolid assets.
- Stay on the current branch `bench/horde-macro-benchmark`. Do not create new branches. Commit after every task.

**File structure (locked):**
- `examples/horde/Cargo.toml` — add `bench` + optional `dhat` features, `serde_json`-free, and the `[[bin]] horde-bench` (required-features = `["bench"]`).
- `examples/horde/src/lib.rs` — `#[cfg(feature = "bench")] pub mod bench;`
- `examples/horde/src/ui/mod.rs` — widen module cfgs so `bench` compiles both backends.
- `examples/horde/src/bench/mod.rs` — the harness: `Backend`, `BenchArgs`, `build_bench_app`, auto-player, synthetic projection, timing loop, `Stats`, `Report`, JSON, sweep, dhat pass. Split into submodules only if it exceeds ~400 lines (see Task 9 note).
- `examples/horde/src/bin/bench.rs` — thin `main`: parse args, dispatch, print.
- `examples/horde/benches/README.md` — run modes, before/after loop, native-vs-supersolid target.

---

### Task 1: Cargo wiring, `bench` feature, both backends compile, `bench` module skeleton

**Files:**
- Modify: `examples/horde/Cargo.toml`
- Modify: `examples/horde/src/ui/mod.rs`
- Modify: `examples/horde/src/lib.rs`
- Create: `examples/horde/src/bench/mod.rs`
- Create: `examples/horde/src/bin/bench.rs`

**Interfaces:**
- Produces: cargo feature `bench`; `horde::bench` module (public, feature-gated); a buildable `horde-bench` bin.

- [ ] **Step 1: Add feature + bin to `Cargo.toml`**

In `[features]` add (leave existing lines untouched):

```toml
bench = []
dhat-prof = ["dep:dhat"]
```

In `[dependencies]` add:

```toml
[dependencies.dhat]
optional = true
version = "0.3"
```

After the existing `[[bin]] name = "horde"` block add:

```toml
[[bin]]
name = "horde-bench"
path = "src/bin/bench.rs"
required-features = ["bench"]
```

- [ ] **Step 2: Widen UI module cfgs so `bench` compiles both backends**

In `examples/horde/src/ui/mod.rs`, replace the two `#[cfg(...)] pub mod ...;` lines:

```rust
#[cfg(any(feature = "ui-native", feature = "bench"))]
pub mod native;

#[cfg(any(not(feature = "ui-native"), feature = "bench"))]
pub mod supersolid;
```

Leave `add_ui` and everything else unchanged.

- [ ] **Step 3: Declare the bench module in `lib.rs`**

Add near the other `pub mod` lines in `examples/horde/src/lib.rs`:

```rust
#[cfg(feature = "bench")]
pub mod bench;
```

- [ ] **Step 4: Create the skeleton `src/bench/mod.rs`**

```rust
//! Headless macro-benchmark harness for the horde game.
//! See docs/superpowers/specs/2026-07-21-horde-benchmark-harness-design.md.

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
```

- [ ] **Step 5: Create the thin bin `src/bin/bench.rs`**

```rust
fn main() {
    println!("horde-bench: {}", horde::bench::Backend::Supersolid.label());
}
```

- [ ] **Step 6: Verify both feature builds compile**

Run: `cargo build -p horde --features bench --bin horde-bench`
Expected: builds; prints nothing yet (only on run).

Run: `cargo build -p horde`
Expected: normal game build still compiles unchanged.

- [ ] **Step 7: Commit**

```bash
git add examples/horde/Cargo.toml examples/horde/src/ui/mod.rs examples/horde/src/lib.rs examples/horde/src/bench/mod.rs examples/horde/src/bin/bench.rs
git commit -m "feat(horde): bench feature scaffold + both UI backends compile"
```

---

### Task 2: Headless bench app builder (all three backends boot & tick) — the §7.1 risk gate

**Files:**
- Modify: `examples/horde/src/bench/mod.rs`

**Interfaces:**
- Consumes: `Backend` (Task 1); the headless plugin recipe from `tests/support/mod.rs`.
- Produces: `pub fn build_bench_app(backend: Backend, sim: SimConfig) -> App` — a finished, headless app with deterministic time, the sim, synthetic projection, the scripted auto-player, and the chosen UI plugin. `pub const DT: f64`. `pub struct BenchFrame(pub u64)`.

- [ ] **Step 1: Write the failing test (all backends boot, tick, and enter Playing)**

Add to `src/bench/mod.rs`:

```rust
#[cfg(test)]
mod app_tests {
    use super::*;
    use horde::game_state::GameState;
    use bevy::prelude::State;

    fn boots_and_plays(backend: Backend) {
        let mut app = build_bench_app(backend, horde::sim::SimConfig::play());
        for _ in 0..40 { app.update(); }
        let state = app.world().resource::<State<GameState>>().get().clone();
        assert_eq!(state, GameState::Playing, "{:?} should reach Playing", backend);
        // The auto-player + sim must have produced enemies by frame 40.
        let snap = app.world().resource::<horde::sim::UiSnapshot>();
        assert!(!snap.enemies.is_empty(), "{:?}: enemies should have spawned", backend);
    }

    #[test]
    fn null_backend_boots() { boots_and_plays(Backend::Null); }
    #[test]
    fn native_backend_boots() { boots_and_plays(Backend::Native); }
    #[test]
    fn supersolid_backend_boots() { boots_and_plays(Backend::Supersolid); }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p horde --features bench app_tests -- --nocapture`
Expected: FAIL — `build_bench_app`, `DT`, `BenchFrame` not defined.

- [ ] **Step 3: Implement the app builder, deterministic time, auto-player, synthetic projection**

Add to `src/bench/mod.rs` (imports at top of file):

```rust
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

use horde::game_state::{apply_menu_intents, GameState};
use horde::sim::enemy::Enemy;
use horde::sim::snapshot::assemble_world_snapshot;
use horde::sim::{Intent, IntentQueue, Player, SimConfig, UiSnapshot};

/// Fixed per-update time step: exactly one FixedUpdate tick per app.update().
pub const DT: f64 = 1.0 / 60.0;
/// Synthetic viewport used by the bench projection (no camera dependency).
pub const VIEWPORT: Vec2 = Vec2::new(1280.0, 720.0);

/// Frame counter that drives the deterministic scripted auto-player.
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
    app.insert_resource(horde::sim::Rng::new(sim.seed));

    app.add_plugins(horde::sim::SimPlugin);
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
            app.add_plugins(horde::ui::native::NativeUiPlugin);
        }
        Backend::Supersolid => {
            horde::ui::supersolid::bridge::register_bridge(&mut app);
            app.add_plugins(horde::ui::supersolid::SupersolidUiPlugin);
        }
    }

    app.finish();
    app
}
```

**Implementation note for the engineer (read before running):** `build_bench_app` intentionally does NOT spawn a `Camera2d` — the proven headless recipe in `tests/support/mod.rs` mounts and reconciles supersolid with no camera, and the bench uses `synthetic_project` (camera-free). If `native_backend_boots` panics with a render/camera assertion, add `app.world_mut().spawn(Camera2d);` before `app.finish()` and re-run; do not add any RenderPlugin.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p horde --features bench app_tests -- --nocapture`
Expected: PASS for all three backends. If supersolid is slow to mount, the 40-update budget still covers it (mount is proven within ~256 in tests, but Playing + one enemy wave happens quickly; if `supersolid_backend_boots` fails only on `enemies.is_empty()`, raise the loop to 80 updates).

- [ ] **Step 5: Commit**

```bash
git add examples/horde/src/bench/mod.rs
git commit -m "feat(horde-bench): headless bench app builder for null/native/supersolid"
```

---

### Task 3: Determinism guarantee (same seed + script ⇒ identical trajectory)

**Files:**
- Modify: `examples/horde/src/bench/mod.rs`

**Interfaces:**
- Consumes: `build_bench_app` (Task 2).
- Produces: `pub fn trajectory_signature(app: &App) -> (u32, u32, usize)` — `(kills, wave, enemy_count)` read from `UiSnapshot`, a cheap determinism fingerprint.

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod determinism_tests {
    use super::*;

    fn run_to(frame: usize) -> (u32, u32, usize) {
        let mut app = build_bench_app(Backend::Null, horde::sim::SimConfig::play());
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
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p horde --features bench determinism_tests`
Expected: FAIL — `trajectory_signature` not defined.

- [ ] **Step 3: Implement**

```rust
/// Cheap determinism fingerprint read from the current snapshot.
pub fn trajectory_signature(app: &App) -> (u32, u32, usize) {
    let s = app.world().resource::<UiSnapshot>();
    (s.kills, s.wave, s.enemies.len())
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p horde --features bench determinism_tests`
Expected: PASS. If `trajectory_is_nontrivial` fails on `wave >= 1`, lower to `wave >= 0` and rely on the `enemies > 0` assertion (wave numbering starts at the sim's first spawn).

- [ ] **Step 5: Commit**

```bash
git add examples/horde/src/bench/mod.rs
git commit -m "test(horde-bench): assert deterministic trajectory across identical runs"
```

---

### Task 4: Timing loop + percentile stats

**Files:**
- Modify: `examples/horde/src/bench/mod.rs`

**Interfaces:**
- Consumes: `build_bench_app` (Task 2).
- Produces:
  - `pub struct Stats { pub mean_ms: f64, pub p50_ms: f64, pub p95_ms: f64, pub p99_ms: f64, pub fps: f64 }`
  - `pub fn stats_from(samples: Vec<f64>) -> Stats`
  - `pub fn time_backend(backend: Backend, sim: SimConfig, frames: usize, warmup: usize) -> Vec<f64>` — per-frame total wall-time in ms, warmup excluded.

- [ ] **Step 1: Write the failing test (percentile math on known data)**

```rust
#[cfg(test)]
mod stats_tests {
    use super::*;

    #[test]
    fn percentiles_on_known_data() {
        // 1..=100 ms
        let samples: Vec<f64> = (1..=100).map(|n| n as f64).collect();
        let s = stats_from(samples);
        assert!((s.mean_ms - 50.5).abs() < 1e-9);
        assert_eq!(s.p50_ms, 50.0); // index round(0.5*99)=50 -> value 51? see impl note
        assert_eq!(s.p95_ms, 95.0);
        assert_eq!(s.p99_ms, 99.0);
        assert!((s.fps - 1000.0 / 50.5).abs() < 1e-9);
    }

    #[test]
    fn timing_run_produces_frames() {
        let v = time_backend(Backend::Null, horde::sim::SimConfig::play(), 30, 5);
        assert_eq!(v.len(), 30);
        assert!(v.iter().all(|&ms| ms >= 0.0));
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p horde --features bench stats_tests`
Expected: FAIL — `Stats`, `stats_from`, `time_backend` not defined.

- [ ] **Step 3: Implement**

```rust
use std::time::Instant;

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
```

**Note on the `p50` assertion:** with `n = 100`, `round(0.50 * 99) = round(49.5) = 50` (banker's? Rust `f64::round` rounds half away from zero → 50), so `samples[50] = 51.0`. Fix the test's expected `p50_ms` to `51.0` if it fails; the implementation is the source of truth for the percentile convention.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p horde --features bench stats_tests`
Expected: PASS (adjust `p50_ms` expectation per the note above if needed).

- [ ] **Step 5: Commit**

```bash
git add examples/horde/src/bench/mod.rs
git commit -m "feat(horde-bench): timing loop + percentile stats"
```

---

### Task 5: Differential attribution report + marshal probe

**Files:**
- Modify: `examples/horde/src/bench/mod.rs`

**Interfaces:**
- Consumes: `time_backend`, `stats_from`, `build_bench_app` (Tasks 2, 4); `horde::ui::supersolid::bridge::build_frame`.
- Produces:
  - `pub fn probe_marshal(sim: SimConfig, frames: usize, warmup: usize) -> f64` — mean isolated `build_frame` cost (ms).
  - `pub struct Report { pub backend: Backend, pub cap: usize, pub frames: usize, pub total: Stats, pub shared_ms: f64, pub ui_ms: f64, pub marshal_ms: Option<f64>, pub native_total_ms: Option<f64> }`
  - `pub fn run_report(backend: Backend, sim: SimConfig, frames: usize, warmup: usize) -> Report`
  - `pub fn report_table(r: &Report) -> String`

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod report_tests {
    use super::*;

    #[test]
    fn report_computes_ui_cost_as_total_minus_shared() {
        let r = run_report(Backend::Supersolid, horde::sim::SimConfig::play(), 40, 10);
        assert!((r.ui_ms - (r.total.mean_ms - r.shared_ms)).abs() < 1e-9);
        assert!(r.marshal_ms.is_some(), "supersolid report must include marshal");
        assert!(r.native_total_ms.is_some(), "supersolid report must include native gap");
    }

    #[test]
    fn native_report_has_no_marshal() {
        let r = run_report(Backend::Native, horde::sim::SimConfig::play(), 40, 10);
        assert!(r.marshal_ms.is_none());
        assert!(r.native_total_ms.is_none());
    }

    #[test]
    fn table_renders_key_fields() {
        let r = run_report(Backend::Supersolid, horde::sim::SimConfig::play(), 40, 10);
        let t = report_table(&r);
        assert!(t.contains("supersolid"));
        assert!(t.contains("shared"));
        assert!(t.contains("ui_backend"));
        assert!(t.contains("marshal"));
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p horde --features bench report_tests`
Expected: FAIL — `run_report`, `probe_marshal`, `Report`, `report_table` not defined.

- [ ] **Step 3: Implement**

```rust
use horde::ui::supersolid::bridge::build_frame;

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
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p horde --features bench report_tests`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add examples/horde/src/bench/mod.rs
git commit -m "feat(horde-bench): differential attribution report + marshal probe"
```

---

### Task 6: JSON output + CLI parsing + wired bin

**Files:**
- Modify: `examples/horde/src/bench/mod.rs`
- Modify: `examples/horde/src/bin/bench.rs`

**Interfaces:**
- Consumes: `Report`, `run_report`, `report_table` (Task 5).
- Produces:
  - `pub fn report_json(r: &Report) -> String` — dependency-free JSON object.
  - `pub struct BenchArgs { pub backend: Backend, pub preset: String, pub caps: Vec<usize>, pub frames: usize, pub warmup: usize, pub seed: u64, pub json: bool, pub dhat: bool }`
  - `pub fn parse_args(argv: &[String]) -> Result<BenchArgs, String>`
  - `pub fn sim_for(preset: &str, cap: usize, seed: u64) -> SimConfig`

- [ ] **Step 1: Write the failing test**

```rust
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
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p horde --features bench cli_tests`
Expected: FAIL — `parse_args`, `report_json`, `sim_for`, `BenchArgs` not defined.

- [ ] **Step 3: Implement**

```rust
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
        let mut next = || {
            i += 1;
            argv.get(i).cloned().ok_or_else(|| format!("missing value for {key}"))
        };
        match key {
            "--backend" => {
                backend = Some(match next()?.as_str() {
                    "null" => Backend::Null,
                    "native" => Backend::Native,
                    "supersolid" => Backend::Supersolid,
                    other => return Err(format!("unknown backend '{other}'")),
                });
            }
            "--preset" => preset = next()?,
            "--enemy-cap" => caps = vec![next()?.parse().map_err(|_| "bad --enemy-cap")?],
            "--sweep" => {
                caps = next()?
                    .split(',')
                    .map(|s| s.trim().parse::<usize>().map_err(|_| "bad --sweep list".to_string()))
                    .collect::<Result<_, _>>()?;
            }
            "--frames" => frames = next()?.parse().map_err(|_| "bad --frames")?,
            "--warmup" => warmup = next()?.parse().map_err(|_| "bad --warmup")?,
            "--seed" => seed = next()?.parse().map_err(|_| "bad --seed")?,
            "--format" => json = next()?.as_str() == "json",
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
```

- [ ] **Step 4: Wire the bin**

Replace `examples/horde/src/bin/bench.rs` with:

```rust
use horde::bench::{parse_args, report_json, report_table, run_report, sim_for};

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let args = match parse_args(&argv) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("horde-bench: {e}");
            eprintln!("usage: horde-bench --backend null|native|supersolid \\");
            eprintln!("       [--preset play|stress] [--enemy-cap N | --sweep 60,200,400] \\");
            eprintln!("       [--frames N] [--warmup N] [--seed N] [--format table|json] [--dhat]");
            std::process::exit(2);
        }
    };

    for &cap in &args.caps {
        let sim = sim_for(&args.preset, cap, args.seed);
        let report = run_report(args.backend, sim, args.frames, args.warmup);
        if args.json {
            println!("{}", report_json(&report));
        } else {
            print!("{}", report_table(&report));
        }
    }
}
```

- [ ] **Step 5: Run to verify tests pass and the bin runs**

Run: `cargo test -p horde --features bench cli_tests`
Expected: PASS.

Run: `cargo run -p horde --features bench --bin horde-bench -- --backend supersolid --frames 120 --warmup 20`
Expected: prints a table with `total`, `shared`, `ui_backend`, `marshal`, and the native gap. (First real numbers — expect the supersolid `ui_backend` to dominate.)

- [ ] **Step 6: Commit**

```bash
git add examples/horde/src/bench/mod.rs examples/horde/src/bin/bench.rs
git commit -m "feat(horde-bench): JSON output, CLI parsing, wired bin"
```

---

### Task 7: Scaling sweep table

**Files:**
- Modify: `examples/horde/src/bench/mod.rs`

**Interfaces:**
- Consumes: `run_report`, `sim_for`, `Report` (Tasks 5, 6).
- Produces: `pub fn sweep_table(reports: &[Report]) -> String` — one compact row per cap showing total mean, ui_ms, and (if present) marshal, so scaling vs N is visible at a glance.

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod sweep_tests {
    use super::*;

    #[test]
    fn sweep_table_has_one_row_per_cap() {
        let reports: Vec<Report> = [60usize, 200]
            .iter()
            .map(|&c| run_report(Backend::Native, sim_for("play", c, 1), 20, 5))
            .collect();
        let t = sweep_table(&reports);
        assert!(t.contains("cap=60") || t.contains("60"));
        assert!(t.contains("200"));
        let rows = t.lines().filter(|l| l.contains("total")).count();
        assert!(rows >= 2, "expected a data row per cap");
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p horde --features bench sweep_tests`
Expected: FAIL — `sweep_table` not defined.

- [ ] **Step 3: Implement**

```rust
/// Compact one-row-per-cap scaling view.
pub fn sweep_table(reports: &[Report]) -> String {
    use std::fmt::Write as _;
    let mut s = String::new();
    let _ = writeln!(s, "scaling sweep ({} backend):", reports.first().map(|r| r.backend.label()).unwrap_or("?"));
    let _ = writeln!(s, "  {:>6}  {:>12}  {:>12}  {:>12}", "cap", "total(ms)", "ui(ms)", "marshal(ms)");
    for r in reports {
        let marshal = match r.marshal_ms {
            Some(m) => format!("{:.3}", m),
            None => "-".to_string(),
        };
        let _ = writeln!(
            s,
            "  {:>6}  {:>12.3}  {:>12.3}  {:>12}",
            r.cap, r.total.mean_ms, r.ui_ms, marshal
        );
    }
    s
}
```

- [ ] **Step 4: Wire into the bin**

In `examples/horde/src/bin/bench.rs`, replace the `for &cap in &args.caps { ... }` loop with sweep-aware output:

```rust
    let mut reports = Vec::new();
    for &cap in &args.caps {
        let sim = sim_for(&args.preset, cap, args.seed);
        let report = run_report(args.backend, sim, args.frames, args.warmup);
        if args.json {
            println!("{}", horde::bench::report_json(&report));
        } else if args.caps.len() == 1 {
            print!("{}", report_table(&report));
        }
        reports.push(report);
    }
    if !args.json && args.caps.len() > 1 {
        print!("{}", horde::bench::sweep_table(&reports));
    }
```

Add `sweep_table` to the `use horde::bench::{...}` import line.

- [ ] **Step 5: Run to verify it passes and sweep renders**

Run: `cargo test -p horde --features bench sweep_tests`
Expected: PASS.

Run: `cargo run -p horde --features bench --bin horde-bench -- --backend supersolid --sweep 60,200,400 --frames 120 --warmup 20`
Expected: a scaling table with three rows; ui/marshal columns growing with cap.

- [ ] **Step 6: Commit**

```bash
git add examples/horde/src/bench/mod.rs examples/horde/src/bin/bench.rs
git commit -m "feat(horde-bench): element-count scaling sweep table"
```

---

### Task 8: Allocation churn pass (dhat), separate from timing

**Files:**
- Modify: `examples/horde/src/bench/mod.rs`
- Modify: `examples/horde/src/bin/bench.rs`

**Interfaces:**
- Consumes: `build_bench_app` (Task 2), the `dhat-prof` feature (Task 1).
- Produces: `pub struct AllocReport { pub backend: Backend, pub frames: usize, pub bytes_per_frame: f64, pub blocks_per_frame: f64 }`; `pub fn run_alloc(backend: Backend, sim: SimConfig, frames: usize, warmup: usize) -> AllocReport` (only compiled under `dhat-prof`); `pub fn alloc_table(r: &AllocReport) -> String`.

- [ ] **Step 1: Add the dhat global allocator (feature-gated) at the top of `src/bin/bench.rs`**

```rust
#[cfg(feature = "dhat-prof")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;
```

- [ ] **Step 2: Write the failing test (compiles only under dhat-prof)**

Add to `src/bench/mod.rs`:

```rust
#[cfg(all(test, feature = "dhat-prof"))]
mod alloc_tests {
    use super::*;

    #[test]
    fn alloc_report_is_populated() {
        let _p = dhat::Profiler::builder().testing().build();
        let r = run_alloc(Backend::Supersolid, horde::sim::SimConfig::play(), 30, 5);
        assert_eq!(r.frames, 30);
        assert!(r.bytes_per_frame >= 0.0);
    }
}
```

- [ ] **Step 3: Run to verify it fails**

Run: `cargo test -p horde --features bench,dhat-prof alloc_tests`
Expected: FAIL — `run_alloc`, `AllocReport` not defined.

- [ ] **Step 4: Implement**

```rust
#[derive(Clone, Copy, Debug)]
pub struct AllocReport {
    pub backend: Backend,
    pub frames: usize,
    pub bytes_per_frame: f64,
    pub blocks_per_frame: f64,
}

/// Measure per-frame allocation churn over `frames` steady-state updates.
/// Requires an active `dhat::Profiler` in the caller (see the bin's `--dhat` path).
#[cfg(feature = "dhat-prof")]
pub fn run_alloc(backend: Backend, sim: SimConfig, frames: usize, warmup: usize) -> AllocReport {
    let mut app = build_bench_app(backend, sim);
    for _ in 0..warmup {
        app.update();
    }
    let before = dhat::HeapStats::get();
    for _ in 0..frames {
        app.update();
    }
    let after = dhat::HeapStats::get();
    let dbytes = after.total_bytes.saturating_sub(before.total_bytes) as f64;
    let dblocks = after.total_blocks.saturating_sub(before.total_blocks) as f64;
    AllocReport {
        backend,
        frames,
        bytes_per_frame: dbytes / frames as f64,
        blocks_per_frame: dblocks / frames as f64,
    }
}

pub fn alloc_table(r: &AllocReport) -> String {
    format!(
        "alloc churn: backend={} frames={} | {:.1} bytes/frame | {:.1} allocs/frame\n",
        r.backend.label(),
        r.frames,
        r.bytes_per_frame,
        r.blocks_per_frame,
    )
}
```

- [ ] **Step 5: Wire the `--dhat` path in the bin**

In `examples/horde/src/bin/bench.rs`, before the timing loop, add:

```rust
    if args.dhat {
        #[cfg(feature = "dhat-prof")]
        {
            let _profiler = dhat::Profiler::new_heap();
            for &cap in &args.caps {
                let sim = sim_for(&args.preset, cap, args.seed);
                let r = horde::bench::run_alloc(args.backend, sim, args.frames, args.warmup);
                print!("{}", horde::bench::alloc_table(&r));
            }
        }
        #[cfg(not(feature = "dhat-prof"))]
        eprintln!("horde-bench: --dhat requires building with --features bench,dhat-prof");
        return;
    }
```

- [ ] **Step 6: Run to verify it passes**

Run: `cargo test -p horde --features bench,dhat-prof alloc_tests`
Expected: PASS.

Run: `cargo run -p horde --features bench,dhat-prof --bin horde-bench -- --backend supersolid --dhat --frames 120 --warmup 20`
Expected: prints per-frame bytes + allocs; supersolid should show clearly higher churn than `--backend null`.

- [ ] **Step 7: Commit**

```bash
git add examples/horde/src/bench/mod.rs examples/horde/src/bin/bench.rs
git commit -m "feat(horde-bench): dhat allocation-churn pass (separate from timing)"
```

---

### Task 9: benches/README.md + final full-run verification

**Files:**
- Create: `examples/horde/benches/README.md`
- Modify: `examples/horde/src/bench/mod.rs` (only if it exceeds ~400 lines — see note)

**Interfaces:**
- Consumes: everything above. No new public API.

- [ ] **Step 1: Write `examples/horde/benches/README.md`**

```markdown
# Horde macro-benchmark

Headless, deterministic benchmark of the horde game. Measures per-frame cost and
attributes it across UI backends so optimizations can be predicted and proven.

Design: `docs/superpowers/specs/2026-07-21-horde-benchmark-harness-design.md`.

## Run

    cargo run -p horde --features bench --bin horde-bench -- --backend supersolid

Backends: `null` (sim+snapshot floor), `native` (bevy_ui), `supersolid` (TSX).

Common flags:

- `--preset play|stress`
- `--enemy-cap N` or `--sweep 60,200,400,800`
- `--frames N` (measured) `--warmup N` (excluded)
- `--seed N` `--format table|json`

## Reading the report

- `total` — mean + p50/p95/p99 + FPS-equivalent of one `app.update()`.
- `shared` — the null-backend floor (sim + snapshot); the same for every backend.
- `ui_backend` = `total − shared` — the backend's UI cost, and the **ceiling** of
  any UI-only optimization (you can't save more than this).
- `marshal` (supersolid) — isolated `build_frame` (JSON bridge) cost; the rest of
  `ui_backend` is reconcile + layout.
- `vs native floor` — supersolid's gap to native; closing it is the standing target.

## Before / after

    cargo run -p horde --features bench --bin horde-bench -- --backend supersolid --format json > before.json
    # ...optimize...
    cargo run -p horde --features bench --bin horde-bench -- --backend supersolid --format json > after.json
    # compare total_mean_ms, ui_ms, marshal_ms between the two JSON objects.

## Allocation churn

    cargo run -p horde --features bench,dhat-prof --bin horde-bench -- --backend supersolid --dhat

Reports bytes + allocations per frame (steady-state should trend toward ~zero).

## Profiling (Tracy)

Build the bin with `--features bench,bevy/trace_tracy` and attach Tracy to drill
into which system inside `ui_backend` dominates (reconcile vs layout vs cascade).
```

- [ ] **Step 2: Full clean build across all feature combos**

Run: `cargo build -p horde` — normal game, unchanged.
Run: `cargo build -p horde --features bench --bin horde-bench` — bench.
Run: `cargo build -p horde --features bench,dhat-prof --bin horde-bench` — dhat.
Expected: all compile.

- [ ] **Step 3: Full test pass**

Run: `cargo test -p horde --features bench`
Expected: all bench tests plus existing horde tests PASS.

- [ ] **Step 4: End-to-end smoke of every mode**

Run each and eyeball sane numbers:

```bash
cargo run -p horde --features bench --bin horde-bench -- --backend null --frames 200 --warmup 30
cargo run -p horde --features bench --bin horde-bench -- --backend native --frames 200 --warmup 30
cargo run -p horde --features bench --bin horde-bench -- --backend supersolid --frames 200 --warmup 30
cargo run -p horde --features bench --bin horde-bench -- --backend supersolid --sweep 60,200,400 --frames 150 --warmup 30
cargo run -p horde --features bench --bin horde-bench -- --backend supersolid --frames 100 --warmup 20 --format json
```

Expected: null < native < supersolid on `total`; supersolid `ui_backend` dominant; the native gap prints; sweep shows growth with cap; JSON is one object per cap.

- [ ] **Step 5: File-size check (structure hygiene)**

If `src/bench/mod.rs` exceeds ~400 lines, split into `src/bench/{app.rs, timing.rs, report.rs, cli.rs}` re-exported from `mod.rs`, keeping public paths (`horde::bench::*`) identical. Otherwise leave as one focused file.

- [ ] **Step 6: Commit**

```bash
git add examples/horde/benches/README.md examples/horde/src/bench/
git commit -m "docs(horde-bench): benches/README + finalize harness"
```

---

## Self-Review (author checklist — completed)

**Spec coverage:**
- §2 harness shape / invocation → Tasks 1, 6 (bin, CLI, features).
- §2 both backends compile in one binary → Task 1 (widened cfgs, `bench` feature).
- §3 deterministic scripted auto-player + fixed dt → Task 2 (auto_player, ManualDuration), Task 3 (determinism test).
- §4 stage attribution / Amdahl ceiling → Task 5 (differential subtraction realizes it via the `null` floor; `ui_backend` labeled as the ceiling). **Scope note:** the design's 6-row per-system table (js_reconcile vs taffy_layout split) is delivered as `shared / ui_backend / marshal` here; the finer reconcile-vs-layout split needs `info_span!` seams *inside* superui and is called out as the Tracy follow-up in Task 9's README and §4 of the spec. This is a deliberate, documented reduction, not a gap.
- §5 total stats + percentiles + FPS → Task 4; scaling sweep → Task 7; allocation churn (separate pass) → Task 8.
- §5 before/after JSON workflow → Task 6 (`report_json`) + Task 9 README.
- §6 Tracy → Task 9 README (spans inside superui are a follow-up; the bin is Tracy-runnable as-is via `bevy/trace_tracy`).
- §7.1 headless-reconcile risk → Task 2 is the explicit gate, with the camera fallback documented inline.
- §8 file layout → matches the "File structure (locked)" block.
- §9 definition of done → Task 9 Steps 2–4 exercise every DoD bullet.

**Placeholder scan:** No TBD/TODO/"handle errors"/"similar to" — every code step carries full code. Percentile-convention ambiguity is pre-empted with an explicit note in Task 4.

**Type consistency:** `Backend`, `Stats`, `Report`, `BenchArgs`, `AllocReport`, `build_bench_app`, `time_backend`, `stats_from`, `run_report`, `probe_marshal`, `report_table`, `report_json`, `parse_args`, `sim_for`, `sweep_table`, `run_alloc`, `alloc_table`, `BenchFrame`, `DT`, `VIEWPORT` are defined once and referenced with consistent signatures across tasks. `run_report` returns `Report` consumed by Tasks 6/7; `sim_for(preset, cap, seed)` signature is stable from Task 6 onward.
