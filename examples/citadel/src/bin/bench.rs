#[cfg(feature = "dhat-prof")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

#[cfg(feature = "dhat-prof")]
use citadel::bench::alloc_table;
use citadel::bench::{
    parse_args, report_json, report_table, run_report, sample_workload, sim_for, sweep_table,
    workload_line, workload_summary,
};
use citadel::bench::profile::run_profile;

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let args = match parse_args(&argv) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("citadel-bench: {e}");
            eprintln!("usage: citadel-bench --backend null|supersolid \\");
            eprintln!("       [--building-count N | --sweep 60,120,240] \\");
            eprintln!("       [--frames N] [--warmup N] [--seed N] [--format table|json] [--dhat] [--profile]");
            std::process::exit(2);
        }
    };

    if args.profile {
        let cap = args.caps.first().copied().unwrap_or(citadel::sim::CitadelConfig::default().building_count);
        let cfg = sim_for(cap, args.seed);
        run_profile(cfg, args.frames, args.warmup);
        return;
    }

    if args.dhat {
        #[cfg(feature = "dhat-prof")]
        {
            let _profiler = dhat::Profiler::new_heap();
            for &cap in &args.caps {
                let cfg = sim_for(cap, args.seed);
                let r = citadel::bench::run_alloc(args.backend, cfg, args.frames, args.warmup);
                print!("{}", alloc_table(&r));
            }
        }
        #[cfg(not(feature = "dhat-prof"))]
        eprintln!("citadel-bench: --dhat requires building with --features bench,dhat-prof");
        return;
    }

    let mut reports = Vec::new();
    let mut sweep_workloads = Vec::new();
    for &cap in &args.caps {
        let cfg = sim_for(cap, args.seed);
        // Live element counts (what drives UI node/render cost) — backend-independent.
        let workload = sample_workload(cfg.clone(), args.frames, args.warmup);
        let report = run_report(args.backend, cfg, args.frames, args.warmup);
        if args.json {
            println!("{}", report_json(&report));
        } else if args.caps.len() == 1 {
            print!("{}", report_table(&report));
            print!("{}", workload_line(&workload));
        }
        reports.push(report);
        sweep_workloads.push((cap, workload));
    }
    if !args.json && args.caps.len() > 1 {
        print!("{}", sweep_table(&reports));
        println!("workload per cap (live element counts):");
        for (cap, w) in &sweep_workloads {
            println!("  cap {:>4}: {}", cap, workload_summary(w));
        }
    }
}
