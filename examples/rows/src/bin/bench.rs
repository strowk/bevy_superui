//! `rows-bench` — the js-framework-benchmark rows workload on superui.
//! See `examples/rows/benchmark.md`.

use rows::bench::{untraced_json, untraced_table, run_ops, Backend};
use superui_bench_support::{parse_args, ArgDefaults};

const DEFAULTS: ArgDefaults = ArgDefaults {
    frames: 20, // reps, not frames — rows measures ops, not steady-state frames
    warmup: 3,
    cap_flags: &["--rows"],
};

fn main() {
    // deno_core's V8 platform posts delayed tasks (e.g. idle GC) that require an
    // entered tokio runtime context, even though the bench drives the engine
    // fully synchronously. Boa has no such requirement.
    #[cfg(feature = "engine-v8")]
    let _tokio_rt = tokio::runtime::Runtime::new().expect("tokio runtime for engine-v8");
    #[cfg(feature = "engine-v8")]
    let _tokio_guard = _tokio_rt.enter();

    let argv: Vec<String> = std::env::args().skip(1).collect();
    let args = match parse_args(&argv, DEFAULTS) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("error: {e}");
            eprintln!(
                "usage: rows-bench --backend vanilla|supersolid [--rows 1000|10000] \
                 [--reps N] [--warmup N] [--format table|json] [--profile]"
            );
            std::process::exit(2);
        }
    };

    let backend = match args.backend.as_deref() {
        Some("vanilla") => Backend::Vanilla,
        Some("supersolid") => Backend::Supersolid,
        Some(other) => {
            eprintln!("unknown backend '{other}' (vanilla|supersolid)");
            std::process::exit(2);
        }
        None => {
            eprintln!("--backend is required (vanilla|supersolid)");
            std::process::exit(2);
        }
    };

    let scales = if args.caps.is_empty() { vec![1000] } else { args.caps.clone() };

    for rows in scales {
        if args.profile {
            rows::bench::profile::run_profile(backend, rows, args.frames, args.warmup);
            continue;
        }
        let reports = run_ops(backend, rows, args.frames, args.warmup);
        if args.json {
            println!("{}", untraced_json(backend, rows, &reports));
        } else {
            print!("{}", untraced_table(backend, rows, &reports));
        }
    }
}
