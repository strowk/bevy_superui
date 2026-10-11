//! Per-module ESM → CommonJS-factory-body lowering (pure, I/O-free; see
//! `crate::pipeline::run_module` for the parse → lower → codegen pipeline).
//!
//! Unlike `transpile`/`imports::rewrite` (which strip every import because the
//! engine runs plain, non-module scripts), `transpile_module` resolves every
//! non-runtime import to a `require(...)` binding and every `export` to an
//! `exports.*` assignment, so the result can be dropped verbatim inside a
//! registry factory body (spec §2). Purely additive: `transpile`'s strip-path
//! is untouched.

use crate::{Diagnostic, TranspileOptions};

/// One ESM module lowered to a CommonJS-style factory body.
#[derive(Debug, Clone, Default)]
pub struct Module {
    /// The module body only — lowered statements, no registry wrapper and no
    /// `function(...)` envelope — suitable to drop inside `assemble`'s factory.
    pub factory_code: String,
    /// Raw specifiers this module `require(...)`s (runtime/`.css`/`.json`
    /// imports excluded — those are reported via the fields below instead).
    pub deps: Vec<String>,
    /// `.css` import specifiers, in declaration order.
    pub css_imports: Vec<String>,
    /// `(local_binding_name, specifier)` for every `.json` import, in
    /// declaration order.
    pub json_imports: Vec<(String, String)>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Lower one ESM `.tsx`/`.ts` module to a factory body: JSX → `$ss.*`,
/// `import` → `require(...)`, `export` → `exports.*` (spec §2).
pub fn transpile_module(source: &str, options: &TranspileOptions) -> Module {
    let (factory_code, diagnostics, deps, css_imports, json_imports) =
        crate::pipeline::run_module(source, options);
    Module { factory_code, deps, css_imports, json_imports, diagnostics }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{reparses_as_plain_js, Severity, TranspileOptions};

    // Imports → require
    #[test]
    fn named_import_lowers_to_require_destructure() {
        let m = transpile_module(r#"import { a, b as c } from "./x";"#, &TranspileOptions::default());
        assert!(m.factory_code.contains(r#"const { a, b: c } = require("./x")"#), "{}", m.factory_code);
        assert_eq!(m.deps, vec!["./x".to_string()]);
        assert!(m.diagnostics.is_empty());
        assert!(reparses_as_plain_js(&m.factory_code));
    }
    #[test]
    fn default_import_lowers_to_require_default() {
        let m = transpile_module(r#"import d from "./x";"#, &TranspileOptions::default());
        assert!(m.factory_code.contains(r#"const d = require("./x").default"#), "{}", m.factory_code);
    }
    #[test]
    fn namespace_import_lowers_to_require() {
        let m = transpile_module(r#"import * as ns from "./x";"#, &TranspileOptions::default());
        assert!(m.factory_code.contains(r#"const ns = require("./x")"#), "{}", m.factory_code);
    }
    #[test]
    fn side_effect_import_lowers_to_bare_require() {
        let m = transpile_module(r#"import "./x";"#, &TranspileOptions::default());
        assert!(m.factory_code.contains(r#"require("./x")"#), "{}", m.factory_code);
        assert_eq!(m.deps, vec!["./x".to_string()]);
    }
    #[test]
    fn bare_specifier_is_a_dep_not_a_warning() {
        let m = transpile_module(r#"import { Btn } from "ui-kit";"#, &TranspileOptions::default());
        assert_eq!(m.deps, vec!["ui-kit".to_string()]);
        assert!(m.diagnostics.is_empty(), "bare imports resolve now; no warn: {:?}", m.diagnostics);
    }
    // Runtime / css / json classification unchanged
    #[test]
    fn runtime_import_still_strips_silently() {
        let m = transpile_module(
            r#"import { createSignal } from "solid-js"; createSignal(0);"#,
            &TranspileOptions::default(),
        );
        assert!(!m.factory_code.contains("require"), "{}", m.factory_code);
        assert!(m.deps.is_empty());
        assert!(m.diagnostics.is_empty());
    }
    #[test]
    fn css_import_recorded_and_stripped() {
        let m = transpile_module(r#"import "./a.css";"#, &TranspileOptions::default());
        assert_eq!(m.css_imports, vec!["./a.css".to_string()]);
        assert!(!m.factory_code.contains("require"), "css not a require: {}", m.factory_code);
    }
    #[test]
    fn json_import_recorded_and_stripped() {
        let m = transpile_module(r#"import data from "./d.json";"#, &TranspileOptions::default());
        assert_eq!(m.json_imports, vec![("data".into(), "./d.json".into())]);
    }
    // Exports → exports.*
    #[test]
    fn export_function_emits_decl_and_exports_binding() {
        let m = transpile_module("export function F(){ return 1; }", &TranspileOptions::default());
        assert!(m.factory_code.contains("function F"), "{}", m.factory_code);
        assert!(m.factory_code.contains("exports.F = F"), "{}", m.factory_code);
    }
    #[test]
    fn export_const_emits_exports_binding() {
        let m = transpile_module("export const x = 1;", &TranspileOptions::default());
        assert!(m.factory_code.contains("exports.x = x"), "{}", m.factory_code);
    }
    #[test]
    fn export_named_list_with_alias() {
        let m = transpile_module("const a=1,b=2; export { a, b as c };", &TranspileOptions::default());
        assert!(m.factory_code.contains("exports.a = a"), "{}", m.factory_code);
        assert!(m.factory_code.contains("exports.c = b"), "{}", m.factory_code);
    }
    #[test]
    fn export_default_emits_default_binding() {
        let m = transpile_module("export default 42;", &TranspileOptions::default());
        assert!(m.factory_code.contains("exports.default = 42"), "{}", m.factory_code);
    }
    #[test]
    fn exported_component_is_registered_and_exported() {
        let m = transpile_module(
            "export function Counter(){ return <div/>; }",
            &TranspileOptions { module_id: Some("counter.tsx".into()), ..Default::default() },
        );
        assert!(m.factory_code.contains(r#"$ss.hot("counter.tsx#Counter", Counter)"#), "{}", m.factory_code);
        assert!(m.factory_code.contains("exports.Counter = Counter"), "{}", m.factory_code);
    }
    #[test]
    fn export_star_warns_and_skips() {
        let m = transpile_module(r#"export * from "./barrel";"#, &TranspileOptions::default());
        assert_eq!(m.diagnostics.len(), 1);
        assert_eq!(m.diagnostics[0].severity, Severity::Warning);
        assert!(reparses_as_plain_js(&m.factory_code));
    }
    #[test]
    fn dynamic_import_warns_and_skips() {
        let m = transpile_module(r#"const p = import("./x");"#, &TranspileOptions::default());
        assert_eq!(m.diagnostics.iter().filter(|d| d.severity == Severity::Warning).count(), 1);
        assert!(reparses_as_plain_js(&m.factory_code));
    }
}
