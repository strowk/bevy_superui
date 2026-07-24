//! Pre-transpile this example's `.tsx` to `.superui/build/*.js` for wasm / no-HMR
//! native builds. Runs on the HOST; skips under `--features hmr`.
fn main() {
    supersolid::build::transpile_dir("assets/ui/citadel");
}
