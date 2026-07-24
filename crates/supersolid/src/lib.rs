//! `supersolid` — transpiles Solid-style `.tsx`/`.ts` to plain JavaScript:
//! TypeScript type-stripping (via `oxc`) plus reactivity-aware element-walk JSX
//! lowering to the `$ss` runtime ABI. Bevy-free; the asset loader lives in
//! `superui` (native-only) so `oxc` never enters a wasm build (direction spec §11.3).

mod imports;
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
    /// Module id baked into component HMR registrations (`"<module_id>#<Name>"`);
    /// the native loader / CLI supply the asset path. `None` => `"#<Name>"`.
    pub module_id: Option<String>,
}

impl Default for TranspileOptions {
    fn default() -> Self {
        TranspileOptions {
            runtime_specifiers: vec!["supersolid".into(), "solid-js".into()],
            tsx: true,
            module_id: None,
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

/// Transpile one `.tsx`/`.ts` file to `output` (plain JS). Used by the CLI for
/// the wasm build-time pre-transpile path (direction spec §11.3).
pub fn transpile_file(input: &std::path::Path, output: &std::path::Path) -> std::io::Result<TranspileResult> {
    let src = std::fs::read_to_string(input)?;
    let tsx = input.extension().and_then(|e| e.to_str()) != Some("ts");
    let module_id = Some(input.to_string_lossy().into_owned());
    let result = transpile(&src, &TranspileOptions { tsx, module_id, ..Default::default() });
    std::fs::write(output, &result.code)?;
    Ok(result)
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
        assert!(!out.contains("$ss.bind") && !out.contains("onclick"),
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

    #[test]
    fn component_handler_is_plain_prop_not_event() {
        let out = code("const a = <Btn onClick={h}/>;");
        assert!(out.contains("$ss.cmp(Btn"), "{out}");
        assert!(
            out.contains("onClick: h") || out.contains("onClick:h"),
            "handler is a plain prop:\n{out}"
        );
        assert!(!out.contains("$ss.on"), "component handler must NOT become a DOM listener:\n{out}");
        assert!(reparses_as_plain_js(&out), "{out}");
    }

    #[test]
    fn component_child_inside_element_lowers_not_dropped() {
        let out = code("const a = <div><Counter/></div>;");
        assert!(out.contains(r#"$ss.el("div")"#), "{out}");
        assert!(out.contains("$ss.cmp(Counter"), "nested component child must lower, not drop:\n{out}");
        // A component's return value is opaque (may be an accessor), so it must be
        // inserted (resolved reactively), not statically appended via $ss.child.
        assert!(out.contains("$ss.insert("), "component child must be inserted:\n{out}");
        assert!(reparses_as_plain_js(&out), "{out}");
    }

    #[test]
    fn bare_control_flow_child_is_inserted() {
        // <For> directly inside <ul> (no {…} wrapper) must resolve its accessor
        // via $ss.insert, not be dropped by $ss.child.
        let out = code("const a = <ul><For each={items()}>{f}</For></ul>;");
        assert!(out.contains("$ss.cmp(For"), "For must lower to a component call:\n{out}");
        assert!(out.contains("$ss.insert("), "bare control-flow child must be inserted:\n{out}");
        assert!(!out.contains("$ss.child(_el0, $ss.cmp"),
            "component child must NOT be statically appended:\n{out}");
        assert!(reparses_as_plain_js(&out), "{out}");
    }

    #[test]
    fn fragment_child_of_element_is_inserted() {
        let out = code("const a = <div><><span/><em/></></div>;");
        assert!(out.contains(r#"$ss.el("div")"#), "{out}");
        // The fragment must survive as a $ss.frag routed through insert (not dropped).
        assert!(out.contains("$ss.frag("), "fragment child must be lowered, not dropped:\n{out}");
        assert!(out.contains("$ss.insert("), "fragment child inserted around anchor:\n{out}");
        assert!(out.contains(r#"$ss.el("span")"#) && out.contains(r#"$ss.el("em")"#), "{out}");
        assert!(reparses_as_plain_js(&out), "{out}");
    }

    #[test]
    fn runtime_imports_are_stripped_silently() {
        let r = transpile(
            "import { createSignal } from \"solid-js\"; const [a, b] = createSignal(0);",
            &TranspileOptions::default(),
        );
        assert!(!r.code.contains("import"), "runtime import must be stripped:\n{}", r.code);
        assert!(r.code.contains("createSignal(0)"), "usage kept:\n{}", r.code);
        assert!(r.diagnostics.is_empty(), "runtime import must not warn: {:?}", r.diagnostics);
        assert!(reparses_as_plain_js(&r.code), "{}", r.code);
    }

    #[test]
    fn css_imports_are_recorded_not_warned() {
        let r = transpile("import \"./todo.css\"; const x = 1;", &TranspileOptions::default());
        assert!(!r.code.contains("import"), "css import stripped from JS:\n{}", r.code);
        assert_eq!(r.style_imports, vec!["./todo.css".to_string()]);
        assert!(r.diagnostics.is_empty(), "css import must not warn: {:?}", r.diagnostics);
        assert!(reparses_as_plain_js(&r.code));
    }

    #[test]
    fn unknown_module_imports_warn() {
        let r = transpile("import { X } from \"./other\"; const x = X;", &TranspileOptions::default());
        assert!(!r.code.contains("import"), "unknown import still stripped:\n{}", r.code);
        assert_eq!(r.diagnostics.len(), 1, "one warning expected: {:?}", r.diagnostics);
        assert_eq!(r.diagnostics[0].severity, Severity::Warning);
        assert!(r.diagnostics[0].message.contains("./other"), "names specifier: {:?}", r.diagnostics);
        assert!(reparses_as_plain_js(&r.code));
    }

    #[test]
    fn whitespace_only_text_between_elements_is_skipped() {
        let out = code("const a = <div>\n  <span/>\n</div>;");
        assert!(!out.contains("$ss.txt"), "whitespace-only text must not become a text node:\n{out}");
        assert!(out.contains(r#"$ss.el("span")"#), "{out}");
        assert!(reparses_as_plain_js(&out), "{out}");
    }

    #[test]
    fn inline_whitespace_around_expression_is_preserved() {
        // `clicked {count()} times` must render "clicked 0 times", not
        // "clicked0times" — the single spaces flanking the expression are
        // meaningful and must survive transpilation (standard JSX behavior).
        let out = code("const a = <button>\n  clicked {count()} times\n</button>;");
        assert!(out.contains(r#"$ss.txt("clicked ")"#), "leading text must keep its trailing space:\n{out}");
        assert!(out.contains(r#"$ss.txt(" times")"#), "trailing text must keep its leading space:\n{out}");
        assert!(reparses_as_plain_js(&out), "{out}");
    }

    #[test]
    fn newline_adjacent_indentation_is_trimmed() {
        // A text child that is only indentation + a word + trailing newline
        // collapses to the bare word: newline-adjacent whitespace is dropped.
        let out = code("const a = <div>\n  hello\n</div>;");
        assert!(out.contains(r#"$ss.txt("hello")"#), "newline-adjacent indentation must be trimmed:\n{out}");
        assert!(reparses_as_plain_js(&out), "{out}");
    }

    #[test]
    fn space_between_two_expressions_is_preserved() {
        // `{a} {b}` — the lone space between two holes is meaningful text.
        let out = code("const a = <div>{a} {b}</div>;");
        assert!(out.contains(r#"$ss.txt(" ")"#), "single space between expressions must be preserved:\n{out}");
        assert!(reparses_as_plain_js(&out), "{out}");
    }

    #[test]
    fn literal_expression_child_becomes_static_text() {
        let out = code("const a = <div>{42}</div>;");
        assert!(out.contains(r#"$ss.txt("42")"#), "literal child stringified to static text:\n{out}");
        assert!(!out.contains("$ss.insert"), "a literal child must NOT be a dynamic insert:\n{out}");
        assert!(reparses_as_plain_js(&out), "{out}");
    }

    #[test]
    fn top_level_function_component_is_registered() {
        let out = code("function Counter(){ return <div/>; } render(() => <Counter/>, root);");
        assert!(out.contains(r##"$ss.hot("#Counter", Counter)"##), "component registered:\n{out}");
        assert!(reparses_as_plain_js(&out), "{out}");
    }

    #[test]
    fn const_arrow_component_is_registered() {
        let out = code("const Item = () => <li/>;");
        assert!(out.contains(r##"$ss.hot("#Item", Item)"##), "const-arrow registered:\n{out}");
        assert!(reparses_as_plain_js(&out), "{out}");
    }

    #[test]
    fn module_id_qualifies_the_hot_id() {
        let r = transpile(
            "function App(){ return <div/>; }",
            &TranspileOptions { module_id: Some("app.tsx".into()), ..Default::default() },
        );
        assert!(r.code.contains(r#"$ss.hot("app.tsx#App", App)"#), "path-qualified id:\n{}", r.code);
        assert!(reparses_as_plain_js(&r.code), "{}", r.code);
    }

    #[test]
    fn lowercase_and_non_function_bindings_are_not_registered() {
        let out = code("function helper(){ return 1; } const value = 2; const Config = 3;");
        assert!(!out.contains("$ss.hot"), "no registration for non-components:\n{out}");
        assert!(reparses_as_plain_js(&out), "{out}");
    }
}
