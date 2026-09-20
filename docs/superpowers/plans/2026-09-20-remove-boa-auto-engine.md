# Remove Boa + Auto-Select Engine by Target Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Drop the Boa JS engine entirely and make `superui` pick its engine automatically per target — the browser engine (`engine-web`) on wasm, V8/deno_core (`engine-v8`) on native — so downstream users just depend on `superui` with no engine flags and no dual-dependency trick.

**Architecture:** The op-wire foundation (`opwire/` codec+idmap+applier, `js/dom.js` shadow DOM, `runtime.js`/`render.js`) is engine-agnostic and stays. Only the Boa *adapter* + vendored Boa fork crates are removed. Boa was also the in-process test harness for the JS-behavior tests and the pinned engine for `superui_test_engine`, so those move to V8 (native) first. Auto-selection is done with `[target.'cfg(...)'.dependencies]` inside `superui`'s own manifest plus re-exporting the `superui_bridge` types users need, so consumers never set an engine feature.

**Tech Stack:** Rust, Bevy 0.17, `deno_core` (native V8), `wasm-bindgen`/`js-sys` (web), the existing op-wire codec, `oxc` transpile (unchanged).

**Spec:** none written — this plan is self-contained. Background: `docs/superpowers/specs/2026-09-19-js-engine-swap-design.md` (the swap this builds on) and `docs/superpowers/bench/raw/*.json` (captured benchmark numbers: `*-boa-baseline`, `*-boa-shadow`, `*-v8[-release]`).

## Global Constraints

- After this plan, the workspace contains **no Boa**: no `boa_engine`/`boa_gc` deps, no `crates/superui_boa_engine`/`crates/superui_boa_parser`, no `engine-boa` feature anywhere. Verify with `cargo tree -i boa_engine` → "did not match any packages" on both native and `--target wasm32-unknown-unknown`, and `grep -rn "boa_engine\|BoaEngine\|engine-boa" crates/ examples/` → nothing (outside this plan/spec docs).
- Exactly ONE engine active per build. `superui_js` keeps `compile_error!` guards: none-selected, both-selected, `engine-v8` on wasm, `engine-web` on native.
- Native (`cfg(not(target_arch="wasm32"))`) → `engine-v8`. Wasm (`cfg(target_arch="wasm32")`) → `engine-web`. This is fixed per target; there is no Boa fallback.
- `WINDOWS: cargo runs strictly serially` — two concurrent cargo processes corrupt the incremental-compile dir (`os error 5`). Never run two cargo commands at once.
- The op-wire byte format, `js/dom.js`, `runtime.js`, `render.js`, and `crates/superui_js/src/opwire/*` are NOT changed by this plan (engine-agnostic; leave them).
- First `engine-v8` build compiles/downloads V8 via deno_core (~10 min, network). CI that runs `cargo test` will pay this once.
- Commit style: summary says what; body says why, not a diff restatement.
- Do NOT use git worktrees (huge shared `target/`).

## Review Focus

- **A wasm build silently pulling deno_core or a native build pulling wasm-bindgen** — the target-conditional deps must be exclusive; pinned by the `cargo tree` checks in Tasks 3 & 4.
- **`superui_test_engine` running with no engine** — it was Boa-pinned; after removal it must resolve to `engine-v8` on native and its suite must actually pass, not just compile (Task 2).
- **A JS-behavior regression hidden by losing the Boa test harness** — the shadow_dom/dispatch/bootstrap tests must assert the *same* ops/behavior under V8, not be weakened to pass (Task 1).
- **`state.rs`/`HostState` deletion breaking V8/Web** — confirm those types were Boa-only before deleting (Task 3).
- **An example still carrying a stale `engine-boa`/`default-features=false` line** after auto-selection lands, tripping the one-engine guard (Task 5).

---

### Task 1: Move superui_js's JS-behavior tests + default onto V8

**Files:**
- Modify: `crates/superui_js/Cargo.toml` (default feature)
- Modify: `crates/superui_js/tests/shadow_dom.rs`, `crates/superui_js/tests/dispatch.rs`, `crates/superui_js/tests/bootstrap.rs` (or wherever `bootstrap_js` is tested)
- Delete: `crates/superui_js/tests/boa_engine.rs` (superseded by `tests/v8_engine.rs`)

**Interfaces:**
- Consumes: `superui_js::V8Engine` (exists, `engine_v8.rs`) implementing `JsEngine`; `superui_js::opwire::{OpBatch, OpApplier}`.
- Produces: all superui_js JS-behavior tests run under V8; `cargo test -p superui_js` (default) is Boa-free.

- [ ] **Step 1: Point superui_js's default at V8.** In `crates/superui_js/Cargo.toml`, change `default = ["engine-boa"]` to `default = ["engine-v8"]`. (Native test target → V8. superui overrides per-target later; standalone wasm builds of superui_js are not a supported use.)

- [ ] **Step 2: Find the Boa harness usage.** Run `grep -rn "boa_engine\|BoaEngine\|Context::" crates/superui_js/tests crates/superui_js/src`. Every `boa_engine::Context`/`BoaEngine` in the test files is a JS-eval harness that must become a `V8Engine`.

- [ ] **Step 3: Port each JS-eval test to V8.** In `shadow_dom.rs`, `dispatch.rs`, and the bootstrap test: replace the bare `boa_engine::Context` setup with `let mut engine = superui_js::V8Engine::new(dom.clone());` and drive the same author JS through `engine.eval(...)`, read the batch through `engine.flush_ops()`, dispatch through `engine.dispatch_event(...)`, exactly as `tests/v8_engine.rs` already does. Keep every existing assertion (same ops, same ids, same strings, same dispatch/prevented results, same `getElementById`/tree checks) — do NOT weaken them. Delete `tests/boa_engine.rs` (its create→flush→dispatch→outbox coverage already exists in `tests/v8_engine.rs`).

- [ ] **Step 4: Run the suites under V8.** `cargo test -p superui_js` (default is now engine-v8; first run builds V8). Expected: codec/idmap/applier (pure Rust) + shadow_dom + dispatch + bootstrap + v8_engine all PASS, pristine. If the transient Windows `os error 5` appears, re-run once.

- [ ] **Step 5: Commit.**
```bash
git add crates/superui_js/Cargo.toml crates/superui_js/tests
git commit -m "test(superui_js): run JS-behavior tests on V8; default to engine-v8"
```

---

### Task 2: Repoint the remaining crates' default to V8 (keep them green off Boa)

**Files:**
- Modify: `crates/superui_bridge/Cargo.toml`, `crates/supersolid_runtime/Cargo.toml` (default feature)
- Modify: `crates/superui_test_engine/Cargo.toml` if it names an engine (it is Boa-pinned today)

**Interfaces:**
- Consumes: superui_js default engine-v8 (Task 1).
- Produces: `superui_bridge`, `supersolid_runtime`, `superui_test_engine` build+test on V8 (native), Boa-free.

- [ ] **Step 1: Default these to engine-v8.** In `crates/superui_bridge/Cargo.toml` and `crates/supersolid_runtime/Cargo.toml`, change `default = ["engine-boa"]` to `default = ["engine-v8"]`.

- [ ] **Step 2: Check the test engine's engine wiring.** Run `grep -nE "engine-|boa|default-features" crates/superui_test_engine/Cargo.toml`. `superui_test_engine` was migrated off `context_mut`/Boa types already (commit `46fc0b4`) and depends on `superui_bridge`. Ensure it does NOT pin `engine-boa`; if it pulls `superui_bridge` with `default-features = false` + an engine, set that engine to `engine-v8`; otherwise it inherits `superui_bridge`'s new default (engine-v8). It must remain a single-engine, native crate.

- [ ] **Step 3: Verify the suites pass on V8.** Run (serially): `cargo test -p supersolid_runtime`, then `cargo test -p superui_bridge`, then `cargo test -p superui_test_engine`. Expected: all PASS, pristine. `superui_test_engine`'s DOM-assertion specs must actually pass (they read the render-mirror `Dom` which the V8 op-wire populates), not just compile.

- [ ] **Step 4: Commit.**
```bash
git add crates/superui_bridge/Cargo.toml crates/supersolid_runtime/Cargo.toml crates/superui_test_engine/Cargo.toml
git commit -m "test: default superui_bridge/supersolid_runtime/test-engine to engine-v8"
```

---

### Task 3: Delete the Boa adapter, the vendored Boa fork, and the engine-boa feature

**Files:**
- Delete: `crates/superui_boa_engine/` and `crates/superui_boa_parser/` (whole crates)
- Delete: `crates/superui_js/src/engine.rs` (BoaEngine) and `crates/superui_js/src/state.rs` (Boa `HostState`/`Timer`) — after confirming Boa-only
- Modify: root `Cargo.toml` (`[workspace] members`, `[workspace.dependencies]`), `crates/superui_js/Cargo.toml`, `crates/superui_js/src/lib.rs`
- Modify: `crates/superui_bridge/Cargo.toml`, `crates/supersolid_runtime/Cargo.toml` (drop `engine-boa` feature entries)

**Interfaces:**
- Consumes: V8 default from Tasks 1–2.
- Produces: workspace has no Boa; `engine-v8`/`engine-web` are the only engines; `superui_js::new_engine` has no boa arm.

- [ ] **Step 1: Confirm state.rs is Boa-only.** Run `grep -rn "state::\|HostState\|use crate::state" crates/superui_js/src`. Expect references only from `engine.rs` (BoaEngine) — `engine_v8.rs` uses its own `V8HostState`, `engine_web.rs` its own scope. If anything outside `engine.rs` uses `state.rs`, STOP and report (do not delete blindly).

- [ ] **Step 2: Remove the vendored fork from the workspace.** In root `Cargo.toml`: delete `crates/superui_boa_engine` and `crates/superui_boa_parser` from `[workspace] members`; delete the `boa_engine = { package = "superui_boa_engine", ... }` and `boa_gc = ...` lines from `[workspace.dependencies]`. Then `rm -rf crates/superui_boa_engine crates/superui_boa_parser`.

- [ ] **Step 3: Strip Boa from superui_js.** In `crates/superui_js/Cargo.toml`: remove `boa_engine`/`boa_gc` deps, the `engine-boa` feature, and the Boa-only wasm shims (`web-time`, and the `getrandom`/`wasm_js` line if it exists solely for Boa — the browser engine doesn't need them). Change `default = ["engine-v8"]` (keep). Delete `crates/superui_js/src/engine.rs` and `crates/superui_js/src/state.rs`; remove their `mod engine; mod state;` and any `pub use engine::BoaEngine` / `pub use state::...` from `src/lib.rs`.

- [ ] **Step 3b: Fix lib.rs engine selection.** In `crates/superui_js/src/lib.rs`: remove the `#[cfg(feature="engine-boa")]` arm of `new_engine` and any `BoaEngine` reference; keep the guards but reduce to two engines:
```rust
#[cfg(all(feature = "engine-v8", feature = "engine-web"))]
compile_error!("superui_js: enable exactly one engine feature (engine-v8 XOR engine-web)");
#[cfg(not(any(feature = "engine-v8", feature = "engine-web")))]
compile_error!("superui_js: select an engine feature: engine-v8 (native) or engine-web (wasm)");
#[cfg(all(feature = "engine-v8", target_arch = "wasm32"))]
compile_error!("superui_js: engine-v8 is native-only");
#[cfg(all(feature = "engine-web", not(target_arch = "wasm32")))]
compile_error!("superui_js: engine-web is wasm-only");
```

- [ ] **Step 4: Drop engine-boa from the other crates.** In `crates/superui_bridge/Cargo.toml` and `crates/supersolid_runtime/Cargo.toml`, delete the `engine-boa = [...]` feature lines (keep `engine-v8`/`engine-web`). Grep each crate's `src` for `feature = "engine-boa"` cfg and remove those blocks (there should be few/none; the `#[cfg(all(test, feature="engine-boa"))]` gates in `supersolid_runtime/src/lib.rs` tests become `#[cfg(test)]` since Boa is gone and engine-v8 is default).

- [ ] **Step 5: Verify no Boa remains + everything builds.**
```bash
grep -rn "boa_engine\|BoaEngine\|engine-boa\|boa_gc" crates/ examples/    # expect nothing (docs excluded)
cargo build --workspace                                                   # native → engine-v8
cargo build -p superui --target wasm32-unknown-unknown --no-default-features --features engine-web
cargo tree -i boa_engine                                                  # "did not match any packages"
cargo test -p superui_js -p superui_bridge -p supersolid_runtime -p superui_test_engine
```
Expected: all green; no boa in the tree. Serial cargo only.

- [ ] **Step 6: Commit.**
```bash
git add -A
git commit -m "refactor: remove the Boa engine and vendored boa fork entirely"
```

---

### Task 4: Auto-select the engine by target inside superui + re-export bridge types

**Files:**
- Modify: `crates/superui/Cargo.toml`
- Modify: `crates/superui/src/lib.rs`

**Interfaces:**
- Consumes: `superui_js`/`superui_bridge` engine features `engine-v8`/`engine-web` (Task 3).
- Produces: depending on `superui` alone yields `engine-web` on wasm and `engine-v8` on native, automatically; `superui` re-exports `UiRuntime`, `DomNode`, `emit_bevy_inbox_system`, `reconcile_system` (the `superui_bridge` items examples import directly today).

- [ ] **Step 1: Replace superui's engine features with target-conditional deps.** In `crates/superui/Cargo.toml`: remove `superui_js`/`superui_bridge` from `[dependencies]`, remove the `default = ["engine-boa"]` line and the `engine-boa`/`engine-v8`/`engine-web` feature entries, and add:
```toml
[target.'cfg(target_arch = "wasm32")'.dependencies]
superui_js     = { path = "../superui_js",     version = "0.3.3", default-features = false, features = ["engine-web"] }
superui_bridge = { path = "../superui_bridge", version = "0.3.3", default-features = false, features = ["engine-web"] }

[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
superui_js     = { path = "../superui_js",     version = "0.3.3", default-features = false, features = ["engine-v8"] }
superui_bridge = { path = "../superui_bridge", version = "0.3.3", default-features = false, features = ["engine-v8"] }
```
Keep `hmr` and any non-engine features. Leave `superui_css`, `supersolid` (build-dep), and other non-engine deps where they are.

- [ ] **Step 2: Re-export the bridge types users need.** In `crates/superui/src/lib.rs`, add near the other `pub use`:
```rust
pub use superui_bridge::{UiRuntime, DomNode, emit_bevy_inbox_system, reconcile_system};
```
and add them to `pub mod prelude { ... }` too. (Grep `examples/*/src` + note from the swap work: these four are what forces the direct `superui_bridge` dep.)

- [ ] **Step 3: Verify auto-selection.**
```bash
cargo build -p superui                                              # native → engine-v8
cargo tree -p superui -i deno_core                                 # present (native)
cargo build -p superui --target wasm32-unknown-unknown             # wasm → engine-web
cargo tree -p superui --target wasm32-unknown-unknown -i deno_core # "did not match" (not in wasm)
cargo tree -p superui --target wasm32-unknown-unknown -i wasm-bindgen  # present (wasm)
```
Expected: native pulls deno_core (not wasm-bindgen); wasm pulls wasm-bindgen (not deno_core); no engine feature was passed.

- [ ] **Step 4: Commit.**
```bash
git add crates/superui/Cargo.toml crates/superui/src/lib.rs
git commit -m "feat(superui): auto-select engine by target (web on wasm, v8 on native) + re-export bridge types"
```

---

### Task 5: Simplify examples + build-demos to rely on auto-selection

**Files:**
- Modify: `examples/{counter,todomvc,todomvc_supersolid,game_menu,citadel,horde,rows}/Cargo.toml`
- Modify: `tools/build-demos.sh`

**Interfaces:**
- Consumes: superui auto-selection + re-exports (Task 4).
- Produces: examples build on both targets with no engine flags; `build-demos.sh` no longer passes an engine feature.

- [ ] **Step 1: Strip engine plumbing from each example.** For each example Cargo.toml: remove the `engine-boa`/`engine-v8`/`engine-web` feature entries and the `engine-boa` entry from `default`; set the `superui` dep back to plain `superui = { path = "../../crates/superui" }` (drop `default-features = false`); and where the example depends on `superui_bridge`/`supersolid_runtime` **only** for `UiRuntime`/`DomNode`/`reconcile_system`/`emit_bevy_inbox_system`, remove that dep and change the `use superui_bridge::X` imports in its `src` to `use superui::X` (now re-exported). Keep `superui_bridge` only if the example uses a type superui does NOT re-export (grep to confirm; extend the re-export list in Task 4 and note it if so).

- [ ] **Step 2: Drop the engine flag from build-demos.** In `tools/build-demos.sh`: remove the `ENGINE`/`--features "$ENGINE"` machinery; the wasm target auto-selects `engine-web`. Keep `--no-default-features` ONLY where it was needed to drop the WebGL2-incompatible FPS overlay (`bevy_dev_tools`) — i.e. restore per-slug `--no-default-features` for `citadel`/`horde` (and add for any demo whose `default` still enables `debug-ui`), since that's an overlay/GPU concern, not an engine one. The cargo line becomes `cargo build -p "$slug" --release --target "$TARGET" $args`.

- [ ] **Step 3: Verify a supersolid example on both targets.**
```bash
cargo run -q -p game_menu --bin game_menu 2>&1 | head -3   # or just: cargo build -p game_menu   (native → v8)
cargo build -p game_menu --target wasm32-unknown-unknown   # wasm → engine-web, no flags
bash tools/build-demos.sh game_menu                         # release wasm demo builds
```
Expected: native builds pull v8, wasm builds pull web, no engine feature anywhere. (Full browser render was already validated for game_menu in the swap work.)

- [ ] **Step 4: Commit.**
```bash
git add examples tools/build-demos.sh
git commit -m "chore(examples): rely on superui auto-engine-selection; drop engine flags + direct bridge deps"
```

---

### Task 6: Update documentation (benchmarks + Boa removal)

**Files:**
- Modify: `docs/BENCHMARKS.md`
- Modify: `README.md` and any doc/crate-doc that names Boa as the engine (grep first)

**Interfaces:**
- Consumes: benchmark numbers in `docs/superpowers/bench/raw/*.json` (native original-Boa baseline + Boa-shadow + V8) and, if re-run, fresh V8 release numbers.
- Produces: `docs/BENCHMARKS.md` presents the V8 engine numbers and the swap's before/after; no doc claims Boa is the engine.

- [ ] **Step 1: Capture clean V8 release numbers for the doc (all three benches).** Run serially:
```bash
cargo run -q -p horde   --bin horde-bench   --release --features bench -- --backend supersolid --preset stress --sweep 60,200,400 --frames 120 --warmup 30 --seed 1 --format json
cargo run -q -p citadel --bin citadel-bench --release --features bench -- --backend supersolid --sweep 60,120,240 --frames 120 --warmup 30 --seed 1 --format json
cargo run -q -p rows    --bin rows-bench    --release --features bench -- --backend supersolid --rows 1000 --reps 20 --warmup 3 --format json
```
(These now run on V8 via auto-selection. Record `ui_ms` per cap for horde/citadel and per-op `p50_ms`/`mean_ms` for rows.)

- [ ] **Step 2: Read the existing doc + the historical baseline.** Read `docs/BENCHMARKS.md` (current content) and the session captures `docs/superpowers/bench/raw/{horde,citadel,rows}-boa-baseline*.json` (the pre-swap Boa numbers, debug) for the "before" comparison.

- [ ] **Step 3: Write the benchmark doc.** Update `docs/BENCHMARKS.md` to state: the engine setup (native V8 via deno_core, web = browser engine, no Boa); the machine/method (seed, frames, release); a table of the V8 release numbers from Step 1; and a "vs the pre-swap Boa engine" note using the session's measurements (label which are debug vs release; original-Boa is historical since Boa is removed). Use the reference-docs / technical-writing skill conventions: terse, present-tense, a capability/results table, no mechanism narration. Do NOT claim a bevy_react comparison unless separately measured.

- [ ] **Step 4: Purge Boa from prose docs.** Run `grep -rn "Boa\|boa" README.md docs/ crates/*/src/lib.rs --include=*.md --include=*.rs | grep -vi "docs/superpowers/plans\|docs/superpowers/specs"`. Update user-facing docs/crate docs that describe Boa as the engine to say V8 (native) / browser (web); leave historical spec/plan/ledger docs untouched.

- [ ] **Step 5: Commit.**
```bash
git add docs/BENCHMARKS.md README.md docs
git commit -m "docs: engine is V8 (native) / browser (web); benchmark results + Boa removed from docs"
```

---

## Self-Review

- **Coverage:** remove-Boa = Tasks 1–3 (tests/defaults off Boa, then delete adapter+fork+feature); auto-select = Task 4 (target deps + re-exports) + Task 5 (examples/demos consume it); docs = Task 6 (benchmarks + prose). All three asks covered.
- **Ordering:** tests/defaults move to V8 (1–2) BEFORE deleting Boa (3) so nothing is stranded; auto-select (4) before examples simplify (5); docs last (6) so the V8 release numbers reflect the final engine.
- **Placeholder scan:** exact files, TOML, cfg, and commands given; where a repo fact must be checked (state.rs Boa-only, which types to re-export, test-engine engine pin) the step says "grep to confirm / STOP and report" — an instruction, not a TODO.
- **Type/name consistency:** engine features `engine-v8`/`engine-web` used identically across tasks; `new_engine` guard set consistent; re-export list (`UiRuntime`, `DomNode`, `emit_bevy_inbox_system`, `reconcile_system`) consistent between Task 4 and Task 5.
- **Review Focus:** wasm/native dep exclusivity → Tasks 3/4 `cargo tree` checks; test-engine-on-V8 → Task 2 Step 3; JS-behavior parity under V8 → Task 1 Step 3 (keep assertions); state.rs Boa-only → Task 3 Step 1; stale example engine lines → Task 5 Step 1 + the workspace build.
- **Known trade-off (documented, not a gap):** auto-selection is per-target-fixed; there is no Boa fallback and no simple feature override (enabling a second engine trips the guard). This is intended per the goal ("remove Boa").
