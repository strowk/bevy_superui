# Supersolid Phase 2 — Plan 2: `supersolid` transpiler (`.tsx`/`.ts` → JS) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a new `supersolid` crate that transpiles Solid-style `.tsx`/`.ts` to plain JavaScript — TypeScript type-stripping via `oxc`, plus our **own reactivity-aware JSX lowering** (element-walk) to a fixed runtime ABI — and wire it into the browser as a **native-only** Bevy `AssetLoader` producing the existing `JsSource`, with a build-time CLI for the wasm pre-transpile path.

**Architecture:** `supersolid::transpile(src, opts)` runs the verified oxc pipeline — `Parser` → `SemanticBuilder` → `Transformer` (TypeScript strip, **JSX preserved**) → **our `VisitMut` JSX-lowering pass** → `Codegen`. JSX lowers by **element-walk**: each element becomes a sequence of `$ss.*` runtime-helper calls (create/attr/child) with dynamic holes wrapped in thunks/getters (`$ss.insert`/`$ss.bind`/`$ss.on`, components via `$ss.cmp`, fragments via `$ss.frag`). The compiler emits references to a `$ss` runtime namespace it *defines here* but that Plans 3–4 implement; Plan 2 is therefore tested in isolation by (a) re-parsing the output as plain JS (proves valid JS + no JSX remains) and (b) asserting the presence of each helper call. The Bevy loader lives in `superui`, is `#[cfg(not(target_arch="wasm32"))]`, and depends on `supersolid` via a **target-gated dependency** so `oxc` never enters the wasm binary (spec §11.3).

**Tech Stack:** Rust, edition 2021, `oxc` 0.140 (umbrella crate: `parser`/`semantic`/`transformer`/`codegen`/`ast`/`ast_visit`/`allocator`/`span`), Bevy 0.17 (loader only, in `superui`), the existing `superui_*` crates. `supersolid` core is **Bevy-free**.

## Global Constraints

- **Bevy 0.17**, edition 2021.
- `supersolid`'s transpiler core is **Bevy-free**. Its Bevy-facing part is the loader, which lives in `superui` (an already-Bevy-facing crate) — this keeps `oxc` out of any wasm build (spec §11.3).
- **`oxc` must never be compiled into the `wasm32-unknown-unknown` app binary.** Enforce structurally: `superui` depends on `supersolid` only under `[target.'cfg(not(target_arch = "wasm32"))'.dependencies]`, and the `.tsx`/`.ts` loader is registered under `#[cfg(not(target_arch = "wasm32"))]`.
- **TDD** throughout; **frequent commits**; work on `main` (per project CLAUDE.md — no feature branch needed).
- **Graceful degradation (design §1):** a transpile error never panics the loader — diagnostics are logged and whatever `code` oxc produced (possibly empty) is returned as `JsSource`.
- The transpiler **defines** the `$ss` runtime ABI and the author-facing global names below; it does **not** implement them (Plans 3–4 do).

### Runtime ABI this plan emits (implemented later, in Plans 3–4)

Compiler-internal helpers (namespaced under a single injected global `$ss` to avoid author-scope clashes):

| Helper | Meaning |
|---|---|
| `$ss.el(tag)` | create an element node, return it |
| `$ss.txt(data)` | create a static text node, return it |
| `$ss.attr(el, name, value)` | set a **static** attribute (value already a JS string) |
| `$ss.child(parent, node)` | append a **static** child node |
| `$ss.insert(parent, thunk)` | **dynamic** child: `thunk` is `() => expr` (text / node / component / list) |
| `$ss.bind(el, name, thunk)` | **dynamic** attribute: `thunk` is `() => expr` |
| `$ss.on(el, type, handler)` | add an event listener (`handler` passed as-is, not thunked) |
| `$ss.cmp(Comp, props)` | instantiate a component with a props object (dynamic props are getters) |
| `$ss.frag(children)` | a fragment: `children` is an array |

Author-facing runtime globals (the compiler **strips their imports**; Plans 3–4 inject them as globals): `createSignal`, `createEffect`, `createMemo`, `onMount`, `onCleanup`, `createContext`, `useContext`, `render`, `Show`, `For`, `Index`, `Switch`, `Match`.

### Lowering rules (element-walk)

- **Tag case:** lowercase tag ⇒ element (`$ss.el`); Capitalized tag ⇒ component (`$ss.cmp`).
- **Element with no attrs/children:** emit the bare `$ss.el("tag")` expression.
- **Element with any attr/child:** emit an IIFE that creates a temp local, applies attrs/children in source order, and returns the local:
  `(() => { const _el = $ss.el("tag"); /* … */ return _el; })()`. Temp locals are globally unique (`_el0`, `_el1`, …) via a per-transpile counter.
- **Attributes:**
  - `name="literal"` or `name={"literal"}` / `name={123}` (string- or numeric-literal value) ⇒ **static** `$ss.attr(_el, "name", "<stringified>")`.
  - `onX={expr}` ⇒ `$ss.on(_el, "<x>", expr)` where `<x>` = `X` with the leading `on` removed and the remainder lowercased (`onClick`→`"click"`, `onInput`→`"input"`).
  - any other `name={expr}` ⇒ **dynamic** `$ss.bind(_el, "name", () => expr)`.
- **Children (in source order):**
  - static text ⇒ `$ss.child(_el, $ss.txt("text"))`.
  - a nested element ⇒ recurse to an expression, then `$ss.child(_el, <that expression>)`.
  - `{literal}` (string/number) ⇒ `$ss.child(_el, $ss.txt("<stringified>"))`.
  - any other `{expr}` (incl. a component element, ternary, `.map`, etc.) ⇒ `$ss.insert(_el, () => expr)`.
- **Components:** `<Comp a="x" n={y()} >kids</Comp>` ⇒ `$ss.cmp(Comp, { a: "x", get n() { return y(); }, get children() { return <lowered kids>; } })`. Component prop values are **JS values** (not stringified); static literal props are plain properties, dynamic props are **getters**; children (if any) become a `children` getter whose body is the lowered child expression (or a `$ss.frag([...])` when multiple).
- **Fragments** `<>…</>` ⇒ `$ss.frag([ <lowered child exprs> ])`.

### Import handling

Boa runs plain scripts (no ESM loader). **All `import` declarations are removed from the output.** For each:
- specifier ∈ `runtime_specifiers` (default `["supersolid", "solid-js"]`) ⇒ strip silently (names are runtime globals).
- specifier ends in `.css` ⇒ strip and push the specifier onto `result.style_imports` (no diagnostic — the intended co-located-CSS idiom; wiring is a later plan).
- anything else ⇒ strip and push a **Warning** diagnostic naming the specifier ("cross-module imports are not supported yet").

---

## Task 1: `supersolid` crate scaffold + oxc pipeline (TypeScript strip)

Establishes the crate, the public API types, and the verified oxc pipeline doing **TS type-stripping with JSX preserved**. No JSX lowering yet. Pins the exact oxc 0.140 API and feature set empirically.

**Files:**
- Modify: `Cargo.toml` (workspace) — add `oxc` to `[workspace.dependencies]`.
- Create: `crates/supersolid/Cargo.toml`
- Create: `crates/supersolid/src/lib.rs` (public API + pipeline)
- Create: `crates/supersolid/src/pipeline.rs` (oxc parse→semantic→transform→codegen)
- Test: `crates/supersolid/src/lib.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Produces:
  - `supersolid::TranspileOptions { pub runtime_specifiers: Vec<String>, pub tsx: bool }` with `Default` = `{ runtime_specifiers: vec!["supersolid".into(), "solid-js".into()], tsx: true }`.
  - `supersolid::Severity { Warning, Error }` (derive `Debug, Clone, Copy, PartialEq, Eq`).
  - `supersolid::Diagnostic { pub severity: Severity, pub message: String }` (derive `Debug, Clone`).
  - `supersolid::TranspileResult { pub code: String, pub diagnostics: Vec<Diagnostic>, pub style_imports: Vec<String> }` (derive `Debug, Clone, Default`).
  - `supersolid::transpile(source: &str, options: &TranspileOptions) -> TranspileResult`.
  - `pub(crate) fn pipeline::run(source: &str, options: &TranspileOptions, lower_jsx: bool) -> (String, Vec<Diagnostic>, Vec<String>)` — later tasks pass `lower_jsx = true`; Task 1 calls with the JSX-lowering step absent (JSX preserved through codegen).
  - Test helper `fn reparses_as_plain_js(code: &str) -> bool` (parse `code` with a non-JSX JS `SourceType`; true iff zero parser diagnostics). Used by all later tasks to prove "valid JS + no JSX remains".

- [ ] **Step 1: Add oxc to the workspace and create the crate manifest**

In the workspace `Cargo.toml`, add under `[workspace.dependencies]`:

```toml
oxc = { version = "0.140", features = ["semantic", "transformer", "codegen", "ast_visit"] }
```

Create `crates/supersolid/Cargo.toml`:

```toml
[package]
name = "supersolid"
edition.workspace = true
version.workspace = true
license.workspace = true

[dependencies]
oxc.workspace = true
```

> **Spike note (confirm against installed oxc 0.140):** the umbrella `oxc` crate re-exports `oxc::allocator`, `oxc::parser`, `oxc::semantic`, `oxc::transformer`, `oxc::codegen`, `oxc::span`, `oxc::ast`, `oxc::ast_visit`. If a needed item (e.g. `AstBuilder`, `VisitMut`) sits behind a feature not listed above, add it here and note it. If `oxc` needs `features = ["full"]` to expose everything, use that — correctness first (this crate is native-only).

- [ ] **Step 2: Write the failing test**

Add to `crates/supersolid/src/lib.rs` a `#[cfg(test)] mod tests`:

```rust
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
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test -p supersolid`
Expected: FAIL to compile (`transpile`, `TranspileOptions`, `reparses_as_plain_js` don't exist yet).

- [ ] **Step 4: Implement the public API types in `lib.rs`**

Write `crates/supersolid/src/lib.rs` (above the test module):

```rust
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
    // `.cjs`/`.mjs`-style plain JS SourceType: NO JSX. If JSX survived, this errors.
    let source_type = SourceType::mjs();
    let ret = Parser::new(&allocator, code, source_type).parse();
    ret.errors.is_empty()
}
```

> **Spike note:** confirm the exact "plain-JS, no-JSX" `SourceType` constructor (`SourceType::mjs()` / `SourceType::default()` / a builder). The invariant we need: a `SourceType` where a lingering `<div>` is a *parse error*. Also confirm the parser return field for errors (`ret.errors` vs `ret.diagnostics`) — the transformer example uses `ret.diagnostics`; the parser may expose `errors`. Adjust to whichever the installed version uses.

- [ ] **Step 5: Implement the oxc pipeline in `pipeline.rs`**

Write `crates/supersolid/src/pipeline.rs`. This mirrors the official oxc `transformer` example, configured for **TypeScript strip + JSX preserve**, and stubs the JSX-lowering hook that later tasks fill:

```rust
//! The oxc transpile pipeline: parse → semantic → transform (TS strip, JSX
//! preserve) → [our JSX lowering] → codegen. Only the JSX-lowering step is ours;
//! oxc owns the mature TS-strip / parse / print.

use oxc::allocator::Allocator;
use oxc::codegen::Codegen;
use oxc::parser::Parser;
use oxc::semantic::SemanticBuilder;
use oxc::span::SourceType;
use oxc::transformer::{TransformOptions, Transformer};

use crate::{Diagnostic, TranspileOptions};

pub(crate) fn run(
    source: &str,
    options: &TranspileOptions,
    lower_jsx: bool,
) -> (String, Vec<Diagnostic>, Vec<String>) {
    let allocator = Allocator::default();
    let source_type = if options.tsx { SourceType::tsx() } else { SourceType::ts() };

    let mut diagnostics: Vec<Diagnostic> = Vec::new();
    let style_imports: Vec<String> = Vec::new();

    let parsed = Parser::new(&allocator, source, source_type).parse();
    // Parser errors are non-fatal (graceful degradation): record and continue with
    // whatever partial program oxc produced.
    for e in &parsed.errors {
        diagnostics.push(Diagnostic { severity: crate::Severity::Warning, message: e.to_string() });
    }
    let mut program = parsed.program;

    // TypeScript strip with JSX PRESERVED. Build TransformOptions so that the
    // typescript transform runs but JSX is left intact for our own pass.
    let scoping = SemanticBuilder::new().build(&program).semantic.into_scoping();
    let transform_options = ts_strip_jsx_preserve_options();
    let path = std::path::Path::new(if options.tsx { "input.tsx" } else { "input.ts" });
    let _ = Transformer::new(&allocator, path, &transform_options)
        .build_with_scoping(scoping, &mut program);

    if lower_jsx {
        // Filled in from Task 2 onward: crate::jsx::lower(&allocator, &mut program, ...)
        // and crate::imports::rewrite(...) which mutate `program`, appending to
        // `diagnostics` / `style_imports`.
    }

    let code = Codegen::new().build(&program).code;
    (code, diagnostics, style_imports)
}

/// TransformOptions that strip TypeScript types but leave JSX untouched.
fn ts_strip_jsx_preserve_options() -> TransformOptions {
    // Start from an all-off baseline, then enable ONLY what we need. The precise
    // field to set JSX to "preserve" is confirmed during implementation (see note).
    let mut opts = TransformOptions::default();
    // TODO(impl): ensure `opts.jsx` is set to preserve (disable JSX transform);
    // TypeScript stripping is on by default for a .ts/.tsx SourceType.
    opts
}
```

> **Spike note (the one API detail to pin):** the exact way to say "strip TS, preserve JSX" in `TransformOptions`. The transformer guide states JSX can be set to `preserve` to disable JSX transformation; find the corresponding Rust field (likely `opts.jsx.runtime = JsxRuntime::Preserve` or a `preserve`/`JsxOptions` flag) and set it. Verify with the `jsx_is_preserved_for_the_next_pass` test: if it fails because oxc lowered JSX to `React.createElement`/`jsx(...)`, JSX-preserve is not configured. `TransformOptions::default()` may already preserve JSX (default runtime unset) — the test tells you. Keep `EnvOptions` at default (no down-leveling): Boa handles modern syntax; add targeted down-leveling later only if Boa chokes.

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p supersolid`
Expected: PASS (all three). If `strips_typescript_types` fails, TS-strip isn't running for the SourceType; if `jsx_is_preserved_for_the_next_pass` fails, fix the JSX-preserve option per the spike note.

- [ ] **Step 7: Add the crate to the workspace build and commit**

Run: `cargo build -p supersolid && cargo test -p supersolid`
Expected: PASS.

```bash
git add Cargo.toml crates/supersolid/
git commit -m "feat(supersolid): crate scaffold + oxc TS-strip pipeline (JSX preserved)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 2: JSX lowering core — elements + static attributes

Introduce the `jsx` module: a `VisitMut` pass that replaces JSX element/fragment expressions with `$ss.*` calls via `AstBuilder`. This task covers **plain elements and static attributes**; children arrive in Task 3. From here on, `transpile` calls the pipeline with `lower_jsx = true`.

**Files:**
- Create: `crates/supersolid/src/jsx.rs` (the lowering pass + temp-local counter)
- Modify: `crates/supersolid/src/pipeline.rs` (call `jsx::lower` when `lower_jsx`)
- Modify: `crates/supersolid/src/lib.rs` (`transpile` → `pipeline::run(.., true)`; add `mod jsx;`; replace the Task-1 JSX-preserve test)
- Test: `crates/supersolid/src/lib.rs` tests

**Interfaces:**
- Consumes: `pipeline::run(.., lower_jsx: true)`; `oxc::ast::AstBuilder`; `oxc::ast_visit::VisitMut`; `oxc::allocator::Allocator`.
- Produces: `pub(crate) fn jsx::lower(allocator: &oxc::allocator::Allocator, program: &mut oxc::ast::ast::Program)` — walks the program, replacing every `Expression::JSXElement`/`Expression::JSXFragment` with lowered `$ss.*` call expressions. Uses a monotonic counter for temp-local names (`_el0`, `_el1`, …).

- [ ] **Step 1: Write the failing tests**

Replace the Task-1 `jsx_is_preserved_for_the_next_pass` test with these, in `crates/supersolid/src/lib.rs` tests:

```rust
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
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p supersolid empty_element_lowers element_with_static_attrs`
Expected: FAIL — JSX is currently preserved, so `$ss.el` is absent and `reparses_as_plain_js` is false (the `<div>` is a parse error in plain-JS mode).

- [ ] **Step 3: Implement the lowering pass in `jsx.rs`**

Write `crates/supersolid/src/jsx.rs`. The pass replaces JSX expressions post-order (innermost first) so nested/container children are already lowered when a parent is built. This task handles elements + static attributes; a child-less element with no attrs emits the bare `$ss.el("tag")`, otherwise an IIFE.

```rust
//! Element-walk JSX lowering: JSX → `$ss.*` runtime calls (see plan ABI table).
//! We own only this transform; oxc has already stripped TS types and preserved JSX.

use oxc::allocator::Allocator;
use oxc::ast::ast::{Expression, Program};
use oxc::ast::AstBuilder;
use oxc::ast_visit::VisitMut;

struct Lower<'a> {
    ast: AstBuilder<'a>,
    next_local: u32,
}

impl<'a> Lower<'a> {
    fn fresh_local(&mut self) -> String {
        let n = self.next_local;
        self.next_local += 1;
        format!("_el{n}")
    }
}

impl<'a> VisitMut<'a> for Lower<'a> {
    fn visit_expression(&mut self, expr: &mut Expression<'a>) {
        // Post-order: lower nested JSX inside this expression first.
        oxc::ast_visit::walk_mut::walk_expression(self, expr);
        match expr {
            Expression::JSXElement(_) | Expression::JSXFragment(_) => {
                let lowered = self.lower_jsx_expression(expr);
                *expr = lowered;
            }
            _ => {}
        }
    }
}

impl<'a> Lower<'a> {
    /// Lower a JSXElement/JSXFragment expression to a `$ss.*` expression.
    /// Task 2: element + static attributes only. Children (Task 3), dynamic
    /// holes (Task 4), events (Task 5), components/fragments (Task 6) extend this.
    fn lower_jsx_expression(&mut self, expr: &Expression<'a>) -> Expression<'a> {
        // 1. Read tag name + collect static string/number attributes.
        // 2. If no attrs/children: return `$ss.el("tag")` (a call expression).
        // 3. Else build an IIFE:
        //      (() => { const _elN = $ss.el("tag"); $ss.attr(_elN,"k","v"); ...; return _elN; })()
        // Use `self.ast` (AstBuilder) to construct identifiers, member exprs,
        // call exprs, arrow function, and the block statement.
        unimplemented!("build $ss.el / $ss.attr calls via AstBuilder — see plan ABI")
    }
}

/// Entry point called from the pipeline.
pub(crate) fn lower(allocator: &Allocator, program: &mut Program) {
    let mut pass = Lower { ast: AstBuilder::new(allocator), next_local: 0 };
    pass.visit_program(program);
}
```

> **Implementation guidance (the subagent fills `lower_jsx_expression` via TDD):**
> - `AstBuilder` (`oxc::ast::AstBuilder::new(allocator)`) constructs arena AST nodes. The exact constructor method names (e.g. for a call expression, member expression, string literal, arrow function, IIFE) must be read from the installed oxc 0.140 `AstBuilder` docs — **do not guess signatures**; let the two concrete tests drive you.
> - Build `$ss.el("div")` as a `CallExpression` whose callee is the member expression `$ss.el` and whose one argument is a string literal.
> - For the IIFE form, build an arrow function `() => { … }` with a body block of statements, then wrap it in a call expression with no args.
> - Confirm `walk_mut::walk_expression` is the correct free-function path in `oxc::ast_visit` (it may be `oxc::ast_visit::walk_mut::walk_expression` or a `walk_expression` re-export). The invariant: children are visited before the parent node is replaced.

- [ ] **Step 4: Wire the pass into the pipeline and flip `lower_jsx` on**

In `crates/supersolid/src/lib.rs`, change `transpile` to request lowering:

```rust
pub fn transpile(source: &str, options: &TranspileOptions) -> TranspileResult {
    let (code, diagnostics, style_imports) = pipeline::run(source, options, /* lower_jsx */ true);
    TranspileResult { code, diagnostics, style_imports }
}
```

Add `mod jsx;` next to `mod pipeline;`. In `crates/supersolid/src/pipeline.rs`, fill the `if lower_jsx` block:

```rust
    if lower_jsx {
        crate::jsx::lower(&allocator, &mut program);
    }
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p supersolid`
Expected: PASS (TS-strip tests from Task 1 still green; both new element tests green).

- [ ] **Step 6: Commit**

```bash
git add crates/supersolid/src/
git commit -m "feat(supersolid): JSX lowering core — elements + static attributes

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 3: Static children — text and nested elements

Extend `lower_jsx_expression` to append static children: literal text via `$ss.txt` + `$ss.child`, and nested elements by recursing then `$ss.child`.

**Files:**
- Modify: `crates/supersolid/src/jsx.rs`
- Test: `crates/supersolid/src/lib.rs` tests

**Interfaces:**
- Consumes/Produces: same `jsx::lower` entry; `lower_jsx_expression` now also walks `JSXElement.children`.

- [ ] **Step 1: Write the failing tests**

```rust
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
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p supersolid static_text_child nested_element_child`
Expected: FAIL (children not emitted yet).

- [ ] **Step 3: Implement static-children lowering**

In `jsx.rs`, extend `lower_jsx_expression`: after attributes, iterate `JSXElement.children`. For a `JSXChild::Text` that is non-whitespace, emit `$ss.child(_el, $ss.txt("<text>"))`. For a `JSXChild::Element`/`Fragment`, recurse via `self.lower_jsx_expression(child_as_expression)` and emit `$ss.child(_el, <expr>)`. (JSX whitespace-only text between elements is collapsed/skipped, matching JSX semantics — trim per Solid's rule: drop text that is empty after trimming when it sits between elements/newlines.) An element that now has children must use the IIFE form.

> **Guidance:** JSX text trimming — collapse runs of whitespace containing a newline to nothing (Solid/JSX rule); keep meaningful inner spaces. Start simple: `trim()` each text child and skip if empty; refine only if a test needs it.

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test -p supersolid`
Expected: PASS (all prior + two new).

- [ ] **Step 5: Commit**

```bash
git add crates/supersolid/src/jsx.rs crates/supersolid/src/lib.rs
git commit -m "feat(supersolid): static JSX children — text + nested elements

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 4: Dynamic holes — expression children and dynamic attributes

Wrap non-literal expression containers in thunks: dynamic children → `$ss.insert(el, () => expr)`, dynamic attributes → `$ss.bind(el, "name", () => expr)`. String/number-literal values stay static.

**Files:**
- Modify: `crates/supersolid/src/jsx.rs`
- Test: `crates/supersolid/src/lib.rs` tests

- [ ] **Step 1: Write the failing tests**

```rust
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
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p supersolid dynamic_child dynamic_attribute literal_expression`
Expected: FAIL.

- [ ] **Step 3: Implement dynamic-hole lowering**

In `jsx.rs`:
- Attribute value that is a `JSXExpressionContainer`: if the inner expression is a `StringLiteral` or `NumericLiteral`, emit static `$ss.attr(_el, "name", "<stringified>")`; otherwise emit `$ss.bind(_el, "name", () => <expr>)` (build an arrow wrapping the expression). A plain string attribute value (`class="box"`) remains static as in Task 2.
- Child that is a `JSXExpressionContainer`: if `StringLiteral`/`NumericLiteral`, emit `$ss.child(_el, $ss.txt("<stringified>"))`; otherwise emit `$ss.insert(_el, () => <expr>)`.
- Add a helper `is_static_literal(expr) -> Option<String>` returning the stringified value for string/number literals, else `None`.

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test -p supersolid`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/supersolid/src/jsx.rs crates/supersolid/src/lib.rs
git commit -m "feat(supersolid): dynamic JSX holes — thunked insert + bind, literal static

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 5: Event handlers — `onX` → `$ss.on`

`onClick={h}` lowers to `$ss.on(el, "click", h)` — handler passed as-is (not thunked), event name = remainder after `on`, lowercased.

**Files:**
- Modify: `crates/supersolid/src/jsx.rs`
- Test: `crates/supersolid/src/lib.rs` tests

- [ ] **Step 1: Write the failing tests**

```rust
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
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p supersolid on_click on_input`
Expected: FAIL (currently `onClick` would go through the attr path → `$ss.bind`).

- [ ] **Step 3: Implement event lowering**

In `jsx.rs`, before the static/dynamic attribute branches: if the attribute name starts with `on` and has more chars, emit `$ss.on(_el, "<name[2..].to_ascii_lowercase()>", <handler expr>)`. The handler expression is passed directly (no thunk). Requires the element to use the IIFE form.

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test -p supersolid`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/supersolid/src/jsx.rs crates/supersolid/src/lib.rs
git commit -m "feat(supersolid): JSX event handlers — onX -> \$ss.on with name normalization

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 6: Components and fragments

Capitalized tags → `$ss.cmp(Comp, props)` with static props as plain properties, dynamic props as getters, and children as a `children` getter. `<>…</>` → `$ss.frag([…])`.

**Files:**
- Modify: `crates/supersolid/src/jsx.rs`
- Test: `crates/supersolid/src/lib.rs` tests

- [ ] **Step 1: Write the failing tests**

```rust
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
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p supersolid capitalized_tag component_static component_children fragment_lowers`
Expected: FAIL (components currently go through the element path → `$ss.el("Counter")`, which is wrong).

- [ ] **Step 3: Implement component + fragment lowering**

In `jsx.rs`:
- In `lower_jsx_expression`, branch on tag case first: if the tag's first character is uppercase, lower as a **component**; else as an element (existing path). Determine "component" from the `JSXElementName` (an `Identifier` starting uppercase; also treat member names like `Foo.Bar` as components — start with the uppercase-identifier case, extend only if a test needs it).
- Component: build an object expression of props. For each attribute: `name="lit"`/`name={literal}` → plain property `name: <value>` (JS value, **not** stringified — string literal stays a string, numeric literal stays a number); `name={expr}` (non-literal) → a **getter** property `get name() { return <expr>; }`; `onX` handlers are ordinary props on components (`onX: handler`) — components receive them as props, not DOM listeners. If the component has children, add a `get children() { return <lowered children expr>; }` where multiple children become `$ss.frag([...])` and a single child is that child's lowered expression. Emit `$ss.cmp(<TagIdent>, <propsObject>)`.
- Fragment (`JSXFragment`): lower each child to an expression and emit `$ss.frag([ <exprs> ])`. Whitespace-only text children are skipped (Task 3 rule).

> **Guidance:** building a getter property (`get name() { … }`) via `AstBuilder` uses an object property with `kind = Get` and a function value; confirm the exact builder API against oxc 0.140. The `component_static_and_dynamic_props` test is the contract.

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test -p supersolid`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/supersolid/src/jsx.rs crates/supersolid/src/lib.rs
git commit -m "feat(supersolid): components (\$ss.cmp + getter props) and fragments (\$ss.frag)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 7: Import handling — strip runtime, record CSS, warn others

Remove all `import` declarations from the output; record `.css` specifiers into `style_imports`; warn on unknown JS-module specifiers.

**Files:**
- Create: `crates/supersolid/src/imports.rs`
- Modify: `crates/supersolid/src/pipeline.rs` (run import rewriting; thread diagnostics + style_imports)
- Modify: `crates/supersolid/src/lib.rs` (`mod imports;`)
- Test: `crates/supersolid/src/lib.rs` tests

**Interfaces:**
- Produces: `pub(crate) fn imports::rewrite(allocator, program, options) -> (Vec<Diagnostic>, Vec<String>)` — removes all `ImportDeclaration` statements from `program.body`, returning `(diagnostics, style_imports)`.

- [ ] **Step 1: Write the failing tests**

```rust
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
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p supersolid runtime_imports css_imports unknown_module`
Expected: FAIL (imports are currently passed through; note `reparses_as_plain_js` may already reject an `import` at mjs scope — the assertions on stripping/recording still fail).

- [ ] **Step 3: Implement import rewriting in `imports.rs`**

```rust
//! Import rewriting: Boa runs plain scripts (no ESM). Strip ALL imports; classify
//! each by specifier — runtime (silent), `.css` (record), else warn.

use oxc::allocator::Allocator;
use oxc::ast::ast::{Program, Statement};

use crate::{Diagnostic, Severity, TranspileOptions};

pub(crate) fn rewrite(
    _allocator: &Allocator,
    program: &mut Program,
    options: &TranspileOptions,
) -> (Vec<Diagnostic>, Vec<String>) {
    let mut diagnostics = Vec::new();
    let mut style_imports = Vec::new();

    // Inspect every import's specifier, classify, then drop all import statements.
    for stmt in program.body.iter() {
        if let Statement::ImportDeclaration(decl) = stmt {
            let specifier = decl.source.value.as_str().to_string();
            if options.runtime_specifiers.iter().any(|s| s == &specifier) {
                // silent
            } else if specifier.ends_with(".css") {
                style_imports.push(specifier);
            } else {
                diagnostics.push(Diagnostic {
                    severity: Severity::Warning,
                    message: format!(
                        "supersolid: cross-module import from {specifier:?} is not supported yet (stripped)"
                    ),
                });
            }
        }
    }
    program.body.retain(|stmt| !matches!(stmt, Statement::ImportDeclaration(_)));

    (diagnostics, style_imports)
}
```

> **Spike note:** confirm the AST shapes — `Statement::ImportDeclaration(Box<ImportDeclaration>)`, `decl.source` is a `StringLiteral` with `.value` (an oxc `Atom`; `.as_str()`). `program.body` is an oxc `Vec` (arena) supporting `iter()` and `retain()`; if `retain` isn't available on the arena `Vec`, rebuild the vector via `AstBuilder` keeping non-import statements. The three tests are the contract.

- [ ] **Step 4: Wire into the pipeline**

In `crates/supersolid/src/lib.rs` add `mod imports;`. In `pipeline.rs`, inside `if lower_jsx`, after `crate::jsx::lower(...)`, call import rewriting and merge results:

```rust
    if lower_jsx {
        crate::jsx::lower(&allocator, &mut program);
        let (mut import_diags, imports) = crate::imports::rewrite(&allocator, &mut program, options);
        diagnostics.append(&mut import_diags);
        return finish(&program, diagnostics, imports);
    }
    // (non-lowering path unchanged)
```

Refactor the codegen tail into a small `finish` helper (or inline the codegen + returns) so both paths produce `(code, diagnostics, style_imports)`. Ensure the non-lowering Task-1 path still returns `(code, diagnostics, vec![])`.

- [ ] **Step 5: Run to verify they pass**

Run: `cargo test -p supersolid`
Expected: PASS (all prior + three new).

- [ ] **Step 6: Commit**

```bash
git add crates/supersolid/src/
git commit -m "feat(supersolid): import rewriting — strip runtime, record CSS, warn unknown

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 8: Native-only Bevy `AssetLoader` (`.tsx`/`.ts` → `JsSource`)

Wire the transpiler into the browser: a `TsxLoader` in `superui` that transpiles `.tsx`/`.ts` and yields the existing `JsSource`, registered native-only, with `supersolid` as a target-gated dependency so `oxc` never enters wasm.

**Files:**
- Modify: `crates/superui/Cargo.toml` (target-gated `supersolid` dep)
- Modify: `crates/superui/src/assets.rs` (add `TsxLoader`, native-only)
- Modify: `crates/superui/src/lib.rs` (re-export `TsxLoader` native-only)
- Modify: `crates/superui/src/mount.rs` (register `TsxLoader` native-only in `SuperUiPlugin::build`)
- Test: `crates/superui/src/assets.rs` (`#[cfg(test)]`)

**Interfaces:**
- Consumes: `supersolid::{transpile, TranspileOptions}`; existing `JsSource`, `read_to_string` (in `assets.rs`); Bevy `AssetLoader`.
- Produces: `superui::TsxLoader` (native-only) — `AssetLoader<Asset = JsSource>`, `extensions() = ["tsx", "ts"]`.

- [ ] **Step 1: Add the target-gated dependency**

In `crates/superui/Cargo.toml`, add:

```toml
[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
supersolid = { path = "../supersolid" }
```

- [ ] **Step 2: Write the failing test**

Add to the `#[cfg(test)] mod tests` in `crates/superui/src/assets.rs` (native-only):

```rust
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn tsx_loader_transpiles_to_jssource() {
    let dir = Dir::new("assets".into());
    dir.insert_asset(
        "app.tsx".as_ref(),
        b"const n: number = 1; const a = <div class=\"x\">{n}</div>;",
    );

    let mut app = App::new();
    app.register_asset_source(
        AssetSourceId::Default,
        AssetSource::build().with_reader(move || Box::new(MemoryAssetReader { root: dir.clone() })),
    );
    app.add_plugins((bevy::app::TaskPoolPlugin::default(), AssetPlugin::default()));
    app.init_asset::<JsSource>().register_asset_loader(TsxLoader);
    app.finish();

    let handle = {
        let server = app.world().resource::<AssetServer>().clone();
        server.load::<JsSource>("app.tsx")
    };
    for _ in 0..64 {
        app.update();
        if matches!(
            app.world().resource::<AssetServer>().load_state(handle.id()),
            LoadState::Loaded
        ) {
            break;
        }
    }
    let jss = app.world().resource::<Assets<JsSource>>();
    let out = &jss.get(&handle).unwrap().0;
    assert!(!out.contains(": number"), "types stripped by loader:\n{out}");
    assert!(out.contains(r#"$ss.el("div")"#), "JSX lowered by loader:\n{out}");
}
```

- [ ] **Step 3: Run to verify it fails**

Run: `cargo test -p superui tsx_loader_transpiles_to_jssource`
Expected: FAIL to compile (`TsxLoader` doesn't exist).

- [ ] **Step 4: Implement `TsxLoader`**

In `crates/superui/src/assets.rs`, add (native-only). Reuse the existing `read_to_string` helper:

```rust
/// Loads `.tsx`/`.ts`, transpiles via `supersolid`, and yields a `JsSource`
/// (so mount/hot-reload treat it identically to hand-written `.js`). Native-only:
/// `oxc` must not enter the wasm binary (direction spec §11.3).
#[cfg(not(target_arch = "wasm32"))]
#[derive(Default)]
pub struct TsxLoader;

#[cfg(not(target_arch = "wasm32"))]
impl AssetLoader for TsxLoader {
    type Asset = JsSource;
    type Settings = ();
    type Error = std::io::Error;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        lc: &mut LoadContext<'_>,
    ) -> Result<JsSource, std::io::Error> {
        let src = read_to_string(reader).await?;
        let tsx = lc.path().extension().and_then(|e| e.to_str()) != Some("ts");
        let opts = supersolid::TranspileOptions { tsx, ..Default::default() };
        let result = supersolid::transpile(&src, &opts);
        for d in &result.diagnostics {
            bevy::log::warn!("supersolid: {}", d.message);
        }
        // Graceful degradation (design §1): return whatever JS was produced even on
        // diagnostics; never fail the load for a transpile warning.
        Ok(JsSource(result.code))
    }

    fn extensions(&self) -> &[&str] {
        &["tsx", "ts"]
    }
}
```

- [ ] **Step 5: Re-export and register (native-only)**

In `crates/superui/src/lib.rs`, extend the assets re-export:

```rust
pub use assets::{HtmlLoader, HtmlSource, JsLoader, JsSource};
#[cfg(not(target_arch = "wasm32"))]
pub use assets::TsxLoader;
```

In `crates/superui/src/mount.rs`, `SuperUiPlugin::build`, register the loader native-only (after the existing `.register_asset_loader(JsLoader)`):

```rust
        #[cfg(not(target_arch = "wasm32"))]
        app.register_asset_loader(crate::assets::TsxLoader);
```

(The `.tsx`/`.ts` loader outputs `JsSource`, which is already `init_asset`-ed — no new asset registration needed. On wasm the loader is absent; wasm apps load pre-transpiled `.js` via `JsLoader`.)

- [ ] **Step 6: Run to verify it passes + no regressions**

Run: `cargo test -p superui`
Expected: PASS (new test + existing `loads_html_and_js_sources`).

- [ ] **Step 7: Verify wasm build excludes oxc (structural check) and commit**

Run: `cargo build -p superui --target wasm32-unknown-unknown`
Expected: builds without pulling `supersolid`/`oxc` (they're target-gated out). If the target isn't installed, note it and rely on the `cfg` gating; do not add a non-gated dep.

```bash
git add crates/superui/Cargo.toml crates/superui/src/assets.rs crates/superui/src/lib.rs crates/superui/src/mount.rs
git commit -m "feat(superui): native-only .tsx/.ts AssetLoader -> JsSource via supersolid

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 9: Build-time CLI for the wasm pre-transpile path

A minimal `supersolid` binary + a reusable `transpile_file` lib function that transpiles a `.tsx`/`.ts` file to a sibling `.js`, so wasm builds ship pre-transpiled JS (spec §11.3). The full `cargo superui` cargo-metadata projector (§9) stays deferred.

**Files:**
- Create: `crates/supersolid/src/bin/supersolid.rs` (thin CLI wrapper)
- Modify: `crates/supersolid/src/lib.rs` (add `transpile_file`)
- Test: `crates/supersolid/tests/cli.rs` (integration test of `transpile_file`)

**Interfaces:**
- Produces: `supersolid::transpile_file(input: &std::path::Path, output: &std::path::Path) -> std::io::Result<supersolid::TranspileResult>` — reads `input`, transpiles (tsx inferred from extension: `.ts` ⇒ `tsx=false`, else true), writes `result.code` to `output`, returns the result (so callers can inspect diagnostics/style_imports).

- [ ] **Step 1: Write the failing test**

Create `crates/supersolid/tests/cli.rs`:

```rust
use std::fs;

#[test]
fn transpile_file_writes_plain_js_sibling() {
    let dir = std::env::temp_dir().join("supersolid_cli_test");
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
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p supersolid --test cli`
Expected: FAIL to compile (`transpile_file` missing).

- [ ] **Step 3: Implement `transpile_file`**

In `crates/supersolid/src/lib.rs`:

```rust
use std::path::Path;

/// Transpile one `.tsx`/`.ts` file to `output` (plain JS). Used by the CLI for
/// the wasm build-time pre-transpile path (direction spec §11.3).
pub fn transpile_file(input: &Path, output: &Path) -> std::io::Result<TranspileResult> {
    let src = std::fs::read_to_string(input)?;
    let tsx = input.extension().and_then(|e| e.to_str()) != Some("ts");
    let result = transpile(&src, &TranspileOptions { tsx, ..Default::default() });
    std::fs::write(output, &result.code)?;
    Ok(result)
}
```

- [ ] **Step 4: Implement the CLI wrapper**

Create `crates/supersolid/src/bin/supersolid.rs`:

```rust
//! Minimal build-time transpiler CLI: `supersolid <input.tsx> <output.js>`.
//! Transpiles one file so wasm builds can ship pre-transpiled `.js`
//! (direction spec §11.3). The cargo-metadata projector (§9) is a later plan.

use std::path::PathBuf;

fn main() -> std::io::Result<()> {
    let mut args = std::env::args().skip(1);
    let (Some(input), Some(output)) = (args.next(), args.next()) else {
        eprintln!("usage: supersolid <input.tsx|.ts> <output.js>");
        std::process::exit(2);
    };
    let result = supersolid::transpile_file(&PathBuf::from(input), &PathBuf::from(output))?;
    for d in &result.diagnostics {
        eprintln!("warning: {}", d.message);
    }
    Ok(())
}
```

- [ ] **Step 5: Run to verify it passes**

Run: `cargo test -p supersolid --test cli && cargo build -p supersolid --bin supersolid`
Expected: PASS + the binary builds.

- [ ] **Step 6: Commit**

```bash
git add crates/supersolid/src/lib.rs crates/supersolid/src/bin/ crates/supersolid/tests/cli.rs
git commit -m "feat(supersolid): build-time transpile_file + CLI for the wasm pre-transpile path

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Done-when

- `cargo test -p supersolid` and `cargo test -p superui` both green.
- `supersolid::transpile` strips TypeScript types and lowers Solid-style JSX to the `$ss` ABI: elements + static attrs (`$ss.el`/`$ss.attr`), static children (`$ss.txt`/`$ss.child`), dynamic holes thunked (`$ss.insert`/`$ss.bind`), events (`$ss.on`), components with getter props (`$ss.cmp`), fragments (`$ss.frag`) — every emitted output re-parses as plain JS with no JSX remaining.
- Imports are stripped: runtime specifiers silently, `.css` recorded into `style_imports`, unknown JS modules warned.
- `superui` loads `.tsx`/`.ts` as `JsSource` **natively**, hot-reloading through the existing `AssetEvent::Modified<JsSource>` seam (mount/hot_reload/reconcile unchanged).
- `oxc` is excluded from `wasm32-unknown-unknown` builds of `superui` (target-gated dep + `cfg`-gated loader); a `supersolid` CLI provides the wasm pre-transpile path.
- Backlog for the clone-based template optimization exists at `docs/future_backlog/2026-07-19-clone-based-template-lowering.md` (out of scope here).

## Self-review (author)

- **Spec coverage:** implements direction-spec §3 ("own the compiler": oxc-in-Rust, TS-strip + JSX lowering), §5 (build-once + surgical bindings via element-walk — the arena-DOM analog the spec names), §11.3 (native/build-time only; wasm ships pre-transpiled `.js`, no transpiler in the binary — enforced by target-gated dep + `cfg` loader + CLI). Deferred with rationale: React-ism lints (§11.1) → own later plan (diagnostics channel shipped); ambient `.d.ts`/tsconfig (§3) → needs Plans 3–4 runtime API; cross-module imports/projector (§9) → later plan (warn-only today); clone-based template optimization → backlog doc. Runtime ABI (`$ss.*` + author globals) is defined here and implemented in Plans 3–4.
- **No placeholders:** every task has concrete tests (the contracts) and real pipeline/loader/CLI code. The two genuinely version-sensitive spots — the `TransformOptions` JSX-preserve field and the exact `AstBuilder` constructor names in oxc 0.140 — are flagged as **spike notes** to confirm against installed docs rather than guessed; the concrete failing tests drive them to the correct API. This is deliberate: guessing a fast-moving external signature would be a worse plan failure than an explicit "confirm this call" marker beside an executable contract.
- **Type consistency:** `TranspileOptions { runtime_specifiers, tsx }`, `TranspileResult { code, diagnostics, style_imports }`, `Diagnostic { severity, message }`, `Severity::{Warning, Error}`, `transpile`, `transpile_file`, `pipeline::run(.., lower_jsx)`, `jsx::lower(allocator, program)`, `imports::rewrite(allocator, program, options) -> (Vec<Diagnostic>, Vec<String>)`, and `TsxLoader` (Asset = `JsSource`, extensions `["tsx","ts"]`) are used identically across tasks. The `$ss.*` helper names and author-global names match the ABI table throughout.
