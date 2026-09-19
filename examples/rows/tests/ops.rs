//! Spec §8: each op must leave the DOM in the expected state, on BOTH backends.
//! These guard the harness, not the framework — a benchmark that measures the
//! wrong mutation fast is worse than no benchmark.
//!
//! Each check is split into a `_vanilla` / `_supersolid` test pair (sharing a
//! `Backend`-parameterized helper) rather than looping both backends inside one
//! `#[test]`. A shared loop panics on the first failing backend and never
//! exercises the second for that run — which is exactly what happened during
//! development, when vanilla's `remove1`/`removeEvery2nd` bug meant supersolid
//! was never checked by those assertions in the same run. Splitting means a
//! failure report always names which backend failed, and the other backend's
//! coverage can never be silently skipped by the first one's panic.

use rows::bench::{
    build_bench_app, first_row_ids, measure_op, precondition, row_count, row_labels,
    row_warm_flags, Backend, OPS,
};

fn every_op_settles_in_one_frame_on(b: Backend) {
    for op in OPS {
        let mut app = build_bench_app(b);
        precondition(&mut app, if op == "create" { 0 } else { 1000 });
        let s = measure_op(&mut app, op, 1000);
        assert_eq!(
            s.frames,
            1,
            "{}/{op} settled in {} frames, not 1 — the published Frames column \
             must read 1 or the total is being spread across frames",
            b.label(),
            s.frames
        );
        assert!(
            s.total_ms > 0.0,
            "{}/{op} settled in 1 frame but total_ms was {} — a settled op cannot \
             have taken zero time, so this points at a measurement/clock bug \
             rather than a driver bug",
            b.label(),
            s.total_ms
        );
    }
}

#[test]
fn every_op_settles_in_one_frame_vanilla() {
    every_op_settles_in_one_frame_on(Backend::Vanilla);
}

#[test]
fn every_op_settles_in_one_frame_supersolid() {
    every_op_settles_in_one_frame_on(Backend::Supersolid);
}

fn row_counts_after_each_op_on(b: Backend) {
    // (op, rows before, expected rows after)
    let cases: [(&str, usize, usize); 8] = [
        ("create", 0, 1000),
        ("append1", 1000, 1001),
        ("append1k", 1000, 2000),
        ("insert1", 1000, 1001),
        // N/2 = 500 inserts, not 1000 — see the Every2nd rule in Global Constraints.
        ("insertEvery2nd", 1000, 1500),
        ("remove1", 1000, 999),
        ("removeEvery2nd", 1000, 500),
        ("clear", 1000, 0),
    ];
    for (op, before, after) in cases {
        let mut app = build_bench_app(b);
        precondition(&mut app, before);
        measure_op(&mut app, op, before);
        assert_eq!(row_count(&app), after, "{}/{op}", b.label());
    }
}

#[test]
fn row_counts_after_each_op_vanilla() {
    row_counts_after_each_op_on(Backend::Vanilla);
}

#[test]
fn row_counts_after_each_op_supersolid() {
    row_counts_after_each_op_on(Backend::Supersolid);
}

fn swap1_swaps_exactly_two_rows_and_preserves_order_on(b: Backend) {
    let mut app = build_bench_app(b);
    precondition(&mut app, 1000);
    let before = first_row_ids(&app, 1000);
    measure_op(&mut app, "swap1", 1000);
    let after = first_row_ids(&app, 1000);

    assert_eq!(after[1], before[998], "{}: row 1 must hold old row 998", b.label());
    assert_eq!(after[998], before[1], "{}: row 998 must hold old row 1", b.label());
    for i in 0..1000 {
        if i != 1 && i != 998 {
            assert_eq!(after[i], before[i], "{}: row {i} moved but should not have", b.label());
        }
    }
}

#[test]
fn swap1_swaps_exactly_two_rows_and_preserves_order_vanilla() {
    swap1_swaps_exactly_two_rows_and_preserves_order_on(Backend::Vanilla);
}

#[test]
fn swap1_swaps_exactly_two_rows_and_preserves_order_supersolid() {
    swap1_swaps_exactly_two_rows_and_preserves_order_on(Backend::Supersolid);
}

fn remove_every_2nd_leaves_the_right_ids_in_order_on(b: Backend) {
    let mut app = build_bench_app(b);
    precondition(&mut app, 1000);
    let before = first_row_ids(&app, 1000);
    measure_op(&mut app, "removeEvery2nd", 1000);
    let after = first_row_ids(&app, 500);

    let expected: Vec<i64> = before.iter().skip(1).step_by(2).copied().collect();
    assert_eq!(after, expected, "{}", b.label());
}

#[test]
fn remove_every_2nd_leaves_the_right_ids_in_order_vanilla() {
    remove_every_2nd_leaves_the_right_ids_in_order_on(Backend::Vanilla);
}

#[test]
fn remove_every_2nd_leaves_the_right_ids_in_order_supersolid() {
    remove_every_2nd_leaves_the_right_ids_in_order_on(Backend::Supersolid);
}

// The four checks below close the gap the final whole-branch review found:
// `every_op_settles_in_one_frame` only asserts `frames == 1` and `total_ms > 0`,
// properties a completely inert op also satisfies — `drain_dom_events_system`
// sets `dirty` unconditionally after dispatching any event, so `reconciles` bumps
// even if the JS handler did nothing or threw and was silently swallowed. These
// four ops are all in-place (no node added/removed), so a no-op handler would be
// invisible in `Rows`/`Nodes`/`Frames` alike — exactly how three ops previously
// shipped as "fast" while doing nothing.

fn update_text_every_2nd_changes_exactly_half_the_labels_on(b: Backend) {
    let mut app = build_bench_app(b);
    precondition(&mut app, 1000);
    let before = row_labels(&app);
    assert_eq!(before.len(), 1000, "{}: expected 1000 rows before the op", b.label());
    measure_op(&mut app, "updateTextEvery2nd", 1000);
    let after = row_labels(&app);
    assert_eq!(after.len(), 1000, "{}: expected 1000 rows after the op", b.label());

    let mut changed = 0usize;
    for i in 0..1000 {
        if i % 2 == 0 {
            assert_ne!(
                after[i], before[i],
                "{}/updateTextEvery2nd: row {i} (even) must have a new label",
                b.label()
            );
            changed += 1;
        } else {
            assert_eq!(
                after[i], before[i],
                "{}/updateTextEvery2nd: row {i} (odd) must be byte-identical",
                b.label()
            );
        }
    }
    assert_eq!(
        changed, 500,
        "{}/updateTextEvery2nd: expected exactly 500 changed labels",
        b.label()
    );
}

#[test]
fn update_text_every_2nd_changes_exactly_half_the_labels_vanilla() {
    update_text_every_2nd_changes_exactly_half_the_labels_on(Backend::Vanilla);
}

#[test]
fn update_text_every_2nd_changes_exactly_half_the_labels_supersolid() {
    update_text_every_2nd_changes_exactly_half_the_labels_on(Backend::Supersolid);
}

fn update_color_every_2nd_warms_exactly_half_the_rows_on(b: Backend) {
    let mut app = build_bench_app(b);
    precondition(&mut app, 1000);
    measure_op(&mut app, "updateColorEvery2nd", 1000);
    let warm = row_warm_flags(&app);
    assert_eq!(warm.len(), 1000, "{}: expected 1000 rows after the op", b.label());

    let mut warmed = 0usize;
    for (i, &is_warm) in warm.iter().enumerate() {
        assert_eq!(
            is_warm,
            i % 2 == 0,
            "{}/updateColorEvery2nd: row {i} warm={is_warm}, expected warm iff even",
            b.label()
        );
        if is_warm {
            warmed += 1;
        }
    }
    assert_eq!(
        warmed, 500,
        "{}/updateColorEvery2nd: expected exactly 500 warmed rows",
        b.label()
    );
}

#[test]
fn update_color_every_2nd_warms_exactly_half_the_rows_vanilla() {
    update_color_every_2nd_warms_exactly_half_the_rows_on(Backend::Vanilla);
}

#[test]
fn update_color_every_2nd_warms_exactly_half_the_rows_supersolid() {
    update_color_every_2nd_warms_exactly_half_the_rows_on(Backend::Supersolid);
}

fn update_color1_warms_only_row_zero_on(b: Backend) {
    let mut app = build_bench_app(b);
    precondition(&mut app, 1000);
    measure_op(&mut app, "updateColor1", 1000);
    let warm = row_warm_flags(&app);
    assert_eq!(warm.len(), 1000, "{}: expected 1000 rows after the op", b.label());

    assert!(warm[0], "{}/updateColor1: row 0 must be warmed", b.label());
    for (i, &is_warm) in warm.iter().enumerate().skip(1) {
        assert!(!is_warm, "{}/updateColor1: row {i} must not be warmed", b.label());
    }
}

#[test]
fn update_color1_warms_only_row_zero_vanilla() {
    update_color1_warms_only_row_zero_on(Backend::Vanilla);
}

#[test]
fn update_color1_warms_only_row_zero_supersolid() {
    update_color1_warms_only_row_zero_on(Backend::Supersolid);
}

fn swap_every_2nd_swaps_exact_adjacent_pairs_on(b: Backend) {
    let mut app = build_bench_app(b);
    precondition(&mut app, 1000);
    let before = first_row_ids(&app, 1000);
    measure_op(&mut app, "swapEvery2nd", 1000);
    let after = first_row_ids(&app, 1000);

    let mut i = 0;
    while i + 1 < 1000 {
        assert_eq!(
            after[i], before[i + 1],
            "{}/swapEvery2nd: row {i} must hold old row {}",
            b.label(),
            i + 1
        );
        assert_eq!(
            after[i + 1], before[i],
            "{}/swapEvery2nd: row {} must hold old row {i}",
            b.label(),
            i + 1
        );
        i += 2;
    }
}

#[test]
fn swap_every_2nd_swaps_exact_adjacent_pairs_vanilla() {
    swap_every_2nd_swaps_exact_adjacent_pairs_on(Backend::Vanilla);
}

#[test]
fn swap_every_2nd_swaps_exact_adjacent_pairs_supersolid() {
    swap_every_2nd_swaps_exact_adjacent_pairs_on(Backend::Supersolid);
}
