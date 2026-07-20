//! Minimal build-time transpiler CLI: `supersolid <input.tsx> <output.js>`.
//! Transpiles one file so wasm builds can ship pre-transpiled `.js`
//! (direction spec §11.3). The cargo-metadata projector (§9) is a later plan.

use std::path::PathBuf;

fn main() -> std::io::Result<()> {
    let mut args = std::env::args().skip(1);
    let (Some(input), Some(output)) = (args.next(), args.next()) else {
        eprintln!("usage: supersolid <input.tsx|.ts> <output.js>");
        std::process::exit(2);
    };
    let result = supersolid::transpile_file(&PathBuf::from(input), &PathBuf::from(output))?;
    for d in &result.diagnostics {
        eprintln!("warning: {}", d.message);
    }
    Ok(())
}
