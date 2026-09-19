//! Per-stage profiling of the supersolid frame (the `--profile` bench mode).
//!
//! The attribution machinery lives in `superui_bench_support::profile`. Horde adds
//! one thing citadel does not need: god-moding the player so a long warmup under a
//! stress swarm stays in `Playing` with a full swarm rendered, instead of dying and
//! dropping to the cheap `GameOver` screen where the profile would measure an
//! almost-empty UI.

use bevy::prelude::*;

use crate::bench::{build_bench_app, Backend};
use crate::sim::{Health, Player, SimConfig};

const REBUILD_HINT: &str = "cargo run --release -p horde --features bench,bevy/trace,bevy/debug --bin horde-bench -- --profile ...";

/// Keep the auto-player alive so the game stays in `Playing` with a full swarm
/// rendered. Runs in `First`, before any sim damage.
fn keep_player_alive(mut q: Query<&mut Health, With<Player>>) {
    if let Ok(mut hp) = q.single_mut() {
        hp.current = hp.max.max(1.0e9);
    }
}

pub fn run_profile(sim: SimConfig, frames: usize, warmup: usize) {
    superui_bench_support::run_profile_with(
        || {
            let mut app = build_bench_app(Backend::Supersolid, sim);
            app.add_systems(First, keep_player_alive);
            app
        },
        frames,
        warmup,
        REBUILD_HINT,
    );
}
