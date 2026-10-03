//! `.json` imports in a spec must resolve relative to the spec file's own
//! directory and inline as a top-level `const` (see
//! `supersolid::json_binding`). A missing or invalid JSON file is a
//! test-authoring bug, so it must fail loudly (`Err`), not silently.

use std::path::PathBuf;

use superui_test_engine::transpile::transpile_spec;

fn temp_ui_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("superui_json_import_{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn inlines_json_import_relative_to_spec_dir() {
    let dir = temp_ui_dir("inlines");
    std::fs::write(dir.join("skills.json"), r#"[{"id":1}]"#).unwrap();

    let src = r#"import skills from "./skills.json"; const n = skills.length;"#;
    let js = transpile_spec(src, "t.spec.ts", &dir).unwrap();

    assert!(js.contains("const skills ="), "expected inlined const:\n{js}");
    assert!(
        js.replace(' ', "").contains(r#""id":1"#),
        "expected inlined value:\n{js}"
    );
    assert!(!js.contains("import"), "import statement must be stripped:\n{js}");
}

#[test]
fn missing_json_file_is_fatal() {
    let dir = temp_ui_dir("missing");

    let src = r#"import skills from "./missing.json"; const n = skills.length;"#;
    let result = transpile_spec(src, "t.spec.ts", &dir);

    assert!(result.is_err(), "expected Err for missing JSON file, got {result:?}");
}
