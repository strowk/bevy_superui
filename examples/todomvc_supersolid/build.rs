//! Pre-transpile the Supersolid app so wasm and no-HMR native builds have plain
//! `.js` to load (direction spec §11.3). Build scripts + their deps compile for
//! the HOST, so `supersolid` (oxc) never enters the wasm binary. Runs on every
//! build (transpiling one file is cheap) to keep the output fresh and present.

use std::path::PathBuf;

fn main() {
    let dir = PathBuf::from("assets/ui/todomvc_supersolid");
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
            // Missing input etc. — surface as a warning, don't hard-fail the build.
            println!("cargo:warning=supersolid: could not transpile {}: {e}", input.display());
        }
    }
}
