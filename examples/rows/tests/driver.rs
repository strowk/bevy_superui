use rows::bench::{
    build_bench_app, dom_node_count, first_row_label, measure_op, precondition, row_count,
    Backend, OPS,
};

#[test]
fn op_list_is_the_frozen_contract() {
    // Spec §2: these names and this order are not implementation choices.
    assert_eq!(
        OPS,
        [
            "create", "append1", "append1k", "insert1", "insertEvery2nd",
            "updateText1", "updateTextEvery2nd", "updateColor1", "updateColorEvery2nd",
            "swap1", "swapEvery2nd", "remove1", "removeEvery2nd", "clear",
        ]
    );
}

#[test]
fn an_op_settles_and_reports_its_frame_count() {
    let mut app = build_bench_app(Backend::Vanilla);
    precondition(&mut app, 1000);
    assert_eq!(row_count(&app), 1000, "precondition did not reach 1000 rows");

    let s = measure_op(&mut app, "append1", 1000);
    assert!(s.total_ms > 0.0, "op reported zero time");
    assert!(s.frames >= 1, "op reported zero frames");
    assert_eq!(row_count(&app), 1001, "append1 must add exactly one row");
}

#[test]
fn an_in_place_op_is_measured_not_skipped() {
    let mut app = build_bench_app(Backend::Vanilla);
    precondition(&mut app, 1000);

    let nodes_before = dom_node_count(&app);
    let label_before = first_row_label(&app);

    let s = measure_op(&mut app, "updateText1", 1000);

    assert_eq!(dom_node_count(&app), nodes_before,
        "updateText1 must not change the node count — if it does, the op is wrong");
    assert_ne!(first_row_label(&app), label_before,
        "updateText1 did not actually change the label; the test proves nothing");
    assert!(s.total_ms > 0.0 && s.frames >= 1,
        "in-place op reported total_ms={} frames={} — the predicate cannot see work \
         that leaves the node count unchanged", s.total_ms, s.frames);
}

use rows::bench::{untraced_json, untraced_table, run_ops};

#[test]
fn untraced_reports_every_op_in_contract_order() {
    let reports = run_ops(Backend::Vanilla, 1000, 2, 1);
    assert_eq!(reports.len(), OPS.len(), "every op must be reported");
    for (r, expected) in reports.iter().zip(OPS) {
        assert_eq!(r.op, expected, "op order is the comparability contract");
    }

    let t = untraced_table(Backend::Vanilla, 1000, &reports);
    assert!(t.contains("swap1"), "{t}");
    assert!(t.contains("Frames"), "the Frames column must be published: {t}");
    assert!(!t.to_lowercase().contains("traced"), "the untraced pass must not claim traced data: {t}");

    let j = untraced_json(Backend::Vanilla, 1000, &reports);
    assert!(j.contains("\"traced\":false"), "the untraced pass's JSON must mark itself untraced: {j}");
    assert!(j.contains("\"p50_ms\""), "{j}");
    assert!(j.contains("\"p95_ms\""), "{j}");
    assert!(j.contains("\"p99_ms\""), "{j}");
}

#[test]
fn traced_pass_is_labelled_and_never_merged_with_untraced() {
    // The traced pass's own JSON must be distinguishable from the untraced pass's (spec §5).
    let j = rows::bench::profile::traced_json_header(Backend::Vanilla, 1000);
    assert!(j.contains("\"traced\":true"), "{j}");
}
