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
//! Generic over the example: `run_profile_with` takes the finished `App` as an
//! `impl FnOnce() -> App` and a `rebuild_hint` command string to print when no
//! system spans were recorded (i.e. the caller forgot `bevy/trace`), so this
//! module never needs to know the caller's config type.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use bevy::log::tracing;
use bevy::log::tracing_subscriber;
use bevy::prelude::App;

use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id};
use tracing::subscriber::Interest;
use tracing::{Metadata, Subscriber};
use tracing_subscriber::layer::{Context, Layer, SubscriberExt};
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::Registry;

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
pub enum Bucket {
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
pub fn is_wrapper(name: &str) -> bool {
    name.contains("run_main") || name.contains("run_fixed_main")
}

/// Map a system path to its stage bucket.
pub fn bucket_for(name: &str) -> Bucket {
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

/// The one accumulator that ever gets wired into the global subscriber. `install`
/// must return this SAME `Arc` on every call within a process, not a fresh one —
/// `tracing::subscriber::set_global_default` only ever succeeds once per process,
/// so a second call's freshly-built layer is never actually installed and reading
/// from a fresh `Agg` would silently see zero spans. horde and citadel each call
/// `install` once per process so this was latent; rows calls it once per op (14
/// times in one process) and would otherwise report real numbers for the first op
/// only and "no system spans were recorded" for the following thirteen.
static GLOBAL_AGG: OnceLock<Arc<Mutex<Agg>>> = OnceLock::new();

/// Install the timing layer as the global subscriber (idempotent). Returns the
/// shared accumulator so the caller can gate recording and read results.
fn install() -> Arc<Mutex<Agg>> {
    GLOBAL_AGG
        .get_or_init(|| {
            let agg = Arc::new(Mutex::new(Agg::default()));
            let layer = SystemTimingLayer { agg: agg.clone() };
            let subscriber = Registry::default().with(layer);
            let _ = tracing::subscriber::set_global_default(subscriber);
            agg
        })
        .clone()
}

/// Run a profiled session and print the per-stage attribution.
///
/// Generic over the example: `build` supplies the finished `App`, so this crate
/// never needs the caller's config type. `rebuild_hint` is the exact command to
/// print when no system spans were recorded (i.e. the caller forgot `bevy/trace`).
///
/// This is a thin wrapper over [`run_profile_driven`] with `prepare` a no-op and
/// `measure` the steady-state driver `|app| { app.update(); }` — one iteration is
/// one plain frame, and since nothing needs restoring between frames, every frame
/// is measured. horde and citadel are steady-state benchmarks (the cost under test
/// is "what does a frame cost", not "what does one discrete operation cost"), so
/// their behaviour is unchanged by this indirection.
pub fn run_profile_with(
    build: impl FnOnce() -> App,
    frames: usize,
    warmup: usize,
    rebuild_hint: &str,
) {
    run_profile_driven(build, |_app| {}, |app| app.update(), frames, warmup, rebuild_hint);
}

/// Like [`run_profile_with`], but the caller supplies both halves of an iteration
/// separately: `prepare` (untraced) and `measure` (traced). Only `measure` runs
/// with recording on, so the printed frame cost and the stage percentages both
/// describe the same window — the measured op, not whatever setup `prepare` needed
/// to get there.
///
/// Steady-state benchmarks pass an empty `prepare` and `|app| { app.update(); }`
/// for `measure`. Per-op benchmarks (rows) put the untimed precondition reset in
/// `prepare` and the op itself in `measure`, since their unit of measurement is an
/// operation, not a frame, and the reset must not be attributed to it.
pub fn run_profile_driven(
    build: impl FnOnce() -> App,
    mut prepare: impl FnMut(&mut App),
    mut measure: impl FnMut(&mut App),
    iters: usize,
    warmup: usize,
    rebuild_hint: &str,
) {
    let agg = install();
    let mut app = build();

    for _ in 0..warmup {
        prepare(&mut app);
        measure(&mut app);
    }
    {
        let mut a = agg.lock().unwrap();
        a.per_system.clear();
        a.recording = false;
    }

    // Wall time accumulates ONLY the measured halves, so the printed frame cost and
    // the stage percentages describe the same window.
    let mut wall_total_ms = 0.0;
    for _ in 0..iters {
        prepare(&mut app);
        agg.lock().unwrap().recording = true;
        let t = Instant::now();
        measure(&mut app);
        wall_total_ms += t.elapsed().as_secs_f64() * 1000.0;
        agg.lock().unwrap().recording = false;
    }

    print_report(&agg, iters, wall_total_ms, rebuild_hint);
}

fn print_report(agg: &Arc<Mutex<Agg>>, frames: usize, wall_total_ms: f64, rebuild_hint: &str) {
    use std::fmt::Write as _;
    let a = agg.lock().unwrap();

    if a.per_system.is_empty() {
        println!(
            "profile: no system spans were recorded.\n\
             This mode needs Bevy's per-system instrumentation. Rebuild with:\n\
             \n    {rebuild_hint}\n"
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buckets_map_known_system_paths() {
        assert_eq!(bucket_for("supersolid::emit_bevy_inbox_system"), Bucket::BoaRender);
        assert_eq!(bucket_for("superui_bridge::reconcile_system"), Bucket::Reconcile);
        assert_eq!(bucket_for("bevy_flair_style::systems::recalculate"), Bucket::Flair);
        assert_eq!(bucket_for("bevy_ui::layout::ui_layout_system"), Bucket::Taffy);
        assert_eq!(bucket_for("superui_bridge::push_ui_frame"), Bucket::Marshal);
        assert_eq!(bucket_for("bevy_text::text_system"), Bucket::UiOther);
        assert_eq!(bucket_for("my_game::sim::advance"), Bucket::Other);
    }

    #[test]
    fn schedule_runner_wrappers_are_excluded() {
        // These WRAP every other system, so counting them double-counts the frame.
        assert!(is_wrapper("bevy_app::main_schedule::Main::run_main"));
        assert!(is_wrapper("bevy_app::main_schedule::run_fixed_main_schedule"));
        assert!(!is_wrapper("superui_bridge::reconcile_system"));
    }
}
