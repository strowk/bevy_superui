//! Headless macro-benchmark harness for the citadel example.
//! Mirrors examples/horde/src/bench/mod.rs, dropping the Native backend.

pub mod profile;

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

use crate::sim::{CitadelConfig, UiSnapshot};
use crate::ui::supersolid::bridge::build_frame;

// ── Asset constants ───────────────────────────────────────────────────────────

const HTML: &str = include_str!("../../assets/ui/citadel/index.html");
const CSS: &str = include_str!("../../assets/ui/citadel/theme.css");
const TSX: &str = include_str!("../../assets/ui/citadel/app.tsx");
const JS: &str = include_str!("../../assets/ui/citadel/app.generated.js");

// ── Backend enum ──────────────────────────────────────────────────────────────

/// Which UI backend the bench app assembles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backend {
    /// sim + snapshot only — the shared floor.
    Null,
    /// supersolid TSX UI.
    Supersolid,
}

impl Backend {
    pub fn label(self) -> &'static str {
        match self {
            Backend::Null => "null",
            Backend::Supersolid => "supersolid",
        }
    }
}

// ── Fixed time step ───────────────────────────────────────────────────────────

/// Fixed per-update time step: exactly one FixedUpdate tick per app.update().
pub const DT: f64 = 1.0 / 60.0;

// ── Memory asset source ───────────────────────────────────────────────────────

fn memory_asset_dir() -> Dir {
    let dir = Dir::new("assets".into());
    dir.insert_asset("ui/citadel/index.html".as_ref(), HTML.as_bytes());
    dir.insert_asset("ui/citadel/theme.css".as_ref(), CSS.as_bytes());
    dir.insert_asset("ui/citadel/app.tsx".as_ref(), TSX.as_bytes());
    dir.insert_asset("ui/citadel/app.generated.js".as_ref(), JS.as_bytes());
    dir
}

// ── App builder ───────────────────────────────────────────────────────────────

/// Build a finished, headless, deterministic bench app for `backend`.
pub fn build_bench_app(backend: Backend, cfg: CitadelConfig) -> App {
    let mut app = App::new();

    // Memory asset source so supersolid loads the real authored assets headlessly.
    let dir = memory_asset_dir();
    app.register_asset_source(
        AssetSourceId::Default,
        AssetSource::build().with_reader(move || Box::new(MemoryAssetReader { root: dir.clone() })),
    );

    // Identical base plugin recipe for every backend.
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

    // Insert CitadelConfig before SimPlugin so it picks up our config.
    app.insert_resource(cfg);

    // SimPlugin initialises Economy + UiSnapshot and runs advance_sim + assemble_snapshot.
    app.add_plugins(crate::sim::SimPlugin);

    // Chosen UI backend (Null adds nothing).
    match backend {
        Backend::Null => {}
        Backend::Supersolid => {
            app.add_plugins(crate::ui::supersolid::SupersolidUiPlugin);
        }
    }

    app.finish();
    app
}

// ── Stats ─────────────────────────────────────────────────────────────────────

/// Per-frame timing statistics from a benchmark run.
#[derive(Clone, Copy, Debug)]
pub struct Stats {
    pub mean_ms: f64,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub fps: f64,
}

/// Nearest-rank percentile over sorted samples.
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

// ── Timing ────────────────────────────────────────────────────────────────────

/// Drive `frames` measured updates (after `warmup`), returning per-frame ms.
pub fn time_backend(backend: Backend, cfg: CitadelConfig, frames: usize, warmup: usize) -> Vec<f64> {
    let mut app = build_bench_app(backend, cfg);
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

/// Mean isolated cost of building the FrameDto (the JSON marshal) in ms.
pub fn probe_marshal(cfg: CitadelConfig, frames: usize, warmup: usize) -> f64 {
    let mut app = build_bench_app(Backend::Supersolid, cfg);
    for _ in 0..warmup {
        app.update();
    }
    let mut total = 0.0;
    for _ in 0..frames {
        app.update();
        let w = app.world();
        let snap = w.resource::<UiSnapshot>();
        let t = Instant::now();
        let dto = build_frame(snap);
        total += t.elapsed().as_secs_f64() * 1000.0;
        std::hint::black_box(&dto);
    }
    total / frames as f64
}

// ── Report ────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct Report {
    pub backend: Backend,
    /// building_count of the CitadelConfig used.
    pub cap: usize,
    pub frames: usize,
    pub total: Stats,
    /// Shared floor: mean total of the null backend (sim + snapshot).
    pub shared_ms: f64,
    /// UI-backend cost = total.mean − shared.
    pub ui_ms: f64,
    /// Supersolid only: isolated marshal cost (subset of ui_ms).
    pub marshal_ms: Option<f64>,
}

/// Run the full differential: chosen backend + null floor (+ marshal for supersolid).
pub fn run_report(backend: Backend, cfg: CitadelConfig, frames: usize, warmup: usize) -> Report {
    let total = stats_from(time_backend(backend, cfg.clone(), frames, warmup));
    let shared = stats_from(time_backend(Backend::Null, cfg.clone(), frames, warmup)).mean_ms;

    let marshal_ms = if backend == Backend::Supersolid {
        Some(probe_marshal(cfg.clone(), frames, warmup))
    } else {
        None
    };

    Report {
        backend,
        cap: cfg.building_count,
        frames,
        ui_ms: total.mean_ms - shared,
        total,
        shared_ms: shared,
        marshal_ms,
    }
}

// ── Formatting ────────────────────────────────────────────────────────────────

/// Human-readable attribution table.
pub fn report_table(r: &Report) -> String {
    use std::fmt::Write as _;
    let mut s = String::new();
    let _ = writeln!(
        s,
        "backend={} building_count={} frames={}",
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
    let ui_disp = r.ui_ms.max(0.0);
    let _ = writeln!(s, "    shared (sim+snapshot) : {:.3} ms ({:.1}%)", r.shared_ms, pct(r.shared_ms));
    let _ = writeln!(s, "    ui_backend            : {:.3} ms ({:.1}%)  [optimization ceiling]", ui_disp, pct(ui_disp));
    if let Some(m) = r.marshal_ms {
        let _ = writeln!(s, "      of which marshal    : {:.3} ms ({:.1}%)", m, pct(m));
        let reconcile = (ui_disp - m).max(0.0);
        let _ = writeln!(s, "      reconcile+layout    : {:.3} ms ({:.1}%)  [finer split = --profile follow-up]", reconcile, pct(reconcile));
    }
    s
}

/// Dependency-free JSON serialization of a report.
pub fn report_json(r: &Report) -> String {
    let opt = |v: Option<f64>| match v {
        Some(x) => format!("{:.6}", x),
        None => "null".to_string(),
    };
    format!(
        "{{\"backend\":\"{}\",\"building_count\":{},\"frames\":{},\
         \"total_mean_ms\":{:.6},\"p50_ms\":{:.6},\"p95_ms\":{:.6},\"p99_ms\":{:.6},\"fps\":{:.3},\
         \"shared_ms\":{:.6},\"ui_ms\":{:.6},\"marshal_ms\":{}}}",
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
    )
}

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

// ── Workload sampler ──────────────────────────────────────────────────────────

/// Live element counts sampled over a run.
#[derive(Clone, Copy, Debug)]
pub struct Workload {
    pub frames: usize,
    pub buildings_avg: f64,
    pub buildings_max: usize,
    pub units_avg: f64,
    pub units_max: usize,
    pub techs_avg: f64,
    pub techs_max: usize,
}

/// Sample live `UiSnapshot` element counts over `frames` measured updates.
pub fn sample_workload(cfg: CitadelConfig, frames: usize, warmup: usize) -> Workload {
    let mut app = build_bench_app(Backend::Null, cfg);
    for _ in 0..warmup {
        app.update();
    }
    let (mut b_sum, mut u_sum, mut t_sum) = (0usize, 0usize, 0usize);
    let (mut b_max, mut u_max, mut t_max) = (0usize, 0usize, 0usize);
    for _ in 0..frames {
        app.update();
        let snap = app.world().resource::<UiSnapshot>();
        let (b, u, t) = (snap.buildings.len(), snap.units.len(), snap.techs.len());
        b_sum += b; u_sum += u; t_sum += t;
        b_max = b_max.max(b);
        u_max = u_max.max(u);
        t_max = t_max.max(t);
    }
    let n = frames.max(1) as f64;
    Workload {
        frames,
        buildings_avg: b_sum as f64 / n,
        buildings_max: b_max,
        units_avg: u_sum as f64 / n,
        units_max: u_max,
        techs_avg: t_sum as f64 / n,
        techs_max: t_max,
    }
}

/// One-line workload summary.
pub fn workload_summary(w: &Workload) -> String {
    format!(
        "buildings avg {:.1} max {} | units avg {:.1} max {} | techs avg {:.1} max {}",
        w.buildings_avg, w.buildings_max, w.units_avg, w.units_max, w.techs_avg, w.techs_max
    )
}

/// Full workload line for a single-cap report.
pub fn workload_line(w: &Workload) -> String {
    format!("  workload: {}\n", workload_summary(w))
}

// ── CitadelConfig helper ──────────────────────────────────────────────────────

/// Build a CitadelConfig for a given building_count and seed.
pub fn sim_for(cap: usize, seed: u64) -> CitadelConfig {
    CitadelConfig {
        building_count: cap,
        seed: if seed != 0 { seed } else { 1 },
        ..CitadelConfig::default()
    }
}

// ── CLI arg parsing ───────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct BenchArgs {
    pub backend: Backend,
    pub caps: Vec<usize>,
    pub frames: usize,
    pub warmup: usize,
    pub seed: u64,
    pub json: bool,
    pub dhat: bool,
    pub profile: bool,
}

/// Minimal `--key value` / `--flag` parser.
pub fn parse_args(argv: &[String]) -> Result<BenchArgs, String> {
    let mut backend: Option<Backend> = None;
    let mut caps: Vec<usize> = Vec::new();
    let mut frames = 1000usize;
    let mut warmup = 100usize;
    let mut seed = 0u64;
    let mut json = false;
    let mut dhat = false;
    let mut profile = false;

    let mut i = 0;
    while i < argv.len() {
        let key = argv[i].as_str();
        let advance = |i: &mut usize| -> Result<&str, String> {
            *i += 1;
            argv.get(*i).map(|s| s.as_str()).ok_or_else(|| format!("missing value for {key}"))
        };
        match key {
            "--backend" => {
                backend = Some(match advance(&mut i)? {
                    "null" => Backend::Null,
                    "supersolid" => Backend::Supersolid,
                    other => return Err(format!("unknown backend '{other}'")),
                });
            }
            // Accept --preset and ignore it (tolerated for script compatibility)
            "--preset" => { advance(&mut i)?; }
            "--building-count" | "--enemy-cap" => {
                let v = advance(&mut i)?.parse().map_err(|_| "bad --building-count".to_string())?;
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
            "--profile" => profile = true,
            other => return Err(format!("unknown arg '{other}'")),
        }
        i += 1;
    }

    // `--profile` only makes sense for supersolid, so it makes --backend optional.
    let backend = match backend {
        Some(b) => b,
        None if profile => Backend::Supersolid,
        None => return Err("--backend is required (null|supersolid)".to_string()),
    };
    if caps.is_empty() {
        caps = vec![CitadelConfig::default().building_count];
    }
    Ok(BenchArgs { backend, caps, frames, warmup, seed, json, dhat, profile })
}

// ── dhat support ─────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug)]
pub struct AllocReport {
    pub backend: Backend,
    pub frames: usize,
    pub bytes_per_frame: f64,
    pub blocks_per_frame: f64,
}

#[cfg(feature = "dhat-prof")]
pub fn run_alloc(backend: Backend, cfg: CitadelConfig, frames: usize, warmup: usize) -> AllocReport {
    let mut app = build_bench_app(backend, cfg);
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

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod report_tests {
    use super::*;

    #[test]
    fn report_computes_ui_cost_as_total_minus_shared() {
        let cfg = sim_for(CitadelConfig::default().building_count, 1);
        let r = run_report(Backend::Supersolid, cfg, 40, 10);
        assert!(
            (r.ui_ms - (r.total.mean_ms - r.shared_ms)).abs() < 1e-9,
            "ui_ms must equal total.mean - shared_ms"
        );
        assert!(r.ui_ms >= 0.0, "ui_ms must be non-negative");
        assert!(r.marshal_ms.is_some(), "supersolid report must include marshal_ms");
    }

    #[test]
    fn reaches_populated_ui() {
        // Build the supersolid app and run 120 updates; count DOM nodes via UiRuntime.
        let cfg = sim_for(CitadelConfig::default().building_count, 1);
        let mut app = build_bench_app(Backend::Supersolid, cfg);
        for _ in 0..120 {
            app.update();
        }
        // Count nodes via the DOM (mirrors horde's count_ui_nodes approach).
        let dom_nodes = app
            .world()
            .get_non_send_resource::<superui_bridge::UiRuntime>()
            .map(|rt| {
                let d = rt.dom.borrow();
                d.query_selector_all(d.document(), "*").len()
            })
            .unwrap_or(0);
        assert!(
            dom_nodes > 300,
            "expected >300 DOM nodes after mount, got {}",
            dom_nodes
        );
    }

    #[test]
    fn json_has_expected_fields() {
        let cfg = sim_for(CitadelConfig::default().building_count, 1);
        let r = run_report(Backend::Supersolid, cfg, 20, 5);
        let j = report_json(&r);
        assert!(j.contains("\"backend\":\"supersolid\""), "missing backend field");
        assert!(j.contains("\"building_count\""), "missing building_count field");
        assert!(j.contains("\"total_mean_ms\""), "missing total_mean_ms field");
        assert!(j.contains("\"shared_ms\""), "missing shared_ms field");
        assert!(j.contains("\"ui_ms\""), "missing ui_ms field");
        assert!(j.contains("\"marshal_ms\""), "missing marshal_ms field");
        assert!(j.contains("\"fps\""), "missing fps field");
    }
}

#[cfg(test)]
mod stats_tests {
    use super::*;

    #[test]
    fn percentiles_on_known_data() {
        let samples: Vec<f64> = (1..=100).map(|n| n as f64).collect();
        let s = stats_from(samples);
        assert!((s.mean_ms - 50.5).abs() < 1e-9);
        assert_eq!(s.p50_ms, 51.0);
        assert_eq!(s.p95_ms, 95.0);
        assert_eq!(s.p99_ms, 99.0);
        assert!((s.fps - 1000.0 / 50.5).abs() < 1e-9);
    }

    #[test]
    fn timing_run_produces_frames() {
        let cfg = sim_for(CitadelConfig::default().building_count, 1);
        let v = time_backend(Backend::Null, cfg, 30, 5);
        assert_eq!(v.len(), 30);
        assert!(v.iter().all(|&ms| ms >= 0.0));
    }
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
            "--backend", "supersolid",
            "--sweep", "60,120", "--frames", "500", "--warmup", "50",
            "--seed", "7", "--format", "json",
        ])).unwrap();
        assert_eq!(a.backend, Backend::Supersolid);
        assert_eq!(a.caps, vec![60, 120]);
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
    fn preset_is_tolerated() {
        // --preset should be accepted and ignored
        let a = parse_args(&args(&["--backend", "null", "--preset", "stress"])).unwrap();
        assert_eq!(a.backend, Backend::Null);
    }
}

#[cfg(test)]
mod sweep_tests {
    use super::*;

    #[test]
    fn sweep_table_has_one_row_per_cap() {
        let reports: Vec<Report> = [60usize, 120]
            .iter()
            .map(|&c| run_report(Backend::Null, sim_for(c, 1), 20, 5))
            .collect();
        let t = sweep_table(&reports);
        assert!(t.contains("60"), "cap 60 row missing");
        assert!(t.contains("120"), "cap 120 row missing");
        let data_rows = t
            .lines()
            .filter(|l| l.split_whitespace().next().map_or(false, |tok| tok.parse::<usize>().is_ok()))
            .count();
        assert_eq!(data_rows, 2, "expected exactly one data row per cap");
    }
}
