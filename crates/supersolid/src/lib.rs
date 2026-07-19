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

    #[test]
    fn dynamic_child_expression_is_thunked_via_insert() {
        let out = code("const a = <div>{count()}</div>;");
        assert!(out.contains("$ss.insert("), "{out}");
        // Must be a thunk (lazy), NOT eager `count()` passed directly.
        assert!(out.contains("() =>") && out.contains("count()"), "child must be thunked:\n{out}");
        assert!(!out.contains("$ss.child(_el0, count())") && !out.contains("$ss.txt(count"),
            "dynamic child must not be eagerly evaluated:\n{out}");
        assert!(reparses_as_plain_js(&out), "{out}");
    }

    #[test]
    fn dynamic_attribute_is_thunked_via_bind() {
        let out = code("const a = <div class={cls()}/>;");
        assert!(out.contains("$ss.bind("), "{out}");
        assert!(out.contains(r#""class""#), "{out}");
        assert!(out.contains("() =>") && out.contains("cls()"), "attr must be thunked:\n{out}");
        assert!(reparses_as_plain_js(&out), "{out}");
    }

    #[test]
    fn literal_expression_attribute_stays_static() {
        let out = code("const a = <input tabindex={0} value={\"hi\"}/>;");
        assert!(out.contains("$ss.attr("), "literal attrs are static:\n{out}");
        assert!(out.contains(r#""tabindex", "0""#), "numeric literal stringified:\n{out}");
        assert!(out.contains(r#""value", "hi""#), "string literal kept:\n{out}");
        assert!(!out.contains("$ss.bind("), "no dynamic binding for literals:\n{out}");
        assert!(reparses_as_plain_js(&out), "{out}");
    }

    #[test]
    fn on_click_lowers_to_event_listener() {
        let out = code("const a = <button onClick={handler}>x</button>;");
        assert!(out.contains("$ss.on("), "{out}");
        assert!(out.contains(r#""click""#), "event name normalized:\n{out}");
        assert!(out.contains("handler"), "{out}");
        assert!(!out.contains(r#"$ss.bind("_el"#) || !out.contains("onclick"),
            "onClick must not become an attribute:\n{out}");
        assert!(reparses_as_plain_js(&out), "{out}");
    }

    #[test]
    fn on_input_normalizes_event_name() {
        let out = code("const a = <input onInput={e => f(e)}/>;");
        assert!(out.contains("$ss.on("), "{out}");
        assert!(out.contains(r#""input""#), "{out}");
        assert!(reparses_as_plain_js(&out), "{out}");
    }

    #[test]
    fn capitalized_tag_lowers_to_component() {
        let out = code("const a = <Counter/>;");
        assert!(out.contains("$ss.cmp(Counter"), "component call:\n{out}");
        assert!(reparses_as_plain_js(&out), "{out}");
    }

    #[test]
    fn component_static_and_dynamic_props() {
        let out = code("const a = <Counter start={5} label=\"hi\" n={x()}/>;");
        assert!(out.contains("$ss.cmp(Counter"), "{out}");
        assert!(out.contains("start: 5"), "static numeric prop kept as JS value:\n{out}");
        assert!(out.contains(r#"label: "hi""#), "static string prop:\n{out}");
        assert!(out.contains("get n()") && out.contains("x()"), "dynamic prop is a getter:\n{out}");
        assert!(reparses_as_plain_js(&out), "{out}");
    }

    #[test]
    fn component_children_become_children_getter() {
        let out = code("const a = <Box>{kid}</Box>;");
        assert!(out.contains("$ss.cmp(Box"), "{out}");
        assert!(out.contains("get children()"), "children passed as getter:\n{out}");
        assert!(reparses_as_plain_js(&out), "{out}");
    }

    #[test]
    fn fragment_lowers_to_frag_array() {
        let out = code("const a = <><A/><B/></>;");
        assert!(out.contains("$ss.frag(["), "{out}");
        assert!(out.contains("$ss.cmp(A") && out.contains("$ss.cmp(B"), "{out}");
        assert!(reparses_as_plain_js(&out), "{out}");
    }
}
