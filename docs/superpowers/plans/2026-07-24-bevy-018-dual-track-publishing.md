# Bevy 0.18 Upgrade + Dual-Track Publishing — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Publish the superui crates + `cargo-superui` CLI to crates.io on two tracks — `0.1.x` for bevy 0.17 and `0.2.x` for bevy 0.18 — with a maintainable fork of bevy_flair and a documented backport/branching workflow.

**Architecture:** Centralize the Bevy version behind `[workspace.dependencies]` so a track is a one-line diff. Rename the vendored flair forks into our namespace and make every deviation from upstream machine-findable (registry + paired markers). Set up publish metadata + a topological publish script on the 0.17 state, cut a long-lived `release/bevy-0.17` maintenance branch, then bump `main` to bevy 0.18 (vendoring upstream flair's 0.18 release as the new fork base). `main` always tracks newest bevy; fixes cherry-pick back to the maintenance branch.

**Tech Stack:** Rust workspace, Bevy 0.17→0.18, vendored bevy_flair fork, `cargo` publishing, `xtask` (plain-arg CLI), mdBook (`website/`).

## Global Constraints

- **Version scheme:** `0.1.x` = bevy 0.17, `0.2.x` = bevy 0.18. All crates share `version.workspace`.
- **Branch model:** `main` = newest bevy (→ 0.2.x / 0.18). Long-lived `release/bevy-0.17` = 0.1.x. Fixes land on `main`, cherry-pick back.
- **Fork rename:** `bevy_flair_core→superui_flair_core`, `bevy_flair_style→superui_flair_style`, `bevy_flair_css_parser→superui_flair_css_parser`.
- **Fork patch markers (exact):** `// >>> SUPERUI-FORK-PATCH: <id>  (docs/fork-patches.md#<id>)` … `// <<< SUPERUI-FORK-PATCH: <id>`.
- **Publish scope:** every crate including `superui_test_engine`; drop all `publish = false`.
- **Never auto-`cargo publish`.** Publishing to crates.io is irreversible. The executing agent (Claude) MUST NOT run `cargo publish` or `xtask publish --execute` under any circumstances — not even if asked mid-execution. It drives everything to `cargo package` dry-run-green, then **stops and hands the maintainer written, copy-pasteable publish instructions** (Task 13). The maintainer runs the real publish themselves.
- **Do not touch `superui_test_engine` source logic** (parallel bugfix in flight); only its manifest metadata + bevy version.
- **DRY / YAGNI / TDD / frequent commits.** Preserve upstream flair attribution in NOTICE + per-crate metadata.

---

## Publish dependency order (topological — referenced by Tasks 6 & 13)

```
superui_dom   supersolid_runtime  superui_flair_core   superui_paths (leaf, no deps)
superui_html  superui_js          superui_flair_style        (← flair_core)
superui_api   superui_flair_css_parser                       (← flair_core, flair_style)
supersolid           (← superui_paths)
superui_css          (← 3 forks)
superui_bridge       (← dom, js, api, css, supersolid_runtime)
superui              (← dom, html, js, api, css, bridge, supersolid, superui_paths)
cargo-superui        superui_test_engine (← superui, bridge, dom, css, supersolid, js)
```

**15 publishable crates total** (includes `superui_paths`, a zero-dep leaf added
after this plan was first drafted; it must precede `supersolid` and `superui`).

---

## Phase 0 — Workspace prep (still bevy 0.17, no behavior change)

### Task 1: Hoist Bevy into a single workspace version knob

**Files:**
- Modify: `Cargo.toml` (root `[workspace.package]` + `[workspace.dependencies]`)
- Modify: every `crates/*/Cargo.toml` that names `bevy` or a `bevy_*` crate (`superui`, `superui_css`, `superui_bridge`, `superui_test_engine`, and the three forks)
- Modify: every `examples/*/Cargo.toml` (`citadel`, `game_menu`, `horde`, `todomvc`, `todomvc_supersolid`)

**Interfaces:**
- Produces: `[workspace.dependencies]` entries `bevy`, `bevy_app`, `bevy_asset`, `bevy_camera`, `bevy_color`, `bevy_ecs`, `bevy_image`, `bevy_input_focus`, `bevy_math`, `bevy_picking`, `bevy_reflect`, `bevy_text`, `bevy_time`, `bevy_ui`, `bevy_utils` — all pinned `"0.17"` for now. Consumers use `{ workspace = true, features = [...] }`.

- [ ] **Step 1: Add the version knob + shared metadata to root `Cargo.toml`**

Under `[workspace.package]` add inheritable publish metadata:
```toml
[workspace.package]
edition = "2021"
version = "0.1.0"
license = "MIT OR Apache-2.0"
repository = "https://github.com/strowk/bevy_superui"
```
Under `[workspace.dependencies]` add (keep the existing slotmap/html5ever/boa/oxc lines):
```toml
bevy = { version = "0.17", default-features = false }
bevy_app = "0.17"
bevy_asset = "0.17"
bevy_camera = "0.17"
bevy_color = "0.17"
bevy_ecs = "0.17"
bevy_image = "0.17"
bevy_input_focus = "0.17"
bevy_math = "0.17"
bevy_picking = "0.17"
bevy_reflect = "0.17"
bevy_text = "0.17"
bevy_time = "0.17"
bevy_ui = "0.17"
bevy_utils = "0.17"
```

- [ ] **Step 2: Convert umbrella-`bevy` consumers to workspace inheritance**

In `crates/superui/Cargo.toml`, `crates/superui_bridge/Cargo.toml`, and the `[dev-dependencies]` of `crates/superui_css/Cargo.toml`, replace `bevy = { version = "0.17", default-features = false, features = [...] }` with `bevy = { workspace = true, features = [...] }` (keep each crate's exact feature list). In `crates/superui_css/Cargo.toml` `[dependencies]`, replace `bevy_app = "0.17"` with `bevy_app = { workspace = true }`.

- [ ] **Step 3: Convert the three forks' granular `bevy_*` deps to workspace inheritance**

In each of `crates/bevy_flair_core/Cargo.toml`, `crates/bevy_flair_style/Cargo.toml`, `crates/bevy_flair_css_parser/Cargo.toml`, change every `[dependencies.bevy_*]` (and `[dev-dependencies.bevy_math]`) block from `version = "0.17"` to `workspace = true`. Preserve any `features` (e.g. `bevy_ui` in flair_core keeps `features = ["bevy_ui_picking_backend"]`). Leave the inter-fork `path`+`version` deps as-is for now (renamed in Task 2).

- [ ] **Step 4: Convert `superui_test_engine` + examples**

In `crates/superui_test_engine/Cargo.toml` replace `bevy = { version = "0.17" }` with `bevy = { workspace = true }`. In each `examples/*/Cargo.toml`, replace every `bevy = { version = "0.17", ... }` with `bevy = { workspace = true, ... }` preserving feature lists (some examples add `features = ["file_watcher"]` / `["webgl2"]`).

- [ ] **Step 5: Verify the workspace builds unchanged**

Run: `cargo build --workspace`
Expected: builds green (same as before — only dependency *sourcing* changed, not versions).

- [ ] **Step 6: Verify tests still pass**

Run: `cargo test --workspace`
Expected: PASS (includes the flair EOF-guard regression test in `crates/superui_css/tests/selectors.rs`).

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml crates/*/Cargo.toml examples/*/Cargo.toml
git commit -m "refactor(workspace): hoist bevy to a single [workspace.dependencies] version knob"
```

---

## Phase 1 — Fork rename + patch registry (still bevy 0.17)

### Task 2: Rename the flair forks into our namespace

**Files:**
- Rename dirs: `crates/bevy_flair_core → crates/superui_flair_core` (and `_style`, `_css_parser`)
- Modify: the three renamed `Cargo.toml` (`name`, `[lib] name`, inter-fork dep keys + paths)
- Modify: `crates/superui_css/Cargo.toml` (dependency keys)
- Modify: any Rust source with `use bevy_flair_core::` / `bevy_flair_style::` / `bevy_flair_css_parser::` across the forks + `superui_css`

**Interfaces:**
- Produces: crates `superui_flair_core`, `superui_flair_style`, `superui_flair_css_parser` (lib names identical to package names).

- [ ] **Step 1: Move the crate directories**

```bash
git mv crates/bevy_flair_core crates/superui_flair_core
git mv crates/bevy_flair_style crates/superui_flair_style
git mv crates/bevy_flair_css_parser crates/superui_flair_css_parser
```

- [ ] **Step 2: Rename packages + lib names in the three manifests**

In each renamed `Cargo.toml`: set `name = "superui_flair_<x>"` and `[lib] name = "superui_flair_<x>"`. In `superui_flair_style` change `[dependencies.bevy_flair_core]` → `[dependencies.superui_flair_core]` with `path = "../superui_flair_core"`. In `superui_flair_css_parser` do the same for both `bevy_flair_core` and `bevy_flair_style`.

- [ ] **Step 3: Point `superui_css` at the renamed crates**

In `crates/superui_css/Cargo.toml` replace the three `bevy_flair_* = { path = "../bevy_flair_*", version = "0.6.0" }` lines with `superui_flair_* = { path = "../superui_flair_*", version = "0.6.0" }`.

- [ ] **Step 4: Fix `use` paths in Rust sources**

Find and rewrite every `bevy_flair_core` / `bevy_flair_style` / `bevy_flair_css_parser` module path in Rust source under the three renamed crates and `crates/superui_css/`:
```bash
grep -rl "bevy_flair_core\|bevy_flair_style\|bevy_flair_css_parser" crates/superui_flair_core crates/superui_flair_style crates/superui_flair_css_parser crates/superui_css --include=*.rs
```
Replace each occurrence with the `superui_flair_*` equivalent (crate-path identifiers only — do NOT rewrite the upstream repo URL `github.com/eckz/bevy_flair` in metadata/comments/NOTICE).

- [ ] **Step 5: Verify build + tests**

Run: `cargo build --workspace && cargo test -p superui_css`
Expected: green; EOF-guard test passes.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "refactor(flair): rename vendored forks to superui_flair_* (publishable namespace)"
```

### Task 3: Fork-patch registry + paired source markers

**Files:**
- Create: `docs/fork-patches.md`
- Modify: `crates/superui_flair_css_parser/src/error.rs` (retrofit the EOF guard markers)

**Interfaces:**
- Produces: marker grammar `SUPERUI-FORK-PATCH: <id>` (opening `// >>>`, closing `// <<<`), and registry rows keyed by `<id>`. Consumed by Task 4's drift check.

- [ ] **Step 1: Write the registry file**

Create `docs/fork-patches.md`:
```markdown
# Vendored bevy_flair fork — patch registry

Every deviation of `crates/superui_flair_*` from upstream bevy_flair is wrapped
in paired source markers and listed here. Markers let us (a) upstream a patch and
(b) reapply patches when vendoring a newer flair release.

Marker grammar (both lines required):

    // >>> SUPERUI-FORK-PATCH: <id>  (docs/fork-patches.md#<id>)
    ...our code...
    // <<< SUPERUI-FORK-PATCH: <id>

Upstream base: bevy_flair 0.6.0 (https://github.com/eckz/bevy_flair).

## Patches

### css-eof-guard
- **Crate/file:** `superui_flair_css_parser` — `src/error.rs`
- **Upstream location:** `CssErrorLocation::into_range`, the `lines().nth(...)` lookup.
- **What:** Replace the `unwrap_or_else(panic)` with a `let-else` returning an empty end-of-input span, so a trailing block-less malformed rule degrades instead of crashing the asset loader.
- **Why:** Graceful degradation of malformed CSS (design §1). Regression test: `malformed_trailing_rule_degrades_without_panic` in `crates/superui_css/tests/selectors.rs`.
- **Upstream status:** local (not yet submitted).
```

- [ ] **Step 2: Retrofit the EOF guard with paired markers**

In `crates/superui_flair_css_parser/src/error.rs`, wrap the existing fork code. Replace the current `// SUPERUI FORK PATCH ...` prose comment + `let Some(line) = ... else { return ...; };` block with:
```rust
                // >>> SUPERUI-FORK-PATCH: css-eof-guard  (docs/fork-patches.md#css-eof-guard)
                // A parse error can carry a SourceLocation one line past EOF
                // (e.g. a trailing block-less malformed rule). Upstream panicked
                // here; instead fall back to an empty end-of-input span so the
                // bad rule is skipped, not fatal.
                let Some(line) = contents.lines().nth(source_location.line as usize) else {
                    return contents.len()..contents.len();
                };
                // <<< SUPERUI-FORK-PATCH: css-eof-guard
```

- [ ] **Step 3: Verify the regression test still passes**

Run: `cargo test -p superui_css malformed_trailing_rule_degrades_without_panic`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add docs/fork-patches.md crates/superui_flair_css_parser/src/error.rs
git commit -m "docs(flair): add fork-patch registry + paired markers; retrofit css-eof-guard"
```

### Task 4: `xtask fork-patches` drift check

**Files:**
- Modify: `xtask/src/main.rs` (add subcommand + helper)
- Create: `xtask/tests/fork_patches.rs`

**Interfaces:**
- Consumes: marker grammar + registry from Task 3.
- Produces: `fn check_fork_patches(root: &Path) -> Result<Vec<String>, String>` returning the sorted list of patch ids found; errors on any of: unclosed `>>>`, `<<<` with no `>>>`, marker id absent from registry, registry id (under `## Patches` `###` headings) absent from source.

- [ ] **Step 1: Write the failing test**

Create `xtask/tests/fork_patches.rs`:
```rust
use std::path::PathBuf;

// Resolve repo root from the xtask crate dir.
fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

#[test]
fn registry_and_markers_agree() {
    let ids = xtask::check_fork_patches(&repo_root()).expect("fork patches should be consistent");
    assert!(ids.contains(&"css-eof-guard".to_string()), "expected css-eof-guard, got {ids:?}");
}
```
(If `xtask` has no `[lib]`, add one: `[lib] name = "xtask" path = "src/lib.rs"`, move the checker into `src/lib.rs`, and have `main.rs` call it.)

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p xtask --test fork_patches`
Expected: FAIL — `check_fork_patches` not found.

- [ ] **Step 3: Implement `check_fork_patches`**

Add to `xtask/src/lib.rs` (create it; re-export from `main.rs`):
```rust
use std::collections::BTreeSet;
use std::path::Path;

/// Scan the three fork crates for SUPERUI-FORK-PATCH markers and cross-check
/// them against docs/fork-patches.md. Returns sorted patch ids on success.
pub fn check_fork_patches(root: &Path) -> Result<Vec<String>, String> {
    let mut in_source: BTreeSet<String> = BTreeSet::new();
    for crate_dir in ["superui_flair_core", "superui_flair_style", "superui_flair_css_parser"] {
        let src = root.join("crates").join(crate_dir).join("src");
        for entry in walk_rs(&src) {
            let text = std::fs::read_to_string(&entry).map_err(|e| e.to_string())?;
            let mut open: Vec<String> = Vec::new();
            for line in text.lines() {
                if let Some(id) = marker_id(line, ">>>") {
                    open.push(id.clone());
                    in_source.insert(id);
                } else if let Some(id) = marker_id(line, "<<<") {
                    match open.pop() {
                        Some(o) if o == id => {}
                        _ => return Err(format!("{}: unmatched <<< for id `{id}`", entry.display())),
                    }
                }
            }
            if let Some(id) = open.pop() {
                return Err(format!("{}: unclosed >>> for id `{id}`", entry.display()));
            }
        }
    }
    let registry = std::fs::read_to_string(root.join("docs/fork-patches.md")).map_err(|e| e.to_string())?;
    let in_registry: BTreeSet<String> = registry
        .lines()
        .filter_map(|l| l.strip_prefix("### ").map(|s| s.trim().to_string()))
        .collect();
    for id in &in_source {
        if !in_registry.contains(id) {
            return Err(format!("marker `{id}` has no registry entry in docs/fork-patches.md"));
        }
    }
    for id in &in_registry {
        if !in_source.contains(id) {
            return Err(format!("registry entry `{id}` has no source marker"));
        }
    }
    Ok(in_source.into_iter().collect())
}

fn marker_id(line: &str, arrow: &str) -> Option<String> {
    let needle = format!("{arrow} SUPERUI-FORK-PATCH:");
    let rest = line.split(&needle).nth(1)?;
    Some(rest.split_whitespace().next()?.to_string())
}

fn walk_rs(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() { out.extend(walk_rs(&p)); }
            else if p.extension().map(|x| x == "rs").unwrap_or(false) { out.push(p); }
        }
    }
    out
}
```
Wire a `Some("fork-patches") => { xtask::check_fork_patches(&repo_root)?; Ok(()) }` arm into `main.rs`'s match (printing the ids), following the existing `host-page` arm style.

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p xtask --test fork_patches`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add xtask/
git commit -m "feat(xtask): fork-patches drift check (markers <-> registry)"
```

---

## Phase 2 — Publishing infrastructure (still bevy 0.17, versions 0.1.x)

### Task 5: Add crates.io publish metadata; drop `publish = false`

**Files:**
- Modify: every publishable `crates/*/Cargo.toml` (add `description` + `keywords`; inherit `repository`)
- Modify: `crates/cargo-superui/Cargo.toml` and `crates/superui_test_engine/Cargo.toml` (drop `publish = false`; give test_engine workspace inheritance)

**Interfaces:**
- Produces: every crate has the crates.io-required `description` + `license` and inherited `repository`.

- [ ] **Step 1: Add `description`, `keywords`, `repository` to each library crate**

For each of `superui`, `superui_dom`, `superui_html`, `superui_js`, `superui_api`, `superui_css`, `superui_bridge`, `supersolid`, `supersolid_runtime`, `superui_flair_core`, `superui_flair_style`, `superui_flair_css_parser`, add to `[package]` a one-line `description = "..."` specific to the crate, `repository.workspace = true`, and `keywords = ["bevy", "ui", ...]` (≤5). Example for `crates/superui/Cargo.toml`:
```toml
description = "Browser-like HTML/CSS/JS + Solid-style TSX UI for Bevy"
repository.workspace = true
keywords = ["bevy", "ui", "html", "css", "gamedev"]
categories = ["game-development"]
```
The flair forks already have `description`/`keywords`/`repository = "https://github.com/eckz/bevy_flair"` — leave their upstream `repository` and attribution intact.

- [ ] **Step 2: Make `cargo-superui` publishable**

In `crates/cargo-superui/Cargo.toml`: delete the `publish = false` line; add `description = "CLI to scaffold superui editor types & tsconfig into a project"`, `repository.workspace = true`, `keywords = ["bevy", "superui", "cli", "cargo-subcommand"]`.

- [ ] **Step 3: Make `superui_test_engine` publishable + workspace-inherited**

In `crates/superui_test_engine/Cargo.toml`: delete `publish = false`; replace the standalone `version = "0.1.0"` / `edition = "2021"` with `version.workspace = true` / `edition.workspace = true`; add `license.workspace = true`, `repository.workspace = true`, `description = "Playwright-shaped E2E test framework for superui apps"`, `keywords = ["bevy", "superui", "testing", "e2e"]`. **Do not touch `src/`.**

- [ ] **Step 4: Verify metadata by packaging a leaf crate (dry run)**

Run: `cargo package -p superui_dom --allow-dirty --no-verify`
Expected: succeeds, no "missing field `description`" / "missing `license`" error.

- [ ] **Step 5: Commit**

```bash
git add crates/*/Cargo.toml
git commit -m "chore(publish): add crates.io metadata; make CLI + test_engine publishable"
```

### Task 6: `xtask publish` — topological dry-run/publish driver

**Files:**
- Modify: `xtask/src/lib.rs` (add ordered crate list + runner)
- Modify: `xtask/src/main.rs` (add `publish` subcommand)
- Create: `xtask/tests/publish_order.rs`

**Interfaces:**
- Consumes: nothing from prior tasks except the crate manifests.
- Produces: `fn publish_order() -> Vec<&'static str>` (topological, per the order table) and `fn run_publish(dry_run: bool) -> Result<(), String>` shelling out to `cargo package`/`cargo publish -p <crate>` in order.

- [ ] **Step 1: Write the failing test**

Create `xtask/tests/publish_order.rs`:
```rust
#[test]
fn order_is_topological() {
    let order = xtask::publish_order();
    let pos = |name: &str| order.iter().position(|c| *c == name)
        .unwrap_or_else(|| panic!("{name} missing from publish_order"));
    // forks before their dependents
    assert!(pos("superui_flair_core") < pos("superui_flair_style"));
    assert!(pos("superui_flair_style") < pos("superui_flair_css_parser"));
    assert!(pos("superui_flair_css_parser") < pos("superui_css"));
    // leaf libs before aggregators
    assert!(pos("superui_css") < pos("superui_bridge"));
    assert!(pos("superui_bridge") < pos("superui"));
    assert!(pos("superui") < pos("superui_test_engine"));
    // superui_paths (leaf) must precede its dependents
    assert!(pos("superui_paths") < pos("supersolid"));
    assert!(pos("superui_paths") < pos("superui"));
    // all 15 publishable crates present, each once
    assert_eq!(order.len(), 15);
    let mut sorted = order.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), 15, "publish_order has duplicates");
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p xtask --test publish_order`
Expected: FAIL — `publish_order` not found.

- [ ] **Step 3: Implement `publish_order` + `run_publish`**

Add to `xtask/src/lib.rs`:
```rust
pub fn publish_order() -> Vec<&'static str> {
    vec![
        "superui_dom", "superui_paths", "superui_flair_core",
        "superui_html", "superui_js", "superui_api", "supersolid_runtime", "superui_flair_style",
        "superui_flair_css_parser", "superui_css",
        "supersolid",            // ← superui_paths
        "superui_bridge",
        "superui",
        "cargo-superui", "superui_test_engine",
        // Exactly 15 publishable crates — must match publish_order test's len assert.
        // superui_paths is a zero-dep leaf; supersolid depends on it, so paths precedes supersolid.
    ]
}

pub fn run_publish(dry_run: bool) -> Result<(), String> {
    for name in publish_order() {
        let mut cmd = std::process::Command::new("cargo");
        if dry_run {
            cmd.args(["package", "-p", name, "--no-verify"]);
        } else {
            cmd.args(["publish", "-p", name]);
        }
        let status = cmd.status().map_err(|e| e.to_string())?;
        if !status.success() {
            return Err(format!("`cargo {}` failed for {name}", if dry_run {"package"} else {"publish"}));
        }
    }
    Ok(())
}
```
(Adjust the vec so `order.len() == 15` — it must list every publishable crate exactly once; the comment marks where to reconcile the count with the test.)

Wire into `main.rs`: `Some("publish") => xtask::run_publish(flag(&args, "--execute").is_none()).map_err(Into::into),` — **dry-run by default**, real publish only with `--execute`.

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p xtask --test publish_order`
Expected: PASS.

- [ ] **Step 5: Full workspace dry-run**

Run: `cargo run -p xtask -- publish`
Expected: `cargo package` runs for every crate in order with no missing-metadata or path-dependency errors. (Path+version deps let cargo rewrite paths; a crate whose dep isn't yet on crates.io may warn on `package` — acceptable for a pre-first-publish dry run. Record any such crate in the Task 13 note.)

- [ ] **Step 6: Commit**

```bash
git add xtask/
git commit -m "feat(xtask): topological publish driver (dry-run default, --execute to publish)"
```

### Task 7: CLI forward-compatibility invariant test

**Files:**
- Modify: `crates/cargo-superui/src/lib.rs` (add test only — no logic change)

**Interfaces:**
- Consumes: existing `find_module_dts`, `projected_modules` (a module has a `required` bool).
- Produces: a regression test proving an absent **optional** module is a skip, not an error.

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests` in `crates/cargo-superui/src/lib.rs`:
```rust
#[test]
fn optional_module_absent_is_skip_not_error() {
    // A project whose resolved deps include supersolid (required) but NOT
    // superui (optional superui/test types) must still resolve cleanly.
    let json = r#"{ "packages": [
        { "name": "supersolid", "manifest_path": "/w/crates/supersolid/Cargo.toml" }
    ] }"#;
    for m in projected_modules() {
        let found = find_module_dts(json, m.package, m.dts_filename);
        if m.required {
            assert!(found.is_some(), "required module {} must resolve", m.specifier);
        } else {
            // optional module simply resolves to None -> caller skips it
            assert!(found.is_none(), "optional module {} absent -> None", m.specifier);
        }
    }
}
```

- [ ] **Step 2: Run the test**

Run: `cargo test -p cargo-superui optional_module_absent_is_skip_not_error`
Expected: PASS (logic already supports this; the test locks the invariant in).

- [ ] **Step 3: Commit**

```bash
git add crates/cargo-superui/src/lib.rs
git commit -m "test(cli): lock forward-compat invariant (absent optional module = skip)"
```

### Task 8: Compatibility table — README + website

**Files:**
- Modify: `README.md` (add `## Compatibility`)
- Create: `website/src/docs/reference/compatibility.md`
- Modify: `website/src/SUMMARY.md` (add Reference entry)

**Interfaces:** none (docs).

Both the README and the website must **explain** superui↔bevy compatibility —
the version-mapping table plus prose on how to pick a version and how the tracks
are supported — not just drop a bare table.

- [ ] **Step 1: Add the README `## Compatibility` section**

Insert after the `## Status` section in `README.md`:
```markdown
## Compatibility

Each superui release targets one Bevy release. Bevy makes breaking changes every
minor version, so superui bumps its own **minor** version in lockstep. Pick the
superui version that matches the Bevy version your project uses:

| superui | bevy | branch | status |
| --- | --- | --- | --- |
| 0.2.x | 0.18 | `main` | current |
| 0.1.x | 0.17 | `release/bevy-0.17` | maintained |

`main` always tracks the **newest** supported Bevy; older Bevy versions live on
long-lived `release/bevy-<ver>` branches. New features land on `main`; fixes are
backported to the maintenance branch when they apply and shipped as patch
releases (e.g. `0.1.1`).

The `cargo-superui` CLI is versioned alongside the libraries, so
`cargo install cargo-superui` matches the current track and
`cargo install cargo-superui@0.1` pins the 0.17 track.

> The full mapping and version policy also live on the docs site under
> [Reference → Compatibility](https://strowk.github.io/bevy_superui/docs/reference/compatibility.html).
```

- [ ] **Step 2: Create the website Compatibility page**

Create `website/src/docs/reference/compatibility.md` with the table above plus
fuller prose (the site is the canonical, more detailed home):
```markdown
# Compatibility

superui is built on `bevy_ui`, and Bevy makes breaking API changes each minor
release. To keep things predictable, **each superui minor version targets exactly
one Bevy minor version**, and superui bumps its minor in lockstep with Bevy.

## Version matrix

| superui | bevy | branch | status |
| --- | --- | --- | --- |
| 0.2.x | 0.18 | `main` | current |
| 0.1.x | 0.17 | `release/bevy-0.17` | maintained |

## Choosing a version

Match superui to the Bevy version already in your `Cargo.toml`. For example, on
Bevy 0.17:

    [dependencies]
    bevy = "0.17"
    superui = "0.1"

Mixing a superui version with a different Bevy minor is not supported — Cargo will
usually fail to resolve, and even when it links, the ECS/UI types won't match.

## Support policy

- `main` tracks the **newest** Bevy release and receives new features.
- Older Bevy versions are kept on long-lived `release/bevy-<ver>` branches.
- Bug fixes land on `main` first and are **backported** to the maintenance branch
  when they apply, shipped as patch releases (`0.1.1`, `0.1.2`, …).
- When a new Bevy version ships, superui cuts a maintenance branch for the
  outgoing Bevy and bumps `main` to the new Bevy (next superui minor).

## The `cargo-superui` CLI

The CLI is versioned with the libraries and reads your project's resolved superui
version, so a single global `cargo install cargo-superui` works across projects.
If you need a specific track: `cargo install cargo-superui@0.1` (0.17) or
`cargo install cargo-superui@0.2` (0.18). See
[Getting Started](../getting-started.md) for per-project pinning.
```

- [ ] **Step 3: Add it to the book summary**

In `website/src/SUMMARY.md`, under `# Reference`, add:
```markdown
- [Compatibility](docs/reference/compatibility.md)
```

- [ ] **Step 4: Verify the book builds**

Run: `mdbook build website`
Expected: builds with no "file not found in SUMMARY" error; `compatibility.html` present in output.

- [ ] **Step 5: Commit**

```bash
git add README.md website/src/docs/reference/compatibility.md website/src/SUMMARY.md
git commit -m "docs: document bevy/superui compatibility + version policy (README + website)"
```

---

## Phase 3 — Branch cut, then bevy 0.18 upgrade

### Task 9: Cut the maintenance branch + document the workflow

**Files:**
- Create: `CONTRIBUTING.md` (branching/backport section)

**Interfaces:** none.

- [ ] **Step 1: Write the branching doc**

Create `CONTRIBUTING.md`:
```markdown
# Contributing

## Branches & Bevy versions

- `main` tracks the **newest** supported Bevy. Its crate versions are `0.2.x` (bevy 0.18).
- `release/bevy-0.17` is a long-lived maintenance branch: crate versions `0.1.x` (bevy 0.17).

### Where fixes land
Land fixes on `main` first. To backport, cherry-pick onto the maintenance branch:

    git checkout release/bevy-0.17
    git cherry-pick <sha>
    # bump the patch version (0.1.(x+1)), then: cargo run -p xtask -- publish --execute

The single `[workspace.dependencies]` bevy knob and the fork markers
(`docs/fork-patches.md`) keep cross-branch conflicts small.

### Cutting the next maintenance branch
When Bevy 0.19 lands: cut `release/bevy-0.18` from `main`, then bump `main` to
`0.3.0` + bevy 0.19 (vendor the matching flair release, reapply fork patches).

## Publishing
`cargo run -p xtask -- publish` dry-runs the whole workspace in dependency order.
Add `--execute` to publish for real (irreversible). See `docs/fork-patches.md`
before vendoring a new flair release.
```

- [ ] **Step 2: Commit the doc on `main`**

```bash
git add CONTRIBUTING.md
git commit -m "docs: branching, backport, and publishing workflow"
```

- [ ] **Step 3: Cut the maintenance branch from this exact commit**

```bash
git branch release/bevy-0.17
git push -u origin release/bevy-0.17
```
This preserves the fully-publish-ready bevy-0.17 / `0.1.x` state. **All subsequent tasks happen on `main`.**

- [ ] **Step 4: Verify the branch exists and matches**

Run: `git log --oneline -1 release/bevy-0.17 && git log --oneline -1 main`
Expected: both point at the same commit (the CONTRIBUTING commit).

### Task 10: Vendor upstream flair's bevy-0.18 release + reapply fork patches

**Files:**
- Modify: sources under `crates/superui_flair_core/`, `crates/superui_flair_style/`, `crates/superui_flair_css_parser/`
- Modify: `docs/fork-patches.md` (record the new upstream base version)

**Interfaces:**
- Consumes: fork-patch markers/registry (Tasks 3–4).
- Produces: forks whose upstream base is the bevy-0.18-compatible flair release, with all registry patches reapplied.

- [ ] **Step 1: Confirm the upstream flair release that targets bevy 0.18**

Already resolved during pre-flight: **`bevy_flair 0.7.0`** is the release whose `bevy_*` deps are `^0.18` (verified via crates.io; `0.6.0`=bevy 0.17, `0.7.0`=bevy 0.18, `0.8.0`=bevy 0.19). Use `0.7.0` as the new vendored base and record it in `docs/fork-patches.md`. Re-confirm with `curl -s https://crates.io/api/v1/crates/bevy_flair/0.7.0/dependencies` if desired.

- [ ] **Step 2: Re-vendor the three crates' `src/` from that release**

Replace each fork's `src/` (and Cargo dep lists for any new/removed `bevy_*` subcrates) with the upstream 0.18 release sources. Keep our package/lib names (`superui_flair_*`) and the `path` inter-fork deps. Keep every `bevy_* = { workspace = true }` inheritance (add workspace entries for any new subcrate the new release introduces).

- [ ] **Step 3: Reapply every registered fork patch via its markers**

For each `### <id>` in `docs/fork-patches.md`, locate the upstream code at its "Upstream location" in the freshly vendored source and re-wrap the deviation in `>>> / <<< SUPERUI-FORK-PATCH: <id>` markers. For `css-eof-guard`, reapply the `let-else` empty-span guard (Task 3 Step 2). Update the "Upstream base" line in `docs/fork-patches.md` to the new version.

- [ ] **Step 4: Verify the drift check still passes**

Run: `cargo run -p xtask -- fork-patches`
Expected: prints `css-eof-guard` (and any other ids), no drift error.

- [ ] **Step 5: Commit (compiles fully only after Task 11 flips bevy)**

```bash
git add crates/superui_flair_* docs/fork-patches.md
git commit -m "feat(flair): rebase forks onto upstream bevy-0.18 release; reapply fork patches"
```

### Task 11: Flip the workspace to bevy 0.18 and fix breakage

**Files:**
- Modify: `Cargo.toml` (`[workspace.dependencies]` bevy pins → `0.18`; `[workspace.package] version` → `0.2.0`)
- Modify: `crates/superui_test_engine/Cargo.toml` (`bevy_egui` → bevy-0.18-compatible version)
- Modify: whatever `crates/*` and `examples/*` Rust sources the 0.18 API changes require (discovery-driven)

**Interfaces:** none new — this is the version flip + porting.

- [ ] **Step 1: Flip the single version knob + workspace version**

In root `Cargo.toml`: change every `bevy`/`bevy_*` entry under `[workspace.dependencies]` from `"0.17"` to `"0.18"`, and `[workspace.package] version` from `0.1.0` to `0.2.0`.

Then bump the **internal path-dep version requirements** from `0.1.0` to `0.2.0`. Task 5 pinned every normal intra-workspace path dep as `{ path = "...", version = "0.1.0" }` (and the flair forks use table-form `version = "0.6.0"` → these become the fork's own new version). Because all crates share one workspace version, the simplest reliable way is `cargo install cargo-edit` then `cargo set-version --workspace 0.2.0`, which rewrites BOTH `[workspace.package] version` AND every intra-workspace dependency `version` req in lockstep. If doing it by hand instead, grep for `version = "0.1.0"` across `crates/*/Cargo.toml` and bump each intra-workspace dep (leave third-party deps alone). Verify afterward: `grep -rn 'version = "0.1.0"' crates/` returns nothing for intra-workspace deps.

- [ ] **Step 2: Bump ecosystem deps that track bevy**

In `crates/superui_test_engine/Cargo.toml`, change `bevy_egui = "0.37"` to **`"0.39"`** (pre-resolved: bevy_egui 0.39.x targets bevy ^0.18; 0.38=bevy0.17, 0.40=bevy0.19). Check `examples/*/Cargo.toml` for any other bevy-ecosystem crates and bump likewise (none found at plan time, but re-check).

- [ ] **Step 3: Compile and fix breakage iteratively**

Run: `cargo build --workspace`
Work through each compile error from bevy 0.18 API changes across `superui_*`, `supersolid*`, forks, and `examples/*`. Repeat until green. (Breakage is discovery-driven; the passing build is the objective spec.)
Expected end state: `cargo build --workspace` succeeds on bevy 0.18.

- [ ] **Step 4: Run the full test suite**

Run: `cargo test --workspace`
Expected: PASS — including the flair EOF-guard regression and the xtask drift/publish-order tests.

- [ ] **Step 5: Smoke-test a native example and a wasm build**

Run: `cargo run -p todomvc_supersolid --features hmr` (launch, confirm UI renders, close) and `cargo build -p todomvc_supersolid --target wasm32-unknown-unknown --release`
Expected: native window renders the TodoMVC UI; wasm build succeeds.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat!: upgrade workspace to bevy 0.18 (0.2.0 track)"
```

### Task 12: Reconcile docs for the 0.18/0.2.x current track

**Files:**
- Modify: `README.md` (Status/Compatibility wording if needed)
- Modify: `website/src/docs/reference/compatibility.md` (confirm current row)

**Interfaces:** none.

- [ ] **Step 1: Confirm the compat table reflects reality**

Verify both tables show `0.2.x / 0.18 / main / current` and `0.1.x / 0.17 / release/bevy-0.17 / maintained`. Update the exact flair base version in `docs/fork-patches.md` if not already done in Task 10.

- [ ] **Step 2: Verify book builds + dry-run still green on 0.18**

Run: `mdbook build website && cargo run -p xtask -- publish`
Expected: book builds; workspace packages in order with no metadata errors on the 0.18 tree.

- [ ] **Step 3: Commit**

```bash
git add README.md website/ docs/fork-patches.md
git commit -m "docs: confirm 0.2.x/bevy-0.18 as current track"
```

### Task 13: Publish handoff (MAINTAINER runs the real publish — Claude does NOT)

**Files:** none (release action).

**Interfaces:** none.

> **⚠️ Execution rule:** When Claude executes this plan, Task 13 is a **handoff,
> not an action**. Claude does ONLY its two checkboxes (a safe dry run, then
> printing the handoff text) and then stops. Claude must NEVER run `cargo publish`
> or `xtask publish --execute`, and must NOT run anything in the "MAINTAINER
> HANDOFF" block — that block is text Claude hands to the maintainer, who runs it
> themselves. The plan is "complete" for Claude once the dry runs are green and
> the handoff has been delivered.

> **Name availability (checked 2026-07-24):** all 15 crate names — `superui`,
> `supersolid`, `cargo-superui`, `superui_dom`, `superui_html`, `superui_js`,
> `superui_api`, `superui_css`, `superui_bridge`, `supersolid_runtime`,
> `superui_paths`, `superui_test_engine`, `superui_flair_core`,
> `superui_flair_style`, `superui_flair_css_parser` — were **free** on crates.io
> (404). The short names `superui`/`supersolid` are squattable; publish promptly
> once the tree builds.

**Claude's only actions for this task are the two checkboxes below — both are
safe (network-read-only dry runs / printing text). Everything under "MAINTAINER
HANDOFF" is NOT a Claude step: Claude copies that block into its final message
and stops.**

- [ ] **Step 1 (Claude): Final dry run on each branch**

On `main`: `cargo run -p xtask -- publish` (expect green — this is `cargo package`, no upload). On `release/bevy-0.17`: `git checkout release/bevy-0.17 && cargo run -p xtask -- publish` (expect green), then `git checkout main`.

- [ ] **Step 2 (Claude): Ask the user to do the publishing, and explain how**

In the final message, explicitly **ask the user to perform the publishing
themselves** (Claude cannot and will not), and explain how: confirm the dry runs
are green, then hand over the "MAINTAINER HANDOFF" block below verbatim as the
step-by-step instructions. Say plainly why it's on them — it's irreversible,
needs their `cargo login` token + crates.io ownership, and the first publish of
each name claims it under their account. Offer to answer questions or help
troubleshoot the output afterward, but do **not** run any command from the
handoff block. This completes Claude's involvement in the plan.

Suggested wording for Claude's final message:
> "The workspace is publish-ready and both branches dry-run green. I can't run
> the actual publish for you — it's irreversible, uses your crates.io login, and
> the first publish claims each crate name under your account. Please run it
> yourself with the steps below, and I'll help if anything errors:"  *(then the
> MAINTAINER HANDOFF block)*

---

#### MAINTAINER HANDOFF — run these yourself (not Claude)

> These are the irreversible publish steps. They require *your* `cargo login`
> token and crates.io ownership. Claude will not and must not run them.

Run every block below from the repo root (`C:\work\bevy_superui`). Each block is
copy-pasteable as-is; they are meant to be run in order.

**H0. Log in + preflight.** Authenticate to crates.io and re-verify names are
still free (anyone can claim them before you publish):
```bash
cargo login                 # paste your crates.io API token from https://crates.io/settings/tokens
git status                  # confirm a clean working tree before releasing
for n in superui supersolid cargo-superui superui_dom superui_html superui_js \
         superui_api superui_css superui_bridge supersolid_runtime superui_paths \
         superui_test_engine superui_flair_core superui_flair_style superui_flair_css_parser; do
  printf '%s ' "$n"; curl -s -o /dev/null -w '%{http_code}\n' "https://crates.io/api/v1/crates/$n"
done   # 404 = free, 200 = taken
```
Any `200` = name already taken → stop and resolve (rename the crate or contact the owner) before continuing.

**H1. Publish the 0.17 track (versions `0.1.0`).** Switch to the maintenance
branch, dry-run once more, then publish:
```bash
git checkout release/bevy-0.17
git pull --ff-only                       # make sure it's up to date
cargo run -p xtask -- publish            # dry run (cargo package, no upload) — expect green
cargo run -p xtask -- publish --execute  # THE REAL PUBLISH — irreversible
```
Publishes `0.1.0` for all 14 crates in dependency order. If one fails because a just-published dependency isn't indexed on crates.io yet, wait ~30s and re-run the `--execute` line — already-published versions error harmlessly, so it resumes from the failure point.

**H2. Publish the 0.18 track (versions `0.2.0`).** Switch back to `main` and do
the same:
```bash
git checkout main
git pull --ff-only
cargo run -p xtask -- publish            # dry run — expect green
cargo run -p xtask -- publish --execute  # THE REAL PUBLISH — irreversible
```
Publishes `0.2.0` for all 14 crates.

**H3. Tag the releases (optional but recommended):**
```bash
git checkout release/bevy-0.17 && git tag v0.1.0 && git push origin v0.1.0
git checkout main            && git tag v0.2.0 && git push origin v0.2.0
```

**H4. Verify on crates.io.** Confirm `superui`, `cargo-superui`,
`superui_flair_*`, and the rest each show both `0.1.0` and `0.2.0`, and that the
CLI installs both tracks:
```bash
cargo install cargo-superui         # latest = 0.2.x
cargo install cargo-superui@0.1     # explicit 0.17-track pin
```

---

## Self-Review — spec coverage

- Spec §1 workspace knob → Task 1. §2 fork rename → Task 2. §3 patch registry/markers/drift check → Tasks 3–4. §4 0.18 upgrade (vendor upstream flair + reapply + fix breakage) → Tasks 10–11. §5 versioning (0.1.x/0.2.x) → Tasks 1 (0.1.0), 11 (0.2.0). §6 branching → Task 9. §7 publishing (metadata, drop publish=false, topological driver, dry-run gate) → Tasks 5–6, 13. §7 CLI distribution/`cargo install` → Task 5 (publishable) + Task 13 handoff H3. §7a CLI forward-compat invariant + test + pinning docs → Task 7 (test) + Task 9 CONTRIBUTING / compatibility pointer. §8 compat table README + website → Task 8.
- Every `publish = false` dropped (Task 5). `superui_test_engine` source untouched (constraint honored). No auto-publish (Task 13 manual). Marker grammar identical across Tasks 3/4/10. `publish_order` (Task 6) matches the topological table.
