//! The traced pass: the per-stage breakdown (spec §5).
//!
//! Requires **both** `bevy/trace` and `bevy/debug`. `trace` creates the per-system
//! spans; `debug` is what makes their names resolvable — without it every name reads
//! `<Enable the debug feature to see the name>` and the whole frame collapses into the
//! `other` bucket, which looks like a working report but attributes nothing.
//! The stage columns sum to the traced total, not to the untraced pass's `Total` —
//! the two passes are separate runs and this module never emits the untraced pass's
//! numbers.
//!
//! **Reset-attribution:** horde/citadel are steady-state — one iteration is one
//! plain `app.update()`, so nothing but frame work is ever inside the measured
//! window. Rows is per-op, and each iteration needs an untimed step first: reset
//! back to `rows` rows via [`crate::bench::precondition_reset`] before firing the
//! op. Folding that reset into the traced window buried the op's own signal under
//! it — a `swap1` (2 rows touched) and a `create` (up to 10,000 rows built) traced
//! within roughly the same margin of each other, because both windows were
//! dominated by the same clear-and-rebuild reset. [`superui_bench_support::run_profile_driven`]
//! therefore takes the untraced and traced halves of an iteration separately:
//! the reset runs in `prepare` (recording off), the op runs in `measure` (recording
//! on). Only `measure`'s wall time and spans are ever counted, so the printed frame
//! cost and the stage percentages both describe the op alone.
use crate::bench::{build_bench_app, button_for, precondition, rows_before_for, Backend, OPS};

const REBUILD_HINT: &str = "cargo run --release -p rows --features bench,bevy/trace,bevy/debug --bin rows-bench -- --profile --backend vanilla --rows 1000";

/// JSON header identifying this as traced output, so a traced-pass file can never be
/// mistaken for an untraced-pass file.
pub fn traced_json_header(backend: Backend, rows: usize) -> String {
    format!(
        "{{\"backend\":\"{}\",\"rows\":{},\"traced\":true,\"note\":\"stage columns sum to the traced total, not to the untraced pass's Total\"",
        backend.label(),
        rows
    )
}

/// Run every op under the tracing attributor and print the per-stage table.
pub fn run_profile(backend: Backend, rows: usize, reps: usize, warmup: usize) {
    println!(
        "\n=== rows traced pass: per-stage breakdown (rows={rows}, backend={}) ===",
        backend.label()
    );
    println!(
        "NOTE: these totals include tracing overhead. The citable Total is the \
         untraced pass (run without --profile and without bevy/trace)."
    );

    for op in OPS {
        let before = rows_before_for(op, rows);
        println!("\n-- op: {op} --");
        superui_bench_support::run_profile_driven(
            || {
                let mut app = build_bench_app(backend);
                precondition(&mut app, before);
                app
            },
            |app| {
                // Re-establish the precondition. UNTRACED — this is bulk setup work,
                // not the op under measurement.
                crate::bench::precondition_reset(app, before);
            },
            |app| {
                // The op under measurement. TRACED. Routed through `button_for` —
                // `op` alone is the published name, but `create` at the 10k scale
                // must click `create10k` or this measures a 1,000-row build at
                // every scale (the bug `button_for`'s doc comment describes).
                crate::bench::measure_op(app, button_for(op, rows), before);
            },
            reps,
            warmup,
            REBUILD_HINT,
        );
    }
}
