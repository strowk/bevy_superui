//! Per-module ESM → CommonJS-factory-body lowering (pure, I/O-free; see
//! `crate::pipeline::run_module` for the parse → lower → codegen pipeline),
//! plus the `$ssmod` runtime registry ABI, the bundle assembler, the
//! specifier resolver, and a generic synchronous graph walker (spec §1-§3).
//!
//! Unlike `transpile` (a one-module wrapper over `transpile_module` +
//! `assemble`, spec §2), `transpile_module` resolves every non-runtime import
//! to a `require(...)` binding and every `export` to an `exports.*`
//! assignment, so the result can be dropped verbatim inside a registry
//! factory body.

use crate::{Diagnostic, Severity, TranspileOptions};
use std::collections::{BTreeMap, HashSet, VecDeque};

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

/// A module whose imports/css/json have been resolved to ids and text,
/// ready to drop into `assemble`'s registry.
#[derive(Debug, Clone, Default)]
pub struct ResolvedModule {
    /// The lowered factory body (same shape as `Module::factory_code`).
    pub factory_code: String,
    /// Raw specifier → resolved module id, in declaration order.
    pub edges: Vec<(String, String)>,
    /// This module's own `.css` imports' text, concatenated in source order.
    pub css: String,
    /// Resolved `const <binding> = <json>;` lines to inject at the top of
    /// the factory body, in declaration order.
    pub json_prelude: String,
}

/// The output of `assemble`/`walk_sync`: one self-contained script plus the
/// concatenated CSS (post-order over the import graph) and diagnostics.
#[derive(Debug, Clone, Default)]
pub struct Bundle {
    pub code: String,
    pub css: String,
    /// Module ids in CSS post-order (dependency before dependent).
    pub module_ids: Vec<String>,
    pub diagnostics: Vec<Diagnostic>,
}

/// The default bare-import resolution root (spec §3).
pub const DEFAULT_MODULES_ROOT: &str = "superui_modules";

/// The `$ssmod` runtime registry (spec §1) — emitted verbatim by `assemble`.
/// `def` registers a module's edges + factory; `run`/`req` lazily
/// instantiates a module (and its transitive deps) exactly once, caching the
/// result by id. An unresolved id (no `def`) logs and degrades to `{}`
/// rather than throwing, matching the transpiler's graceful-degradation rule.
const SSMOD_PRELUDE: &str = r#"var $ssmod = (function () {
  var mods = {}, cache = {};
  function def(id, edges, factory) { mods[id] = { edges: edges, factory: factory }; }
  function req(id) {
    if (cache[id]) return cache[id].exports;
    var m = mods[id];
    if (!m) { console.error("superui: module not found: " + id); return {}; }
    var module = { exports: {} };
    cache[id] = module;
    var require = function (spec) { return req(m.edges[spec]); };
    if (typeof $ss !== "undefined" && $ss.modEnter) $ss.modEnter(id);
    try {
      m.factory(module.exports, module, require);
    } finally {
      if (typeof $ss !== "undefined" && $ss.modExit) $ss.modExit(id);
    }
    return module.exports;
  }
  return { def: def, run: req };
})();
"#;

/// A JS string literal for `s` (also valid as a JSON-object key/value).
fn js_string(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| format!("{s:?}"))
}

/// `{"rawspec": "resolvedid", ...}` edges object literal for one `def(...)` call.
fn edges_object(edges: &[(String, String)]) -> String {
    let body = edges
        .iter()
        .map(|(spec, id)| format!("{}: {}", js_string(spec), js_string(id)))
        .collect::<Vec<_>>()
        .join(", ");
    format!("{{{body}}}")
}

/// DFS from `entry_id` over each module's `edges` (values = resolved ids),
/// concatenating `.css` post-order (deps before the importer, so an importer's
/// CSS wins) and collecting `module_ids` in that same order. An edge target
/// with no registered module (a sentinel, or anything else missing) is
/// skipped — it has no CSS and is not a real module.
fn post_order(entry_id: &str, modules: &BTreeMap<String, ResolvedModule>) -> (Vec<String>, String) {
    fn visit(
        id: &str,
        modules: &BTreeMap<String, ResolvedModule>,
        visited: &mut HashSet<String>,
        order: &mut Vec<String>,
        css: &mut String,
    ) {
        if !visited.insert(id.to_string()) {
            return;
        }
        let Some(m) = modules.get(id) else { return };
        for (_, target) in &m.edges {
            visit(target, modules, visited, order, css);
        }
        css.push_str(&m.css);
        order.push(id.to_string());
    }

    let mut visited = HashSet::new();
    let mut order = Vec::new();
    let mut css = String::new();
    visit(entry_id, modules, &mut visited, &mut order, &mut css);
    (order, css)
}

/// Assemble a registry bundle (spec §1): the `$ssmod` prelude, one `def(...)`
/// per module (order irrelevant — `run` resolves lazily), then
/// `$ssmod.run("<entry_id>");`. Pure: performs no I/O and does not validate
/// that `entry_id` is present in `modules`.
pub fn assemble(entry_id: &str, modules: &BTreeMap<String, ResolvedModule>) -> Bundle {
    let mut code = String::from(SSMOD_PRELUDE);
    for (id, m) in modules {
        code.push_str(&format!(
            "$ssmod.def({}, {}, function (exports, module, require) {{\n{}{}\n}});\n",
            js_string(id),
            edges_object(&m.edges),
            m.json_prelude,
            m.factory_code
        ));
    }
    code.push_str(&format!("$ssmod.run({});\n", js_string(entry_id)));

    let (module_ids, css) = post_order(entry_id, modules);
    Bundle { code, css, module_ids, diagnostics: Vec::new() }
}

/// Normalize `.`/`..` path segments after a `join_asset` join (which does not
/// normalize them itself). Forward-slash only; a leading `..` past the root
/// has nothing left to pop.
fn normalize_segments(path: &str) -> String {
    let mut stack: Vec<&str> = Vec::new();
    for seg in path.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                stack.pop();
            }
            s => stack.push(s),
        }
    }
    stack.join("/")
}

/// The extension/index resolution ladder for a candidate `base` without an
/// exact-file hit (spec §3): `<base>.tsx`, `.ts`, `.js`, then the same three
/// under `<base>/index.*`.
fn extension_ladder(base: &str) -> Vec<String> {
    vec![
        format!("{base}.tsx"),
        format!("{base}.ts"),
        format!("{base}.js"),
        format!("{base}/index.tsx"),
        format!("{base}/index.ts"),
        format!("{base}/index.js"),
    ]
}

/// Ordered, pure candidate ids for a module `specifier` imported by
/// `importer_id` (spec §3). No existence check — the caller (the walker)
/// tries each in order via its own `read`. `.css`/`.json` specifiers are NOT
/// resolved here (relative `join_asset` only, done by the walker directly).
pub fn resolve_candidates(importer_id: &str, specifier: &str, modules_root: &str) -> Vec<String> {
    let base = if specifier.starts_with('.') || specifier.starts_with('/') {
        let joined = superui_paths::join_asset(superui_paths::parent_dir(importer_id), specifier);
        normalize_segments(&joined)
    } else {
        format!("{modules_root}/{specifier}")
    };
    // A specifier that already carries a JS/TS extension IS the exact file
    // (spec §3's ladder is for a base "without an exact-file hit") — e.g.
    // `./counter.tsx` must resolve to `counter.tsx`, not `counter.tsx.tsx`.
    if base.ends_with(".tsx") || base.ends_with(".ts") || base.ends_with(".js") {
        vec![base]
    } else {
        extension_ladder(&base)
    }
}

/// BFS a module graph from `entry_id` using `read` for all I/O (pure walker —
/// `supersolid` stays I/O-free; the caller supplies `read`). Per module:
/// `transpile_module`, then resolve each dep via `resolve_candidates` (first
/// candidate that `read`s without error wins; otherwise a sentinel id +
/// Warning), each `.css` import via `join_asset` (warn + skip on miss), and
/// each `.json` import via `join_asset` + `json_binding` (warn + skip on
/// miss/invalid). `seen` dedupes by resolved id, so a diamond-shared module is
/// read and `def`'d exactly once, and an import cycle terminates.
pub fn walk_sync(
    entry_id: &str,
    options_for: impl Fn(&str) -> TranspileOptions,
    mut read: impl FnMut(&str) -> std::io::Result<Vec<u8>>,
) -> Bundle {
    let mut modules: BTreeMap<String, ResolvedModule> = BTreeMap::new();
    let mut diagnostics: Vec<Diagnostic> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<String> = VecDeque::new();

    seen.insert(entry_id.to_string());
    queue.push_back(entry_id.to_string());

    while let Some(id) = queue.pop_front() {
        let bytes = match read(&id) {
            Ok(b) => b,
            Err(e) => {
                diagnostics.push(Diagnostic {
                    severity: Severity::Warning,
                    message: format!("supersolid: could not read module {id:?}: {e}"),
                });
                continue;
            }
        };
        let source = String::from_utf8_lossy(&bytes).into_owned();
        let options = options_for(&id);
        let m = transpile_module(&source, &options);
        diagnostics.extend(m.diagnostics);

        let dir = superui_paths::parent_dir(&id);

        let mut edges = Vec::with_capacity(m.deps.len());
        for spec in &m.deps {
            let candidates = resolve_candidates(&id, spec, DEFAULT_MODULES_ROOT);
            let resolved = candidates.into_iter().find(|c| read(c).is_ok());
            match resolved {
                Some(resolved_id) => {
                    if seen.insert(resolved_id.clone()) {
                        queue.push_back(resolved_id.clone());
                    }
                    edges.push((spec.clone(), resolved_id));
                }
                None => {
                    diagnostics.push(Diagnostic {
                        severity: Severity::Warning,
                        message: format!(
                            "supersolid: could not resolve {spec:?} imported from {id:?}"
                        ),
                    });
                    edges.push((spec.clone(), format!("superui:unresolved:{spec}")));
                }
            }
        }

        let mut css = String::new();
        for spec in &m.css_imports {
            let path = superui_paths::join_asset(dir, spec);
            match read(&path) {
                Ok(bytes) => css.push_str(&String::from_utf8_lossy(&bytes)),
                Err(e) => diagnostics.push(Diagnostic {
                    severity: Severity::Warning,
                    message: format!("supersolid: could not read {path:?} ({spec:?}): {e}"),
                }),
            }
        }

        let mut json_prelude = String::new();
        for (binding, spec) in &m.json_imports {
            let path = superui_paths::join_asset(dir, spec);
            match read(&path) {
                Ok(bytes) => {
                    let text = String::from_utf8_lossy(&bytes);
                    match crate::json_binding(binding, &text) {
                        Ok(line) => json_prelude.push_str(&line),
                        Err(e) => diagnostics.push(Diagnostic {
                            severity: Severity::Warning,
                            message: format!("supersolid: {path:?} ({spec:?}): {e}"),
                        }),
                    }
                }
                Err(e) => diagnostics.push(Diagnostic {
                    severity: Severity::Warning,
                    message: format!("supersolid: could not read {path:?} ({spec:?}): {e}"),
                }),
            }
        }

        modules.insert(id, ResolvedModule { factory_code: m.factory_code, edges, css, json_prelude });
    }

    let mut bundle = assemble(entry_id, &modules);
    bundle.diagnostics = diagnostics;
    bundle
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
    // Runtime / css / json classification
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

    // resolve_candidates
    #[test]
    fn relative_candidate_ladder_order() {
        let c = resolve_candidates("ui/app.tsx", "./label", "superui_modules");
        assert_eq!(c[0], "ui/label.tsx");
        assert_eq!(c, vec!["ui/label.tsx","ui/label.ts","ui/label.js",
            "ui/label/index.tsx","ui/label/index.ts","ui/label/index.js"]);
    }
    #[test]
    fn parent_relative_normalizes_dotdot() {
        let c = resolve_candidates("ui/screens/main.tsx", "../lib/util", "superui_modules");
        assert_eq!(c[0], "ui/lib/util.tsx");
    }
    #[test]
    fn bare_resolves_under_modules_root() {
        let c = resolve_candidates("ui/app.tsx", "ui-kit", "superui_modules");
        assert!(c.contains(&"superui_modules/ui-kit.tsx".to_string()));
        assert!(c.contains(&"superui_modules/ui-kit/index.tsx".to_string()));
    }
    #[test]
    fn extensioned_specifier_resolves_to_exact_path() {
        let c = resolve_candidates("ui/app.tsx", "./counter.tsx", "superui_modules");
        assert_eq!(c, vec!["ui/counter.tsx".to_string()]);
        // Same importer/specifier shape as `parent_relative_normalizes_dotdot`
        // (importer two levels deep so `..` has somewhere to pop to), but with
        // an explicit extension: dotdot-normalization and the extension-exact
        // short-circuit must compose correctly.
        let c2 = resolve_candidates("ui/screens/main.tsx", "../lib/util.ts", "superui_modules");
        assert_eq!(c2, vec!["ui/lib/util.ts".to_string()]);
    }
    // assemble
    #[test]
    fn assemble_emits_prelude_and_entry_run() {
        let mut m = std::collections::BTreeMap::new();
        m.insert("app.tsx".to_string(), ResolvedModule { factory_code: "exports.x=1;".into(),
            edges: vec![], css: String::new(), json_prelude: String::new() });
        let b = assemble("app.tsx", &m);
        assert!(b.code.contains("var $ssmod ="), "{}", b.code);
        assert!(b.code.contains(r#"$ssmod.def("app.tsx""#), "{}", b.code);
        assert!(b.code.contains(r#"$ssmod.run("app.tsx")"#), "{}", b.code);
        assert!(reparses_as_plain_js(&b.code), "{}", b.code);
    }
    #[test]
    fn prelude_brackets_factory_with_module_frame() {
        let mut m = std::collections::BTreeMap::new();
        m.insert("app.tsx".to_string(), ResolvedModule { factory_code: "exports.x=1;".into(),
            edges: vec![], css: String::new(), json_prelude: String::new() });
        let b = assemble("app.tsx", &m);
        assert!(b.code.contains("$ss.modEnter(id)"), "{}", b.code);
        assert!(b.code.contains("$ss.modExit(id)"), "{}", b.code);
        assert!(b.code.contains("m.factory(module.exports, module, require)"), "{}", b.code);
        assert!(b.code.contains("} finally {"), "{}", b.code);
        assert!(reparses_as_plain_js(&b.code), "{}", b.code);
    }
    #[test]
    fn prelude_bracket_is_guarded() {
        let mut m = std::collections::BTreeMap::new();
        m.insert("app.tsx".to_string(), ResolvedModule { factory_code: "exports.x=1;".into(),
            edges: vec![], css: String::new(), json_prelude: String::new() });
        let b = assemble("app.tsx", &m);
        assert!(b.code.contains(r#"if (typeof $ss !== "undefined" && $ss.modEnter) $ss.modEnter(id)"#), "{}", b.code);
        assert!(b.code.contains(r#"if (typeof $ss !== "undefined" && $ss.modExit) $ss.modExit(id)"#), "{}", b.code);
    }
    #[test]
    fn assemble_edges_map_raw_spec_to_id() {
        let mut m = std::collections::BTreeMap::new();
        m.insert("app.tsx".into(), ResolvedModule { factory_code: r#"var c = require("./c.tsx");"#.into(),
            edges: vec![("./c.tsx".into(), "c.tsx".into())], css: String::new(), json_prelude: String::new() });
        m.insert("c.tsx".into(), ResolvedModule { factory_code: "exports.v=1;".into(),
            edges: vec![], css: String::new(), json_prelude: String::new() });
        let b = assemble("app.tsx", &m);
        assert!(b.code.contains(r#""./c.tsx": "c.tsx""#), "{}", b.code);
    }
    #[test]
    fn assemble_concats_css_post_order() {
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
    #[test]
    fn walk_sync_diamond_shares_singleton() {
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
    #[test]
    fn walk_sync_cycle_terminates_with_partial_exports() {
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
    #[test]
    fn walk_sync_unresolved_dep_warns_and_sentinels() {
        let files = std::collections::HashMap::from([
            ("a.tsx".to_string(), r#"import {x} from "./gone";"#.to_string())]);
        let b = walk_sync("a.tsx", |id| TranspileOptions{ module_id: Some(id.into()), ..Default::default() },
            |p| files.get(p).map(|s| s.clone().into_bytes())
                .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::NotFound)));
        assert!(b.diagnostics.iter().any(|d| d.severity == Severity::Warning && d.message.contains("./gone")));
        assert!(b.code.contains("superui:unresolved:./gone"), "sentinel edge: {}", b.code);
        assert!(reparses_as_plain_js(&b.code));
    }
    #[test]
    fn walk_sync_collects_css_import() {
        let files = std::collections::HashMap::from([
            ("a.tsx".to_string(), r#"import "./a.css"; export const a=1;"#.to_string()),
            ("a.css".to_string(), ".x{color:red}".to_string())]);
        let b = walk_sync("a.tsx", |id| TranspileOptions{ module_id: Some(id.into()), ..Default::default() },
            |p| files.get(p).map(|s| s.clone().into_bytes())
                .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::NotFound)));
        assert!(b.css.contains(".x{color:red}"), "css collected: {}", b.css);
    }
}
