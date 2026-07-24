//! Build-script helper: pre-transpile the `.tsx`/`.ts` entries in a UI directory
//! to `<dir>/.superui/build/<stem>.js` for wasm / no-HMR native builds. Runs on
//! the HOST, so `oxc` never enters the wasm binary.

use std::path::Path;

/// Transpile every top-level `.tsx`/`.ts` file in `ui_dir` to its generated `.js`
/// under `<ui_dir>/.superui/build/`. Intended to be the whole body of a `build.rs`.
///
/// Skips work entirely when `CARGO_FEATURE_HMR` is set: those builds load the live
/// `.tsx` through the transpiling asset loader, so the artifact is unused.
pub fn transpile_dir(ui_dir: &str) {
    transpile_dir_impl(ui_dir, std::env::var_os("CARGO_FEATURE_HMR").is_some());
}

fn transpile_dir_impl(ui_dir: &str, skip: bool) {
    if skip {
        return;
    }
    let entries = match std::fs::read_dir(ui_dir) {
        Ok(e) => e,
        Err(e) => {
            println!("cargo:warning=supersolid: cannot read {ui_dir}: {e}");
            return;
        }
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !(name.ends_with(".tsx") || name.ends_with(".ts")) {
            continue;
        }
        let src = format!("{ui_dir}/{name}");
        let out = superui_paths::generated_js(&src);
        let _ = std::fs::create_dir_all(superui_paths::parent_dir(&out));
        println!("cargo:rerun-if-changed={src}");
        match crate::transpile_file(Path::new(&src), Path::new(&out)) {
            Ok(result) => {
                for d in &result.diagnostics {
                    println!("cargo:warning=supersolid: {}", d.message);
                }
            }
            Err(e) => println!("cargo:warning=supersolid: could not transpile {src}: {e}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::transpile_dir_impl;
    use std::path::PathBuf;

    // Isolated temp UI dir under the target dir. No Date/rand (unavailable here in
    // some sandboxes anyway) — key the name off the test name.
    fn temp_ui_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("superui_build_test_{tag}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn writes_generated_js_for_each_tsx() {
        let dir = temp_ui_dir("writes");
        std::fs::write(dir.join("app.tsx"), "const a = <div class=\"x\"/>;").unwrap();
        let dir_str = dir.to_string_lossy().replace('\\', "/");

        transpile_dir_impl(&dir_str, false);

        let out = dir.join(".superui").join("build").join("app.js");
        assert!(out.exists(), "expected generated {out:?}");
        let js = std::fs::read_to_string(out).unwrap();
        assert!(js.contains("$ss.el(\"div\")"), "JSX must be lowered:\n{js}");
    }

    #[test]
    fn skip_flag_writes_nothing() {
        let dir = temp_ui_dir("skip");
        std::fs::write(dir.join("app.tsx"), "const a = 1;").unwrap();
        let dir_str = dir.to_string_lossy().replace('\\', "/");

        transpile_dir_impl(&dir_str, true);

        assert!(!dir.join(".superui").exists(), "skip=true must not transpile");
    }
}
