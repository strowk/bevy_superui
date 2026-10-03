//! Library entry point for the superui E2E test runner.
//!
//! `run_tests` returns the process exit code rather than calling
//! `process::exit`, so both the `superui_test` bin and `cargo superui test`
//! share one implementation and own the exit themselves.
//!
//! Exit codes: 0 all tests pass, 1 some test failed, 2 config / project error.
//!
//! ISOLATION: one render app is built per spec file via
//! `build_render_app_and_mount`, and `run_spec_with` runs every test in that
//! spec against it. Tests within a spec therefore share DOM state (one rendered
//! tree, not a fresh page per test).

use std::path::PathBuf;

use crate::{config, driver, render, report, snapshot, trace, transpile};

/// Options for one runner invocation. Config is loaded from
/// `superui.test.toml` in the current working directory.
pub struct TestRunConfig {
    /// Overwrite snapshot baselines instead of diffing them.
    pub update: bool,
    /// Launch interactive UI mode instead of a headless run.
    pub ui: bool,
    /// Substring filter; only spec files whose path contains it run.
    pub filter: Option<String>,
}

/// Run the engine against the project in the current working directory.
/// Returns the process exit code (0 pass, 1 failures, 2 config / project error).
pub fn run_tests(cfg: TestRunConfig) -> i32 {
    let TestRunConfig { update, ui, filter } = cfg;

    let cfg_path = PathBuf::from("superui.test.toml");
    let config = match config::load_config(&cfg_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: config: {e}");
            return 2;
        }
    };

    let project = match config::load_project(&config.project) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: project: {e}");
            return 2;
        }
    };

    let specs: Vec<PathBuf> = config::discover_specs(&config.spec_dir)
        .into_iter()
        .filter(|p| {
            filter
                .as_ref()
                .map(|f| p.to_string_lossy().contains(f.as_str()))
                .unwrap_or(true)
        })
        .collect();

    if specs.is_empty() {
        eprintln!("warning: no spec files found in {:?}", config.spec_dir);
        return 0;
    }

    if ui {
        crate::ui_mode::run(&config, &project, &specs);
        return 0;
    }

    let snap_cfg = snapshot::SnapshotConfig {
        dir: config.spec_dir.clone(),
        update,
        max_diff_ratio: config.max_diff_ratio,
        platform: std::env::consts::OS.to_string(),
    };

    let mut all: Vec<(String, Vec<trace::TestResult>)> = Vec::new();

    for spec in &specs {
        let src = match std::fs::read_to_string(spec) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("error: reading {:?}: {e}", spec);
                return 2;
            }
        };

        let file = spec
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let base_dir = spec.parent().unwrap_or_else(|| std::path::Path::new("."));
        let js = match transpile::transpile_spec(&src, &file, base_dir) {
            Ok(j) => j,
            Err(e) => {
                eprintln!("error: transpile {file}: {e}");
                return 2;
            }
        };

        // One fresh render app per spec (see ISOLATION in the module doc).
        let mut app = render::build_render_app_and_mount(&project, config.width, config.height);

        let opts = driver::RunOptions {
            snapshot: Some(snapshot::SnapshotConfig {
                dir: snap_cfg.dir.clone(),
                update,
                max_diff_ratio: snap_cfg.max_diff_ratio,
                platform: snap_cfg.platform.clone(),
            }),
            spec_file: file.clone(),
            render: true,
        };

        let results = driver::run_spec_with(&mut app, &js, &opts);
        all.push((file, results));
    }

    let report_path = config.spec_dir.join("report.html");
    if let Err(e) = report::write_html_report(&report_path, &all) {
        eprintln!("warning: could not write HTML report: {e}");
    }

    if report::print_summary(&all) {
        0
    } else {
        1
    }
}
