#[cfg(feature = "dhat-prof")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

#[cfg(feature = "dhat-prof")]
use horde::bench::alloc_table;
use horde::bench::{
    parse_args, report_json, report_table, run_report, sample_workload, sim_for, sweep_table,
    workload_line, workload_summary,
};

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let args = match parse_args(&argv) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("horde-bench: {e}");
            eprintln!("usage: horde-bench --backend null|native|supersolid \\");
            eprintln!("       [--preset play|stress] [--enemy-cap N | --sweep 60,200,400] \\");
            eprintln!("       [--frames N] [--warmup N] [--seed N] [--format table|json] [--dhat]");
            std::process::exit(2);
        }
    };

    if args.dhat {
        #[cfg(feature = "dhat-prof")]
        {
            let _profiler = dhat::Profiler::new_heap();
            for &cap in &args.caps {
                let sim = sim_for(&args.preset, cap, args.seed);
                let r = horde::bench::run_alloc(args.backend, sim, args.frames, args.warmup);
                print!("{}", alloc_table(&r));
            }
        }
        #[cfg(not(feature = "dhat-prof"))]
        eprintln!("horde-bench: --dhat requires building with --features bench,dhat-prof");
        return;
    }

    let mut reports = Vec::new();
    let mut sweep_workloads = Vec::new();
    for &cap in &args.caps {
        let sim = sim_for(&args.preset, cap, args.seed);
        // Live element counts (what drives UI node/render cost) — backend-independent.
        let workload = sample_workload(sim.clone(), args.frames, args.warmup);
        let report = run_report(args.backend, sim, args.frames, args.warmup);
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
