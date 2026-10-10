use std::path::PathBuf;

// Resolve repo root from the xtask crate dir.
fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

#[test]
fn registry_and_markers_agree() {
    let ids = xtask::check_fork_patches(&repo_root()).expect("fork patches should be consistent");
    // `css-eof-guard` and `css-import-relative-resolution` were dropped during the
    // bevy 0.20 / flair 0.9 reconciliation (both are upstreamed in flair 0.9 now;
    // see docs/fork-patches.md and crates/superui_css/tests/{selectors,imports_relative}.rs
    // for the regression coverage that proves it). The remaining four patches are
    // still genuinely ours.
    assert!(
        ids.contains(&"css-rem-unit".to_string()),
        "expected css-rem-unit, got {ids:?}"
    );
    assert!(
        !ids.contains(&"css-eof-guard".to_string()),
        "css-eof-guard was dropped (upstreamed in flair 0.9), but is still registered: {ids:?}"
    );
}
