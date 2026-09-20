//! Headless, deterministic harness for the rows workload.

pub mod profile;
pub mod report;
pub use report::{untraced_json, untraced_table, run_ops, OpReport};

use std::time::Duration;

use bevy::asset::io::memory::{Dir, MemoryAssetReader};
use bevy::asset::io::{AssetSourceBuilder, AssetSourceId};
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

use superui::prelude::{SuperUiPlugin, SuperUiRoot};

const V_HTML: &str = include_str!("../../assets/ui/rows_vanilla/index.html");
const V_CSS: &str = include_str!("../../assets/ui/rows_vanilla/rows.css");
const V_JS: &str = include_str!("../../assets/ui/rows_vanilla/app.js");

const S_HTML: &str = include_str!("../../assets/ui/rows_solid/index.html");
const S_CSS: &str = include_str!("../../assets/ui/rows_solid/rows.css");
const S_TSX: &str = include_str!("../../assets/ui/rows_solid/app.tsx");
const S_JS: &str = include_str!("../../assets/ui/rows_solid/.superui/build/app.js");

/// Fixed per-update time step: exactly one FixedUpdate tick per app.update().
pub const DT: f64 = 1.0 / 60.0;

/// Which UI backend drives the identical rows markup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backend {
    Vanilla,
    Supersolid,
}

impl Backend {
    pub fn label(self) -> &'static str {
        match self {
            Backend::Vanilla => "vanilla",
            Backend::Supersolid => "supersolid",
        }
    }
    pub fn asset_dir(self) -> &'static str {
        match self {
            Backend::Vanilla => "ui/rows_vanilla",
            Backend::Supersolid => "ui/rows_solid",
        }
    }
}

fn memory_asset_dir(backend: Backend) -> Dir {
    let dir = Dir::new("assets".into());
    match backend {
        Backend::Vanilla => {
            dir.insert_asset("ui/rows_vanilla/index.html".as_ref(), V_HTML.as_bytes());
            dir.insert_asset("ui/rows_vanilla/rows.css".as_ref(), V_CSS.as_bytes());
            dir.insert_asset("ui/rows_vanilla/app.js".as_ref(), V_JS.as_bytes());
        }
        Backend::Supersolid => {
            dir.insert_asset("ui/rows_solid/index.html".as_ref(), S_HTML.as_bytes());
            dir.insert_asset("ui/rows_solid/rows.css".as_ref(), S_CSS.as_bytes());
            dir.insert_asset("ui/rows_solid/app.tsx".as_ref(), S_TSX.as_bytes());
            dir.insert_asset("ui/rows_solid/.superui/build/app.js".as_ref(), S_JS.as_bytes());
        }
    }
    dir
}

/// Build a finished, headless, deterministic bench app for `backend`.
pub fn build_bench_app(backend: Backend) -> App {
    let mut app = App::new();

    let dir = memory_asset_dir(backend);
    app.register_asset_source(
        AssetSourceId::Default,
        AssetSourceBuilder::new(move || Box::new(MemoryAssetReader { root: dir.clone() })),
    );

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

    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(DT)));
    app.insert_resource(Time::<Fixed>::from_seconds(DT));

    app.add_plugins(SuperUiPlugin);

    let dir_name = backend.asset_dir();
    app.add_systems(Startup, move |mut commands: Commands, assets: Res<AssetServer>| {
        commands.spawn(SuperUiRoot::from_asset_dir(dir_name, &assets));
    });

    app.finish();
    app
}

/// Number of DOM elements the app contains, via the live `UiRuntime`.
pub fn dom_node_count(app: &App) -> usize {
    app.world()
        .get_non_send::<superui::UiRuntime>()
        .map(|rt| {
            let d = rt.dom.borrow();
            d.query_selector_all(d.document(), "*").len()
        })
        .unwrap_or(0)
}

/// Boot chrome: DOM elements present with ZERO rows. Backend-dependent — the
/// supersolid app mounts inside a `<div id="root">` (the convention every other
/// supersolid example uses) that the vanilla app has no equivalent of, so chrome is
/// 24 vs 23. Constant and op-independent, so it does not affect timings, but the
/// published `Nodes` column exists for cross-implementation normalisation and must
/// therefore be chrome-free.
pub fn chrome_node_count(backend: Backend) -> usize {
    let mut app = build_bench_app(backend);
    for _ in 0..30 {
        app.update();
    }
    dom_node_count(&app)
}

use std::time::Instant;
use superui::{PendingDomEvents, UiRuntime};

/// The frozen op set (spec §2). Names and order are the comparability contract.
pub const OPS: [&str; 14] = [
    "create", "append1", "append1k", "insert1", "insertEvery2nd",
    "updateText1", "updateTextEvery2nd", "updateColor1", "updateColorEvery2nd",
    "swap1", "swapEvery2nd", "remove1", "removeEvery2nd", "clear",
];

/// One measured operation.
#[derive(Clone, Copy, Debug)]
pub struct OpSample {
    pub total_ms: f64,
    /// Frames the op took to settle. Expected 1; published so a spill is visible.
    pub frames: usize,
    pub rows_before: usize,
    pub nodes_after: usize,
}

/// Live `.row` count, read from the DOM.
pub fn row_count(app: &App) -> usize {
    app.world()
        .get_non_send::<UiRuntime>()
        .map(|rt| {
            let d = rt.dom.borrow();
            d.query_selector_all(d.document(), ".row").len()
        })
        .unwrap_or(0)
}

/// Text of the first row's `.lbl` anchor. Proves an in-place update really changed
/// something, so the regression test cannot pass vacuously.
pub fn first_row_label(app: &App) -> String {
    let Some(rt) = app.world().get_non_send::<UiRuntime>() else { return String::new() };
    let d = rt.dom.borrow();
    d.query_selector(d.document(), ".row .lbl")
        .map(|n| d.text_content(n))
        .unwrap_or_default()
}

/// Text of every row's `.lbl` anchor, in DOM order. Unlike `first_row_label`,
/// this covers the whole table — needed by op tests that must check more than
/// just row 0 (`updateTextEvery2nd`).
pub fn row_labels(app: &App) -> Vec<String> {
    let Some(rt) = app.world().get_non_send::<UiRuntime>() else { return Vec::new() };
    let d = rt.dom.borrow();
    d.query_selector_all(d.document(), ".row .lbl")
        .into_iter()
        .map(|n| d.text_content(n))
        .collect()
}

/// Whether each row's `.lbl` anchor carries the `warm` class, in DOM order.
/// Used by `updateColor1`/`updateColorEvery2nd` op tests.
pub fn row_warm_flags(app: &App) -> Vec<bool> {
    let Some(rt) = app.world().get_non_send::<UiRuntime>() else { return Vec::new() };
    let d = rt.dom.borrow();
    d.query_selector_all(d.document(), ".row .lbl")
        .into_iter()
        .map(|n| d.class_contains(n, "warm"))
        .collect()
}

/// The `data-id` of the first `n` rows, in DOM order. Used by the op tests to
/// assert ordering, which is the property <Keyed> would have broken.
pub fn first_row_ids(app: &App, n: usize) -> Vec<i64> {
    let Some(rt) = app.world().get_non_send::<UiRuntime>() else { return Vec::new() };
    let d = rt.dom.borrow();
    d.query_selector_all(d.document(), ".row")
        .into_iter()
        .take(n)
        .map(|node| {
            d.get_attribute(node, "data-id")
                .and_then(|s| s.parse::<i64>().ok())
                .unwrap_or(-1)
        })
        .collect()
}

/// Dispatch a real DOM click at `#op-<op>`, exactly as the picking observer would.
/// Returns false when the button is not mounted.
///
/// `UiRuntime` is NonSend and `PendingDomEvents` is a resource, and `World` will
/// not hand out both borrows at once. `resource_scope` temporarily removes the
/// resource so the closure can hold it mutably beside the runtime borrow.
pub fn click_op(app: &mut App, op: &str) -> bool {
    let world = app.world_mut();
    let Some(rt) = world.get_non_send::<UiRuntime>() else { return false };
    let node = {
        let d = rt.dom.borrow();
        d.query_selector(d.document(), &format!("#op-{op}"))
    };
    let Some(node) = node else { return false };

    world.resource_scope::<PendingDomEvents, _>(|world, mut pending| {
        let rt = world.non_send::<UiRuntime>();
        superui::click_effect(rt, node, &mut pending);
    });
    true
}

/// Completed reconcile passes so far. The only signal that survives a frame
/// boundary: `rt.dirty` is set (by `drain_dom_events_system`) and cleared (by
/// `reconcile_system`) within the same `.chain()`ed `Update` run, so reading it
/// after `app.update()` returns always sees `false` — it can never catch a
/// reconcile that happened. And a DOM node-count comparison misses every
/// in-place op (`updateText1`, `updateTextEvery2nd`, `updateColor1`,
/// `updateColorEvery2nd`, `swap1`, `swapEvery2nd`): none of them add or remove a
/// node, so `before == after` would falsely declare the op already settled and
/// report a zero-time, zero-frame sample for it. `reconciles` is a monotonic
/// counter bumped once per completed reconcile pass (see
/// `crates/superui_bridge/src/reconcile.rs`), so it is the one signal that
/// actually crosses the `app.update()` boundary intact. Do not "simplify" this
/// back to `dirty` or node counts — both were tried and both under-report.
fn reconcile_count(app: &App) -> u64 {
    app.world()
        .get_non_send::<UiRuntime>()
        .map(|rt| rt.reconciles)
        .unwrap_or(0)
}

/// Step `app.update()` until a frame completes zero reconcile passes.
///
/// Returns (summed ms of the frames that did work, that frame count). A fixed
/// frame count would silently absorb an op that spills; this makes it visible.
pub fn step_to_quiescence(app: &mut App) -> (f64, usize) {
    const MAX_FRAMES: usize = 64;
    let mut total_ms = 0.0;
    let mut worked = 0usize;

    for _ in 0..MAX_FRAMES {
        let before = reconcile_count(app);
        let t = Instant::now();
        app.update();
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        let after = reconcile_count(app);

        if after == before {
            // Settled: this frame reconciled nothing, so it is not counted.
            return (total_ms, worked);
        }
        total_ms += ms;
        worked += 1;
    }
    panic!("op did not settle within {MAX_FRAMES} frames");
}

/// Drive the app to `rows` rows without timing it.
pub fn precondition(app: &mut App, rows: usize) {
    // Mount first.
    for _ in 0..30 {
        app.update();
    }
    let op = match rows {
        0 => "clear",
        1_000 => "create",
        10_000 => "create10k",
        other => panic!("unsupported precondition: {other} rows"),
    };
    assert!(click_op(app, op), "precondition button #op-{op} not found");
    let _ = step_to_quiescence(app);
    assert_eq!(row_count(app), rows, "precondition did not reach {rows} rows");
}

/// Reset an already-mounted app back to `rows` rows, untimed.
pub fn precondition_reset(app: &mut App, rows: usize) {
    assert!(click_op(app, "clear"), "#op-clear not found");
    let _ = step_to_quiescence(app);
    if rows > 0 {
        let op = match rows {
            1_000 => "create",
            10_000 => "create10k",
            other => panic!("unsupported precondition: {other} rows"),
        };
        assert!(click_op(app, op), "#op-{op} not found");
        let _ = step_to_quiescence(app);
    }
    assert_eq!(row_count(app), rows, "reset did not reach {rows} rows");
}

/// Trigger `op` and measure it. The app must already be at its precondition.
pub fn measure_op(app: &mut App, op: &str, rows_before: usize) -> OpSample {
    assert!(click_op(app, op), "button #op-{op} not found");
    let (total_ms, frames) = step_to_quiescence(app);
    OpSample { total_ms, frames, rows_before, nodes_after: dom_node_count(app) }
}

/// The button that performs `op` at this scale.
///
/// `create` is the ONLY op whose size follows the table: it builds 1,000 rows at the
/// 1k scale and 10,000 at the 10k scale, so at 10k ours must build 10,000 rows or the
/// two scales measure the same work. The fixture exposes that as a separate `create10k`
/// button, which doubles as the 10k precondition helper.
///
/// No other op needs mapping: `append1k`/`append1`/`insert1` are fixed-size by
/// definition (`append1k` appends 1,000 rows at both scales), and the `*Every2nd`
/// family derives its size from the current table.
///
/// This is an op-driver concern (it decides what button a call to `measure_op`
/// actually clicks), not a reporting concern, so it lives here rather than in
/// `report.rs` — every caller of `measure_op` with a scale-dependent op must route
/// through this, and a second caller (`profile.rs`) once didn't: it called
/// `measure_op(app, op, before)` with the raw op name, so the traced pass's 10k
/// `create` silently measured a 1,000-row build instead of a 10,000-row one. Moving
/// this here and making it `pub` is what lets both callers share one mapping instead
/// of each needing to remember it exists.
pub fn button_for(op: &str, rows: usize) -> &str {
    if op == "create" && rows == 10_000 {
        "create10k"
    } else {
        op
    }
}

/// Pre-op row count for `op` at `rows` scale: 0 before `create` (there is nothing
/// to precondition — `create` builds the table from empty), the current scale for
/// every other op.
///
/// Centralized for the same reason as `button_for`: this rule previously had
/// two call sites (`report.rs`'s `run_ops` and `profile.rs`'s `run_profile`),
/// each re-implementing `if op == "create" { 0 } else { rows }` inline — the
/// identical two-call-sites-one-rule shape that let `button_for`'s bug ship
/// (see its doc comment). Both now call this instead.
pub fn rows_before_for(op: &str, rows: usize) -> usize {
    if op == "create" {
        0
    } else {
        rows
    }
}

#[cfg(test)]
mod button_for_tests {
    use super::*;

    /// Regression test for the bug described on `button_for`'s doc comment: a third
    /// call site must not be able to reintroduce it by forgetting to route through
    /// this function.
    #[test]
    fn only_create_is_remapped_and_only_at_10k() {
        assert_eq!(button_for("create", 1_000), "create");
        assert_eq!(button_for("create", 10_000), "create10k");

        for op in OPS {
            if op == "create" {
                continue;
            }
            assert_eq!(button_for(op, 1_000), op, "{op} must map to itself at 1k");
            assert_eq!(button_for(op, 10_000), op, "{op} must map to itself at 10k");
        }
    }

    /// Sibling of the test above, for `rows_before_for`: `create` preconditions
    /// to 0 at both scales, every other op preconditions to the scale itself.
    #[test]
    fn rows_before_for_is_zero_only_for_create() {
        for rows in [1_000, 10_000] {
            assert_eq!(rows_before_for("create", rows), 0, "create must precondition to 0");
            for op in OPS {
                if op == "create" {
                    continue;
                }
                assert_eq!(
                    rows_before_for(op, rows),
                    rows,
                    "{op} must precondition to the full {rows}-row scale"
                );
            }
        }
    }
}
