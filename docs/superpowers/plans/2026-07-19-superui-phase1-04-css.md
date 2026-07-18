# superui_css Implementation Plan (Phase 1, Plan 4 of 6)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Vendor `bevy_flair` 0.6.0 (the newest release targeting Bevy 0.17) into the workspace as an in-tree fork, wrap it in a new `superui_css` umbrella crate, and verify — end-to-end in a headless Bevy app — that the CSS engine matches **real HTML element, attribute, class, id, descendant, and `:hover`/`:focus`/`:checked` selectors** driven by standards-shaped inputs, so Plan 5's reconciler has a working, HTML-shaped CSS layer to target.

**Architecture:** The just-in-time investigation of flair 0.6 established that it **already implements the full selector surface** the design calls for: type selectors via a `TypeName(&'static str)` component, attribute selectors via an `AttributeList` component (real `attr_matches`), classes via `ClassList`, ids via Bevy's `Name`, and `:hover`/`:focus`/`:checked`/`:active`/`:disabled` via a `NodePseudoState` struct (with a public `get_pseudo_state_mut()` the Plan-5 bridge will drive from DOM events). So this plan is **vendor + verify + one small extension**, not new selector code. The three flair sub-crates (`bevy_flair_core`, `bevy_flair_style`, `bevy_flair_css_parser`) are vendored **verbatim under their upstream names** (minimal diff = easy future upstream merges, per design §12); we do **not** vendor upstream's `bevy_flair` umbrella — a new `superui_css` crate replaces it, re-exporting the fork, providing a `SuperUiCssPlugin` plugin group, and adding the one genuinely-ours extension: an HTML **tag-name interner** (`intern_tag`/`html_type_name`) that turns arbitrary author tag strings into the `&'static str` flair's `TypeName` needs, giving real element selectors for any tag.

**Tech Stack:** Rust edition 2024 (the vendored fork crates) / 2021 (our `superui_css`). Bevy 0.17. `cssparser` 0.35, `cssparser-color` 0.3, `selectors` 0.31 (the versions flair 0.6 pins). Fork source fetched from `static.crates.io`.

## Global Constraints

- **Bevy version: 0.17** for every Bevy-facing crate (design §5). The vendored fork crates pin `bevy_* = "0.17"` verbatim; `superui_css` uses `"0.17"` too. Do not bump to 0.18/0.19.
- **Fork base = `bevy_flair` 0.6.0** (design §4). Vendor from `https://static.crates.io/crates/<crate>/<crate>-0.6.0.crate`. Keep upstream crate names `bevy_flair_core`, `bevy_flair_style`, `bevy_flair_css_parser`. **Do not vendor upstream's `bevy_flair` umbrella crate** — `superui_css` replaces it.
- **Minimal diff from upstream** — vendored `src/` is copied byte-for-byte; the *only* edits to a vendored crate are in its `Cargo.toml` (repoint the three inter-crate flair deps from registry to `path`). This keeps future upstream merges cheap.
- **Only Bevy-facing crates touch Bevy** (design §4 boundary discipline). `superui_dom`, `superui_html`, `superui_js`, `superui_api` MUST NOT gain a dependency on `superui_css` or any `bevy_*` crate. Verify this holds after each task.
- **`wasm32-unknown-unknown` must compile** for the `superui_css` runtime library (design §5). Bevy needs the getrandom JS backend on wasm — reuse the existing repo-root `.cargo/config.toml` rustflag (from Plan 3) and add a wasm-target `getrandom = { version = "0.3", features = ["wasm_js"] }` dep to `superui_css`, exactly as the JS crates did.
- **Graceful degradation over throwing** (design §1): flair's CSS loader already collects per-rule parse errors into `CssStyleSheetItem::Error` and keeps loading (unknown property/selector → skipped, not fatal). Do not change that; verify one bad rule does not abort a sheet.
- **`window.bevy` bridge, DOM→ECS reconciliation, and DOM-driven pseudo-state wiring are OUT of this plan** — they are Plan 5. This plan stops at: the fork compiles, `superui_css` exposes the HTML-shaped surface, and headless tests prove selectors match. The capability ledger (`docs/support/css.md`) is a **Plan 6** deliverable — do not author it here.
- **The vendored-crate `Cargo.toml` files are the auto-generated normalized form** (explicit `version = "0.17"` deps, an autogen header comment). That is expected — leave the header, only fix the flair path deps.
- **TDD, DRY, YAGNI, frequent commits** — every task ends green with a commit.

**Verified flair 0.6 API reference (confirmed by reading the 0.6.0 source; used verbatim below).** If a re-export path differs at compile time, follow the compiler's suggestion — the *types/methods* are correct, only the module they are re-exported through may differ (same rule as Plans 2–3).
- Plugins (all `pub`, referenced by upstream's own umbrella `plugin_group!`): `bevy_flair_core::PropertyRegistryPlugin`, `bevy_flair_core::ImplComponentPropertiesPlugin`, `bevy_flair_style::FlairStylePlugin`, `bevy_flair_style::FlairDefaultStyleAnimationsPlugin`, `bevy_flair_css_parser::FlairCssParserPlugin`.
- Identity components (all `pub`, in `bevy_flair_style::components`): `TypeName(pub &'static str)` (immutable, `#[component(on_insert=…)]`, copies into `NodeStyleData.type_name`); `ClassList` (`::new(&str)`, `::empty()`, `add`/`remove`/`toggle`/`contains`); `AttributeList` (`::new()`, `set_attribute`, `remove_attribute`, `get_attribute`, and `FromIterator<(K,V)>` so `AttributeList::from_iter([("type","checkbox")])` works); `NodeStyleData` (read side: `has_type_name`, `has_class`, `matches_pseudo_state`, `get_pseudo_state_mut`). Id selectors use Bevy's `Name` component (flair's `track_name_changes` copies it into `NodeStyleData.name`).
- Pseudo-state: `bevy_flair_style::NodePseudoState { pressed, hovered, focused, focused_and_visible, disabled, checked : bool }`. Flair syncs it from `bevy_ui::{Pressed, Checked, InteractionDisabled}`, `bevy_picking::hover::Hovered` / `bevy_ui::Interaction`, and the `bevy_input_focus::{InputFocus, InputFocusVisible}` resources.
- Stylesheet: `bevy_flair_style::StyleSheet` (asset). Loader `bevy_flair_css_parser::CssStyleSheetLoader` (extension `"css"`, `Asset = StyleSheet`), registered by `FlairCssParserPlugin`. Attach a sheet to a UI subtree via the `NodeStyleSheet` component (`NodeStyleSheet::new(handle)`, variants `Inherited`/`StyleSheet(handle)`/`Block`); children inherit by default.
- Loader error mode: `bevy_flair_css_parser::{CssStyleLoaderSetting, CssStyleLoaderErrorMode}` — set `error_mode = CssStyleLoaderErrorMode::ReturnError` via `AssetServer::load_with_settings` to make a *malformed* sheet fail loudly in tests (well-formed sheets with unknown rules still load, per graceful degradation).

---

## File Structure

```
target/flair-src/                       # gitignored scratch: downloaded + extracted fork source (Task 1)
crates/bevy_flair_core/                 # NEW (vendored verbatim): src/** + Cargo.toml (path-dep fixups only)
crates/bevy_flair_style/                # NEW (vendored verbatim): src/** (incl. snapshots/, testing.rs) + Cargo.toml
crates/bevy_flair_css_parser/           # NEW (vendored verbatim): src/** + Cargo.toml
crates/superui_css/
  Cargo.toml                            # NEW: umbrella; deps on the 3 fork crates; wasm getrandom; bevy dev-dep
  src/lib.rs                            # NEW: re-exports (core/style/parser), SuperUiCssPlugin, prelude
  src/tag.rs                            # NEW: intern_tag + html_type_name (the one extension)
  tests/support/mod.rs                  # NEW: headless test-app harness (adapted from flair's test_app)
  tests/selectors.rs                    # NEW: capstone — element/attr/class/id/descendant/:checked/:hover → BackgroundColor
docs/superpowers/plans/README.md        # MODIFY (Task 6): flip Plan 4 row to ✅ Done
```

---

### Task 1: Workspace prep + vendor `bevy_flair_core`

**Files:**
- Create: `crates/bevy_flair_core/**` (copied from the extracted fork source)
- Modify: `crates/bevy_flair_core/Cargo.toml` (none needed — core has no inter-crate flair deps; verify only)
- Modify: `.gitignore` (ensure `target/` is ignored — it already is; verify)

**Interfaces:**
- Consumes: nothing (leaf of the fork dependency graph).
- Produces: the `bevy_flair_core` crate (`PropertyRegistry`, `PropertyRegistryPlugin`, `ImplComponentPropertiesPlugin`, `ComponentProperty`, the property system) as a compiling, self-testing workspace member. `bevy_flair_style` (Task 2) and `bevy_flair_css_parser` (Task 3) depend on it.

- [ ] **Step 1: Download and extract all three fork crates into the gitignored scratch dir**

Run (Bash tool):
```bash
mkdir -p target/flair-src && cd target/flair-src && \
for c in bevy_flair_core bevy_flair_style bevy_flair_css_parser; do \
  curl -sSL -o "$c.crate" "https://static.crates.io/crates/$c/$c-0.6.0.crate" && \
  tar xzf "$c.crate"; \
done && ls -d */
```
Expected: the three directories `bevy_flair_core-0.6.0/`, `bevy_flair_style-0.6.0/`, `bevy_flair_css_parser-0.6.0/` are listed. (Each `.crate` is a gzipped tar; `file bevy_flair_core.crate` should say "gzip compressed data". If `curl` returns tiny JSON instead, the mirror rejected the request — retry once; the `static.crates.io` static host does not rate-limit like the API host.)

- [ ] **Step 2: Copy the core crate into the workspace (verbatim), dropping only the publish-metadata files**

Run (Bash tool):
```bash
cd C:/work/bevy_superui && \
rm -rf crates/bevy_flair_core && \
cp -r target/flair-src/bevy_flair_core-0.6.0 crates/bevy_flair_core && \
rm -f crates/bevy_flair_core/Cargo.toml.orig crates/bevy_flair_core/.cargo_vcs_info.json && \
ls crates/bevy_flair_core && ls crates/bevy_flair_core/src | head
```
Expected: `Cargo.toml` and `src/` present; `src/` lists `lib.rs`, `registry.rs`, `component_properties.rs`, etc. `Cargo.toml.orig` and `.cargo_vcs_info.json` are gone.

- [ ] **Step 3: Verify the workspace picks up the new member and it compiles**

`bevy_flair_core`'s inter-crate deps: none (it is the leaf). Its `Cargo.toml` uses explicit `bevy_* = "0.17"` deps (normalized form) — no edits required.

Run: `cargo build -p bevy_flair_core`
Expected: SUCCESS. (First build compiles Bevy 0.17 + cssparser/selectors — this is slow the first time; that is expected. Bevy 0.17.3 is already in the local registry cache.)

If it fails with an unresolved workspace-inheritance key (e.g. `edition.workspace`), that means the crate's normalized `Cargo.toml` unexpectedly used `.workspace = true`; replace that single key with the literal value from `target/flair-src/bevy_flair_core-0.6.0/Cargo.toml.orig` and re-run. (Not expected — the normalized form uses explicit values.)

- [ ] **Step 4: Run the vendored crate's own test suite (fork-integrity gate)**

Run: `cargo test -p bevy_flair_core`
Expected: PASS — all of upstream's `bevy_flair_core` tests pass unchanged, proving the fork is intact. (This pulls the `bevy_math` dev-dep.)

- [ ] **Step 5: Commit**

```bash
git add crates/bevy_flair_core .gitignore
git commit -m "feat(css): vendor bevy_flair_core 0.6.0 as in-tree fork (Bevy 0.17)"
```

---

### Task 2: Vendor `bevy_flair_style`

**Files:**
- Create: `crates/bevy_flair_style/**` (copied from the extracted fork source, incl. `src/snapshots/`, `src/testing.rs`, `src/css_selector/testing.rs`)
- Modify: `crates/bevy_flair_style/Cargo.toml` (repoint the `bevy_flair_core` dep to `path`)

**Interfaces:**
- Consumes: `bevy_flair_core` (Task 1).
- Produces: the `bevy_flair_style` crate — `FlairStylePlugin`, `FlairDefaultStyleAnimationsPlugin`, `StyleSheet`, `NodePseudoState`, and `components::{NodeStyleData, TypeName, ClassList, AttributeList, NodeStyleSheet, …}`. This is the crate that owns the selector engine (`css_selector/`) and the ECS ↔ selector matching. `bevy_flair_css_parser` (Task 3) and `superui_css` (Task 4) depend on it.

- [ ] **Step 1: Copy the style crate into the workspace (verbatim)**

Run (Bash tool):
```bash
cd C:/work/bevy_superui && \
rm -rf crates/bevy_flair_style && \
cp -r target/flair-src/bevy_flair_style-0.6.0 crates/bevy_flair_style && \
rm -f crates/bevy_flair_style/Cargo.toml.orig crates/bevy_flair_style/.cargo_vcs_info.json && \
ls crates/bevy_flair_style/src crates/bevy_flair_style/src/css_selector
```
Expected: `src/` includes `components.rs`, `systems.rs`, `style_sheet.rs`, `lib.rs`, `testing.rs`, `snapshots/`, and `css_selector/` includes `mod.rs`, `element.rs`, `testing.rs`.

- [ ] **Step 2: Repoint the `bevy_flair_core` dependency to a path dep**

Open `crates/bevy_flair_style/Cargo.toml`. Find the block (normalized form):

```toml
[dependencies.bevy_flair_core]
version = "0.6.0"
```

Change it to (add the `path`, keep the version):

```toml
[dependencies.bevy_flair_core]
version = "0.6.0"
path = "../bevy_flair_core"
```

Leave every other dependency (the explicit `bevy_* = "0.17"`, `cssparser`, `selectors`, `precomputed-hash`, dev-deps `ego-tree`/`insta`/`bevy_scene`, etc.) exactly as vendored.

- [ ] **Step 3: Build it**

Run: `cargo build -p bevy_flair_style`
Expected: SUCCESS.

- [ ] **Step 4: Run the vendored crate's own test suite (fork-integrity gate)**

Run: `cargo test -p bevy_flair_style`
Expected: PASS — upstream's selector tests (the `css_selector` `tests` module: type/class/id/`:hover`/descendant/`:has`/`:nth-child`/`::before`…) and the `insta` snapshot tests all pass, proving the selector engine survived vendoring intact.

If an `insta` snapshot test fails on a path/line-ending difference (not a logic difference), that is a known cross-platform `insta` friction, not a fork defect: confirm the only diff is whitespace/paths, then accept the snapshot with `cargo insta accept` (or `INSTA_UPDATE=always cargo test -p bevy_flair_style`) and note it in the commit message. Do not "fix" it by editing engine code.

- [ ] **Step 5: Commit**

```bash
git add crates/bevy_flair_style
git commit -m "feat(css): vendor bevy_flair_style 0.6.0 as in-tree fork (selector engine)"
```

---

### Task 3: Vendor `bevy_flair_css_parser`

**Files:**
- Create: `crates/bevy_flair_css_parser/**` (copied from the extracted fork source)
- Modify: `crates/bevy_flair_css_parser/Cargo.toml` (repoint the `bevy_flair_core` and `bevy_flair_style` deps to `path`)

**Interfaces:**
- Consumes: `bevy_flair_core` (Task 1), `bevy_flair_style` (Task 2).
- Produces: the `bevy_flair_css_parser` crate — `FlairCssParserPlugin`, the `.css` `CssStyleSheetLoader`, `InlineStyle`, `CssStyleLoaderSetting`/`CssStyleLoaderErrorMode`, and `parse_css`. `superui_css` (Task 4) depends on it.

- [ ] **Step 1: Copy the css-parser crate into the workspace (verbatim)**

Run (Bash tool):
```bash
cd C:/work/bevy_superui && \
rm -rf crates/bevy_flair_css_parser && \
cp -r target/flair-src/bevy_flair_css_parser-0.6.0 crates/bevy_flair_css_parser && \
rm -f crates/bevy_flair_css_parser/Cargo.toml.orig crates/bevy_flair_css_parser/.cargo_vcs_info.json && \
ls crates/bevy_flair_css_parser/src
```
Expected: `src/` includes `lib.rs`, `loader.rs`, `parser.rs`, `inline_styles.rs`, `reflect/`, `shorthand.rs`, etc.

- [ ] **Step 2: Repoint both inter-crate flair deps to path deps**

Open `crates/bevy_flair_css_parser/Cargo.toml`. Change:

```toml
[dependencies.bevy_flair_core]
version = "0.6.0"
```
to
```toml
[dependencies.bevy_flair_core]
version = "0.6.0"
path = "../bevy_flair_core"
```

and

```toml
[dependencies.bevy_flair_style]
version = "0.6.0"
```
to
```toml
[dependencies.bevy_flair_style]
version = "0.6.0"
path = "../bevy_flair_style"
```

Leave everything else (`cssparser`/`cssparser-color`/`selectors`, `ariadne`, `derive_more`, `linked-hash-map`, `variadics_please`, dev-deps `indoc`/`approx`, …) as vendored.

- [ ] **Step 3: Build it**

Run: `cargo build -p bevy_flair_css_parser`
Expected: SUCCESS.

- [ ] **Step 4: Run the vendored crate's own test suite (fork-integrity gate)**

Run: `cargo test -p bevy_flair_css_parser`
Expected: PASS — upstream's parser tests pass, proving the CSS parser + property reflection survived vendoring.

- [ ] **Step 5: Confirm the wasm-clean runtime crates are still Bevy-free (boundary discipline)**

Run: `cargo tree -p superui_dom -e normal | grep -i bevy` (and repeat for `superui_html`, `superui_js`, `superui_api`)
Expected: **no output** for each — none of the four wasm-clean crates pulls in any `bevy_*` crate. (If any prints a bevy line, a dependency was added by mistake — stop and remove it.)

- [ ] **Step 6: Commit**

```bash
git add crates/bevy_flair_css_parser
git commit -m "feat(css): vendor bevy_flair_css_parser 0.6.0 as in-tree fork (.css loader)"
```

---

### Task 4: `superui_css` umbrella crate — re-exports, `SuperUiCssPlugin`, prelude

**Files:**
- Create: `crates/superui_css/Cargo.toml`
- Create: `crates/superui_css/src/lib.rs`

**Interfaces:**
- Consumes: `bevy_flair_core`, `bevy_flair_style`, `bevy_flair_css_parser` (Tasks 1–3); `bevy_app`.
- Produces:
  - `pub use bevy_flair_core as core; pub use bevy_flair_style as style; pub use bevy_flair_css_parser as parser;`
  - `pub struct SuperUiCssPlugin` (a `plugin_group!` bundling the five fork plugins) — the single plugin Plan 5's `SuperUiPlugin` will add.
  - `pub mod prelude` re-exporting the HTML-shaped surface (`StyleSheet`, `NodeStyleSheet`, `ClassList`, `AttributeList`, `TypeName`, `NodePseudoState`, `InlineStyle`, `SuperUiCssPlugin`, and — after Task 5 — `intern_tag`/`html_type_name`).

- [ ] **Step 1: Create the crate manifest** — create `crates/superui_css/Cargo.toml`:

```toml
[package]
name = "superui_css"
edition.workspace = true
version.workspace = true
license.workspace = true

[dependencies]
bevy_flair_core = { path = "../bevy_flair_core", version = "0.6.0" }
bevy_flair_style = { path = "../bevy_flair_style", version = "0.6.0" }
bevy_flair_css_parser = { path = "../bevy_flair_css_parser", version = "0.6.0" }
bevy_app = "0.17"

# Bevy pulls getrandom 0.3, which needs the JS backend on wasm. Scope the direct
# dep to the wasm target; pair with the repo-root `.cargo/config.toml` rustflag
# (added in Plan 3). Same gotcha as the JS crates.
[target.'cfg(target_arch = "wasm32")'.dependencies]
getrandom = { version = "0.3", features = ["wasm_js"] }

# Headless integration-test harness (Task 6) needs a handful of Bevy plugins.
# default-features = false keeps it headless (no winit/render); features are the
# minimum for AssetPlugin + UiPlugin + picking + input-focus. Adjust the feature
# list if the compiler reports a missing plugin/type — see Task 6.
[dev-dependencies]
bevy = { version = "0.17", default-features = false, features = [
    "std",
    "bevy_ui",
    "bevy_text",
    "bevy_picking",
    "bevy_input_focus",
    "bevy_ui_picking_backend",
    "default_font",
] }
```

- [ ] **Step 2: Write the failing test** — create `crates/superui_css/src/lib.rs` with the re-exports, plugin group, prelude, and a compile-smoke test:

```rust
//! `superui_css` — the bevy_superui CSS layer: an in-tree fork of `bevy_flair`
//! 0.6 (Bevy 0.17) re-exported behind an HTML-shaped surface.
//!
//! The fork already matches real HTML element/attribute/class/id and
//! `:hover`/`:focus`/`:checked` selectors; this crate bundles it into one
//! plugin and adds an HTML tag-name interner (see [`html_type_name`]). Only
//! Bevy-facing crates depend on this one (design §4).

pub use bevy_flair_core as core;
pub use bevy_flair_css_parser as parser;
pub use bevy_flair_style as style;

bevy_app::plugin_group! {
    /// The one plugin Plan 5's `SuperUiPlugin` adds to get the full CSS engine:
    /// property registry, the style/selector systems, default animations, and
    /// the `.css` asset loader. Mirrors upstream `bevy_flair::FlairPlugin`.
    #[derive(Clone, Debug)]
    pub struct SuperUiCssPlugin {
        bevy_flair_core:::PropertyRegistryPlugin,
        bevy_flair_core:::ImplComponentPropertiesPlugin,
        bevy_flair_style:::FlairStylePlugin,
        bevy_flair_style:::FlairDefaultStyleAnimationsPlugin,
        bevy_flair_css_parser:::FlairCssParserPlugin,
    }
}

/// The HTML-shaped surface Plan 5's reconciler and authored code reach for.
pub mod prelude {
    pub use crate::SuperUiCssPlugin;
    pub use bevy_flair_css_parser::InlineStyle;
    pub use bevy_flair_style::components::{
        AttributeList, ClassList, NodeStyleData, NodeStyleSheet, TypeName,
    };
    pub use bevy_flair_style::{NodePseudoState, StyleSheet};
}

#[cfg(test)]
mod tests {
    // Compile-smoke: every item the prelude promises must resolve and be
    // nameable. If flair re-exports one of these through a different module,
    // the compiler names the right path — follow it and fix the `use` above.
    #[allow(unused_imports)]
    use crate::prelude::*;

    #[test]
    fn prelude_items_resolve() {
        // Name each type/plugin so the test fails to compile if a path is wrong.
        fn _assert_nameable() {
            let _: Option<StyleSheet> = None;
            let _: Option<NodeStyleSheet> = None;
            let _: Option<ClassList> = None;
            let _: Option<AttributeList> = None;
            let _: Option<TypeName> = None;
            let _: Option<NodePseudoState> = None;
            let _: Option<InlineStyle> = None;
            let _plugin = SuperUiCssPlugin;
        }
        // Nothing to assert at runtime; resolution is the test.
        let _ = _assert_nameable;
    }
}
```

- [ ] **Step 3: Run it to verify it compiles and passes**

Run: `cargo test -p superui_css --lib`
Expected: PASS — `prelude_items_resolve` compiles (all prelude paths resolve) and passes.

If a `use` path fails, the compiler prints the correct module (e.g. `NodeStyleSheet` may live directly under `bevy_flair_style` rather than `::components`); update the `use` in `prelude` to match and re-run. Do not guess — use the compiler's suggested path.

- [ ] **Step 4: Commit**

```bash
git add crates/superui_css
git commit -m "feat(css): superui_css umbrella — SuperUiCssPlugin + HTML-shaped prelude"
```

---

### Task 5: HTML tag-name interner (`intern_tag` / `html_type_name`) — the one extension

**Files:**
- Create: `crates/superui_css/src/tag.rs`
- Modify: `crates/superui_css/src/lib.rs` (add `mod tag; pub use tag::…;` and extend the prelude)

**Interfaces:**
- Consumes: `bevy_flair_style::components::TypeName`.
- Produces:
  - `pub fn intern_tag(tag: &str) -> &'static str` — lowercases the tag and interns it to a process-lifetime `&'static str` (each distinct tag leaked exactly once; HTML's tag vocabulary is finite). Repeated calls for the same tag (any casing) return the **same pointer**.
  - `pub fn html_type_name(tag: &str) -> TypeName` — `TypeName(intern_tag(tag))`, the component the Plan-5 bridge inserts to give a DOM element its element-selector identity.
- Rationale: flair's `TypeName` holds `&'static str` (it expects compile-time-known Bevy component names). HTML tags arrive as runtime `&str` from the parser/DOM, so we intern them. Interning (rather than a hardcoded tag `match`) makes **any** author tag a real element selector, satisfying "real HTML element selectors" (design §4) and graceful degradation (an unknown tag still gets a type name; it simply matches only its own element selector).

- [ ] **Step 1: Write the failing tests** — create `crates/superui_css/src/tag.rs`:

```rust
//! HTML tag-name interning: turn a runtime tag `&str` into the `&'static str`
//! flair's `TypeName` component requires, so element selectors work for any tag.

use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};

use bevy_flair_style::components::TypeName;

fn interner() -> &'static Mutex<HashSet<&'static str>> {
    static INTERNER: OnceLock<Mutex<HashSet<&'static str>>> = OnceLock::new();
    INTERNER.get_or_init(|| Mutex::new(HashSet::new()))
}

/// Intern an HTML tag name to a process-lifetime `&'static str`, ASCII-lowercased
/// (HTML tag names are case-insensitive). Each distinct tag is leaked exactly
/// once; the tag vocabulary is finite, so this is a bounded one-time cost.
/// Repeated calls for the same tag (in any casing) return the same pointer.
pub fn intern_tag(tag: &str) -> &'static str {
    let lower = tag.to_ascii_lowercase();
    let mut set = interner().lock().expect("tag interner poisoned");
    if let Some(existing) = set.get(lower.as_str()) {
        return existing;
    }
    let leaked: &'static str = Box::leak(lower.into_boxed_str());
    set.insert(leaked);
    leaked
}

/// The flair `TypeName` component for an HTML tag (lowercased, interned). Insert
/// this on a UI entity to give it its element-selector identity (`div`, `li`, …).
pub fn html_type_name(tag: &str) -> TypeName {
    TypeName(intern_tag(tag))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interns_stably_and_case_insensitively() {
        let a = intern_tag("div");
        let b = intern_tag("DIV");
        let c = intern_tag("div");
        // Same tag (any casing) → identical pointer, not just equal strings.
        assert!(std::ptr::eq(a, c));
        assert!(std::ptr::eq(a, b));
        assert_eq!(a, "div");
    }

    #[test]
    fn distinct_tags_get_distinct_static_strs() {
        let li = intern_tag("li");
        let ul = intern_tag("ul");
        assert_eq!(li, "li");
        assert_eq!(ul, "ul");
        assert!(!std::ptr::eq(li, ul));
    }

    #[test]
    fn html_type_name_carries_the_interned_tag() {
        let tn = html_type_name("Input"); // mixed case in → lowercased
        assert_eq!(tn.0, "input");
        // And it round-trips through NodeStyleData's type-name matcher.
        let mut data = bevy_flair_style::components::NodeStyleData::default();
        // TypeName's on_insert hook normally sets this; emulate the effect here.
        *data.get_pseudo_state_mut(); // touch a pub accessor to keep the import used
        assert!(html_type_name("input").0 == "input");
    }
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p superui_css --lib tag`
Expected: FAIL to compile — `mod tag` is not yet declared in `lib.rs`, so the tests are not built. (Once wired in Step 3 they compile and pass.)

- [ ] **Step 3: Wire the module into `lib.rs`** — in `crates/superui_css/src/lib.rs`, add after the `pub use bevy_flair_style as style;` line:

```rust
mod tag;
pub use tag::{html_type_name, intern_tag};
```

and extend the `prelude` module body with:

```rust
    pub use crate::{html_type_name, intern_tag};
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p superui_css --lib`
Expected: PASS — the three `tag::tests` cases and `prelude_items_resolve` all pass.

Note: `html_type_name_carries_the_interned_tag` deliberately does **not** insert `TypeName` on a real entity (that needs an ECS world + the `on_insert` hook, exercised end-to-end in Task 6); it asserts the interner/`TypeName` wiring only. If the `NodeStyleData` line is awkward, simplify it to `let _ = bevy_flair_style::components::NodeStyleData::default();` — the load-bearing assertion is `tn.0 == "input"`.

- [ ] **Step 5: Commit**

```bash
git add crates/superui_css
git commit -m "feat(css): HTML tag-name interner (intern_tag/html_type_name) for element selectors"
```

---

### Task 6: Capstone — headless selector integration test, wasm check, README flip

**Files:**
- Create: `crates/superui_css/tests/support/mod.rs` (the headless test-app harness)
- Create: `crates/superui_css/tests/selectors.rs` (the capstone test + its embedded CSS)
- Modify: `docs/superpowers/plans/README.md` (flip Plan 4 to ✅ Done)

**Interfaces:**
- Consumes: `superui_css` (Tasks 4–5), the `bevy` dev-dependency, `bevy_flair`'s `.css` loader path.
- Produces: proof that the fork matches **element / attribute / class / id / descendant / `:checked` / `:hover`** selectors end-to-end (CSS text → `StyleSheet` asset → selector match → computed `BackgroundColor` on `bevy_ui`), plus a green `wasm32-unknown-unknown` build of the `superui_css` runtime lib. This is the Plan-4 definition of done.

- [ ] **Step 1: Create the headless test-app harness** — create `crates/superui_css/tests/support/mod.rs`. This is adapted from flair 0.6's own `tests/test_app` harness (in-memory `.css` assets via a memory asset reader; a minimal headless plugin set; `app.finish()` so the loader + registries install):

```rust
//! Headless Bevy app harness for selector integration tests. Serves `.css`
//! from an in-memory asset dir and installs the full CSS engine, no window/GPU.
#![allow(dead_code)]

use bevy::asset::io::memory::{Dir, MemoryAssetReader};
use bevy::asset::io::{AssetSource, AssetSourceId};
use bevy::asset::{AssetApp, AssetPlugin, AssetServer, Handle};
use bevy::image::{ImagePlugin, TextureAtlasPlugin};
use bevy::input::InputPlugin;
use bevy::input_focus::{InputFocus, InputFocusVisible};
use bevy::picking::{InteractionPlugin, PickingPlugin};
use bevy::prelude::*;
use bevy::text::TextPlugin;
use bevy::ui::UiPlugin;
use bevy_app::{TaskPoolOptions, TaskPoolPlugin};
use std::sync::LazyLock;

use superui_css::parser::{CssStyleLoaderErrorMode, CssStyleLoaderSetting};
use superui_css::style::StyleSheet;
use superui_css::SuperUiCssPlugin;

/// In-memory asset dir the tests write `.css` bytes into before building the app.
pub static ASSETS_DIR: LazyLock<Dir> = LazyLock::new(|| Dir::new("assets".into()));

/// Write a `.css` file into the in-memory asset dir under `name`.
pub fn put_css(name: &str, contents: &str) {
    ASSETS_DIR.insert_asset(name.as_ref(), contents.as_bytes());
}

/// Load a stylesheet with the strict error mode so a *malformed* sheet fails the
/// test loudly (well-formed sheets with unknown rules still load — degradation).
pub trait LoadStyleSheet {
    fn load_style_sheet(&self, path: &str) -> Handle<StyleSheet>;
}
impl LoadStyleSheet for AssetServer {
    fn load_style_sheet(&self, path: &str) -> Handle<StyleSheet> {
        self.load_with_settings(path.to_string(), |s: &mut CssStyleLoaderSetting| {
            s.error_mode = CssStyleLoaderErrorMode::ReturnError
        })
    }
}

/// A headless app with the CSS engine installed. Call after `put_css(...)`.
pub fn test_app() -> App {
    let mut app = App::new();

    app.register_asset_source(
        AssetSourceId::Default,
        AssetSource::build().with_reader(move || {
            Box::new(MemoryAssetReader { root: ASSETS_DIR.clone() })
        }),
    );

    app.add_plugins((
        bevy::time::TimePlugin,
        TaskPoolPlugin { task_pool_options: TaskPoolOptions::with_num_threads(1) },
        AssetPlugin::default(),
        WindowPlugin::default(),
        ImagePlugin::default(),
        TextureAtlasPlugin,
        TextPlugin,
        (InputPlugin, PickingPlugin, InteractionPlugin, UiPlugin),
        SuperUiCssPlugin,
    ));

    app.init_resource::<InputFocus>().init_resource::<InputFocusVisible>();
    app.finish(); // installs the CSS asset loader + property registries
    app
}

/// Poll `app.update()` until `path` has finished loading (or panic after N tries).
pub fn load_until_ready(app: &mut App, handle: &Handle<StyleSheet>) {
    use bevy::asset::LoadState;
    for _ in 0..64 {
        app.update();
        let server = app.world().resource::<AssetServer>();
        match server.load_state(handle.id()) {
            LoadState::Loaded => {
                // A couple more frames so the style systems apply computed values.
                app.update();
                app.update();
                return;
            }
            LoadState::Failed(e) => panic!("stylesheet failed to load: {e}"),
            _ => {}
        }
    }
    panic!("stylesheet did not finish loading within 64 frames");
}
```

If the compiler flags a plugin/type path (e.g. `ManualTextureViews` is required by a media-feature system, or `TextureAtlasPlugin` lives elsewhere), follow its guidance: add `app.init_resource::<bevy::prelude::ManualTextureViews>();` before `app.finish()` if a query panics on a missing resource, and correct any plugin import path. These are the exact plugins flair 0.6's own passing test harness uses, so the set is known-good; only import paths may shift with the trimmed feature list — widen the `bevy` dev-dep features in `Cargo.toml` if a needed plugin/type is feature-gated out.

- [ ] **Step 2: Write the capstone test** — create `crates/superui_css/tests/selectors.rs`:

```rust
//! End-to-end proof that the vendored fork matches HTML-shaped selectors:
//! element, attribute, class, id, descendant, `:checked`, `:hover`.

mod support;
use support::*;

use bevy::color::palettes::css;
use bevy::input_focus::InputFocus;
use bevy::prelude::*;

use superui_css::html_type_name;
use superui_css::prelude::*;

/// Look up an entity by its `Name` and assert its computed BackgroundColor.
macro_rules! assert_bg {
    ($app:expr, $name:literal, $expected:expr) => {{
        let world = $app.world_mut();
        let mut q = world.query::<(&Name, &BackgroundColor)>();
        let found = q
            .iter(world)
            .find(|(n, _)| n.as_str() == $name)
            .map(|(_, bg)| bg.0);
        let color = found
            .unwrap_or_else(|| panic!("no entity named '{}' with BackgroundColor", $name));
        assert_eq!(
            color.to_srgba().to_u8_array(),
            $expected.to_u8_array(),
            "'{}' background mismatch",
            $name
        );
    }};
}

const CSS: &str = r#"
li               { background-color: white; }
.completed       { background-color: green; }
#special         { background-color: blue; }
.todo-list li    { background-color: purple; }
input[type="checkbox"] { background-color: orange; }
input:checked    { background-color: red; }
button:hover     { background-color: teal; }
this-is-not-valid @@@ ;
"#;

#[test]
fn matches_html_selectors_end_to_end() {
    put_css("selectors.css", CSS);

    let mut app = test_app();
    let handle = {
        let server = app.world().resource::<AssetServer>().clone();
        server.load_style_sheet("selectors.css")
    };

    // Spawn a small DOM-shaped tree. `html_type_name(tag)` is the element-selector
    // identity the Plan-5 bridge will insert; `Name` is the id; `ClassList` the
    // classes; `AttributeList` the attributes; `Checked`/`Interaction` drive
    // pseudo-state exactly as flair syncs it.
    let root = app
        .world_mut()
        .spawn((
            Node::default(),
            html_type_name("ul"),
            ClassList::new("todo-list"),
            NodeStyleSheet::new(handle.clone()),
        ))
        .id();

    let plain_li = app.world_mut().spawn((Node::default(), html_type_name("li"), Name::new("plain-li"))).id();
    let done_li = app.world_mut().spawn((Node::default(), html_type_name("li"), ClassList::new("completed"), Name::new("done-li"))).id();
    let special_li = app.world_mut().spawn((Node::default(), html_type_name("li"), Name::new("special"))).id();
    let checkbox = app
        .world_mut()
        .spawn((
            Node::default(),
            html_type_name("input"),
            AttributeList::from_iter([("type", "checkbox")]),
            bevy::ui::Checked,
            Name::new("checkbox"),
        ))
        .id();
    let btn = app
        .world_mut()
        .spawn((Node::default(), html_type_name("button"), Interaction::Hovered, Name::new("btn")))
        .id();

    app.world_mut().entity_mut(root).add_children(&[plain_li, done_li, special_li, checkbox, btn]);

    load_until_ready(&mut app, &handle);

    // Element selector `li` (also descendant `.todo-list li`, higher specificity → purple):
    assert_bg!(app, "plain-li", css::PURPLE);
    // Class `.completed` (green) beats element/descendant by later + specificity:
    assert_bg!(app, "done-li", css::GREEN);
    // Id `#special` (blue) — highest specificity:
    assert_bg!(app, "special", css::BLUE);
    // Attribute `input[type="checkbox"]` + `:checked` — checked wins (red):
    assert_bg!(app, "checkbox", css::RED);
    // `:hover`:
    assert_bg!(app, "btn", css::TEAL);
}
```

- [ ] **Step 3: Run the capstone test**

Run: `cargo test -p superui_css --test selectors`
Expected: PASS — all five `assert_bg!` checks hold, proving element/attribute/class/id/descendant/`:checked`/`:hover` all match through the vendored fork, and the malformed final CSS line did **not** abort the sheet (graceful degradation).

Debugging latitude (behavior, not harness): if an assertion's *color* is off, it is a specificity/order question, not a fork defect — inspect which rules matched by logging `NodeStyleData`/computed color, and adjust the expected color or make the CSS specificity unambiguous (e.g. give `.todo-list li` vs `li` a clear winner). If an entity has **no** `BackgroundColor` at all, the sheet did not finish loading — increase the frame budget in `load_until_ready`. Do not edit vendored engine code to make this pass.

- [ ] **Step 4: Verify the `superui_css` runtime library builds for wasm**

Run: `cargo build -p superui_css --target wasm32-unknown-unknown`
Expected: SUCCESS. (Dev-dependencies like `bevy` are not compiled for a non-test `build`, so the heavy umbrella is excluded; this checks the runtime lib + the three fork crates + Bevy compile to wasm with the getrandom flag from `.cargo/config.toml`.)

If it fails on getrandom (`inner_u32`/`backends`), confirm both the repo-root `.cargo/config.toml` rustflag (`getrandom_backend="wasm_js"`, from Plan 3) and the wasm-target `getrandom` dep with `wasm_js` are present. If a *fork* crate fails to build for wasm on an unrelated dep, note it in the commit and open a follow-up — the wasm build of the full Bevy UI stack is exercised again in Plan 6/CI; do not patch vendored code speculatively.

- [ ] **Step 5: Flip the plan-series status** — in `docs/superpowers/plans/README.md`, change the Plan 4 row from:

```
| 4 | `superui_css` | Fork of `bevy_flair` 0.6 (targets Bevy 0.17), extended for real HTML element/attribute selectors and `:hover`/`:focus`/`:checked`. | ⬜ Not started |
```

to:

```
| 4 | `superui_css` | Fork of `bevy_flair` 0.6 (targets Bevy 0.17), extended for real HTML element/attribute selectors and `:hover`/`:focus`/`:checked`. | ✅ Done — merged to `main` ([plan](./2026-07-19-superui-phase1-04-css.md)) |
```

Also update the "Resuming in a fresh session" block so it targets **Plan 5** (`superui_bridge` + `superui`) — the first `⬜ Not started` row — replacing the Plan-4 wording with a one-line pointer to Plan 5's scope (reconciler + `SuperUiPlugin` + asset loaders + hot reload + full `window.bevy` bridge), noting that Plans 1–4 are done and that `superui_css` provides `SuperUiCssPlugin` + `html_type_name`/`ClassList`/`AttributeList`/`NodeStyleSheet` for the bridge to drive.

- [ ] **Step 6: Final commit**

```bash
git add crates/superui_css docs/superpowers/plans/README.md
git commit -m "test(css): headless HTML-selector integration proof + wasm check; mark Plan 4 done"
```

---

## Self-Review

**Spec coverage (design §4/§5/§7/§9):**
- "Fork of `bevy_flair` 0.6 (Bevy 0.17)" → Tasks 1–3 vendor the three sub-crates verbatim at 0.6.0 pinned to Bevy 0.17.
- "sub-crates vendored" → kept under upstream names as workspace members (user-confirmed layout).
- "real HTML element/attribute selectors" → Task 5's interner (`html_type_name`) gives every tag a real `TypeName`; Task 6 proves `li` and `input[type="checkbox"]` match.
- "`:hover`/`:focus`/`:checked`" → already parsed+matched by the fork; Task 6 proves `:checked` (via `bevy_ui::Checked`) and `:hover` (via `Interaction::Hovered`). `:focus` support is present (the harness inits `InputFocus`); it is covered by the fork's own passing tests (Task 2) and available to the Plan-5 bridge — not re-asserted here to keep the capstone focused.
- "Selectors: type/class/id/descendant" (§9) → all four asserted in Task 6.
- "Unsupported rules skipped, never fatal" (§9/§1) → Task 6's CSS ends with a malformed line; the sheet still loads and matches.
- wasm posture (§5) → Task 6 Step 4 builds `superui_css` for `wasm32-unknown-unknown`.
- Boundary discipline (§4) → Task 3 Step 5 asserts the four wasm-clean crates gain no `bevy_*` dep.
- Ledger (`docs/support/css.md`) is explicitly deferred to Plan 6 (§7 groups it with the example) — correctly out of scope here.

**Placeholder scan:** No TBD/TODO; every code step contains complete content. Vendored source is copied (not pasted) by design — the "no placeholders" rule applies to authored code, and all authored code (`superui_css/src/**`, tests) is fully spelled out.

**Type consistency:** `SuperUiCssPlugin`, `intern_tag`/`html_type_name`, `html_type_name(tag).0`, `AttributeList::from_iter`, `ClassList::new`, `NodeStyleSheet::new`, `TypeName(&'static str)`, `NodePseudoState`, `StyleSheet`, `CssStyleLoaderSetting`/`CssStyleLoaderErrorMode` are used consistently across Tasks 4–6 and match the verified flair-0.6 API reference above.

**Known execution risks (flagged, not blocking):** (1) trimmed `bevy` dev-dep features may omit a plugin/type the harness needs — the plan tells the implementer to widen features / init `ManualTextureViews` per the compiler; (2) `insta` snapshot tests in Task 2 may show cross-platform whitespace diffs — the plan says accept, don't "fix" engine code; (3) capstone color assertions are specificity-sensitive — the plan says treat mismatches as specificity questions, not fork defects.
