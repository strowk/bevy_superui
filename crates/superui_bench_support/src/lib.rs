//! Shared harness for the superui macro-benchmarks (horde, citadel, rows).
//!
//! Everything here is generic over the example: the app-dependent half is passed
//! in as an `impl FnOnce() -> App`, so this crate never sees an example's config
//! type. Report formatting deliberately stays in each example — the three
//! benchmarks print genuinely different things and unifying them would need a
//! config knob per divergence.

pub mod alloc;
pub mod cli;
pub mod profile;
pub mod stats;

pub use alloc::{alloc_table, AllocReport};
pub use cli::{parse_args, ArgDefaults, BenchArgs};
pub use profile::{run_profile_driven, run_profile_with, Bucket};
pub use stats::{stats_from, Stats};
