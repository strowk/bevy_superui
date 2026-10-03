use supersolid::{transpile, TranspileOptions};

/// Transpile a spec file's source, inlining any `.json` imports (resolved
/// relative to `base_dir`, the spec file's own directory) as top-level
/// `const` declarations. A missing or invalid JSON file is a
/// test-authoring bug, so it is fatal here, matching the existing
/// empty-output fatal check below.
pub fn transpile_spec(source: &str, module_id: &str, base_dir: &std::path::Path) -> Result<String, String> {
    let opts = TranspileOptions {
        runtime_specifiers: vec![
            "supersolid".into(),
            "solid-js".into(),
            "superui/test".into(),
        ],
        tsx: false,
        module_id: Some(module_id.to_string()),
    };
    let result = transpile(source, &opts);
    // Fatal only if codegen produced nothing from non-empty input.
    if result.code.trim().is_empty() && !source.trim().is_empty() {
        return Err(format!("transpile produced empty output for {module_id}"));
    }

    let mut prefix = String::new();
    for (binding, specifier) in &result.json_imports {
        let path = base_dir.join(specifier);
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("read {}: {e}", path.display()))?;
        prefix.push_str(&supersolid::json_binding(binding, &text)?);
    }

    Ok(format!("{prefix}{}", result.code))
}

#[cfg(test)]
mod tests {
    use super::transpile_spec;

    #[test]
    fn strips_superui_test_import_and_keeps_body() {
        let src = r#"
            import { test, expect } from "superui/test";
            test("x", async ({ page }) => {
                const n: number = 1;
                await expect(page.locator(".a")).toHaveCount(n);
            });
        "#;
        let js = transpile_spec(src, "x.spec.ts", std::path::Path::new(".")).unwrap();
        assert!(!js.contains("import"), "imports must be stripped: {js}");
        assert!(js.contains("test("), "body preserved: {js}");
        assert!(!js.contains(": number"), "TS types stripped: {js}");
    }
}
