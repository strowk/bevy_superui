//! UI mode stub — real implementation is Task 11.
//!
//! `superui test --ui` will eventually launch an interactive Egui-based
//! test runner that shows spec results, DOM trees, and screenshot diffs in a
//! Bevy window.  For now, we simply print a message so the binary compiles.

pub fn run(
    _cfg: &crate::config::TestConfig,
    _project: &crate::host::HostProject,
    _specs: &[std::path::PathBuf],
) {
    eprintln!("superui test --ui: UI mode not built yet (see Task 11)");
}
