use supersolid::{walk_sync, TranspileOptions};

/// Transpile a spec file's import graph: `module_id` is the entry (its source
/// is `source`, held in memory, not read from disk — the spec file need not
/// exist as a file at all); any `./`-relative deps (helpers, `.css`, `.json`)
/// are read from `base_dir`. Only a genuinely unresolved module import is
/// fatal here — a spec/helper whose source merely *lowers* with a warning
/// (`export * from`, dynamic `import()`, a dep `walk_sync` warns about for
/// some other non-resolution reason) still returns `Ok`, matching how those
/// same constructs degrade everywhere else (the loader/build paths). Missing
/// or invalid `.css`/`.json` also warn-and-skip now, for the same reason.
pub fn transpile_spec(source: &str, module_id: &str, base_dir: &std::path::Path) -> Result<String, String> {
    let options_for = |id: &str| TranspileOptions {
        runtime_specifiers: vec![
            "supersolid".into(),
            "solid-js".into(),
            "superui/test".into(),
        ],
        tsx: !id.ends_with(".ts"),
        module_id: Some(id.to_string()),
    };
    let read = |id: &str| -> std::io::Result<Vec<u8>> {
        if id == module_id {
            Ok(source.as_bytes().to_vec())
        } else {
            std::fs::read(base_dir.join(id))
        }
    };
    let bundle = walk_sync(module_id, options_for, read);

    // `walk_sync` emits a `superui:unresolved:<spec>` sentinel edge (visible
    // in the assembled code) for every dep no candidate `read` could resolve
    // — that, and only that, is "a module import could not be resolved" (a
    // test-authoring bug). Any other diagnostic (export*, dynamic import,
    // missing css/json) is a warn-and-skip everywhere else and must not trip
    // this gate.
    if bundle.code.contains("superui:unresolved:") {
        let messages: Vec<String> = bundle.diagnostics.iter().map(|d| d.message.clone()).collect();
        return Err(format!(
            "unresolved import in {module_id}: {}",
            messages.join("; ")
        ));
    }
    // Fatal only if codegen produced nothing from non-empty input.
    if bundle.code.trim().is_empty() && !source.trim().is_empty() {
        return Err(format!("transpile produced empty output for {module_id}"));
    }

    Ok(bundle.code)
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

    #[test]
    fn spec_bundles_a_helper_import() {
        let dir = std::env::temp_dir().join("superui_spec_helper_bundles_a_helper_import");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("helpers.ts"),
            r#"export function expectThing() { return 1; }"#,
        )
        .unwrap();
        let src = r#"
            import { expectThing } from "./helpers";
            import { test } from "superui/test";
            test("x", async () => { expectThing(); });
        "#;
        let js = transpile_spec(src, "t.spec.ts", &dir).unwrap();
        assert!(js.contains(r#"$ssmod.def("t.spec.ts""#), "entry defined: {js}");
        assert!(js.contains(r#"$ssmod.def("helpers.ts""#), "helper defined: {js}");
        assert!(js.contains(r#"$ssmod.run("t.spec.ts")"#), "entry run: {js}");
    }

    #[test]
    fn spec_missing_import_is_fatal() {
        assert!(transpile_spec(
            r#"import {x} from "./gone"; x;"#,
            "t.spec.ts",
            std::path::Path::new(".")
        )
        .is_err());
    }

    /// `export * from` is a lowering warning (skipped, not resolved) — see
    /// `supersolid::module::export_star_warns_and_skips` — not an unresolved
    /// import, even though the specifier it names exists on disk. Pins that
    /// the fatal gate keys off the unresolved-sentinel, not "any diagnostic".
    #[test]
    fn spec_export_star_warns_but_not_fatal() {
        let dir = std::env::temp_dir().join("superui_spec_export_star_warns_but_not_fatal");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("helper.ts"), r#"export const thing = 1;"#).unwrap();
        let src = r#"export * from "./helper";"#;
        let js = transpile_spec(src, "t.spec.ts", &dir).unwrap();
        assert!(js.contains(r#"$ssmod.def("t.spec.ts""#), "entry defined: {js}");
    }
}
