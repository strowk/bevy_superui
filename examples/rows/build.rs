//! Pre-transpile this example's `.tsx` to `.superui/build/*.js` for the bench
//! (which loads assets from memory) and for wasm / no-HMR native builds.
fn main() {
    supersolid::build::transpile_dir("assets/ui/rows_solid");
}
