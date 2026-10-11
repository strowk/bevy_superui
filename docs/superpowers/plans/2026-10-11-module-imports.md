# Module Imports Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `import { X } from "./x.tsx"` (and the other ESM import/export forms, plus bare `superui_modules/` packages and co-located `.css`) actually resolve, load, bundle, and hot-reload across a multi-file supersolid app.

**Architecture:** A pure, wasm-clean, I/O-free core in `supersolid` — `transpile_module` (ESM→CommonJS-factory lowering), `resolve_candidates` (ordered candidate paths), `assemble` (emit the `$ssmod` registry bundle), and `walk_sync` (generic graph walk parameterized by a `read` closure). The four existing "transpile + resolve imports" callers (`TsxLoader`, `build.rs`, the test-engine, the playground) all drive this one core — three via `walk_sync` with a `std::fs`/in-memory read, and `TsxLoader` via its own async loop reusing the same pure functions because its reads are `async`.

**Tech Stack:** Rust, `oxc` (parse / TS-strip / codegen), Bevy asset loaders, vendored flair (`superui_flair_style`), Playwright-style test-engine.

**Spec:** `docs/superpowers/specs/2026-10-11-module-imports-design.md`

## Global Constraints

- **`supersolid` stays I/O-free and wasm-capable.** No `std::fs` in `transpile_module` / `resolve_candidates` / `assemble` — they are pure. `walk_sync` takes a `read` closure and performs no I/O itself. Only `build.rs` / `transpile_file` (already native-only) touch `std::fs`.
- **The engine runs classic scripts, not ES modules.** The module system is emitted *into* the single bundled script (the `$ssmod` registry); the engine still does a plain `eval`.
- **Graceful degradation (design §1):** a missing/unreadable/unresolvable module → `Warning` diagnostic + a sentinel edge; the bundle is still produced. The **test-engine keeps a missing import fatal.** `export *` / dynamic `import()` → warn-and-skip. Import cycles → CommonJS partial-exports, no diagnostic.
- **Module id = canonical resolved asset path, forward-slash normalized.** Two importers reaching the same file via different relative paths share one instance (singleton).
- **Paths use forward slashes everywhere** (matches `superui_paths`). Reuse `superui_paths::{parent_dir, join_asset, generated_js, GENERATED_DIR}`.
- **The `$ssmod` registry prelude is fixed copy** — emit it verbatim from spec §1 (reproduced in Task 2).
- **This spec does not change the tsconfig** (IDE alignment is a no-op here).

## Review Focus

- **Import cycle A↔B** → the importer sees partial exports (CommonJS), no infinite recursion. → Task 2 assemble test.
- **Diamond dependency** (two modules import the same third) → one shared singleton instance, factory runs once. → Task 2 `walk_sync` test.
- **Relative path normalization** (`../lib/util`, `./a/../b`) → canonical forward-slash id; two spellings of the same file collapse to one id. → Task 2 resolver test.
- **Unresolved / unreadable module mid-graph** → sentinel edge + warning, bundle still produced in the native loader; the same condition is **fatal** in the test-engine. → Task 4 loader test + Task 7 test-engine test.
- **`export *` / dynamic `import()`** → one `Warning`, skipped, never panics, rest of the module still lowers. → Task 1 transpiler test.

---

### Task 1: `transpile_module` — import/export lowering (pure)

Add the per-module lowering entry point and its return shape. Purely additive: the existing `transpile` / `imports::rewrite` strip-path is left untouched this task (Task 2 retires it), so all existing tests stay green.

**Files:**
- Create: `crates/supersolid/src/module.rs` (the `Module` type + `transpile_module`)
- Modify: `crates/supersolid/src/lib.rs` (`mod module; pub use`)
- Modify: `crates/supersolid/src/imports.rs` (add a `lower_to_require` classification path producing `require(...)` rewrites + `deps`; keep the existing `rewrite` for now)
- Modify: `crates/supersolid/src/pipeline.rs` (add a module-mode entry that lowers JSX, then imports→require, then exports→`exports.*`, returning the factory body + deps/css/json)
- Test: `crates/supersolid/src/module.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Produces:
  - `pub struct Module { pub factory_code: String, pub deps: Vec<String>, pub css_imports: Vec<String>, pub json_imports: Vec<(String, String)>, pub diagnostics: Vec<Diagnostic> }`
  - `pub fn transpile_module(source: &str, options: &TranspileOptions) -> Module`
  - `factory_code` is the module **body only** (no registry wrapper, no `function(...)` envelope) — just the lowered statements, suitable to drop inside `assemble`'s factory.

**Lowering table (from spec §2):**

Imports — classify by specifier, same four-way split as today's `imports.rs`, but the "anything else" arm now rewrites instead of warning:

| Specifier | Action |
| --- | --- |
| runtime (`options.runtime_specifiers`) | strip; names stay global/free inside the factory (unchanged) |
| `*.css` (case-insensitive) | push to `css_imports`; strip from JS |
| `*.json` (case-insensitive) | push `(binding, specifier)` to `json_imports`; strip from JS (unchanged recording) |
| anything else (relative or bare) | rewrite to a `require("<raw specifier>")` binding; push raw specifier to `deps` |

Import binding forms:
- `import { a, b as c } from "x"` → `const { a, b: c } = require("x");`
- `import d from "x"` → `const d = require("x").default;`
- `import * as ns from "x"` → `const ns = require("x");`
- `import "x"` (side-effect) → `require("x");`

Exports (new — none exist today):
- `export function F` / `export const x` / `export class C` → emit the declaration unchanged, then append `exports.F = F;` (and still register `$ss.hot` for component-shaped bindings exactly as the current `jsx::lower` does — exported components get both the `$ss.hot(...)` registration and the `exports.* =` line).
- `export { a, b as c }` → `exports.a = a; exports.c = b;`
- `export default E` → `exports.default = E;`
- `export * from "x"` and dynamic `import()` → push one `Warning` to `diagnostics`, skip (no re-export wiring).

- [x] **Step 1: Write failing tests** in `crates/supersolid/src/module.rs`. Each transpiles a snippet and asserts on `Module`. Reuse the crate-private `reparses_as_plain_js` helper (make it `pub(crate)` if not already) to assert `factory_code` is valid plain JS.

```rust
// Imports → require
#[test] fn named_import_lowers_to_require_destructure() {
    let m = transpile_module(r#"import { a, b as c } from "./x";"#, &TranspileOptions::default());
    assert!(m.factory_code.contains(r#"const { a, b: c } = require("./x")"#), "{}", m.factory_code);
    assert_eq!(m.deps, vec!["./x".to_string()]);
    assert!(m.diagnostics.is_empty());
    assert!(reparses_as_plain_js(&m.factory_code));
}
#[test] fn default_import_lowers_to_require_default() {
    let m = transpile_module(r#"import d from "./x";"#, &TranspileOptions::default());
    assert!(m.factory_code.contains(r#"const d = require("./x").default"#), "{}", m.factory_code);
}
#[test] fn namespace_import_lowers_to_require() {
    let m = transpile_module(r#"import * as ns from "./x";"#, &TranspileOptions::default());
    assert!(m.factory_code.contains(r#"const ns = require("./x")"#), "{}", m.factory_code);
}
#[test] fn side_effect_import_lowers_to_bare_require() {
    let m = transpile_module(r#"import "./x";"#, &TranspileOptions::default());
    assert!(m.factory_code.contains(r#"require("./x")"#), "{}", m.factory_code);
    assert_eq!(m.deps, vec!["./x".to_string()]);
}
#[test] fn bare_specifier_is_a_dep_not_a_warning() {
    let m = transpile_module(r#"import { Btn } from "ui-kit";"#, &TranspileOptions::default());
    assert_eq!(m.deps, vec!["ui-kit".to_string()]);
    assert!(m.diagnostics.is_empty(), "bare imports resolve now; no warn: {:?}", m.diagnostics);
}
// Runtime / css / json classification unchanged
#[test] fn runtime_import_still_strips_silently() {
    let m = transpile_module(r#"import { createSignal } from "solid-js"; createSignal(0);"#, &TranspileOptions::default());
    assert!(!m.factory_code.contains("require"), "{}", m.factory_code);
    assert!(m.deps.is_empty()); assert!(m.diagnostics.is_empty());
}
#[test] fn css_import_recorded_and_stripped() {
    let m = transpile_module(r#"import "./a.css";"#, &TranspileOptions::default());
    assert_eq!(m.css_imports, vec!["./a.css".to_string()]);
    assert!(!m.factory_code.contains("require"), "css not a require: {}", m.factory_code);
}
#[test] fn json_import_recorded_and_stripped() {
    let m = transpile_module(r#"import data from "./d.json";"#, &TranspileOptions::default());
    assert_eq!(m.json_imports, vec![("data".into(), "./d.json".into())]);
}
// Exports → exports.*
#[test] fn export_function_emits_decl_and_exports_binding() {
    let m = transpile_module("export function F(){ return 1; }", &TranspileOptions::default());
    assert!(m.factory_code.contains("function F"), "{}", m.factory_code);
    assert!(m.factory_code.contains("exports.F = F"), "{}", m.factory_code);
}
#[test] fn export_const_emits_exports_binding() {
    let m = transpile_module("export const x = 1;", &TranspileOptions::default());
    assert!(m.factory_code.contains("exports.x = x"), "{}", m.factory_code);
}
#[test] fn export_named_list_with_alias() {
    let m = transpile_module("const a=1,b=2; export { a, b as c };", &TranspileOptions::default());
    assert!(m.factory_code.contains("exports.a = a"), "{}", m.factory_code);
    assert!(m.factory_code.contains("exports.c = b"), "{}", m.factory_code);
}
#[test] fn export_default_emits_default_binding() {
    let m = transpile_module("export default 42;", &TranspileOptions::default());
    assert!(m.factory_code.contains("exports.default = 42"), "{}", m.factory_code);
}
#[test] fn exported_component_is_registered_and_exported() {
    let m = transpile_module("export function Counter(){ return <div/>; }",
        &TranspileOptions { module_id: Some("counter.tsx".into()), ..Default::default() });
    assert!(m.factory_code.contains(r#"$ss.hot("counter.tsx#Counter", Counter)"#), "{}", m.factory_code);
    assert!(m.factory_code.contains("exports.Counter = Counter"), "{}", m.factory_code);
}
#[test] fn export_star_warns_and_skips() {
    let m = transpile_module(r#"export * from "./barrel";"#, &TranspileOptions::default());
    assert_eq!(m.diagnostics.len(), 1);
    assert_eq!(m.diagnostics[0].severity, Severity::Warning);
    assert!(reparses_as_plain_js(&m.factory_code));
}
#[test] fn dynamic_import_warns_and_skips() {
    let m = transpile_module(r#"const p = import("./x");"#, &TranspileOptions::default());
    assert_eq!(m.diagnostics.iter().filter(|d| d.severity == Severity::Warning).count(), 1);
    assert!(reparses_as_plain_js(&m.factory_code));
}
```

- [x] **Step 2: Run tests, verify they fail.** Run: `cargo test -p supersolid module::tests` — Expected: FAIL (`transpile_module` not found).
- [x] **Step 3: Implement `transpile_module`.** Add a module-mode path in `pipeline.rs` that runs parse → TS-strip (JSX preserved) → `jsx::lower` (already appends `$ss.hot`) → import-lowering (`imports::lower_to_require`, new) → export-lowering, then codegen. The import-lowering replaces each `ImportDeclaration` in place with the `require` `const`/statement per the table; export-lowering replaces/augments each `ExportNamedDeclaration` / `ExportDefaultDeclaration` and warns on `ExportAllDeclaration`; detect dynamic `import()` via the call-expression `ImportExpression` node. Build `Module` from the codegen output + collected `deps`/`css_imports`/`json_imports`/`diagnostics`. Component-shaped-binding detection reuses `jsx::top_level_component_name`.
- [x] **Step 4: Run tests, verify they pass.** Run: `cargo test -p supersolid module::tests` — Expected: PASS. Also run `cargo test -p supersolid` — Expected: all existing transpiler tests still PASS (strip-path untouched).
- [x] **Step 5: Commit.**

```bash
git add crates/supersolid/src/module.rs crates/supersolid/src/lib.rs crates/supersolid/src/imports.rs crates/supersolid/src/pipeline.rs
git commit -m "feat(supersolid): add transpile_module ESM->factory lowering"
```

---

### Task 2: Registry ABI, `assemble`, `resolve_candidates`, `walk_sync` + rewire `transpile`

Add the bundle assembler, resolver, and generic graph walker (all pure), then make the existing single-file `transpile` a thin wrapper over `transpile_module` + one-module `assemble` (spec §2) and retire the old strip-path.

**Files:**
- Modify: `crates/supersolid/src/module.rs` (add `ResolvedModule`, `Bundle`, the `$ssmod` prelude, `assemble`, `resolve_candidates`, `walk_sync`)
- Modify: `crates/supersolid/src/lib.rs` (rewrite `transpile` as a one-module wrapper; re-export the new items; delete the now-obsolete warn tests)
- Modify: `crates/supersolid/src/imports.rs` (delete the obsolete `rewrite` strip-path once `transpile` no longer uses it; keep only `lower_to_require`)
- Test: `crates/supersolid/src/module.rs`

**Interfaces:**
- Consumes: `transpile_module` / `Module` (Task 1).
- Produces:
  - `pub struct ResolvedModule { pub factory_code: String, pub edges: Vec<(String, String)>, pub css: String, pub json_prelude: String }` — `edges` = raw-specifier → resolved-id; `css` = this module's own `.css` text (its css imports concatenated in source order); `json_prelude` = resolved `const … = …;` lines to inject at the top of the factory.
  - `pub struct Bundle { pub code: String, pub css: String, pub module_ids: Vec<String>, pub diagnostics: Vec<Diagnostic> }`
  - `pub fn assemble(entry_id: &str, modules: &std::collections::BTreeMap<String, ResolvedModule>) -> Bundle`
  - `pub fn resolve_candidates(importer_id: &str, specifier: &str, modules_root: &str) -> Vec<String>` — ordered, pure, no existence check. `modules_root` is the bare-import root (default `"superui_modules"`).
  - `pub fn walk_sync(entry_id: &str, options_for: impl Fn(&str) -> TranspileOptions, read: impl FnMut(&str) -> std::io::Result<Vec<u8>>) -> Bundle`
    - `options_for(id)` yields the `TranspileOptions` for a given module id (walker sets `module_id = id`, `tsx` from the id's extension).

**The `$ssmod` prelude — emit verbatim (spec §1):**

```js
var $ssmod = (function () {
  var mods = {}, cache = {};
  function def(id, edges, factory) { mods[id] = { edges: edges, factory: factory }; }
  function req(id) {
    if (cache[id]) return cache[id].exports;
    var m = mods[id];
    if (!m) { console.error("superui: module not found: " + id); return {}; }
    var module = { exports: {} };
    cache[id] = module;
    var require = function (spec) { return req(m.edges[spec]); };
    m.factory(module.exports, module, require);
    return module.exports;
  }
  return { def: def, run: req };
})();
```

`assemble` emits: the prelude, then one `$ssmod.def("<id>", <edges-object>, function (exports, module, require) { <json_prelude><factory_code> });` per module (module-id order is fine; `def` order is irrelevant since `run` is lazy), then `$ssmod.run("<entry_id>");`. `edges` is a JS object literal of `"rawspec": "resolvedid"`. `Bundle.css` = graph `.css` concatenated **post-order** (DFS from entry over `edges` values, each module's `css` appended after its deps'); `module_ids` in that post-order; an unresolved edge target with no module is the sentinel — skip it in the CSS/`module_ids` walk.

**Resolution (spec §3):** sentinel id for an unresolved/unreadable target = `format!("superui:unresolved:{specifier}")` (never a real file path, reparses cleanly as a JS string; `req` finds no factory and logs "module not found"). `resolve_candidates`:
1. Relative (`./`, `../`) → `superui_paths::join_asset(parent_dir(importer_id), spec)` then normalize `.`/`..` segments; forward-slash.
2. Bare (no leading `.`/`/`) → `"<modules_root>/<spec>"`.
3. Extension/index ladder for a candidate base without an exact-file hit, in order: `<base>.tsx`, `<base>.ts`, `<base>.js`, `<base>/index.tsx`, `<base>/index.ts`, `<base>/index.js`. For a bare root also include `<modules_root>/<spec>/index.{tsx,ts,js}` and `<modules_root>/<spec>.{tsx,ts,js}`. (`package.json` `main` is a documented fast-follow — not implemented.) The walker treats the **first candidate that `read`s without error** as the canonical id.

`.css` / `.json` specifiers are resolved by `join_asset` (relative) only and are **not** run through this ladder.

- [x] **Step 1: Write failing tests** in `crates/supersolid/src/module.rs`:

```rust
// resolve_candidates
#[test] fn relative_candidate_ladder_order() {
    let c = resolve_candidates("ui/app.tsx", "./label", "superui_modules");
    assert_eq!(c[0], "ui/label.tsx");
    assert_eq!(c, vec!["ui/label.tsx","ui/label.ts","ui/label.js",
        "ui/label/index.tsx","ui/label/index.ts","ui/label/index.js"]);
}
#[test] fn parent_relative_normalizes_dotdot() {
    let c = resolve_candidates("ui/screens/main.tsx", "../lib/util", "superui_modules");
    assert_eq!(c[0], "ui/lib/util.tsx");
}
#[test] fn bare_resolves_under_modules_root() {
    let c = resolve_candidates("ui/app.tsx", "ui-kit", "superui_modules");
    assert!(c.contains(&"superui_modules/ui-kit.tsx".to_string()));
    assert!(c.contains(&"superui_modules/ui-kit/index.tsx".to_string()));
}
// assemble
#[test] fn assemble_emits_prelude_and_entry_run() {
    let mut m = std::collections::BTreeMap::new();
    m.insert("app.tsx".to_string(), ResolvedModule { factory_code: "exports.x=1;".into(),
        edges: vec![], css: String::new(), json_prelude: String::new() });
    let b = assemble("app.tsx", &m);
    assert!(b.code.contains("var $ssmod ="), "{}", b.code);
    assert!(b.code.contains(r#"$ssmod.def("app.tsx""#), "{}", b.code);
    assert!(b.code.contains(r#"$ssmod.run("app.tsx")"#), "{}", b.code);
    assert!(reparses_as_plain_js(&b.code), "{}", b.code);
}
#[test] fn assemble_edges_map_raw_spec_to_id() {
    let mut m = std::collections::BTreeMap::new();
    m.insert("app.tsx".into(), ResolvedModule { factory_code: r#"var c = require("./c.tsx");"#.into(),
        edges: vec![("./c.tsx".into(), "c.tsx".into())], css: String::new(), json_prelude: String::new() });
    m.insert("c.tsx".into(), ResolvedModule { factory_code: "exports.v=1;".into(),
        edges: vec![], css: String::new(), json_prelude: String::new() });
    let b = assemble("app.tsx", &m);
    assert!(b.code.contains(r#""./c.tsx": "c.tsx""#), "{}", b.code);
}
#[test] fn assemble_concats_css_post_order() {
    // entry imports dep; dep css must precede entry css (importer overrides importee).
    let mut m = std::collections::BTreeMap::new();
    m.insert("app.tsx".into(), ResolvedModule { factory_code: String::new(),
        edges: vec![("./d.tsx".into(),"d.tsx".into())], css: ".app{}".into(), json_prelude: String::new() });
    m.insert("d.tsx".into(), ResolvedModule { factory_code: String::new(),
        edges: vec![], css: ".dep{}".into(), json_prelude: String::new() });
    let b = assemble("app.tsx", &m);
    assert!(b.css.find(".dep{}").unwrap() < b.css.find(".app{}").unwrap(), "post-order: {}", b.css);
}
// walk_sync over an in-memory map
#[test] fn walk_sync_diamond_shares_singleton() {
    let files = std::collections::HashMap::from([
        ("app.tsx".to_string(), r#"import {b} from "./b"; import {c} from "./c";"#.to_string()),
        ("b.tsx".to_string(), r#"import {d} from "./d"; export const b = d;"#.to_string()),
        ("c.tsx".to_string(), r#"import {d} from "./d"; export const c = d;"#.to_string()),
        ("d.tsx".to_string(), r#"export const d = 1;"#.to_string()),
    ]);
    let b = walk_sync("app.tsx", |id| TranspileOptions{ module_id: Some(id.into()), ..Default::default() },
        |p| files.get(p).map(|s| s.clone().into_bytes())
            .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::NotFound)));
    assert_eq!(b.code.matches(r#"$ssmod.def("d.tsx""#).count(), 1, "d defined once (singleton)");
    assert!(reparses_as_plain_js(&b.code), "{}", b.code);
}
#[test] fn walk_sync_cycle_terminates_with_partial_exports() {
    let files = std::collections::HashMap::from([
        ("a.tsx".to_string(), r#"import {b} from "./b"; export const a = 1;"#.to_string()),
        ("b.tsx".to_string(), r#"import {a} from "./a"; export const b = 2;"#.to_string()),
    ]);
    let b = walk_sync("a.tsx", |id| TranspileOptions{ module_id: Some(id.into()), ..Default::default() },
        |p| files.get(p).map(|s| s.clone().into_bytes())
            .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::NotFound)));
    assert!(b.code.contains(r#"$ssmod.def("a.tsx""#) && b.code.contains(r#"$ssmod.def("b.tsx""#));
    assert!(reparses_as_plain_js(&b.code));
}
#[test] fn walk_sync_unresolved_dep_warns_and_sentinels() {
    let files = std::collections::HashMap::from([
        ("a.tsx".to_string(), r#"import {x} from "./gone";"#.to_string())]);
    let b = walk_sync("a.tsx", |id| TranspileOptions{ module_id: Some(id.into()), ..Default::default() },
        |p| files.get(p).map(|s| s.clone().into_bytes())
            .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::NotFound)));
    assert!(b.diagnostics.iter().any(|d| d.severity == Severity::Warning && d.message.contains("./gone")));
    assert!(b.code.contains("superui:unresolved:./gone"), "sentinel edge: {}", b.code);
    assert!(reparses_as_plain_js(&b.code));
}
#[test] fn walk_sync_collects_css_import() {
    let files = std::collections::HashMap::from([
        ("a.tsx".to_string(), r#"import "./a.css"; export const a=1;"#.to_string()),
        ("a.css".to_string(), ".x{color:red}".to_string())]);
    let b = walk_sync("a.tsx", |id| TranspileOptions{ module_id: Some(id.into()), ..Default::default() },
        |p| files.get(p).map(|s| s.clone().into_bytes())
            .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::NotFound)));
    assert!(b.css.contains(".x{color:red}"), "css collected: {}", b.css);
}
```

Also update the two obsolete tests in `lib.rs` (`unknown_module_imports_warn`, `unused_unknown_module_import_still_warns_once`): a relative import is no longer a warning — `transpile`'s output now contains `require("./other")` and `r.diagnostics` is empty for it. Convert them to assert `r.code.contains(r#"require(\"./other\")"#)` and `r.diagnostics.is_empty()`, or delete them (the behavior is now owned by Task 1's `bare_specifier_is_a_dep_not_a_warning` / Task 2's walker tests).

- [x] **Step 2: Run tests, verify they fail.** Run: `cargo test -p supersolid module::tests` — Expected: FAIL.
- [x] **Step 3: Implement** `ResolvedModule`, `Bundle`, the prelude constant, `assemble`, `resolve_candidates`, `walk_sync`. Then rewrite `supersolid::transpile` (`lib.rs`) as: `let m = transpile_module(src, opts); let bundle = assemble(entry, &{one ResolvedModule from m with empty edges/css/json_prelude});` and map `m.css_imports`/`m.json_imports`/`m.diagnostics` onto the existing `TranspileResult` fields (single-file `transpile` still only *records* css/json — no I/O). `TranspileResult.code = bundle.code`. Delete `imports::rewrite` and its call site. `walk_sync`: BFS queue from `entry_id` + `seen` set; per id `read` → `transpile_module` → for each dep try `resolve_candidates` in order via `read`, first success = edge target (enqueue if unseen), else sentinel + warning; resolve/read each css import (`join_asset`, warn+skip on miss) into the module's `css`; resolve/read each json import + `json_binding` into `json_prelude` (warn+skip); accumulate diagnostics; finally `assemble`.
- [x] **Step 4: Run tests, verify they pass.** Run: `cargo test -p supersolid` — Expected: ALL pass (new + existing, with the two warn-tests updated).
- [x] **Step 5: Commit.**

```bash
git add crates/supersolid/src/module.rs crates/supersolid/src/lib.rs crates/supersolid/src/imports.rs
git commit -m "feat(supersolid): add $ssmod registry, resolver, and graph walker"
```

---

### Task 3: `JsSource { code, css }` struct migration

Mechanical refactor: turn the tuple struct `JsSource(pub String)` into `JsSource { pub code: String, pub css: String }` (spec §5) and fix every consumer. `css` defaults empty; no behavior change yet (mount ignores `css` until Task 5).

**Files:**
- Modify: `crates/superui/src/assets.rs:16` (struct + both loaders + `TsxLoader` + its tests)
- Modify: `crates/superui/src/mount.rs:378` (`j.0.clone()` → `j.code.clone()`)
- Modify: `crates/superui/src/hot_reload.rs:114` (`j.0.clone()` → `j.code.clone()`)
- Modify: `crates/superui_playground_web/src/lib.rs:173` (`a.0 = code` → `a.code = code`)
- Modify: `crates/superui/tests/integration.rs:60` (`js.0 = …` → `js.code = …`)
- Test: existing tests in the files above are the regression gate.

**Interfaces:**
- Produces: `pub struct JsSource { pub code: String, pub css: String }`. Construct with `JsSource { code, css: String::new() }`. The `HtmlLoader` is untouched; `JsLoader` and the JSON-loader body in `TsxLoader` set `css: String::new()`.

- [x] **Step 1: Change the struct and all call sites.** Update `assets.rs` loaders to `Ok(JsSource { code: …, css: String::new() })`. Update the three reads and the test assignment. In `assets.rs` tests, `jss.get(&handle).unwrap().0` → `.code`.
- [x] **Step 2: Build to find every remaining `.0`.** Run: `cargo build -p superui -p superui_playground_web` — Expected: compiles (fix any missed `.0`).
- [x] **Step 3: Run tests.** Run: `cargo test -p superui -p superui_playground_web` — Expected: PASS (unchanged behavior).
- [x] **Step 4: Commit.**

```bash
git add crates/superui/src/assets.rs crates/superui/src/mount.rs crates/superui/src/hot_reload.rs crates/superui_playground_web/src/lib.rs crates/superui/tests/integration.rs
git commit -m "refactor(superui): give JsSource a css field alongside code"
```

---

### Task 4: `TsxLoader` async graph walk + graph-wide HMR

Replace `TsxLoader`'s single-file transpile with an async graph walk that reuses the pure `transpile_module` / `resolve_candidates` / `assemble` (it cannot call `walk_sync` because `read_asset_bytes` is `async`). Every resolved module id and every `.css`/`.json` file is read via `lc.read_asset_bytes`, which registers it as a load dependency → editing any graph file re-runs the loader → graph-wide HMR (spec §4).

**Files:**
- Modify: `crates/superui/src/assets.rs` (`TsxLoader::load`)
- Test: `crates/superui/src/assets.rs` `#[cfg(test)] mod tests` (extend) and `crates/superui/tests/integration.rs` (HMR)

**Interfaces:**
- Consumes: `supersolid::{transpile_module, resolve_candidates, assemble, ResolvedModule, Bundle}`.
- Produces: `JsSource { code: bundle.code, css: bundle.css }`.

**Async walk shape:** BFS over a `Vec` work-queue of resolved ids + a `HashSet` seen, entry = `lc.path()`. Per id: `lc.read_asset_bytes(id).await` → `transpile_module` (opts: `tsx` from extension, `module_id = id`, default `runtime_specifiers`). For each dep: iterate `resolve_candidates(id, spec, "superui_modules")`, `read_asset_bytes` each in order; first `Ok` = edge target (enqueue if unseen); all-miss = sentinel + `bevy::log::warn!`. For each css import: `read_asset_bytes(join_asset(parent, spec))`, accumulate (warn+skip on miss). For each json import: `read_asset_bytes` + `supersolid::json_binding` → `json_prelude` (warn+skip). Build the `BTreeMap<String, ResolvedModule>`, call `assemble(entry, &map)`, log `bundle.diagnostics`, return `JsSource { code, css }`. Graceful degradation: never return `Err` for a resolve/transpile warning.

- [x] **Step 1: Write failing tests** (MemoryAssetReader, same harness as the existing `TsxLoader` tests):

```rust
#[cfg(not(target_arch = "wasm32"))]
#[test] fn tsx_loader_bundles_a_two_file_graph() {
    // app imports a sibling; the bundle defines both modules and runs the entry.
    let dir = Dir::new("assets".into());
    dir.insert_asset("counter.tsx".as_ref(),
        br#"export function Counter(){ return <div class="c"/>; }"#);
    dir.insert_asset("app.tsx".as_ref(),
        br#"import { Counter } from "./counter.tsx"; render(() => <Counter/>, root);"#);
    // … load "app.tsx" as JsSource, tick to Loaded …
    // assert out.code.contains(r#"$ssmod.def("app.tsx""#) and r#"$ssmod.def("counter.tsx""#)
    // assert out.code.contains(r#"$ss.el("div")"#)  // counter's JSX lowered into the bundle
    // assert out.code.contains(r#"$ssmod.run("app.tsx")"#)
}
#[cfg(not(target_arch = "wasm32"))]
#[test] fn tsx_loader_missing_dep_degrades_gracefully() {
    // app imports "./gone.tsx" which doesn't exist → load still Loaded, bundle has a sentinel edge.
}
#[cfg(not(target_arch = "wasm32"))]
#[test] fn tsx_loader_collects_module_css_onto_jssource() {
    // a module imports "./c.css"; the loaded JsSource.css contains that rule.
}
```

And a graph-wide HMR integration test in `crates/superui/tests/integration.rs` modeled on `hot_reload_js_re_executes_and_reconciles` (lines 33-76), but editing the **imported** module's asset (not the entry) and firing `AssetEvent::Modified` for the entry's `JsSource` handle (re-running the loader re-bundles). Assert the DOM reflects the edit, proving editing a non-entry graph file reloads. (A signal-preservation assertion is a bonus if the harness supports it; the behavior is inherited from the existing `$ss.hot` path and module-qualified ids.)

- [x] **Step 2: Run tests, verify they fail.** Run: `cargo test -p superui --test integration; cargo test -p superui assets::tests` — Expected: FAIL (single-file loader doesn't bundle the sibling).
- [x] **Step 3: Implement the async walk** in `TsxLoader::load`.
- [x] **Step 4: Run tests, verify they pass.** Run: `cargo test -p superui` — Expected: PASS (new + existing loader/json/HMR tests).
- [x] **Step 5: Commit.**

```bash
git add crates/superui/src/assets.rs crates/superui/tests/integration.rs
git commit -m "feat(superui): walk the import graph in TsxLoader for multi-file apps"
```

---

### Task 5: `mount.rs` merges bundled CSS after the `<link>` sheet

Apply the module-graph `.css` (`JsSource.css`) by appending it after the entry `<link>` stylesheet into the single applied `StyleSheet` the runtime carries (spec §5). Needs a `StyleSheet` concatenation primitive on the vendored flair crate.

**Files:**
- Modify: `crates/superui_flair_style/src/style_sheet.rs` (add `StyleSheet::extend`) — vendored flair edit; follow the flair-deviation convention (REQUIRED SUB-SKILL at execution time: `releasing-crates`, for marking a vendored-fork deviation).
- Modify: `crates/superui/src/mount.rs` (Phase 2: parse `js_src.css` and merge into the applied sheet)
- Test: `crates/superui_flair_style/src/style_sheet.rs` (extend unit test) and `crates/superui/tests/integration.rs` (module CSS applies)

**Interfaces:**
- Produces: `impl StyleSheet { pub fn extend(&mut self, other: StyleSheet) }` — appends `other`'s `font_faces`, `block_handles`, `css_selectors_to_blocks`, and merges `resolved_font_faces` / `animation_keyframes` (later wins). Appending `other` after `self` means `other`'s equal-specificity rules win the cascade (source order), so bundled graph CSS layered after the `<link>` sheet overrides it.
- Consumes (mount): `superui_css::parser::InlineCssStyleSheetParser` (`SystemState`-driven, see the playground's `drain_playground_edits` for the exact `SystemState`/scoped-borrow pattern) to parse `js_src.css` into a `StyleSheet`.

**Mount wiring (Phase 2, `mount_when_ready`):** after loading `js_src`, if `!js_src.css.is_empty()`: parse it to a `StyleSheet`; obtain the base applied sheet (the `<link>` sheet via `css_handle`, or `StyleSheet::empty()` when the document declares no `<link>`); `base.extend(parsed)`; store the merged sheet as the asset the runtime carries. Because `UiRuntime::new` takes a `Handle<StyleSheet>`, insert/overwrite the merged `StyleSheet` into `Assets<StyleSheet>` and pass that handle (reuse the existing `css_handle` slot when present; otherwise create one). Keep it inside the existing two-phase mount — no new asset type.

- [x] **Step 1: Write failing tests.**
  - Flair: `#[test] fn extend_appends_rules()` — build two `StyleSheet`s via the builder, `a.extend(b)`, assert `a` now matches a selector only `b` had.
  - Integration (`crates/superui/tests/integration.rs`): mount a `.tsx` entry whose imported module does `import "./c.css"` with a rule (e.g. `.c { width: 10px }`), and assert the reconciled element with that class resolves the property (mirror how `slider_positioning.rs` / existing integration tests read computed style). Minimal viable assertion: the merged applied `StyleSheet` contains the module rule's selector.
- [x] **Step 2: Run tests, verify they fail.** Run: `cargo test -p superui_flair_style style_sheet; cargo test -p superui --test integration` — Expected: FAIL.
- [x] **Step 3: Implement** `StyleSheet::extend` (mark the flair deviation) and the mount merge.
- [x] **Step 4: Run tests, verify they pass.** Run: `cargo test -p superui_flair_style -p superui` — Expected: PASS.
- [x] **Step 5: Commit.**

```bash
git add crates/superui_flair_style/src/style_sheet.rs crates/superui/src/mount.rs crates/superui/tests/integration.rs docs/
git commit -m "feat(superui): merge module-graph CSS after the link stylesheet"
```

---

### Task 6: `build.rs` bundles each entry's graph via `walk_sync`

Switch the build-script helper from per-file `transpile_file` to a per-top-level-entry `walk_sync` bundle, emitting `cargo:rerun-if-changed` for every resolved id + `.css` (spec §4).

**Files:**
- Modify: `crates/supersolid/src/build.rs` (`transpile_dir_impl`)
- Modify: `crates/supersolid/src/lib.rs` (`transpile_file` may delegate to `walk_sync`, or build.rs calls `walk_sync` directly; keep `transpile_file`'s public signature + CLI test green — `crates/supersolid/tests/cli.rs`)
- Test: `crates/supersolid/src/build.rs` `#[cfg(test)] mod tests` (extend)

**Interfaces:**
- Consumes: `supersolid::walk_sync`.
- `build.rs` does not parse `index.html`, so it keeps treating **every top-level `.tsx`/`.ts` as its own entry** (unchanged) and bundles that file's graph. A dependency-only file still gets its own (correct, possibly unused) artifact. Reads via a `std::fs` closure rooted at `ui_dir` (ids are asset-root-relative under `ui_dir`; the read closure maps id → `std::fs::read` of the on-disk path). `module_id` for each walked id must match the asset path convention the runtime uses (so `$ss.hot` ids line up with the non-generated mount path).

- [x] **Step 1: Write a failing test** in `build.rs`:

```rust
#[test] fn bundles_a_two_file_graph_for_the_entry() {
    let dir = temp_ui_dir("graph");
    std::fs::write(dir.join("label.tsx"), r#"export function Label(){ return <span/>; }"#).unwrap();
    std::fs::write(dir.join("app.tsx"),
        r#"import { Label } from "./label.tsx"; render(() => <Label/>, root);"#).unwrap();
    let dir_str = dir.to_string_lossy().replace('\\', "/");
    transpile_dir_impl(&dir_str, false);
    let js = std::fs::read_to_string(dir.join(".superui").join("build").join("app.js")).unwrap();
    assert!(js.contains(r#"$ssmod.def("#) && js.contains("$ss.el(\"span\")"),
        "entry artifact must bundle the imported module:\n{js}");
}
```

Keep `writes_generated_js_for_each_tsx`, `inlines_json_import_as_const`, `skip_flag_writes_nothing` green (a single-file `.tsx` is a one-module bundle; the inlined-json assertion still holds because json is inlined into the factory — adjust the assertion to match the factory-scoped `const data = …` if its position shifted, still a top-level-of-factory `const`).

- [x] **Step 2: Run test, verify it fails.** Run: `cargo test -p supersolid build::tests` — Expected: FAIL.
- [x] **Step 3: Implement.** For each top-level `.tsx`/`.ts`, `walk_sync` from `"<ui_dir-as-id>/<stem>.<ext>"` with a `std::fs` read closure; write `bundle.code` to `generated_js(src)`; `println!("cargo:rerun-if-changed=…")` for every `bundle.module_ids` entry and every css file read (thread the css paths out of the walk — extend `walk_sync` to surface read file paths, or have build.rs's read closure record each path it served). Keep diagnostics → `cargo:warning`.
- [x] **Step 4: Run tests, verify they pass.** Run: `cargo test -p supersolid` — Expected: PASS (incl. `tests/cli.rs`).
- [x] **Step 5: Commit.**

```bash
git add crates/supersolid/src/build.rs crates/supersolid/src/lib.rs
git commit -m "feat(supersolid): bundle each entry's import graph in build.rs"
```

---

### Task 7: test-engine drives the walker (spec imports + app-under-test)

Two sites in the test-engine must bundle graphs: `transpile_spec` (`.spec.ts` imports) and the app-under-test (`load_project` / `host.rs`), so a split multi-file app renders under test (spec §4; demonstrator requirement). A missing import stays **fatal** here.

**Files:**
- Modify: `crates/superui_test_engine/src/transpile.rs` (`transpile_spec` → `walk_sync`, missing = `Err`)
- Modify: `crates/superui_test_engine/src/config.rs` (`load_project` bundles the on-disk app graph via `walk_sync` rooted at `project_dir`; carry the bundle + bundled css on `HostProject`)
- Modify: `crates/superui_test_engine/src/host.rs` (register the pre-bundled app code at `.superui/build/app.js` without re-transpiling; concat bundled css into the registered stylesheet)
- Test: `crates/superui_test_engine/src/transpile.rs` tests + `crates/superui_test_engine/tests/` (new: a two-file spec-import bundle; a missing spec import errors)

**Interfaces:**
- Consumes: `supersolid::walk_sync`.
- `transpile_spec(source, module_id, base_dir)` keeps its signature; internally `walk_sync` from a synthetic spec entry id with a `std::fs` read closure rooted at `base_dir`; a dep that no candidate reads → return `Err` (fatal). Runtime specifiers stay `["supersolid","solid-js","superui/test"]`. `.json` still inlined (now via the walker's `json_prelude`).
- `HostProject` gains `pub bundled_css: String` (default empty). When `app.tsx`/`app.ts` exists, `load_project` runs `walk_sync` from `app.tsx` reading siblings from `project_dir`; stores `js_or_tsx` = `bundle.code`, `bundled_css` = `bundle.css`. `host.rs` registers `js_or_tsx` directly at `<root>/.superui/build/app.js` (no second transpile) and appends `bundled_css` to the stylesheet text it registers at `<root>/theme.css`.

- [x] **Step 1: Write failing tests** in `crates/superui_test_engine/tests/`:

```rust
#[test] fn spec_bundles_a_helper_import() {
    // tmp dir: helpers.ts exports `expectThing`; t.spec.ts imports it from "./helpers".
    // transpile_spec(src, "t.spec.ts", tmp) -> Ok, bundle defines both, runs the spec entry.
}
#[test] fn spec_missing_import_is_fatal() {
    // t.spec.ts imports "./gone"; transpile_spec -> Err.
    assert!(transpile_spec(r#"import {x} from "./gone"; x;"#, "t.spec.ts", std::path::Path::new(".")).is_err());
}
```

Keep `strips_superui_test_import_and_keeps_body` green (a single-file spec is a one-module bundle; assert `test(` body survives and types are stripped; the `import` is gone).

- [x] **Step 2: Run tests, verify they fail.** Run: `cargo test -p superui_test_engine` — Expected: the two new tests FAIL.
- [x] **Step 3: Implement** the `walk_sync` rewires in `transpile.rs`, `config.rs`, `host.rs`. Preserve the existing empty-output fatal check in `transpile_spec`.
- [x] **Step 4: Run tests, verify they pass.** Run: `cargo test -p superui_test_engine` — Expected: PASS.
- [x] **Step 5: Commit.**

```bash
git add crates/superui_test_engine/src/transpile.rs crates/superui_test_engine/src/config.rs crates/superui_test_engine/src/host.rs
git commit -m "feat(test-engine): bundle spec and app-under-test import graphs"
```

---

### Task 8: Expand `game_menu` spec to four screens + commit baselines (single-file)

**Before** touching `app.tsx`, expand `tests/game_menu.spec.ts` to snapshot the **pause** and **game-over** screens and commit those baselines green — the four-screen green baseline the split must preserve (spec §Testing).

**Files:**
- Modify: `examples/game_menu/tests/game_menu.spec.ts`
- Create: `examples/game_menu/tests/__snapshots__/game_menu.spec.ts/pause-windows.png`, `over-windows.png` (generated)

**Interfaces:**
- Consumes: the existing single-file `examples/game_menu/assets/ui/game_menu/app.tsx` (unchanged this task).

- [x] **Step 1: Add two tests** after `"main menu renders"` (which MUST stay first — the spec shares one mount; see the file's test-order note), following the existing SETTINGS pattern (`page.locator(".tabs .tab", { hasText: "…" }).click()`):

```ts
test("tab bar navigates to pause", async ({ page }) => {
  await page.locator(".tabs .tab", { hasText: "PAUSE" }).click();
  await expect(page.locator(".pause-card")).toBeVisible();
  await expect(page.locator(".tabs .tab.active")).toHaveText("PAUSE");
  await expect(page).toHaveScreenshot("pause.png");
});

test("tab bar navigates to game over", async ({ page }) => {
  await page.locator(".tabs .tab", { hasText: "GAME OVER" }).click();
  await expect(page.locator(".screen.over")).toBeVisible();
  await expect(page.locator(".tabs .tab.active")).toHaveText("GAME OVER");
  await expect(page).toHaveScreenshot("over.png");
});
```

- [x] **Step 2: Generate the baselines** by running the game_menu test runner in snapshot-update mode (the same command the repo already uses to produce `main-windows.png` / `settings-windows.png`; e.g. `cargo run -p game_menu --bin … --update-snapshots` or the project's `cargo superui test` update flag — discover it from `examples/game_menu/README.md` / `superui.test.toml`). Expected: `pause-windows.png` and `over-windows.png` written.
- [x] **Step 3: Run the spec, verify all four screenshot tests pass** against the committed baselines. Expected: 5 tests PASS (main, settings, pause, over, toggle).
- [x] **Step 4: Commit** (baselines + spec, still single-file).

```bash
git add examples/game_menu/tests/game_menu.spec.ts examples/game_menu/tests/__snapshots__/
git commit -m "test(game_menu): snapshot pause and game-over screens as split baseline"
```

---

### Task 9: Split `game_menu` into modules + update docs

Split `examples/game_menu/assets/ui/game_menu/app.tsx` across files (one module per screen + a shared tab-switcher/chrome module), import them, and confirm the four-screen spec stays green with the baselines **unchanged** (spec §Demonstrator).

**Files:**
- Create: `examples/game_menu/assets/ui/game_menu/{main.tsx,pause.tsx,settings.tsx,gameover.tsx,chrome.tsx}` (or a shared `widgets.tsx` for the leaf widgets if cleaner)
- Modify: `examples/game_menu/assets/ui/game_menu/app.tsx` (imports + `render`)
- Modify: the authoring-gotcha docs/comments (the `NOTE — everything lives in this one file` block at the top of `app.tsx`, and any docs page that records "imports stripped, one giant file" for this example)

**Interfaces:**
- Consumes: the whole feature (Tasks 1-7). The example is built non-HMR for tests → the test-engine's `walk_sync` (Task 7) bundles it; built with `--features hmr` → `TsxLoader` (Task 4) bundles it.

- [x] **Step 1: Split the modules.** Move each screen component into its own module with `export function MainMenu(props) { … }` etc.; move shared leaf widgets (`MenuItem`, `Toggle`, `Slider`, `Segmented`, `ConfigRow`, `Stat`) and the static data (`STARS`, `RES_OPTS`, `DIFF_OPTS`, `TABS`) into shared module(s) each screen imports. `app.tsx` imports the screens + `Chrome` and keeps `App` + `render(...)`. Preserve the `<Show>`-wrapped-in-`{…}` authoring pattern and all class names (the snapshots depend on exact rendered output).
- [x] **Step 2: Rewrite the top-of-file gotcha note** in `app.tsx` to describe the now-supported multi-file layout (delete the "imports stripped" warning; keep the control-flow `{<Show>}` gotcha, which still applies).
- [x] **Step 3: Run the four-screen spec, verify it passes with baselines UNCHANGED.** Run the game_menu test runner (non-update mode). Expected: all 5 tests PASS, no screenshot diff. If a screen diffs, the split changed rendered output — fix the split, do not regenerate baselines.
- [x] **Step 4: Update docs** — any capability/authoring doc that says cross-module imports are stripped (search `website/src/docs` and the example READMEs). REQUIRED SUB-SKILL at execution time: `documenting-new-features` / `reference-docs` for the docs edits.
- [x] **Step 5: Run the full workspace test suite** as the final gate. Run: `cargo test --workspace` — Expected: PASS.
- [x] **Step 6: Commit.**

```bash
git add examples/game_menu/ website/ docs/
git commit -m "example(game_menu): split the app across modules, now that imports resolve"
```

---

## Notes / documented limitations (carry into docs)

- **Module `.css` edits hot-reload (resolved).** The `<link>` sheet is kept
  pristine and the runtime applies a separate merged sheet (base + module
  CSS) that `hot_reload.rs::apply_hot_reload` rebuilds on any JS/CSS change —
  so editing a module `.css` re-applies live, and editing the entry `<link>`
  sheet no longer drops the merged module CSS. State is preserved via the
  existing `$ss.hot` path, like `.tsx`/`.ts`/`.js` edits.
- **Non-HMR native binaries apply module `.css` (resolved).** `build.rs`
  writes `bundle.css` to a sidecar (`superui_paths::generated_css`) beside the
  generated `.js`; the plain `JsLoader` reads that sidecar for
  `.superui/build/*.js` artifacts (`superui_paths::generated_css_sidecar`) onto
  `JsSource.css`, so `mount.rs` merges it exactly as on the live `TsxLoader`
  path.
- **Out of scope, warn-and-skip:** `export * from` re-export, dynamic `import()`, `import.meta`, `package.json` `main`/`module` resolution (candidate `index.*` only). Bare-package resolution machinery is ready but no package manager populates `superui_modules/`.
