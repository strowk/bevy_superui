//! Build-script helper: pre-transpile the `.tsx`/`.ts` entries in a UI directory
//! to `<dir>/.superui/build/<stem>.js` for wasm / no-HMR native builds. Runs on
//! the HOST, so `oxc` never enters the wasm binary.

use crate::TranspileOptions;
use std::cell::RefCell;

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

        // `walk_sync` doesn't surface css/json paths directly (its signature is
        // shared across other callers) — the read closure records every path it
        // successfully served, which is exactly entry + every resolved module +
        // every css/json file. Failed `resolve_candidates` probes (nonexistent
        // paths tried during resolution) must NOT be recorded: rerun-if-changed
        // on a file that doesn't exist causes spurious rebuilds.
        let served: RefCell<Vec<String>> = RefCell::new(Vec::new());
        let read = |id: &str| -> std::io::Result<Vec<u8>> {
            let bytes = std::fs::read(id)?;
            served.borrow_mut().push(id.to_string());
            Ok(bytes)
        };
        let options_for = |id: &str| TranspileOptions {
            tsx: !id.ends_with(".ts"),
            module_id: Some(id.to_string()),
            ..Default::default()
        };

        let bundle = crate::walk_sync(&src, options_for, read);

        for d in &bundle.diagnostics {
            println!("cargo:warning=supersolid: {}", d.message);
        }

        let mut seen = std::collections::HashSet::new();
        for path in served.borrow().iter() {
            if seen.insert(path.clone()) {
                println!("cargo:rerun-if-changed={path}");
            }
        }

        if let Err(e) = std::fs::write(&out, &bundle.code) {
            println!("cargo:warning=supersolid: could not write {out}: {e}");
        }

        // Persist the module-graph CSS to a sidecar the shipped (no-HMR) loader
        // reads back and merges (`superui::JsLoader` + `mount.rs`). Without this
        // the CSS collected above is dropped and the release build renders
        // unstyled. Remove a stale sidecar when the graph no longer has any CSS.
        let css_out = superui_paths::generated_css(&src);
        if bundle.css.is_empty() {
            let _ = std::fs::remove_file(&css_out);
        } else if let Err(e) = std::fs::write(&css_out, &bundle.css) {
            println!("cargo:warning=supersolid: could not write {css_out}: {e}");
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
    fn inlines_json_import_as_const() {
        let dir = temp_ui_dir("json_import");
        std::fs::write(dir.join("data.json"), r#"{"k":5}"#).unwrap();
        std::fs::write(dir.join("app.tsx"), "import data from \"./data.json\"; const a = data.k;").unwrap();
        let dir_str = dir.to_string_lossy().replace('\\', "/");

        transpile_dir_impl(&dir_str, false);

        let out = dir.join(".superui").join("build").join("app.js");
        let js = std::fs::read_to_string(out).unwrap();
        assert!(
            js.contains("const data = {\"k\":5};") || js.contains("const data = {\"k\": 5};"),
            "expected inlined json const:\n{js}"
        );
    }

    #[test]
    fn bundles_a_two_file_graph_for_the_entry() {
        let dir = temp_ui_dir("graph");
        std::fs::write(dir.join("label.tsx"), r#"export function Label(){ return <span/>; }"#).unwrap();
        std::fs::write(
            dir.join("app.tsx"),
            r#"import { Label } from "./label.tsx"; render(() => <Label/>, root);"#,
        )
        .unwrap();
        let dir_str = dir.to_string_lossy().replace('\\', "/");
        transpile_dir_impl(&dir_str, false);
        let js = std::fs::read_to_string(dir.join(".superui").join("build").join("app.js")).unwrap();
        assert!(
            js.contains(r#"$ssmod.def("#) && js.contains("$ss.el(\"span\")"),
            "entry artifact must bundle the imported module:\n{js}"
        );
    }

    #[test]
    fn writes_sidecar_css_for_module_imports() {
        // A `.css` imported anywhere in the graph must be persisted next to the
        // generated `.js` so the shipped (no-HMR) path can apply it — otherwise
        // module CSS is silently dropped in release builds.
        let dir = temp_ui_dir("css_sidecar");
        std::fs::write(dir.join("theme.css"), ".c { width: 10px; }").unwrap();
        std::fs::write(
            dir.join("app.tsx"),
            "import \"./theme.css\"; const a = <div class=\"c\"/>;",
        )
        .unwrap();
        let dir_str = dir.to_string_lossy().replace('\\', "/");

        transpile_dir_impl(&dir_str, false);

        let css = dir.join(".superui").join("build").join("app.css");
        assert!(css.exists(), "expected generated sidecar {css:?}");
        let css_text = std::fs::read_to_string(css).unwrap();
        assert!(
            css_text.contains(".c") && css_text.contains("10px"),
            "sidecar must carry the module's CSS:\n{css_text}"
        );
    }

    #[test]
    fn no_sidecar_css_when_no_module_imports() {
        // No module `.css` in the graph → no stale sidecar left behind.
        let dir = temp_ui_dir("css_no_sidecar");
        std::fs::write(dir.join("app.tsx"), "const a = <div/>;").unwrap();
        let dir_str = dir.to_string_lossy().replace('\\', "/");

        transpile_dir_impl(&dir_str, false);

        let css = dir.join(".superui").join("build").join("app.css");
        assert!(!css.exists(), "no module CSS must leave no sidecar, found {css:?}");
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
