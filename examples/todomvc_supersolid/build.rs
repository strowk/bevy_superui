//! Pre-transpile this example's `.tsx` to `.superui/build/*.js` and generate the
//! class-utilities sheet for wasm / no-HMR native builds. Runs on the HOST; both
//! steps skip under `--features hmr` (that build uses the live asset-time path).
fn main() {
    const UI_DIR: &str = "assets/ui/todomvc_supersolid";

    supersolid::build::transpile_dir(UI_DIR);

    // Skip under HMR: the live `utilities` path regenerates the sheet at runtime.
    if std::env::var_os("CARGO_FEATURE_HMR").is_some() {
        return;
    }

    // Regenerate when any scanned source changes.
    if let Ok(entries) = std::fs::read_dir(UI_DIR) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.ends_with(".tsx") || name.ends_with(".ts") {
                println!("cargo:rerun-if-changed={UI_DIR}/{name}");
            }
        }
    }

    for d in superui_css_utilities::write_generated(UI_DIR) {
        println!(
            "cargo:warning=superui/utilities: dropped `{}` — {}",
            d.class, d.reason
        );
    }
}
