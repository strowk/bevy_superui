//! Pre-transpile the Supersolid menu app so wasm and no-HMR native builds have
//! plain `.js` to load. Build scripts compile for the HOST, so `supersolid`
//! (oxc) never enters the wasm binary. Cheap enough to run every build.

use std::path::PathBuf;

fn main() {
    let dir = PathBuf::from("assets/ui/game_menu");
    let input = dir.join("app.tsx");
    let output = dir.join("app.generated.js");
    println!("cargo:rerun-if-changed={}", input.display());
    match supersolid::transpile_file(&input, &output) {
        Ok(result) => {
            for d in &result.diagnostics {
                // Warn-only: never fail the build on a transpile diagnostic.
                println!("cargo:warning=supersolid: {}", d.message);
            }
        }
        Err(e) => {
            println!(
                "cargo:warning=supersolid: could not transpile {}: {e}",
                input.display()
            );
        }
    }
}
