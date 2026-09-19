//! Per-stage profiling of the supersolid frame (the `--profile` bench mode).
//!
//! The attribution machinery lives in `superui_bench_support::profile`; citadel
//! only supplies the app and the rebuild hint. Citadel is steady-state (no player,
//! no GameState, no death), so unlike horde it needs no god-mode system.

use crate::bench::{build_bench_app, Backend};
use crate::sim::CitadelConfig;

const REBUILD_HINT: &str = "cargo run --release -p citadel --features bench,bevy/trace,bevy/debug --bin citadel-bench -- --profile ...";

pub fn run_profile(cfg: CitadelConfig, frames: usize, warmup: usize) {
    superui_bench_support::run_profile_with(
        || build_bench_app(Backend::Supersolid, cfg),
        frames,
        warmup,
        REBUILD_HINT,
    );
}
