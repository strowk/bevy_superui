//! `superui test` — CLI for the superui E2E test engine.
//!
//! Usage:
//!   superui test [--update] [--ui] [filter]
//!
//! `--update`  overwrite snapshot baselines instead of diffing them.
//! `--ui`      launch interactive UI mode.
//! `filter`    optional substring; only spec files whose path contains it run.
//!
//! Reads `superui.test.toml` from the current working directory. Exit 0 if all
//! tests pass, 1 if any fail, 2 on config / project errors.

use superui_test_engine::cli::{run_tests, TestRunConfig};

fn print_help() {
    eprintln!(
        "superui test [--update] [--ui] [filter]\n\
         \n\
         Options:\n\
         --update   overwrite snapshot baselines\n\
         --ui       launch interactive UI mode\n\
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

    let cfg = TestRunConfig {
        update: args.iter().any(|a| a == "--update"),
        ui: args.iter().any(|a| a == "--ui"),
        filter: args.iter().find(|a| !a.starts_with("--")).cloned(),
    };

    std::process::exit(run_tests(cfg));
}
