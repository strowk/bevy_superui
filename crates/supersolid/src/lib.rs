//! `supersolid` — transpiles Solid-style `.tsx`/`.ts` to plain JavaScript:
//! TypeScript type-stripping (via `oxc`) plus reactivity-aware element-walk JSX
//! lowering to the `$ss` runtime ABI. Bevy-free; the asset loader lives in
//! `superui` (native-only) so `oxc` never enters a wasm build (direction spec §11.3).

mod jsx;
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
    let (code, diagnostics, style_imports) = pipeline::run(source, options, /* lower_jsx */ true);
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
    fn empty_element_lowers_to_bare_create() {
        let out = code("const a = <div/>;");
        assert!(out.contains(r#"$ss.el("div")"#), "expected $ss.el:\n{out}");
        assert!(reparses_as_plain_js(&out), "not valid plain JS / JSX left:\n{out}");
    }

    #[test]
    fn element_with_static_attrs_lowers_to_attr_calls() {
        let out = code(r#"const a = <div class="box" id="x"/>;"#);
        assert!(out.contains(r#"$ss.el("div")"#), "{out}");
        assert!(out.contains(r#"$ss.attr("#), "{out}");
        assert!(out.contains(r#""class", "box""#), "{out}");
        assert!(out.contains(r#""id", "x""#), "{out}");
        assert!(reparses_as_plain_js(&out), "{out}");
    }

    #[test]
    fn static_text_child_lowers_to_txt_and_child() {
        let out = code("const a = <div>hello</div>;");
        assert!(out.contains(r#"$ss.el("div")"#), "{out}");
        assert!(out.contains(r#"$ss.txt("hello")"#), "{out}");
        assert!(out.contains("$ss.child("), "{out}");
        assert!(reparses_as_plain_js(&out), "{out}");
    }

    #[test]
    fn nested_element_child_lowers_recursively() {
        let out = code("const a = <div><span/></div>;");
        assert!(out.contains(r#"$ss.el("div")"#), "{out}");
        assert!(out.contains(r#"$ss.el("span")"#), "{out}");
        assert!(out.contains("$ss.child("), "parent must append child:\n{out}");
        assert!(reparses_as_plain_js(&out), "{out}");
    }
}
