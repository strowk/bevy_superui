//! Per-stage profiling of the supersolid frame (the `--profile` bench mode).
//!
//! The bench macro-benchmark tells us supersolid's cost is ~all in `ui_backend`,
//! but that is one opaque bucket. This splits it. Bevy instruments every system
//! run with a root `info_span!("system", name = <path>)` when the `trace` feature
//! is on, and each of the five reconcile stages the design cares about lives in a
//! *different* system:
//!
//! | stage                         | system(s)                                   |
//! |-------------------------------|---------------------------------------------|
//! | marshal (Rust→JS JSON)        | `push_ui_frame` / `forward_event_observer`  |
//! | Boa render (reactive re-run)  | `emit_bevy_inbox_system`                    |
//! | DOM diff + bevy_ui apply      | `reconcile_system`                          |
//! | flair cascade / selectors     | `bevy_flair_style::systems::*`              |
//! | taffy layout                  | `bevy_ui::layout::ui_layout_system`         |
//!
//! So a tracing layer that sums busy-time per system name, keyed and bucketed,
//! attributes the whole frame with no edits to the library crates. The same spans
//! feed a Tracy flamegraph under `--features bench bevy/trace_tracy`; this mode is
//! the headless equivalent that prints the "X% cascade, Y% Boa …" one-liner.
//!
//! Build/run:
//! ```text
//! cargo run --release -p citadel --features bench,bevy/trace --bin citadel-bench -- \
//!     --profile --frames 120 --warmup 200
//! ```
//! Without `bevy/trace` the system spans do not exist and the report is empty; the
//! harness detects that and says so.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use bevy::log::tracing;
use bevy::log::tracing_subscriber;

use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id};
use tracing::subscriber::Interest;
use tracing::{Metadata, Subscriber};
use tracing_subscriber::layer::{Context, Layer, SubscriberExt};
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::Registry;

use crate::bench::{build_bench_app, Backend};
use crate::sim::CitadelConfig;

/// Accumulated busy time (ns) and hit count, keyed by system name.
#[derive(Default)]
struct Agg {
    /// Only accumulate while true (skips warmup).
    recording: bool,
    per_system: HashMap<String, (u128, u64)>,
}

/// Marker + per-span state stored in the tracing registry span extensions.
struct SysKey(String);
struct Enter(Instant);

/// Pulls the `name` field value (the system's full path) out of a span's fields,
/// handling both the `record_str` and `record_debug` code paths.
struct NameVisitor(Option<String>);
impl Visit for NameVisitor {
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "name" {
            self.0 = Some(value.to_string());
        }
    }
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "name" && self.0.is_none() {
            // Debug of a string is quoted; strip the surrounding quotes.
            let s = format!("{value:?}");
            self.0 = Some(s.trim_matches('"').to_string());
        }
    }
}

/// A tracing layer that only cares about Bevy's per-system spans and sums their
/// wall-clock busy time. These spans are `parent: None` roots, so they never nest
/// in each other — busy times are disjoint and safe to sum/compare.
#[derive(Clone)]
struct SystemTimingLayer {
    agg: Arc<Mutex<Agg>>,
}

fn is_system_span(meta: &Metadata<'_>) -> bool {
    matches!(meta.name(), "system" | "system_commands")
}

impl<S> Layer<S> for SystemTimingLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn register_callsite(&self, meta: &Metadata<'_>) -> Interest {
        // Ignore every other callsite (executor spans, events, flair's internal
        // trace spans) so they are never even constructed — keeps overhead low and
        // keeps the busy time equal to the full system body.
        if is_system_span(meta) {
            Interest::always()
        } else {
            Interest::never()
        }
    }

    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        if !is_system_span(attrs.metadata()) {
            return;
        }
        let mut v = NameVisitor(None);
        attrs.record(&mut v);
        let raw = v.0.unwrap_or_else(|| "<unknown>".to_string());
        let key = if attrs.metadata().name() == "system_commands" {
            format!("[commands] {raw}")
        } else {
            raw
        };
        if let Some(span) = ctx.span(id) {
            span.extensions_mut().insert(SysKey(key));
        }
    }

    fn on_enter(&self, id: &Id, ctx: Context<'_, S>) {
        // Bevy creates each system span once and RE-ENTERS it every frame, so the
        // timing stamp must be replaced (extensions panic on duplicate insert).
        if let Some(span) = ctx.span(id) {
            let mut ext = span.extensions_mut();
            if ext.get_mut::<SysKey>().is_some() {
                ext.remove::<Enter>();
                ext.insert(Enter(Instant::now()));
            }
        }
    }

    fn on_exit(&self, id: &Id, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(id) else { return };
        let mut ext = span.extensions_mut();
        let Some(enter) = ext.remove::<Enter>() else { return };
        let Some(key) = ext.get_mut::<SysKey>() else { return };
        let elapsed = enter.0.elapsed().as_nanos();
        let mut agg = self.agg.lock().unwrap();
        if agg.recording {
            let e = agg.per_system.entry(key.0.clone()).or_insert((0, 0));
            e.0 += elapsed;
            e.1 += 1;
        }
    }
}

/// The five stages the profiling splits the frame into, plus catch-alls.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Bucket {
    Marshal,
    BoaRender,
    Reconcile,
    Flair,
    Taffy,
    UiOther,
    Other,
}

impl Bucket {
    fn label(self) -> &'static str {
        match self {
            Bucket::Marshal => "marshal (Rust->JS JSON bridge)",
            Bucket::BoaRender => "Boa render (JS reactive re-run)",
            Bucket::Reconcile => "reconcile (DOM diff + bevy_ui apply)",
            Bucket::Flair => "flair cascade / selector matching",
            Bucket::Taffy => "taffy layout",
            Bucket::UiOther => "bevy_ui other (text/stack/prep)",
            Bucket::Other => "other (sim/input/picking/etc.)",
        }
    }
    /// Report order.
    fn all() -> [Bucket; 7] {
        [
            Bucket::BoaRender,
            Bucket::Reconcile,
            Bucket::Flair,
            Bucket::Taffy,
            Bucket::Marshal,
            Bucket::UiOther,
            Bucket::Other,
        ]
    }
}

/// Schedule-runner systems (`run_main`, `run_fixed_main*`) are themselves spans of
/// metadata name "system", but they WRAP every other system in the frame, so their
/// busy time double-counts the whole frame. Exclude them from the leaf attribution.
fn is_wrapper(name: &str) -> bool {
    name.contains("run_main") || name.contains("run_fixed_main")
}

/// Map a system path to its stage bucket.
fn bucket_for(name: &str) -> Bucket {
    let n = name;
    if n.contains("emit_bevy_inbox") {
        Bucket::BoaRender
    } else if n.contains("reconcile") {
        Bucket::Reconcile
    } else if n.contains("flair") {
        Bucket::Flair
    } else if n.contains("ui_layout_system") {
        Bucket::Taffy
    } else if n.contains("push_ui_frame")
        || n.contains("forward_event_observer")
        || n.contains("forward_toggle")
        || n.contains("drain_bevy_outbox")
        || n.contains("drain_dom_events")
        || n.contains("keyboard_events")
        || n.contains("emit_bevy_inbox") // (already handled; keep explicit)
    {
        Bucket::Marshal
    } else if n.contains("bevy_ui") || n.contains("bevy_text") || n.contains("ui_stack") {
        Bucket::UiOther
    } else {
        Bucket::Other
    }
}

/// Install the timing layer as the global subscriber (idempotent). Returns the
/// shared accumulator so the caller can gate recording and read results.
fn install() -> Arc<Mutex<Agg>> {
    let agg = Arc::new(Mutex::new(Agg::default()));
    let layer = SystemTimingLayer { agg: agg.clone() };
    let subscriber = Registry::default().with(layer);
    // If a subscriber is already set (e.g. two profile runs in one process) this is
    // a no-op; we keep the first accumulator wired regardless.
    let _ = tracing::subscriber::set_global_default(subscriber);
    agg
}

/// Run a profiled supersolid session and print the per-stage attribution.
/// Citadel is steady-state (no player, no GameState, no death) — no god-mode needed.
pub fn run_profile(cfg: CitadelConfig, frames: usize, warmup: usize) {
    let agg = install();

    let mut app = build_bench_app(Backend::Supersolid, cfg);

    // Warm up with recording off (mount, initial tick steady state).
    for _ in 0..warmup {
        app.update();
    }

    // Reset and record the measured window; wall-time the whole window too.
    {
        let mut a = agg.lock().unwrap();
        a.per_system.clear();
        a.recording = true;
    }
    let wall = Instant::now();
    for _ in 0..frames {
        app.update();
    }
    let wall_total_ms = wall.elapsed().as_secs_f64() * 1000.0;
    {
        agg.lock().unwrap().recording = false;
    }

    print_report(&agg, frames, wall_total_ms);
}

fn print_report(agg: &Arc<Mutex<Agg>>, frames: usize, wall_total_ms: f64) {
    use std::fmt::Write as _;
    let a = agg.lock().unwrap();

    if a.per_system.is_empty() {
        println!(
            "profile: no system spans were recorded.\n\
             This mode needs Bevy's per-system instrumentation. Rebuild with:\n\
             \n    cargo run --release -p citadel --features bench,bevy/trace --bin citadel-bench -- --profile ...\n"
        );
        return;
    }

    let n = frames.max(1) as f64;
    let frame_ms = wall_total_ms / n;

    // Per-system rows, sorted by total time descending. Drop schedule-runner
    // wrappers so leaf systems are (approximately) disjoint and sum to the frame.
    let mut rows: Vec<(&String, f64, u64)> = a
        .per_system
        .iter()
        .filter(|(k, _)| !is_wrapper(k))
        .map(|(k, (ns, cnt))| (k, *ns as f64 / 1e6, *cnt))
        .collect();
    rows.sort_by(|x, y| y.1.partial_cmp(&x.1).unwrap());

    // Bucket rollup.
    let mut buckets: HashMap<Bucket, f64> = HashMap::new();
    let mut instrumented_ms = 0.0;
    for (name, ms, _cnt) in &rows {
        *buckets.entry(bucket_for(name)).or_insert(0.0) += *ms;
        instrumented_ms += *ms;
    }
    let instrumented_frame_ms = instrumented_ms / n;

    let mut out = String::new();
    let _ = writeln!(out, "\n=== supersolid frame profile (bevy/trace system spans) ===");
    let _ = writeln!(
        out,
        "frames={frames}  mean frame (instrumented) = {frame_ms:.3} ms  |  captured-in-systems = {instrumented_frame_ms:.3} ms/frame"
    );
    let _ = writeln!(out, "\n-- per stage (mean ms/frame, % of frame) --");

    let pct = |ms: f64| if frame_ms > 0.0 { 100.0 * ms / frame_ms } else { 0.0 };
    for b in Bucket::all() {
        let total = buckets.get(&b).copied().unwrap_or(0.0);
        let per_frame = total / n;
        if per_frame < 1e-4 {
            continue;
        }
        let _ = writeln!(
            out,
            "  {:<38} {:>9.3} ms  {:>5.1}%",
            b.label(),
            per_frame,
            pct(per_frame)
        );
    }

    let _ = writeln!(out, "\n-- top systems (mean ms/frame, calls/frame) --");
    for (name, ms, cnt) in rows.iter().take(18) {
        let per_frame = ms / n;
        if per_frame < 1e-4 {
            continue;
        }
        let short = name.rsplit("::").next().unwrap_or(name);
        let _ = writeln!(
            out,
            "  {:<44} {:>9.3} ms  {:>5.1}%   ({:.1}/f)",
            short,
            per_frame,
            pct(per_frame),
            *cnt as f64 / n
        );
    }

    // The requested one-liner.
    let one = |b: Bucket| pct(buckets.get(&b).copied().unwrap_or(0.0) / n);
    let _ = writeln!(
        out,
        "\nof the {:.0} ms frame: {:.0}% Boa render, {:.0}% reconcile, {:.0}% flair cascade, {:.0}% taffy, {:.0}% marshal.",
        frame_ms,
        one(Bucket::BoaRender),
        one(Bucket::Reconcile),
        one(Bucket::Flair),
        one(Bucket::Taffy),
        one(Bucket::Marshal),
    );

    print!("{out}");
}
