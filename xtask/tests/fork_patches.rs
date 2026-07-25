use std::path::PathBuf;

// Resolve repo root from the xtask crate dir.
fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

#[test]
fn registry_and_markers_agree() {
    let ids = xtask::check_fork_patches(&repo_root()).expect("fork patches should be consistent");
    assert!(ids.contains(&"css-eof-guard".to_string()), "expected css-eof-guard, got {ids:?}");
}
