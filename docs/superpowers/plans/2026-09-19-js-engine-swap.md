# JS Engine Swap Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make superui's JS engine selectable at compile time (Boa default, V8/deno_core on native, browser engine on web) behind one small `JsEngine` trait, by moving the DOM + reactive layer onto a declarative op-wire with a JS-side shadow DOM.

**Architecture:** JS owns a shadow DOM and records mutations; Rust decodes a batched op buffer once per frame into the existing `superui_dom` render mirror that flair/taffy lays out. Reads and W3C event dispatch happen in JS; the only JS→Rust read is `__ss_measure`. Each engine implementation only evals the JS bundle and services `dispatch_event`/`run_timers`/`flush_ops` plus a handful of host imports.

**Tech Stack:** Rust, Bevy 0.17, `superui_dom` arena, Boa (forked 0.21.1), `deno_core` (native V8), `wasm-bindgen`/`js-sys`/`web-sys` (web), `oxc` (build-time TSX transpile, unchanged).

**Spec:** `docs/superpowers/specs/2026-09-19-js-engine-swap-design.md`

## Global Constraints

- Engine selection is **compile-time** via Cargo features: `engine-boa` (default), `engine-v8` (native only), `engine-web` (wasm only). Exactly one active per build.
- **All engine dependencies are `optional = true`**; only the selected engine's deps compile. Boa deps move behind `engine-boa`.
- `compile_error!` on invalid combos: `engine-v8` on wasm, `engine-web` on native, two engines at once.
- `UiRuntime` holds `Box<dyn JsEngine>`. No engine type is named outside its own gated module and `superui_js`.
- **Same-thread only.** No engine runs on its own thread; all are pumped per Bevy frame. `dispatch_event` and `__ss_measure` are synchronous.
- One DOM implementation for all engines (the JS `dom.js` shadow DOM). No per-engine DOM bindings.
- `superui_test_engine` stays pinned to `engine-boa`, unchanged.
- Commit style (project CLAUDE.md): summary says what; body says why/how, not a restatement of the diff.
- Do NOT use git worktrees (huge shared `target/`).
- Baseline benchmarks MUST be captured before any code change retires the old path.
- **Windows: cargo runs strictly serially.** Two concurrent cargo processes corrupt Boa's incremental-compile dir (`did not finalize incremental compilation session directory … Access is denied (os error 5)`), which surfaces as a spurious `boa_gc` "multiple versions of crate" trait mismatch. Never run two cargo invocations at once (this bounds subagent parallelism). Writing source files while a cargo build runs is safe; running cargo is not.
- Root accessor on `superui_dom::Dom` is `document()` (not `root()`); structural mutations return `Result<(), DomError>` and never panic; `set_text_content`/`remove_attribute`/`get_element_by_id`/`text_content`/`parent`/`children` already exist and must be reused.
- The JS shadow DOM implements rich DOM methods (e.g. `replaceChild`) in JS but emits only the primitive op set; control-flow anchors are empty text nodes (`createTextNode("")`), so no comment-node op is needed. `runtime.js`/`render.js` stay unchanged — the shadow DOM must match the exact API surface they use.

## Review Focus

- **Node-id reuse after removal:** a `jsId` whose node was removed then a stale op references it — applier must ignore/error deterministically, not corrupt a reused arena slot.
- **Event during mutation:** a listener that mutates the DOM mid-dispatch — queued ops must flush after dispatch completes, dispatch must not observe half-applied Rust state (it reads only the JS shadow tree).
- **`__ss_measure` before first layout:** author JS calling `getBoundingClientRect` on a node not yet laid out — must return a zero rect, not panic or read an unmapped id.
- **Op batch ordering:** `InsertBefore` referencing a `ref` child already removed earlier in the same batch — decode/apply must preserve author order and treat a missing `ref` as append.
- **Empty/oversized batch:** a frame with zero ops (no work) and a frame with a very large batch (bulk `For` insert) — flush must be a cheap no-op for empty and must not overflow fixed-width operands for large.

---

## Task 0: Capture current-Boa baseline benchmarks

**Files:**
- Create: `docs/superpowers/bench/2026-09-19-baseline-boa.md` (results)
- Create: `docs/superpowers/bench/raw/*.json` (machine output)

**Interfaces:**
- Produces: committed baseline numbers for horde, citadel, rows on the current (pre-change) Boa path, referenced by the final comparison (Task F1).

- [ ] **Step 1: Build all three bench bins**

```bash
cargo build -p horde   --bin horde-bench   --features bench
cargo build -p citadel --bin citadel-bench --features bench
cargo build -p rows    --bin rows-bench    --features bench
```

- [ ] **Step 2: Run the supersolid (Boa) backend for each, JSON + fixed seed**

```bash
mkdir -p docs/superpowers/bench/raw
cargo run -q -p horde   --bin horde-bench   --features bench -- --backend supersolid --preset stress --sweep 60,200,400 --frames 300 --warmup 60 --seed 1 --format json > docs/superpowers/bench/raw/horde-boa-baseline.json
cargo run -q -p citadel --bin citadel-bench --features bench -- --backend supersolid --sweep 60,120,240 --frames 300 --warmup 60 --seed 1 --format json > docs/superpowers/bench/raw/citadel-boa-baseline.json
cargo run -q -p rows    --bin rows-bench    --features bench -- --backend supersolid --rows 1000 --reps 20 --warmup 3 --format json > docs/superpowers/bench/raw/rows-boa-baseline.json
```

- [ ] **Step 3: Capture reference backends (context for the deltas)**

```bash
cargo run -q -p horde   --bin horde-bench   --features bench -- --backend native --preset stress --sweep 60,200,400 --frames 300 --warmup 60 --seed 1 --format json > docs/superpowers/bench/raw/horde-native-ref.json
cargo run -q -p rows    --bin rows-bench    --features bench -- --backend vanilla --rows 1000 --reps 20 --warmup 3 --format json > docs/superpowers/bench/raw/rows-vanilla-ref.json
```

- [ ] **Step 4: Summarize into the baseline doc**

Write `docs/superpowers/bench/2026-09-19-baseline-boa.md` with a table per bench: workload (cap/rows), mean frame/op time, and any reported reconcile/marshal split. Record host CPU/OS and `rustc`/commit hash (`git rev-parse HEAD`).

- [ ] **Step 5: Do NOT commit (per user).** Bench outputs stay uncommitted while iterating; `docs/superpowers/bench/.gitignore` ignores everything there except itself. Numbers are preserved in the SDD ledger. Revisit committing at the final comparison (Task 15) if the user wants the results in history.

---

## Task 1: Op-wire value types and flat-buffer codec

**Files:**
- Create: `crates/superui_js/src/opwire/mod.rs`
- Create: `crates/superui_js/src/opwire/codec.rs`
- Test: inline `#[cfg(test)]` in `codec.rs`
- Modify: `crates/superui_js/src/lib.rs` (add `pub mod opwire;`)

**Interfaces:**
- Produces:
  - `type JsNodeId = u32;` (root = `1`)
  - `enum Op { CreateElement{id:JsNodeId, tag:StrId}, CreateText{id:JsNodeId, data:StrId}, SetAttribute{id:JsNodeId, name:StrId, value:StrId}, RemoveAttribute{id:JsNodeId, name:StrId}, SetProperty{id:JsNodeId, name:StrId, value:StrId}, SetStyle{id:JsNodeId, prop:StrId, value:StrId}, SetText{id:JsNodeId, data:StrId}, InsertBefore{parent:JsNodeId, node:JsNodeId, reference:JsNodeId /*0=append*/}, RemoveChild{parent:JsNodeId, node:JsNodeId} }`
  - `type StrId = u32;` (index into the batch string pool)
  - `struct OpBatch { ops: Vec<Op>, strings: Vec<String> }`
  - `fn OpBatch::encode(&self) -> Vec<u8>` and `fn OpBatch::decode(bytes: &[u8]) -> Result<OpBatch, CodecError>`
  - `OpBatch::resolve(&self, s: StrId) -> &str`

- [ ] **Step 1: Write failing codec roundtrip test**

```rust
#[test]
fn roundtrip_preserves_ops_and_strings() {
    let mut b = OpBatch::default();
    let div = b.intern("div");
    let cls = b.intern("class");
    let v = b.intern("row");
    b.ops.push(Op::CreateElement { id: 2, tag: div });
    b.ops.push(Op::SetAttribute { id: 2, name: cls, value: v });
    b.ops.push(Op::InsertBefore { parent: 1, node: 2, reference: 0 });
    let bytes = b.encode();
    let back = OpBatch::decode(&bytes).unwrap();
    assert_eq!(back.ops, b.ops);
    assert_eq!(back.resolve(div), "div");
}

#[test]
fn empty_batch_roundtrips() {
    let b = OpBatch::default();
    assert_eq!(OpBatch::decode(&b.encode()).unwrap().ops.len(), 0);
}

#[test]
fn decode_rejects_truncated_buffer() {
    assert!(OpBatch::decode(&[0xff, 0x00]).is_err());
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p superui_js opwire::codec -- --nocapture`
Expected: FAIL (types not defined).

- [ ] **Step 3: Implement `Op`/`OpBatch`/codec**

Layout: `u32` LE header `{op_count, string_count}`, then op records (`u8` opcode + `u32` operands), then the string pool (`u32` len + UTF-8 bytes each). `intern` dedups into `strings`. `decode` validates opcode range and buffer length, returning `CodecError::Truncated`/`CodecError::BadOpcode`. Derive `PartialEq, Debug, Clone` on `Op`.

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p superui_js opwire::codec`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/superui_js/src/opwire
git commit -m "feat(superui_js): op-wire types and flat-buffer codec"
```

---

## Task 2: jsId ↔ NodeId identity map

**Files:**
- Create: `crates/superui_js/src/opwire/idmap.rs`
- Test: inline in `idmap.rs`
- Modify: `crates/superui_js/src/opwire/mod.rs` (`pub mod idmap;`)

**Interfaces:**
- Consumes: `JsNodeId` from Task 1; `superui_dom::NodeId`.
- Produces: `struct IdMap` with `fn bind(&mut self, js: JsNodeId, node: NodeId)`, `fn node(&self, js: JsNodeId) -> Option<NodeId>`, `fn js(&self, node: NodeId) -> Option<JsNodeId>`, `fn unbind(&mut self, js: JsNodeId)`. Root pre-bound: `IdMap::new(root_node: NodeId)` binds `1 -> root_node`.

- [ ] **Step 1: Failing test**

```rust
#[test]
fn binds_both_directions_and_unbinds() {
    let root = NodeId::from_ffi(10);
    let mut m = IdMap::new(root);
    assert_eq!(m.node(1), Some(root));
    let n = NodeId::from_ffi(42);
    m.bind(7, n);
    assert_eq!(m.node(7), Some(n));
    assert_eq!(m.js(n), Some(7));
    m.unbind(7);
    assert_eq!(m.node(7), None);
    assert_eq!(m.js(n), None);
}
```

- [ ] **Step 2: Run, expect FAIL.** `cargo test -p superui_js opwire::idmap`
- [ ] **Step 3: Implement** with a `HashMap<JsNodeId, NodeId>` + reverse `HashMap<u64, JsNodeId>` keyed on `NodeId::to_ffi()`. Confirm `NodeId::from_ffi`/`to_ffi` exist in `superui_dom` (used already in `state.rs`).
- [ ] **Step 4: Run, expect PASS.**
- [ ] **Step 5: Commit** `feat(superui_js): jsId<->NodeId identity map`

---

## Task 3: OpApplier — decode batch into the render-mirror Dom

**Files:**
- Create: `crates/superui_js/src/opwire/applier.rs`
- Test: inline in `applier.rs` (drives a real `superui_dom::Dom`)
- Modify: `crates/superui_js/src/opwire/mod.rs` (`pub mod applier;`)

**Interfaces:**
- Consumes: `OpBatch` (Task 1), `IdMap` (Task 2), `superui_dom::Dom` methods (`create_element`, `create_text`, attribute/text setters, child insert/remove — confirm exact names in `superui_dom` and reuse them; do not add new Dom methods unless a capability is missing).
- Produces: `struct OpApplier { map: IdMap }` with `fn apply(&mut self, dom: &mut Dom, batch: &OpBatch)`. Unknown/stale ids are skipped (see Review Focus), never panic.

- [ ] **Step 1: Failing test** — apply a batch that creates a div with a text child under root, assert the `Dom` tree shape and attribute.

```rust
#[test]
fn applies_create_and_insert_into_dom() {
    let mut dom = Dom::new();
    let root = dom.root(); // confirm accessor name
    let mut applier = OpApplier::new(root);
    let mut b = OpBatch::default();
    let div = b.intern("div"); let id = b.intern("id"); let v = b.intern("main");
    b.ops.push(Op::CreateElement { id: 2, tag: div });
    b.ops.push(Op::SetAttribute { id: 2, name: id, value: v });
    b.ops.push(Op::InsertBefore { parent: 1, node: 2, reference: 0 });
    applier.apply(&mut dom, &b);
    let node = applier.node(2).unwrap();
    assert_eq!(dom.children(root), vec![node]);
    assert_eq!(dom.get_attribute(node, "id").as_deref(), Some("main"));
}

#[test]
fn stale_id_is_skipped_not_panicked() {
    let mut dom = Dom::new();
    let mut applier = OpApplier::new(dom.root());
    let mut b = OpBatch::default();
    b.ops.push(Op::RemoveChild { parent: 1, node: 999 }); // never created
    applier.apply(&mut dom, &b); // must not panic
}
```

- [ ] **Step 2: Run, expect FAIL.** `cargo test -p superui_js opwire::applier`
- [ ] **Step 3: Implement** the match over `Op`, mapping ids via `IdMap`, calling the confirmed `Dom` methods; `reference==0` → append. `RemoveChild` also `unbind`s the id.
- [ ] **Step 4: Run, expect PASS.**
- [ ] **Step 5: Commit** `feat(superui_js): op-batch applier over the render-mirror Dom`

---

## Task 4: JS shadow DOM bundle — nodes, ops, reads

**Files:**
- Create: `crates/superui_js/js/dom.js`
- Test: `crates/superui_js/tests/shadow_dom.rs` (drives `dom.js` through a minimal Boa context and reads back the flushed `OpBatch`)

**Interfaces:**
- Produces (globals the bundle installs): `document.createElement(tag)`, `document.createTextNode(data)`, `document.getElementById(id)`; node methods `appendChild`, `insertBefore`, `removeChild`, `setAttribute`, `getAttribute`, `removeAttribute`, `set textContent`, `childNodes`, `parentNode`, `.style.<prop>=`, `.value`/`.checked`; a global `__ss_flush()` returning the encoded op buffer (as a JS `Uint8Array` / array of bytes matching Task 1's layout); node ids assigned from a counter starting at `2`.
- Consumes: host import `__ss_measure(id)` (may be absent in this task's test — guard it).

- [ ] **Step 1: Failing Rust test** that evals `dom.js` in a bare Boa context, runs author JS (`const d=document.createElement('div'); d.setAttribute('class','row'); document.body ?? …; root.appendChild(d)`), calls `__ss_flush()`, decodes the bytes with `OpBatch::decode`, and asserts the ops. (Root handle exposed to JS as a global `__ss_root` bound to id `1`.)
- [ ] **Step 2: Run, expect FAIL.** `cargo test -p superui_js --test shadow_dom`
- [ ] **Step 3: Implement `dom.js`:** a `Node` factory recording ops into a module-level array + a string-interning pool, structural reads served from JS-side parent/child arrays and an attribute map, `__ss_flush()` serializing to Task 1's byte layout and clearing the queue. Keep it framework-free ES that Boa, V8, and browsers all accept (no Node/browser globals).
- [ ] **Step 4: Run, expect PASS.**
- [ ] **Step 5: Commit** `feat(superui_js): JS-side shadow DOM emitting op batches`

---

## Task 5: JS event dispatch (W3C walk) + listener table

**Files:**
- Modify: `crates/superui_js/js/dom.js` (add listeners + dispatch)
- Test: `crates/superui_js/tests/dispatch.rs`

**Interfaces:**
- Produces: `addEventListener(type, fn, capture?)`, `removeEventListener(...)`, and a global `__ss_dispatch(jsId, type, key, bubbles, cancelable) -> prevented:boolean` performing capture→target→bubble over the shadow tree, honoring `stopPropagation`/`stopImmediatePropagation`/`preventDefault`, building an `event` object with `target`/`currentTarget`/`type`/`key`.
- Consumes: shadow tree from Task 4.

- [ ] **Step 1: Failing test** — build parent>child, add a bubbling listener on parent and a `preventDefault` listener on child, `__ss_dispatch(childId, "click", null, true, true)`, assert parent listener ran and return is `true` (prevented). Add a `stopPropagation` case asserting parent does not run.
- [ ] **Step 2: Run, expect FAIL.** `cargo test -p superui_js --test dispatch`
- [ ] **Step 3: Implement** the listener map (per node, per type, capture flag) and the ordered walk (reuse the phase model from the retired `superui_dom::build_dispatch_plan` semantics, but in JS).
- [ ] **Step 4: Run, expect PASS.**
- [ ] **Step 5: Commit** `feat(superui_js): JS-side W3C event dispatch`

---

## Task 6: Redefine the `JsEngine` trait for the op-wire

**Files:**
- Modify: `crates/superui_js/src/lib.rs` (trait), `crates/superui_js/src/engine.rs` (Boa impl signature)
- Test: existing `superui_js` tests updated to the new trait

**Interfaces:**
- Produces the trait exactly as in the spec:

```rust
pub trait JsEngine {
    fn eval(&mut self, script: &str) -> Result<(), String>;
    fn dispatch_event(&mut self, target: JsNodeId, ty: &str, key: Option<&str>,
                      bubbles: bool, cancelable: bool) -> bool;
    fn run_timers(&mut self, now_ms: f64);
    fn flush_ops(&mut self) -> OpBatch;
    fn emit(&mut self, name: &str, value: &serde_json::Value);
    fn drain_outbox(&mut self) -> Vec<(String, serde_json::Value)>;
}
```

- [ ] **Step 1** Update the trait; adapt existing trait doctests/tests to compile (dispatch now takes `JsNodeId`, not `superui_dom::NodeId`). `cargo build -p superui_js` — expect the Boa impl to fail to compile (drives Task 7).
- [ ] **Step 2** Commit the trait change together with Task 7 (they are one compile unit). Do not commit a non-building crate; sequence 6→7 in one branch, commit at end of Task 7.

---

## Task 7: BoaEngine on the op-wire

**Files:**
- Rewrite: `crates/superui_js/src/engine.rs`
- Rewrite/retire: `crates/superui_js/src/state.rs` (drop `NodeHandle`/`Protos`/`wrappers`/`listeners`; keep only what host imports need — timers, outbox, `now_ms`, the `OpApplier`/`IdMap` live Rust-side)
- Modify: `crates/superui_js/src/lib.rs` re-exports
- Test: `crates/superui_js/tests/boa_engine.rs`

**Interfaces:**
- Consumes: `dom.js` (Tasks 4–5), codec/applier (Tasks 1–3).
- Produces: `struct BoaEngine` implementing `JsEngine`. Constructor evals `dom.js` then reactive `runtime.js`/`render.js`. `flush_ops()` calls JS `__ss_flush()`, reads the byte array, `OpBatch::decode`s it. `dispatch_event` calls JS `__ss_dispatch`. Host imports registered as Boa `NativeFunction`s: `__ss_measure`, `console.*`, `fetch`, `__superui_bevy_send` (pushes to outbox), time/random.

- [ ] **Step 1: Failing test** — `BoaEngine::new`, `eval` author JS that creates a node, `flush_ops()` returns a decoded batch with the expected `CreateElement`+`InsertBefore`; `dispatch_event` on a listener returns the prevented flag; `drain_outbox` returns a pushed `bevy.send` payload.
- [ ] **Step 2: Run, expect FAIL.** `cargo test -p superui_js --test boa_engine`
- [ ] **Step 3: Implement.** Register host `NativeFunction`s; wire `__ss_measure` to call a Rust closure that reads layout (stubbed to zero rect here; real layout arrives in Task 9 via `superui_bridge`). Keep the wasm `web-time` clock path.
- [ ] **Step 4: Run, expect PASS**, plus `cargo test -p superui_js` (all).
- [ ] **Step 5: Commit** `refactor(superui_js): Boa backend on the op-wire + JS shadow DOM`

---

## Task 8: Retire superui_api Rust DOM modules; slim to host logic

**Files:**
- Delete: `crates/superui_api/src/{document,element,node,text,events}.rs`
- Rewrite: `crates/superui_api/src/lib.rs` — expose engine-agnostic host logic (`console`, `fetch`, `measure`) as plain functions the engine impls bind; no Boa types in the public surface for the shared logic (keep the Boa binding shim behind `engine-boa`).
- Modify: `crates/superui_api/src/{console,fetch,timers}.rs` — split pure logic from Boa binding.
- Test: `crates/superui_api/tests/host_logic.rs`

**Interfaces:**
- Produces: `pub fn console_emit(level: ConsoleLevel, msg: &str)`, `pub struct FetchRequest`/`pub fn fetch_blocking(...) -> FetchResult` (or the existing async shape — confirm current `fetch.rs`), `pub fn measure(dom: &Dom, node: NodeId) -> Rect`. The Boa-specific `NativeFunction` wrappers move into `engine.rs` (Task 7) or a `#[cfg(feature="engine-boa")]` submodule.
- Consumes: nothing new.

- [ ] **Step 1: Failing test** for `measure` returning a zero `Rect` for an unlaid node and the laid rect otherwise (drive with a `Dom` + a fake layout), and `console_emit` routing to the captured sink.
- [ ] **Step 2: Run, expect FAIL.**
- [ ] **Step 3: Implement** the split; delete the retired modules; fix `superui_api::install` callers.
- [ ] **Step 4: Run, expect PASS**; `cargo build -p superui_api`.
- [ ] **Step 5: Commit** `refactor(superui_api): retire Rust DOM bindings; keep engine-agnostic host logic`

---

## Task 9: superui_bridge per-frame loop + UiRuntime holds `Box<dyn JsEngine>`

**Files:**
- Rewrite: `crates/superui_bridge/src/runtime.rs`
- Modify: `crates/superui_bridge/src/bevy_bridge.rs` (outbox now drained via `JsEngine::drain_outbox`; `emit` via `JsEngine::emit`)
- Test: `crates/superui_bridge/tests/frame_loop.rs`

**Interfaces:**
- Consumes: `JsEngine` (Task 6), `OpApplier` (Task 3), `superui_dom::Dom`, taffy layout access for `__ss_measure`.
- Produces: `UiRuntime { engine: Box<dyn JsEngine>, applier: OpApplier, dom: Rc<RefCell<Dom>> }`, method `fn tick(&mut self, now_ms: f64, input: &[InputEvent])` running: dispatch → run_timers → `flush_ops` → `applier.apply` → drain outbox / emit. `__ss_measure` closure reads taffy computed layout for the mapped node.

- [ ] **Step 1: Failing test** — construct `UiRuntime` (engine-boa), eval author JS that on a `bevy.send`-triggering click appends a node; feed one input event; assert after `tick` the render-mirror `Dom` gained the node and the outbox carried the payload.
- [ ] **Step 2: Run, expect FAIL.**
- [ ] **Step 3: Implement** the loop; wire `__ss_measure` to real layout; construct the engine behind `cfg(feature)` (only `engine-boa` exists yet).
- [ ] **Step 4: Run, expect PASS**; then run examples headlessly: `cargo run -q -p todomvc_supersolid` builds; `cargo test -p superui_test_engine` green.
- [ ] **Step 5: Commit** `refactor(superui_bridge): engine-agnostic per-frame op-wire loop`

---

## Task 10: Feature gating + optional deps (engine-boa default)

**Files:**
- Modify: `crates/superui_js/Cargo.toml`, `crates/superui_api/Cargo.toml`, `crates/superui_bridge/Cargo.toml`, `crates/supersolid_runtime/Cargo.toml`, `crates/superui/Cargo.toml`
- Modify: `crates/superui_js/src/lib.rs` (cfg engine selection + `compile_error!` guards)
- Test: `cargo` feature-matrix builds

**Interfaces:**
- Produces: features `engine-boa`(default)/`engine-v8`/`engine-web` on `superui` forwarded to `superui_js`; Boa deps (`boa_engine`, `boa_gc`, wasm `web-time`/`getrandom`) become `optional` and gated under `engine-boa`.

- [ ] **Step 1** Make Boa deps optional; add the three features; add `compile_error!` guards (two engines, wrong target). Add a `type ActiveEngine`/constructor selected by cfg (still only Boa).
- [ ] **Step 2** Verify matrix:

```bash
cargo build -p superui                                   # default = engine-boa
cargo build -p superui --no-default-features --features engine-boa
cargo test  -p superui_js -p superui_api -p superui_bridge
```
Expected: all PASS; a `--features engine-v8,engine-boa` build must fail with the guard message (verify: `cargo build -p superui --features engine-v8 2>&1 | grep "one engine"`).

- [ ] **Step 3: Commit** `feat: compile-time engine selection with Boa as default`

---

## Task 11: Boa-on-shadow-DOM benchmark + regression gate

**Files:**
- Create: `docs/superpowers/bench/raw/*-boa-shadow.json`
- Modify: `docs/superpowers/bench/2026-09-19-baseline-boa.md` → add a "Boa-shadow" column, or a new results doc.

- [ ] **Step 1** Re-run the Task 0 Step 2 commands (backend `supersolid`, same seeds/frames) on the branch; save as `*-boa-shadow.json`.
- [ ] **Step 2** Run all examples for a visual/functional smoke check: `cargo run -p todomvc_supersolid`, `cargo run -p game_menu`, `cargo run -p horde` (brief manual confirm they render + interact).
- [ ] **Step 3** Record the Boa baseline→shadow delta. If regression is severe enough to matter, note it and flag the escape hatch decision (spec §Risks) for the user — do not silently proceed to production removal of the old path beyond what the refactor already did.
- [ ] **Step 4: Commit** `bench: Boa-on-shadow-DOM results vs baseline`

---

## Task 12: engine-v8 (deno_core) adapter

**Files:**
- Create: `crates/superui_js/src/engines/v8.rs`
- Modify: `crates/superui_js/Cargo.toml` (`deno_core` optional, gated `engine-v8`, `not(wasm)`)
- Test: `crates/superui_js/tests/v8_engine.rs` (gated `engine-v8`)

**Interfaces:**
- Produces: `struct V8Engine` implementing `JsEngine`, same contract as `BoaEngine`. `eval` runs on a `deno_core::JsRuntime`; host imports registered as `#[op2]` ops (`__ss_measure`, `console`, `fetch`, `__superui_bevy_send`); `flush_ops` calls JS `__ss_flush` and copies the returned bytes; `run_timers` pumps the event loop.

- [ ] **Step 1: Failing test** mirroring Task 7's `boa_engine.rs` assertions but on `V8Engine` (create→flush→dispatch→outbox). Run: `cargo test -p superui_js --no-default-features --features engine-v8 --test v8_engine`.
- [ ] **Step 2: Run, expect FAIL** (type missing / first V8 build — note the long one-time `deno_core` build).
- [ ] **Step 3: Implement** the adapter; pin the `deno_core` version; reuse the exact `dom.js`/`runtime.js`/`render.js` bundle and the shared codec/applier.
- [ ] **Step 4: Run, expect PASS**; `cargo build -p superui --no-default-features --features engine-v8`.
- [ ] **Step 5: Commit** `feat(superui_js): engine-v8 deno_core adapter`

---

## Task 13: engine-v8 examples + native V8 benchmark

**Files:**
- Create: `docs/superpowers/bench/raw/*-v8.json`

- [ ] **Step 1** Build/run examples under V8: `cargo run -p horde --no-default-features --features engine-v8` (and citadel/rows/todomvc); confirm they render + interact.
- [ ] **Step 2** Run all three benches under V8 (same seeds/frames), save `*-v8.json`.
- [ ] **Step 3: Commit** `bench: native V8 results`

---

## Task 14: engine-web (browser engine) adapter

**Files:**
- Create: `crates/superui_js/src/engines/web.rs`
- Modify: `crates/superui_js/Cargo.toml` (`wasm-bindgen`/`js-sys`/`web-sys` optional, gated `engine-web`, `wasm` only)
- Test: `wasm-bindgen-test` smoke test in `crates/superui_js/tests/web.rs` (gated)

**Interfaces:**
- Produces: `struct WebEngine` implementing `JsEngine`. The `dom.js`+reactive bundle is eval'd in the page (via `js_sys::eval` / a `<script>` injection at init). Host imports exported from Rust with `#[wasm_bindgen]`; `flush_ops` calls the JS `__ss_flush` export and copies the `Uint8Array`. `dispatch_event`/`emit`/`drain_outbox` cross via `wasm-bindgen`.

- [ ] **Step 1** Implement the adapter; the JS side is identical to native (same bundle), only the host-call transport differs.
- [ ] **Step 2** Build the wasm target: `cargo build -p superui --no-default-features --features engine-web --target wasm32-unknown-unknown`. Confirm **no Boa** in the dependency tree (`cargo tree -p superui --no-default-features --features engine-web --target wasm32-unknown-unknown | grep -i boa` returns nothing).
- [ ] **Step 3** Browser smoke test: build one example (todomvc_supersolid) for web and load it; confirm it renders and a click mutates the UI.
- [ ] **Step 4: Commit** `feat(superui_js): engine-web browser-engine adapter`

---

## Task 15: Three-way comparison report

**Files:**
- Create: `docs/superpowers/bench/2026-09-19-engine-comparison.md`

- [ ] **Step 1** Collate `*-boa-baseline.json`, `*-boa-shadow.json`, `*-v8.json` for horde/citadel/rows into one table per bench: workload, current-Boa, Boa-shadow, native-V8, and the ratios. (Web excluded per spec.)
- [ ] **Step 2** Write a short analysis: Boa regression/gain, V8 speedup vs both Boa variants, and whether the escape hatch is warranted.
- [ ] **Step 3: Commit** `docs: three-way engine comparison (Boa vs Boa-shadow vs V8)`

---

## Self-Review

- **Spec coverage:** op-wire (T1–3), shadow DOM + dispatch (T4–5), trait (T6), Boa on wire (T7), superui_api retirement (T8), bridge loop + measure (T9), feature gating/optional deps (T10), Boa bench (T11), V8 (T12–13), web (T14), comparison all-three (T15), baseline-first (T0). Test engine stays Boa (T9 verify). Covered.
- **Placeholder scan:** interfaces named concretely; where a `superui_dom`/`fetch` signature must be confirmed against existing code, the step says "confirm exact name and reuse" rather than inventing one — an intentional instruction, not a TODO.
- **Type consistency:** `JsNodeId=u32`, root `1`, `OpBatch`/`Op`/`StrId` used consistently T1→T3→T4→T7→T12→T14; trait signature identical in T6 and both adapters.
- **Review Focus:** each of the five items has an owning test — stale id (T3), event-during-mutation (T9), measure-before-layout (T8), batch ordering/append (T3), empty/large batch (T1 empty; large operands covered by `u32` width note in T1 Step 3).
