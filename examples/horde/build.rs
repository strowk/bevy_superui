//! Pre-transpile the Supersolid app so wasm and no-HMR native builds have plain
//! `.js` to load. Build scripts compile for the HOST, so `supersolid` (oxc) never
//! enters the wasm binary.

use std::path::PathBuf;

fn main() {
    let dir = PathBuf::from("assets/ui/horde");
    let input = dir.join("app.tsx");
    let output = dir.join("app.generated.js");
    println!("cargo:rerun-if-changed={}", input.display());
    match supersolid::transpile_file(&input, &output) {
        Ok(result) => {
            for d in &result.diagnostics {
                println!("cargo:warning=supersolid: {}", d.message);
            }
        }
        Err(e) => {
            println!("cargo:warning=supersolid: could not transpile {}: {e}", input.display());
        }
    }
}
