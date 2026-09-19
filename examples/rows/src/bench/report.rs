//! The untraced pass: the citable numbers.
//!
//! Spec §5 — the untraced pass and the traced pass are never blended. This module
//! emits only the untraced pass and marks its JSON `"traced": false` so a merged
//! file cannot be misread.

use bevy::prelude::App;
use superui_bench_support::{stats_from, Stats};

use crate::bench::{
    build_bench_app, button_for, chrome_node_count, measure_op, precondition, rows_before_for,
    Backend, OPS,
};

#[derive(Clone, Debug)]
pub struct OpReport {
    pub op: String,
    pub rows_before: usize,
    pub nodes: usize,
    pub frames: usize,
    pub total: Stats,
}

/// Measure every op at `rows` scale, `reps` timed reps after `warmup` discarded.
///
/// Each rep rebuilds the app so the precondition is reached from a clean state —
/// ops mutate the table, so reusing one app would drift the precondition.
pub fn run_ops(backend: Backend, rows: usize, reps: usize, warmup: usize) -> Vec<OpReport> {
    let mut out = Vec::with_capacity(OPS.len());
    // Subtract boot chrome so `Nodes` counts only row-attributable elements. The two
    // backends have different chrome (24 vs 23 — supersolid mounts inside `#root`),
    // and a column that silently differs by one defeats the normalisation it exists for.
    let chrome = chrome_node_count(backend);

    for op in OPS {
        let before = rows_before_for(op, rows);
        let mut samples: Vec<f64> = Vec::with_capacity(reps);
        let mut frames = 0usize;
        let mut nodes = 0usize;

        for rep in 0..(warmup + reps) {
            let mut app: App = build_bench_app(backend);
            precondition(&mut app, before);
            // The logical op name is what gets published; the button may differ —
            // see `button_for` (only `create` at the 10k scale).
            let s = measure_op(&mut app, button_for(op, rows), before);
            if rep >= warmup {
                samples.push(s.total_ms);
                frames = frames.max(s.frames);
                nodes = s.nodes_after;
            }
        }

        out.push(OpReport {
            op: op.to_string(),
            rows_before: before,
            nodes: nodes.saturating_sub(chrome),
            frames,
            total: stats_from(samples),
        });
    }
    out
}

pub fn untraced_table(backend: Backend, rows: usize, reports: &[OpReport]) -> String {
    use std::fmt::Write as _;
    let mut s = String::new();
    let _ = writeln!(
        s,
        "rows={rows} backend={} — no-tracing pass (these are the citable numbers)",
        backend.label()
    );
    let _ = writeln!(
        s,
        "| {:<20} | {:>6} | {:>7} | {:>6} | {:>9} | {:>9} | {:>9} |",
        "Op", "Rows", "Nodes", "Frames", "p50 (ms)", "p95 (ms)", "p99 (ms)"
    );
    for r in reports {
        let _ = writeln!(
            s,
            "| {:<20} | {:>6} | {:>7} | {:>6} | {:>9.3} | {:>9.3} | {:>9.3} |",
            r.op, r.rows_before, r.nodes, r.frames, r.total.p50_ms, r.total.p95_ms, r.total.p99_ms
        );
    }
    s
}

pub fn untraced_json(backend: Backend, rows: usize, reports: &[OpReport]) -> String {
    let ops: Vec<String> = reports
        .iter()
        .map(|r| {
            format!(
                "{{\"op\":\"{}\",\"rows\":{},\"nodes\":{},\"frames\":{},\
                 \"p50_ms\":{:.6},\"p95_ms\":{:.6},\"p99_ms\":{:.6},\"mean_ms\":{:.6}}}",
                r.op, r.rows_before, r.nodes, r.frames,
                r.total.p50_ms, r.total.p95_ms, r.total.p99_ms, r.total.mean_ms
            )
        })
        .collect();
    format!(
        "{{\"backend\":\"{}\",\"rows\":{},\"traced\":false,\"ops\":[{}]}}",
        backend.label(),
        rows,
        ops.join(",")
    )
}
