# JSON Imports (`import data from "./x.json"`) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let `supersolid` resolve a `.json` import and inline the parsed value as a top-level `const` binding, so UI and spec code can read static data from a file without the Bevy bridge.

**Architecture:** Mirror the existing `style_imports` precedent — **detect in the transpiler, resolve in the caller**. `imports.rs` records `(binding, specifier)` pairs; a pure `supersolid::json_binding` helper turns JSON text into a `const b = <literal>;` line; each of the three callers (test engine, runtime/HMR loader, build) reads the file its own way and prepends the lines to the emitted code. The transpiler stays filesystem-free and wasm-safe.

**Tech Stack:** Rust, `oxc` (transpiler), `serde_json` (parse/serialize, added to `supersolid`), Bevy `bevy_asset` `LoadContext` (runtime loader).

**Spec:** `docs/superpowers/specs/2026-10-03-superui-json-imports-design.md`

## Global Constraints

- The pure transpiler (`supersolid::transpile` and everything it calls: `imports.rs`, `pipeline.rs`, `jsx.rs`) must remain filesystem-free — no `std::fs`, no `oxc` I/O — so `oxc` never enters a wasm build (direction spec §11.3). File resolution lives only in the callers.
- `serde_json = "1"` is added to `supersolid`'s `Cargo.toml` only; it is wasm-safe and pulls in no `oxc`. No other crate gains a new dependency (`json_binding` encapsulates all `serde_json` use, so callers need not depend on it directly).
- `.json` detection is case-insensitive on the specifier suffix, matching the existing `.css` check (`specifier.to_ascii_lowercase().ends_with(".json")`).
- Emitted inlined code must re-parse as plain JS (classic script): the bindings are top-level `const`s, prepended in declaration order before the transpiled body.
- Binding semantics: default, namespace (`* as x`), and named (`{ x }`) imports all bind the **whole** parsed value to the local name. Member/sub-path selection is out of scope.

## Review Focus

- **Invalid JSON content** (syntactically broken file): test engine returns `Err` (fatal); runtime loader and build warn + skip the binding. Covered in Tasks 3/4/5.
- **Missing JSON file** (specifier points nowhere): same split — `Err` for test engine, warn + skip for loader/build. Covered in Tasks 3/4/5.
- **JSON string values containing characters significant to JS** (quotes, backslashes, newlines): `serde_json::to_string` must round-trip them to a valid JS literal. Covered in Task 2.
- **Relative specifier with `../`** resolving outside the importing file's directory: must resolve against the importing file's directory, not the asset root. Covered in Task 3 (fs) and Task 4 (asset path).
- **A `.json` import whose statement must still be stripped** from the emitted JS (no stray `import` keyword): covered in Task 1.

---

### Task 1: Transpiler detection — record `json_imports`

**Files:**
- Modify: `crates/supersolid/src/imports.rs`
- Modify: `crates/supersolid/src/pipeline.rs`
- Modify: `crates/supersolid/src/lib.rs` (`TranspileResult`, `transpile`)
- Test: `crates/supersolid/src/imports.rs` (`#[cfg(test)]` module in `lib.rs` exercises `transpile`; add detection tests there alongside `css_imports_are_recorded_not_warned`)

**Interfaces:**
- Produces: `TranspileResult.json_imports: Vec<(String, String)>` — `(local_binding_name, specifier)` in declaration order. `imports::rewrite` returns `(Vec<Diagnostic>, Vec<String>, Vec<(String, String)>)`. `pipeline::run` returns `(String, Vec<Diagnostic>, Vec<String>, Vec<(String, String)>)`.

- [ ] **Step 1: Write failing tests** in the `tests` module of `crates/supersolid/src/lib.rs`:

```rust
#[test]
fn json_default_import_is_recorded_not_warned() {
    let r = transpile("import skills from \"./skills.json\"; const x = skills;", &TranspileOptions::default());
    assert!(!r.code.contains("import"), "json import stripped from JS:\n{}", r.code);
    assert_eq!(r.json_imports, vec![("skills".to_string(), "./skills.json".to_string())]);
    assert!(r.diagnostics.is_empty(), "json import must not warn: {:?}", r.diagnostics);
    assert!(reparses_as_plain_js(&r.code));
}

#[test]
fn json_namespace_and_named_imports_record_local_name() {
    let ns = transpile("import * as data from \"./a.json\";", &TranspileOptions::default());
    assert_eq!(ns.json_imports, vec![("data".to_string(), "./a.json".to_string())]);
    let named = transpile("import { data } from \"./b.json\";", &TranspileOptions::default());
    assert_eq!(named.json_imports, vec![("data".to_string(), "./b.json".to_string())]);
}

#[test]
fn bare_json_side_effect_import_records_nothing() {
    let r = transpile("import \"./x.json\"; const y = 1;", &TranspileOptions::default());
    assert!(r.json_imports.is_empty(), "no binding to record: {:?}", r.json_imports);
    assert!(r.diagnostics.is_empty());
}

#[test]
fn json_suffix_match_is_case_insensitive() {
    let r = transpile("import d from \"./Data.JSON\";", &TranspileOptions::default());
    assert_eq!(r.json_imports, vec![("d".to_string(), "./Data.JSON".to_string())]);
    assert!(r.diagnostics.is_empty());
}
```

- [ ] **Step 2: Run to verify failure** — `cargo test -p supersolid json_`. Expected: FAIL (no `json_imports` field).

- [ ] **Step 3: Implement.** In `imports.rs`: add `let mut json_imports = Vec::new();`; add a branch **before** the warn-else: `else if specifier.to_ascii_lowercase().ends_with(".json")` that, for each specifier in `decl.specifiers` (iterating the `Option<Vec<ImportDeclarationSpecifier>>`), pushes `(local_name, specifier.clone())` — the local name is `spec.local().name.as_str().to_string()` (oxc 0.140: `ImportDeclarationSpecifier::local()` returns `&BindingIdentifier` for the default, namespace, and named variants alike; `BindingIdentifier.name` is an `Atom`). Return the extra `json_imports` vec. Thread it through `pipeline::run`/`finish` and onto `TranspileResult` in `lib.rs` (`transpile` sets `json_imports: ...`; `#[derive(Default)]` already covers the new field).

- [ ] **Step 4: Run to verify pass** — `cargo test -p supersolid json_`. Expected: PASS. Also run full `cargo test -p supersolid` to confirm `css_imports_are_recorded_not_warned` and `unknown_module_imports_warn` still pass (json branch precedes the warn-else).

- [ ] **Step 5: Commit** — `feat: record .json imports in the transpiler`.

---

### Task 2: `json_binding` helper + `serde_json` dependency

**Files:**
- Modify: `crates/supersolid/Cargo.toml` (add `serde_json = "1"`)
- Modify: `crates/supersolid/src/lib.rs` (add `json_binding`)
- Test: `crates/supersolid/src/lib.rs` (`tests` module)

**Interfaces:**
- Produces: `pub fn json_binding(binding: &str, json_text: &str) -> Result<String, String>` — returns `const <binding> = <compact-json>;\n` on success, `Err(message)` on invalid JSON.

- [ ] **Step 1: Write failing tests** in the `tests` module of `lib.rs`:

```rust
#[test]
fn json_binding_inlines_value_as_valid_js() {
    let out = super::json_binding("skills", r#"{"a":1,"b":["x","y"]}"#).unwrap();
    assert!(out.starts_with("const skills = "), "const decl:\n{out}");
    assert!(reparses_as_plain_js(&out), "inlined json must be valid JS:\n{out}");
    assert!(out.contains("\"a\":1") || out.contains("\"a\": 1"));
}

#[test]
fn json_binding_escapes_special_string_chars() {
    let out = super::json_binding("d", r#"{"s":"he said \"hi\"\n\\done"}"#).unwrap();
    assert!(reparses_as_plain_js(&out), "special chars must round-trip to valid JS:\n{out}");
}

#[test]
fn json_binding_rejects_invalid_json() {
    assert!(super::json_binding("d", "{not json").is_err());
}
```

- [ ] **Step 2: Run to verify failure** — `cargo test -p supersolid json_binding`. Expected: FAIL (function not defined).

- [ ] **Step 3: Implement** `json_binding` in `lib.rs`: parse `serde_json::from_str::<serde_json::Value>(json_text)` mapping the error to `format!("invalid JSON: {e}")`; serialize with `serde_json::to_string(&value)`; return `format!("const {binding} = {literal};\n")`. Add `serde_json = "1"` to the crate manifest.

- [ ] **Step 4: Run to verify pass** — `cargo test -p supersolid json_binding`. Expected: PASS.

- [ ] **Step 5: Commit** — `feat: add json_binding inlining helper to supersolid`.

---

### Task 3: Test-engine caller (phase 1)

**Files:**
- Modify: `crates/superui_test_engine/src/transpile.rs` (signature + inlining + inline test)
- Modify: `crates/superui_test_engine/src/cli.rs:95`, `crates/superui_test_engine/src/ui_mode.rs:249` (thread spec dir)
- Modify: all `transpile_spec(...)` call sites in `crates/superui_test_engine/tests/*.rs` (add base-dir arg)
- Test: new `crates/superui_test_engine/tests/json_import.rs`

**Interfaces:**
- Consumes: `supersolid::json_binding` (Task 2), `TranspileResult.json_imports` (Task 1).
- Produces: `pub fn transpile_spec(source: &str, module_id: &str, base_dir: &std::path::Path) -> Result<String, String>` — prepends inlined `const`s; returns `Err` if any imported JSON is missing or invalid.

- [ ] **Step 1: Write failing integration test** `crates/superui_test_engine/tests/json_import.rs`: create a temp dir, write `data.json` = `{"n":3}`, call `transpile_spec("import data from \"./data.json\"; const x = data.n;", "t.spec.ts", &temp_dir)` and assert the result contains `const data = {"n":3};` (allowing optional space after `:`) and `const x = data.n`. Second case: importing `"./missing.json"` returns `Err`. (Key the temp dir off the test name, no `Date`/rand — follow the `build.rs` `temp_ui_dir` pattern.)

- [ ] **Step 2: Run to verify failure** — `cargo test -p superui_test_engine --test json_import`. Expected: FAIL (arity mismatch — `transpile_spec` takes 2 args).

- [ ] **Step 3: Implement.** Change `transpile_spec` to take `base_dir: &Path`. After `transpile`, fold over `result.json_imports`: `let path = base_dir.join(specifier); let text = std::fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))?; let line = supersolid::json_binding(&binding, &text)?;` accumulate lines, then return `format!("{lines}{}", result.code)`. Keep the existing empty-output fatal check (evaluate it against `result.code`). Update `cli.rs` to pass `spec.parent().unwrap_or_else(|| std::path::Path::new("."))` and `ui_mode.rs` likewise (`spec.parent()...`). Update the inline `transpile.rs` test and every `tests/*.rs` call site to pass `std::path::Path::new(".")` as the third arg.

- [ ] **Step 4: Run to verify pass** — `cargo test -p superui_test_engine`. Expected: PASS (new test + all existing specs).

- [ ] **Step 5: Commit** — `feat: inline .json imports in the test engine`.

---

### Task 4: Runtime / HMR loader caller (phase 2)

**Files:**
- Modify: `crates/superui/src/assets.rs` (`TsxLoader::load`)
- Test: `crates/superui/src/assets.rs` (`tests` module, gated `#[cfg(not(target_arch = "wasm32"))]` like the sibling loader tests)

**Interfaces:**
- Consumes: `supersolid::json_binding` (Task 2), `TranspileResult.json_imports` (Task 1), `superui_paths::{parent_dir, join_asset}`, `LoadContext::read_asset_bytes` (registers the JSON as a load dependency for hot reload).

- [ ] **Step 1: Write failing test** in the `assets.rs` `tests` module, modeled on `tsx_loader_transpiles_to_jssource`: insert `data.json` = `{"n":7}` and `app.tsx` = `import data from "./data.json"; const a = <div>{data.n}</div>;` into the `Dir`, load `app.tsx` as `JsSource`, and assert the output contains `const data = {"n":7};` (optional space after `:`).

- [ ] **Step 2: Run to verify failure** — `cargo test -p superui tsx_loader` (or the new test name). Expected: FAIL (output lacks the inlined const).

- [ ] **Step 3: Implement.** In `TsxLoader::load`, after building `result`, iterate `result.json_imports`: resolve `let dir = lc.path().path().to_string_lossy(); let asset_path = superui_paths::join_asset(superui_paths::parent_dir(&dir), &specifier);`, then `match lc.read_asset_bytes(&asset_path).await { Ok(bytes) => match String::from_utf8(bytes) ... then supersolid::json_binding(&binding, &text), Err/parse err => bevy::log::warn!(...) and skip }`. Prepend the accumulated lines to `result.code` before wrapping in `JsSource`. Keep graceful degradation — never return `Err` for a JSON failure.

- [ ] **Step 4: Run to verify pass** — `cargo test -p superui`. Expected: PASS.

- [ ] **Step 5: Commit** — `feat: inline and hot-reload .json imports in the TSX loader`.

---

### Task 5: Build caller (phase 3)

**Files:**
- Modify: `crates/supersolid/src/lib.rs` (`transpile_file`)
- Modify: `crates/supersolid/src/build.rs` (`transpile_dir_impl` — emit `cargo:rerun-if-changed`)
- Test: `crates/supersolid/src/build.rs` (`tests` module)

**Interfaces:**
- Consumes: `supersolid::json_binding` (Task 2), `TranspileResult.json_imports` (Task 1).
- Produces: `transpile_file` writes JS with inlined `const`s prepended; the returned `TranspileResult.json_imports` stays populated so `transpile_dir` can emit rerun-if-changed.

- [ ] **Step 1: Write failing test** in the `build.rs` `tests` module, modeled on `writes_generated_js_for_each_tsx`: write `data.json` = `{"k":5}` and `app.tsx` = `import data from "./data.json"; const a = data.k;` into the temp dir, run `transpile_dir_impl(&dir_str, false)`, read the generated `app.js`, assert it contains `const data = {"k":5};` (optional space after `:`).

- [ ] **Step 2: Run to verify failure** — `cargo test -p supersolid writes_generated`/new test name. Expected: FAIL (generated JS lacks the inlined const).

- [ ] **Step 3: Implement.** In `transpile_file`: after `transpile`, fold `result.json_imports` resolving each against `input.parent()` (`input.parent().unwrap_or_else(|| Path::new(".")).join(specifier)`), `std::fs::read_to_string`; on success call `supersolid::json_binding` and accumulate; on failure push a `Diagnostic { severity: Severity::Warning, message }` into `result.diagnostics` and skip. Prepend accumulated lines to `result.code` before `std::fs::write`. Return the (still-`json_imports`-populated) `result`. In `transpile_dir_impl`, after a successful `transpile_file`, for each `(_, specifier)` in `result.json_imports` compute the resolved source path (`superui_paths::join_asset(superui_paths::parent_dir(&src), specifier)`) and `println!("cargo:rerun-if-changed={resolved}")`.

- [ ] **Step 4: Run to verify pass** — `cargo test -p supersolid`. Expected: PASS (new test + existing build tests).

- [ ] **Step 5: Commit** — `feat: inline .json imports in the build transpile path`.

---

### Task 6: Documentation

**Files:**
- Modify: the supersolid/transpiler support/reference docs under `website/src/docs` (locate the page that lists import handling / CSS imports) and the changelog if one is maintained on this branch.

**Interfaces:** none (docs only).

- [ ] **Step 1:** Find the docs page covering import handling (`.css` imports, cross-module stripping). Use the `reference-docs` / `documenting-new-features` skill conventions.
- [ ] **Step 2:** Document that `import x from "./x.json"` inlines the parsed value as a binding (default/namespace/named all bind the whole value), note the per-caller error behavior, and record member/sub-path selection and non-JSON module imports as out of scope.
- [ ] **Step 3:** Commit — `docs: document .json imports`.
