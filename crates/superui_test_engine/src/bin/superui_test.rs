//! `superui test` — CLI for the superui E2E test engine.
//!
//! Usage:
//!   superui test [--update] [--ui] [filter]
//!
//! Reads `superui.test.toml` from the current working directory.
//! `--update`  overwrites snapshot baselines instead of diffing them.
//! `--ui`      launches interactive UI mode (Task 11 stub for now).
//! `filter`    optional substring; only spec files whose path contains it run.
//!
//! Exit 0 if all tests pass, 1 if any fail, 2 on config / project errors.
//!
//! ISOLATION NOTE: the CLI builds ONE fresh render app per spec file via
//! `build_render_app_and_mount`, and `run_spec_with` runs all tests in that
//! spec against the same mounted app.  Tests within a spec therefore share
//! DOM state (the same rendered tree, not a fresh Playwright-style page per
//! test).  This matches the plan's sanctioned default and is sufficient for
//! most spec patterns.  A future upgrade would call `build_render_app_and_mount`
//! inside the per-test loop to achieve strict per-test isolation.

use std::path::PathBuf;

use superui_test_engine::{config, driver, render, report, snapshot, transpile};

fn print_help() {
    eprintln!(
        "superui test [--update] [--ui] [filter]\n\
         \n\
         Options:\n\
         --update   overwrite snapshot baselines\n\
         --ui       launch interactive UI mode (Task 11)\n\
         filter     only run spec files containing this substring\n\
         --help     show this help\n\
         \n\
         Reads superui.test.toml from the current working directory."
    );
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        std::process::exit(0);
    }

    let update = args.iter().any(|a| a == "--update");
    let ui = args.iter().any(|a| a == "--ui");
    let filter: Option<String> = args
        .iter()
        .find(|a| !a.starts_with("--"))
        .cloned();

    // Load config from CWD.
    let cfg_path = PathBuf::from("superui.test.toml");
    let cfg = config::load_config(&cfg_path).unwrap_or_else(|e| {
        eprintln!("error: config: {e}");
        std::process::exit(2);
    });

    // Load the project's source files.
    let project = config::load_project(&cfg.project).unwrap_or_else(|e| {
        eprintln!("error: project: {e}");
        std::process::exit(2);
    });

    // Discover and optionally filter spec files.
    let specs: Vec<PathBuf> = config::discover_specs(&cfg.spec_dir)
        .into_iter()
        .filter(|p| {
            filter
                .as_ref()
                .map(|f| p.to_string_lossy().contains(f.as_str()))
                .unwrap_or(true)
        })
        .collect();

    if specs.is_empty() {
        eprintln!("warning: no spec files found in {:?}", cfg.spec_dir);
        std::process::exit(0);
    }

    // UI mode: delegate to Task 11 stub and return.
    if ui {
        superui_test_engine::ui_mode::run(&cfg, &project, &specs);
        return;
    }

    // Headless run: build a fresh render app per spec, run all tests in it.
    let snap_cfg = snapshot::SnapshotConfig {
        dir: cfg.spec_dir.clone(),
        update,
        max_diff_ratio: cfg.max_diff_ratio,
        platform: std::env::consts::OS.to_string(),
    };

    let mut all: Vec<(String, Vec<superui_test_engine::trace::TestResult>)> = Vec::new();

    for spec in &specs {
        let src = match std::fs::read_to_string(spec) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("error: reading {:?}: {e}", spec);
                std::process::exit(2);
            }
        };

        let file = spec
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let js = match transpile::transpile_spec(&src, &file) {
            Ok(j) => j,
            Err(e) => {
                eprintln!("error: transpile {file}: {e}");
                std::process::exit(2);
            }
        };

        // Build one fresh render app per spec.  Tests within a single spec
        // share this mounted app (see ISOLATION NOTE in module doc above).
        let mut app = render::build_render_app_and_mount(&project, cfg.width, cfg.height);

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

    // Write HTML report alongside the spec files.
    let report_path = cfg.spec_dir.join("report.html");
    if let Err(e) = report::write_html_report(&report_path, &all) {
        eprintln!("warning: could not write HTML report: {e}");
    }

    let ok = report::print_summary(&all);
    std::process::exit(if ok { 0 } else { 1 });
}
