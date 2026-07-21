use horde::bench::{parse_args, report_json, report_table, run_report, sim_for};

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

    for &cap in &args.caps {
        let sim = sim_for(&args.preset, cap, args.seed);
        let report = run_report(args.backend, sim, args.frames, args.warmup);
        if args.json {
            println!("{}", report_json(&report));
        } else {
            print!("{}", report_table(&report));
        }
    }
}
