use std::fs;

#[test]
fn transpile_file_writes_plain_js_sibling() {
    let dir = std::env::temp_dir().join(format!("supersolid_cli_test_{}", std::process::id()));
    let _ = fs::create_dir_all(&dir);
    let input = dir.join("app.tsx");
    let output = dir.join("app.js");
    fs::write(&input, "const n: number = 1; const a = <div>{n}</div>;").unwrap();

    let result = supersolid::transpile_file(&input, &output).unwrap();

    let js = fs::read_to_string(&output).unwrap();
    assert!(!js.contains(": number"), "types stripped:\n{js}");
    assert!(js.contains(r#"$ss.el("div")"#), "JSX lowered:\n{js}");
    assert!(result.diagnostics.is_empty(), "no warnings expected: {:?}", result.diagnostics);
}
