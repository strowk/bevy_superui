//! `supersolid` — transpiles Solid-style `.tsx`/`.ts` to plain JavaScript:
//! TypeScript type-stripping (via `oxc`) plus reactivity-aware element-walk JSX
//! lowering to the `$ss` runtime ABI. Bevy-free; the asset loader lives in
//! `superui` (native-only) so `oxc` never enters a wasm build (direction spec §11.3).

mod pipeline;

/// Options controlling a transpile.
#[derive(Debug, Clone)]
pub struct TranspileOptions {
    /// Import specifiers whose imports are stripped silently (their names are
    /// provided as runtime globals by Plans 3–4).
    pub runtime_specifiers: Vec<String>,
    /// Parse as `.tsx` (allow JSX) when true, `.ts` when false.
    pub tsx: bool,
}

impl Default for TranspileOptions {
    fn default() -> Self {
        TranspileOptions {
            runtime_specifiers: vec!["supersolid".into(), "solid-js".into()],
            tsx: true,
        }
    }
}

/// Diagnostic severity. `Error` is reserved for future hard failures; today the
/// transpiler is warn-only (graceful degradation, design §1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Warning,
    Error,
}

/// A single transpile diagnostic (human-readable; not a source span yet).
#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
}

/// The result of a transpile: emitted JS, diagnostics, and any co-located CSS
/// imports discovered (recorded for a later cascade-wiring plan).
#[derive(Debug, Clone, Default)]
pub struct TranspileResult {
    pub code: String,
    pub diagnostics: Vec<Diagnostic>,
    pub style_imports: Vec<String>,
}

/// Transpile Solid-style `.tsx`/`.ts` source to plain JavaScript.
pub fn transpile(source: &str, options: &TranspileOptions) -> TranspileResult {
    let (code, diagnostics, style_imports) = pipeline::run(source, options, /* lower_jsx */ false);
    TranspileResult { code, diagnostics, style_imports }
}

/// TEST HELPER: true iff `code` parses as plain (non-JSX) JavaScript with no
/// parser diagnostics. Proves both "valid JS" and "no JSX remains".
#[cfg(test)]
pub(crate) fn reparses_as_plain_js(code: &str) -> bool {
    use oxc::allocator::Allocator;
    use oxc::parser::Parser;
    use oxc::span::SourceType;
    let allocator = Allocator::default();
    // `.mjs`-style plain JS SourceType: NO JSX. If JSX survived, this errors.
    let source_type = SourceType::mjs();
    let ret = Parser::new(&allocator, code, source_type).parse();
    ret.diagnostics.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn code(src: &str) -> String {
        transpile(src, &TranspileOptions::default()).code
    }

    #[test]
    fn strips_typescript_types() {
        let out = code("const x: number = 1; function f(a: string): boolean { return !!a; }");
        assert!(!out.contains(": number"), "type annotation not stripped:\n{out}");
        assert!(!out.contains(": string"), "param type not stripped:\n{out}");
        assert!(!out.contains(": boolean"), "return type not stripped:\n{out}");
        assert!(out.contains("const x = 1"), "value kept:\n{out}");
        assert!(reparses_as_plain_js(&out), "output is not valid plain JS:\n{out}");
    }

    #[test]
    fn strips_type_only_import_and_interface() {
        let out = code("interface Foo { a: number } import type { Bar } from \"x\"; const y = 2;");
        assert!(!out.contains("interface"), "interface not stripped:\n{out}");
        assert!(!out.contains("import type"), "type import not stripped:\n{out}");
        assert!(out.contains("const y = 2"));
        assert!(reparses_as_plain_js(&out));
    }

    #[test]
    fn jsx_is_preserved_for_the_next_pass() {
        // Task 1 does NOT lower JSX; it must survive so Task 2 can transform it.
        // (This test is REPLACED in Task 2 once lowering lands.)
        let out = code("const a = <div/>;");
        assert!(out.contains("<div"), "JSX should be preserved in Task 1:\n{out}");
    }
}
