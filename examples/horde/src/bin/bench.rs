#[cfg(feature = "dhat-prof")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

#[cfg(feature = "dhat-prof")]
use horde::bench::alloc_table;
use horde::bench::{parse_args, report_json, report_table, run_report, sim_for, sweep_table};

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
    for &cap in &args.caps {
        let sim = sim_for(&args.preset, cap, args.seed);
        let report = run_report(args.backend, sim, args.frames, args.warmup);
        if args.json {
            println!("{}", report_json(&report));
        } else if args.caps.len() == 1 {
            print!("{}", report_table(&report));
        }
        reports.push(report);
    }
    if !args.json && args.caps.len() > 1 {
        print!("{}", sweep_table(&reports));
    }
}
