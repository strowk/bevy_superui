//! Pre-transpile the styling_showcase's `.tsx` to `.superui/build/*.js` for wasm / no-HMR
//! native builds. Build scripts run on the HOST, so `oxc` never enters the wasm
//! binary. Skips itself under `--features hmr` (that build loads live `.tsx`).
fn main() {
    supersolid::build::transpile_dir("assets/ui/styling_showcase");
}
