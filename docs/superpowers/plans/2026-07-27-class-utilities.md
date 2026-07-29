# Class utilities (Tailwind-compatible classes) — Implementation Plan

> **For agentic workers:** Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax. Design: `docs/superpowers/specs/2026-07-27-class-utilities-design.md`.

**Goal:** Opt-in Tailwind-compatible utility classes for superui `.tsx` UIs, limited to what flair/bevy_ui can render, via a build/asset-time content-scan that generates a stylesheet merged into flair by `@import`. flair itself is the whitelist oracle. Feature is called "class utilities" in docs; never named "tailwind" in identifiers.

**Architecture:** A pure native-only crate `superui_css_utilities` holds `expand(classes) -> (css, diagnostics)` = `encre-css` generation + a flair-parse oracle filter (headless `SuperUiCssPlugin` app). Two thin callers: an example `build.rs` (build-dependency, wasm/no-HMR) and a `superui` HMR system behind an opt-in `utilities` feature. Output is `<ui>/.superui/build/utilities.generated.css`, `@import`ed from the app's `style.css`. A separate fork patch makes flair resolve `@import` relative to the importing sheet so that import is portable.

**Tech stack:** Rust, `encre-css`, flair fork (`superui_flair_css_parser` / `superui_css`), Bevy 0.19 asset system, `superui_paths`.

## Global constraints

- **Do not use git worktrees** (huge `target/`). Work on branch `feat/class-utilities`.
- Native-only: `superui_css_utilities` and both callers must never enter the wasm binary (mirror how `oxc`/`supersolid` are gated). On wasm the sheet is already generated at build time.
- Fork edits obey `docs/fork-patches.md`: paired `// >>> SUPERUI-FORK-PATCH: <id>` / `// <<< …` markers + a registry entry + a regression test.
- Naming: docs say "class utilities"; identifiers are neutral (`superui_css_utilities`, feature `utilities`); "Tailwind-compatible" only in prose.
- Commit after every task. Run `cargo build`/`cargo test -p <crate>` for touched crates before marking a task done.
- The oracle is the source of truth for "supported": never hand-encode a property allow-list as the gate; flair's parser decides. A property-name pre-filter is allowed only as an optimization, not the gate.

## File structure

| File | New/Modify | Responsibility |
|---|---|---|
| `crates/superui_flair_css_parser/src/loader.rs` | Modify | Resolve `@import` relative to the importing sheet (fork patch). |
| `docs/fork-patches.md` | Modify | Register `css-import-relative-resolution`. |
| `crates/superui_css/tests/imports_relative.rs` | Create | Regression test for the patch. |
| `crates/superui_css_utilities/` | Create | The pure core crate: `expand`, `scan_source`, `generate_for_dir`, `write_generated`, `Diagnostic`. |
| `Cargo.toml` (workspace) | Modify | Add crate to `members`. |
| `crates/superui/Cargo.toml` | Modify | `utilities` feature + optional native-only dep. |
| `crates/superui/src/` (new module) | Create | HMR-time regeneration system (gated). |
| `examples/todomvc_supersolid/build.rs` | Modify | Call `write_generated` after `transpile_dir`. |
| `examples/todomvc_supersolid/Cargo.toml` | Modify | Add `superui_css_utilities` build-dependency. |
| `examples/todomvc_supersolid/assets/ui/todomvc_supersolid/style.css` | Modify | `@import` the generated sheet + demo classes. |
| `examples/todomvc_supersolid/assets/ui/todomvc_supersolid/app.tsx` | Modify | Use a few utility classes. |
| `docs/support/class-utilities.md` (+ tree) | Create | Reference docs generated from the curated catalog. |

---

### Task 1: flair `@import` file-relative resolution (fork patch) — INDEPENDENT

**Files:** Modify `crates/superui_flair_css_parser/src/loader.rs` (~127-134); Modify `docs/fork-patches.md`; Create `crates/superui_css/tests/imports_relative.rs`.

**Interface:** After this task, `@import "sub/x.css"` inside a stylesheet at `a/b.css` loads `a/sub/x.css` (relative to the importer's directory), not root-relative `sub/x.css`. The `Imports` map key stays the original import string.

- [ ] **Step 1:** In `CssStyleSheetLoader::load`, resolve the path passed to `load_value` relative to the current sheet. Keep `import_path` (the original string) as the `imports` map key; only the load target changes:

```rust
// >>> SUPERUI-FORK-PATCH: css-import-relative-resolution  (docs/fork-patches.md#css-import-relative-resolution)
// CSS spec: @import URLs resolve relative to the importing stylesheet, not the asset root.
let load_path = load_context
    .path()
    .resolve_embed_str(&import_path)
    .unwrap_or_else(|_| import_path.clone().into());
let loaded_asset = load_context
    .load_builder()
    .load_value::<StyleSheet>(&load_path)
    .await?;
// <<< SUPERUI-FORK-PATCH: css-import-relative-resolution
```

(Verify `resolve_embed_str` is the right variant — base = the sheet file, resolve relative to its *directory*; `resolve` treats the base as a directory and is wrong here. See `bevy_asset-0.19/src/path.rs` tests `resolve_embed_relative_to_external_path`.)

- [ ] **Step 2:** Add a `### css-import-relative-resolution` entry to `docs/fork-patches.md` (Crate/file, What, Why = CSS-spec compliance, Upstream status: local, to submit to bevy_flair).
- [ ] **Step 3:** Regression test `crates/superui_css/tests/imports_relative.rs`: build a headless app with `SuperUiCssPlugin` + a `MemoryAssetReader` holding `dir/main.css` (`@import "sub/child.css";` + a rule) and `dir/sub/child.css` (a rule matched by a spawned entity); assert the child sheet's rule resolves onto the entity. Model it on the existing loader tests in `crates/superui/src/assets.rs` and `crates/superui_css/tests/`.
- [ ] **Step 4:** `cargo test -p superui_css`. Commit.

### Task 2: `superui_css_utilities` crate — pure core — INDEPENDENT (parallel with Task 1)

**Files:** Create `crates/superui_css_utilities/{Cargo.toml,src/lib.rs,...}`; Modify workspace `Cargo.toml` `members`.

**Interface (public API other tasks depend on — pin these names):**

```rust
pub struct Diagnostic { pub class: String, pub property: Option<String>, pub reason: String }
pub struct GenerateOutput { pub css: String, pub diagnostics: Vec<Diagnostic> }

/// Pure core: for each UNIQUE class, encre-css-generate → flair-probe → keep/drop.
/// Classes encre-css doesn't recognize (empty output) are silently skipped (not diagnostics).
pub fn expand<I, S>(classes: I) -> GenerateOutput where I: IntoIterator<Item = S>, S: AsRef<str>;

/// Liberal candidate-token extraction from .tsx/.ts source text (string-literal contents
/// split on whitespace). Over-collection is fine — the oracle filters non-utilities.
pub fn scan_source(src: &str) -> Vec<String>;

/// Scan every .tsx/.ts in `ui_dir`, expand, return output. (Shared by both callers.)
pub fn generate_for_dir(ui_dir: &str) -> GenerateOutput;

/// build.rs convenience: generate_for_dir + write `<ui_dir>/.superui/build/utilities.generated.css`
/// (always writes, empty if none, so the @import never dangles). Returns diagnostics.
pub fn write_generated(ui_dir: &str) -> Vec<Diagnostic>;
```

- [ ] **Step 1:** Scaffold the crate. Deps: `encre-css`, `superui_css` (for `SuperUiCssPlugin` + `InlineCssStyleSheetParser`), minimal `bevy_app`/`bevy_asset`/`bevy_ecs` for the headless probe, `superui_paths`. Add to workspace `members`. Confirm `encre-css` version + that `encre_css::generate([class], &Config::default())` returns that class's CSS and handles `w-[220px]`.
- [ ] **Step 2:** The oracle. Build a headless `App` with `SuperUiCssPlugin` once; probe a CSS string via `InlineCssStyleSheetParser::load_stylesheet(css)` in `ReturnError` mode (it's a `SystemParam`, so run through `SystemState`/`run_system_once` with world access). Clean `Ok` ⇒ supported; `Err(CssStyleLoaderError::Report(msg))` ⇒ dropped — parse the message for the offending property into `Diagnostic`. (Alternatively probe per-declaration via `parse_inline_properties` + `ErrorReportGenerator` for finer diagnostics; pick whichever gives a usable per-class reason.)
- [ ] **Step 3:** `expand()`: dedup classes; per class call `encre_css::generate` → if empty, skip; else flair-probe → keep CSS (append to output) or drop (push `Diagnostic`). Deterministic ordering (sort classes) so build output is stable.
- [ ] **Step 4:** `scan_source` + `generate_for_dir` + `write_generated` (write via `superui_paths::GENERATED_DIR`).
- [ ] **Step 5:** Tests: `expand(["flex"])` ⇒ css contains `display: flex`, no diagnostics; `expand(["shadow-lg"])` ⇒ empty/near-empty css + a diagnostic naming `box-shadow`; `expand(["w-[220px]"])` ⇒ css contains `width: 220px`; `scan_source` pulls `flex`/`hidden` out of `class={c ? "flex" : "hidden"}`. `cargo test -p superui_css_utilities`. Commit.

### Task 3: build.rs caller (wasm / no-HMR path) — depends on Task 2

**Files:** Modify `examples/todomvc_supersolid/build.rs`, `examples/todomvc_supersolid/Cargo.toml`.

- [ ] **Step 1:** Add `superui_css_utilities` as a `[build-dependencies]` entry (path).
- [ ] **Step 2:** In `build.rs`, after `transpile_dir(...)`, call `superui_css_utilities::write_generated("assets/ui/todomvc_supersolid")` and print each returned `Diagnostic` as `println!("cargo:warning=superui/utilities: dropped `{class}` — {reason}")`. Emit `cargo:rerun-if-changed` for the scanned `.tsx`.
- [ ] **Step 3:** `cargo build -p todomvc_supersolid` (no `hmr`); confirm `utilities.generated.css` appears under `.superui/build/`. Commit.

### Task 4: `superui` `utilities` feature + HMR regeneration — depends on Task 2

**Files:** Modify `crates/superui/Cargo.toml`; Create `crates/superui/src/utilities.rs` (or similar), wire into the plugin.

- [ ] **Step 1:** `crates/superui/Cargo.toml`: add `utilities = ["dep:superui_css_utilities"]`; add `superui_css_utilities` as an OPTIONAL dep under `[target.'cfg(not(target_arch = "wasm32"))'.dependencies]`.
- [ ] **Step 2:** A native-only, feature-gated system that (re)generates the sheet when a UI source changes and logs diagnostics via `warn!("superui/utilities: dropped `{class}` — {reason}")`. Reuse the existing HMR/watch gating (`hmr` feature + AssetServer watching); regenerate only while watching. Follow how `detect_hot_reload`/`apply_hot_reload` are wired in `crates/superui/src/mount.rs`.
- [ ] **Step 3:** `cargo build -p superui --features utilities` and `--features "utilities hmr"`; `cargo build -p superui` (no feature) shows no `superui_css_utilities` in the tree. Commit.

### Task 5: example wiring, reference docs, E2E — depends on Tasks 1–3

**Files:** Modify the example `style.css` + `app.tsx`; Create `docs/support/class-utilities.md` (+ tree).

- [ ] **Step 1:** Add `@import ".superui/build/utilities.generated.css";` to the top of the example `style.css` (works because of Task 1). Use a few supported utilities (`flex`, `pt-4`, `w-[220px]`, a `bg-*`) in `app.tsx`.
- [ ] **Step 2:** Curated catalog: a `pub` catalog (const list or data file) in `superui_css_utilities` of the supported families (layout/flex, spacing, sizing, colors, text, border). Generate `docs/support/class-utilities.md` (Tailwind-shaped tree) from it via the oracle — document only what probes clean. Note the computed-name limitation (`` w-[${x}px] ``).
- [ ] **Step 3:** E2E: build + run the example (or a `superui_test_engine` spec) and confirm the utility-styled element has the expected computed style. `cargo build -p todomvc_supersolid`. Commit.

## Verification

- Per task: `cargo build`/`cargo test` for the touched crate(s) as noted; commit only on green.
- Feature isolation (Task 4 Step 3): `cargo tree -p superui` without `utilities` must not list `encre-css`.
- E2E (Task 5): the example renders utility-styled elements via the generated sheet; dropped classes surface as build/HMR warnings.

## Out of scope / deferred

Runtime (reconcile-time) expansion; probing the full encre-css surface (curated first); expanding flair/bevy_ui property support; per-app theme config.
