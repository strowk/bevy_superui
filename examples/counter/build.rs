//! Pre-transpile the counter's `.tsx` to `.superui/build/*.js` for wasm / no-HMR
//! native builds. Build scripts run on the HOST, so `oxc` never enters the wasm
//! binary. Skips itself under `--features hmr` (that build loads live `.tsx`).
fn main() {
    supersolid::build::transpile_dir("assets/ui/counter");
}
