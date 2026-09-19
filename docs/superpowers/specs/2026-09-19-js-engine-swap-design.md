# JS engine swap: feature-selected engines behind a declarative op-wire

Date: 2026-09-19
Status: Approved design — ready for implementation planning
Branch: to be created (kept; do not merge until benchmarks are compared)

## Context

superui embeds **Boa** as its JavaScript engine, compiled unconditionally into
every crate and every target (native + wasm). Boa is a pure-Rust tree-walking
interpreter with no JIT; it is the CPU bottleneck under load (the horde bench
attributes ~98% of frame time to the reactive reconcile running in Boa) and it
bloats the wasm bundle by shipping a whole JS interpreter *inside* wasm.

We want the same engine setup as `bevy_react`:

- **Native (macOS/Linux/Windows):** V8 via `deno_core`.
- **Web (wasm):** the browser's own JS engine — no engine embedded in the wasm.
- Engine chosen at **compile time via a Cargo feature**; only the selected
  engine's dependencies compile.
- **Boa stays as the default, portable fallback** so CI, unit tests, and the
  E2E test harness keep working unchanged throughout.

This is kept on a long-lived branch and not merged until the before/after
benchmarks are captured and compared.

The obstacle is coupling: Boa is not behind any feature, and its API is woven
through ~17 files. `superui_js` exposes a narrow `JsEngine` trait, but
`BoaEngine::context_mut()` leaks `boa_engine::Context` straight into
`superui_api` and `supersolid_runtime`, and `HostState` is Boa-GC-native
(`Trace`/`Finalize`/`JsData`) in Boa's realm `HostDefined` slot. Swapping the
engine under that shape would mean re-implementing the entire DOM binding layer
per engine.

## Goals

- One `JsEngine` boundary, three interchangeable implementations selected at
  compile time: `engine-boa` (default), `engine-v8`, `engine-web`.
- Preserve the full imperative/standards DOM API (`document.createElement`,
  `el.getAttribute`, `el.childNodes`, `el.textContent`, listeners, events) *and*
  the supersolid TSX reactive reconciler, identically on all engines.
- Native V8 and web browser engine both build and run; existing examples
  (todomvc_supersolid, game_menu, horde, citadel, rows) work.
- Measure Boa before/after and compare against native V8.

## Non-goals

- No thread-isolated engine. All engines are pumped on the Bevy thread each
  frame (one synchronous boundary shape). `bevy_react`'s own-thread V8 is
  explicitly out of scope.
- `superui_test_engine` is **not** ported to the abstraction; it stays pinned to
  `engine-boa`.
- Web is **not** in the first benchmark comparison (built and smoke-tested only).
- No change to the `oxc` TSX→JS transpile (stays a build-time step).

## Resolved decisions

| Decision | Choice |
|----------|--------|
| Boundary shape | Coarse, declarative **op-wire** (bevy_react model) |
| Imperative DOM | Kept, served by a **JS-side shadow DOM**; reads answered in JS, writes batched as ops |
| Threading | **Same-thread**, one synchronous boundary for all engines |
| Engine selection | **Compile-time** Cargo feature; deps optional, only selected engine compiles |
| Runtime holder | `Box<dyn JsEngine>` |
| Event dispatch | **JS-side** W3C walk; Rust only supplies the hit target |
| Op encoding | **Compact flat buffer** default; JSON as a drop-in fallback |
| Boa's role | Default portable fallback; also runs on the shared op-wire (one DOM model) |
| Test engine | Stays `engine-boa`, unchanged |

## Architecture

Source-of-truth inverts. Today the Rust `Dom` is authoritative and
`superui_api` mutates it directly through Boa native functions. New model: the
**JS side owns a shadow DOM**; Rust holds a **render mirror** that flair/taffy
lays out.

```
        author JS  (TSX reconciler  OR  document.createElement …)
                              |  operates on
                    +---------v----------+
                    |  JS shadow DOM     |   reads answered here, no Rust hop
                    |  + listener table  |   (getAttribute, childNodes, textContent…)
                    +---------+----------+
                writes recorded -> op queue
                              |  engine.flush_ops()  (once per frame, synchronous)
                    +---------v----------+
                    | Rust OpApplier     |   -> mutates the render-mirror Dom
                    | (engine-agnostic)  |   -> flair/taffy layout -> Bevy render
                    +---------+----------+
             events ^         |  layout reads
                    |         v
        Rust input -+   __ss_measure(jsId) -> rect   (sync, same-thread)
```

The DOM API, reactive core (`runtime.js`/`render.js`), and reconciler all become
**pure JS shipped as a bundle** and eval'd at startup — engine-independent. Each
engine only has to: eval the bundle, `dispatch_event`, `run_timers`,
`flush_ops`, and expose a small set of host imports. That tiny surface is what
makes the engines interchangeable.

Boa moves onto this model too (not just the new engines) so there is a single
DOM implementation to maintain and test.

## Op-wire protocol

**Node identity.** The JS shadow DOM assigns each node a monotonic integer id.
Ops reference those ids. Rust keeps a `jsId -> NodeId` map (the `superui_dom`
arena handle). The document/root is a fixed id (`1`), pre-registered on both
sides. This replaces the old `NodeHandle` downcast + `wrappers` cache.

**Op set** (what both the reconciler and the imperative API compile to):

| Op | Operands | Applier action (reuses existing `Dom` methods) |
|----|----------|-----------------------------------------------|
| `CreateElement` | id, tag | `dom.create_element(tag)`, record map |
| `CreateText` | id, data | `dom.create_text(data)` |
| `SetAttribute` / `RemoveAttribute` | id, name, value? | attr write |
| `SetProperty` | id, name, value | form state (`value`, `checked`) |
| `SetStyle` | id, prop, value | inline style prop |
| `SetText` | id, data | text / `nodeValue` update |
| `InsertBefore` | parent, node, ref? | `ref=0` -> append |
| `RemoveChild` | parent, node | detach |

**Encoding.** A compact flat buffer (opcode + integer operands, strings interned
into a per-batch pool), behind a single encode/decode boundary so JSON is a
trivial fallback. The op-wire adds a boundary crossing that did not exist in the
in-process model, so cheap encoding matters from day one.

**Event dispatch (JS-side).** Rust input/hit-testing decides which node was hit,
maps `NodeId -> jsId`, and calls `engine.dispatch_event(jsId, ty, key, bubbles,
cancelable) -> prevented`. The engine invokes a JS entrypoint that performs the
W3C capture->target->bubble walk against the shadow tree + JS listener table,
runs `preventDefault` locally, and returns the boolean. This deletes the Rust
listener registry and dispatch-plan walk (`state.rs` listeners, `engine.rs`
dispatch loop); listeners live only in JS.

**Layout reads.** `getBoundingClientRect`/`offsetWidth` call a host import
`__ss_measure(jsId) -> rect`; Rust maps the id and reads taffy's computed
layout. Synchronous (same-thread). This is the only JS->Rust read path.

**Per-frame loop** (in `UiRuntime`, replacing today's direct calls):

1. drain input -> `engine.dispatch_event(...)` per event (listeners may queue
   DOM writes)
2. `engine.run_timers(now)` + pump microtasks -> reactive effects flush, queuing
   more writes
3. `batch = engine.flush_ops()` -> `OpApplier` writes the render-mirror `Dom`
4. drain `window.bevy` outbox -> Bevy events; emit queued Bevy->JS events

## Engine abstraction & feature gating

The trait is the coarse boundary (in `superui_js`, names no engine types):

```rust
pub trait JsEngine {
    fn eval(&mut self, script: &str) -> Result<(), String>;
    fn dispatch_event(&mut self, target: JsNodeId, ty: &str, key: Option<&str>,
                      bubbles: bool, cancelable: bool) -> bool;   // JS does the walk
    fn run_timers(&mut self, now_ms: f64);
    fn flush_ops(&mut self) -> OpBatch;                            // encoded flat buffer
    fn emit(&mut self, name: &str, value: &serde_json::Value);     // Bevy -> JS
    fn drain_outbox(&mut self) -> Vec<(String, serde_json::Value)>;// JS -> Bevy
}
```

The `OpApplier` (decode batch -> mutate render-mirror `Dom`) is an
engine-agnostic module written once. Each engine impl wires the JS bundle plus
the small set of host imports it expects: `__ss_measure`, `console.*`, `fetch`,
`__superui_bevy_send` (outbox), time/random.

| Feature | Type | Deps (optional, gated) | Valid targets |
|---------|------|------------------------|---------------|
| `engine-boa` (default) | `BoaEngine` | `boa_engine` fork | native + wasm |
| `engine-v8` | `V8Engine` | `deno_core` | native only |
| `engine-web` | `WebEngine` | `wasm-bindgen`, `js-sys`, `web-sys` | wasm only |

Gating mechanics:

- `UiRuntime` holds `Box<dyn JsEngine>`; the concrete type is chosen by
  `cfg(feature = …)`. Features flow top-down: `superui = { features =
  ["engine-v8"] }` forwards to `superui_js`.
- **All engine dependencies are `optional = true`**; only the selected engine's
  deps compile. Boa's deps move behind `engine-boa` (they are unconditional
  today). An `engine-v8` native build ships **no Boa**; an `engine-web` build has
  **no Boa-in-wasm** — this is where the binary-size and perf win lands.
- `compile_error!` guards reject nonsense combos (`engine-v8` on wasm,
  `engine-web` on native, two engines at once). Default stays `engine-boa`, so
  `cargo test` / CI / `superui_test_engine` are unaffected.
- The wasm `getrandom` (`wasm_js`) and `web-time` shims currently added for Boa
  move under `engine-boa` (the browser engine needs neither).

## Crate-by-crate change map

| Crate | Change |
|-------|--------|
| `superui_dom` | Unchanged. Becomes the render mirror the applier writes; `create_element` etc. reused. |
| `superui_js` | Keep `JsEngine` trait; add `OpBatch` encode/decode + `OpApplier` + `jsId<->NodeId` map (engine-agnostic). Add gated engine impls. Remove `NodeHandle`/`Protos`/`wrappers`/`listeners`/`HostState` GC machinery. |
| new JS bundle | `dom.js` — the shadow DOM (`document`/`element`/`text`/`node`/`event`, listener table, W3C dispatch walk, op recording, `__ss_measure` calls). `runtime.js`/`render.js` ride on top, mostly unchanged; `$ss.*` retargets to the shadow DOM. Shipped via `include_str!`, eval'd at startup. |
| `superui_api` | The 8 Rust DOM binding modules (`document.rs`, `element.rs`, `node.rs`, `text.rs`, `events.rs`, …) retired into `dom.js`. Slims to engine-agnostic host logic — `console`, `fetch`, `measure` — that each engine binds via its own native-fn ABI. |
| `supersolid_runtime` | Eval via the `JsEngine` trait instead of `boa_engine::Context`; drop direct Boa use. |
| `superui_bridge` | `UiRuntime` holds `Box<dyn JsEngine>`; owns the per-frame loop. `bevy_bridge.rs` outbox stays, registered per-engine. |
| `superui_test_engine` | Unchanged; pinned to `engine-boa`. |
| `superui` (top) | Adds `engine-boa` (default) / `engine-v8` / `engine-web` features, forwarded down. |

## Build targets

- Native default = Boa; native `engine-v8` = deno_core; wasm default = Boa
  (still supported); wasm `engine-web` = browser engine.
- `oxc` TSX->JS transpile unchanged (build-time).
- The first `engine-v8` build compiles/downloads V8 — a heavy one-time cost.

## Benchmarking

Three headless macro-benchmarks exist, each with a `bench` feature and a bench
bin on the shared `superui_bench_support` harness:

- `cargo run -p horde   --bin horde-bench   --features bench`
- `cargo run -p citadel --bin citadel-bench --features bench`
- `cargo run -p rows    --bin rows-bench    --features bench`

Sequence (the old native-binding path is destroyed by the refactor, so the
baseline must be captured first):

1. **Baseline — current Boa.** Run all three benches on `main` (pre-change) and
   commit the numbers to the branch.
2. **Boa on shadow DOM.** After the refactor, run all three under `engine-boa`.
3. **Native V8.** After `engine-v8` lands, run all three under `engine-v8`.

Deliver a **three-way comparison** (current-Boa vs Boa-shadow vs native-V8)
across horde, citadel, and rows. Web is excluded from this comparison.

Expectation to validate, not assume: Boa likely regresses on DOM-mutation-heavy
frames (DOM bookkeeping moves from compiled Rust into interpreted JS), partly
offset by removing the per-op native-call crossing in favor of one batched flush
per frame. If the Boa regression is unacceptable, the escape hatch is to keep
Boa on native DOM bindings and put only V8/web on the shadow-DOM wire — at the
cost of two DOM implementations. Do not take the escape hatch unless the
measured regression forces it.

## Verification

- Unit: op encode/decode roundtrip; `OpApplier` correctness; `jsId` map;
  shadow-DOM behavior driven through the `JsEngine` trait.
- Regression: todomvc_supersolid, game_menu, horde, citadel, rows run under
  `engine-boa` and under `engine-v8`.
- E2E: `superui_test_engine` stays green on Boa.
- Web: `engine-web` builds; one example smoke-tested in a real browser.
- Perf: the three-way horde/citadel/rows comparison table.

## Ordered build (one branch)

1. Capture current-Boa baseline (all three benches); commit numbers.
2. Build op-wire + `dom.js` shadow DOM + `OpApplier`; move Boa onto it; keep the
   default green.
3. Bench Boa-on-shadow-DOM; regression-check examples + `superui_test_engine`.
4. Add `engine-v8` (deno_core adapter); bench native V8.
5. Add `engine-web` (browser adapter); browser smoke test.
6. Present the three-way comparison.

## Risks

- **Boa perf regression** — measured at step 3; escape hatch above.
- **deno_core build weight / API churn** — first build is slow; pin the
  `deno_core` version.
- **Layout-read timing** — `__ss_measure` reads last-laid-out geometry; author
  JS reading layout in the same frame it mutated sees the previous frame's rect
  (same as a browser before reflow). Document the contract.
- **Encoding hotspot** — if the flat buffer underperforms, fall back to JSON
  behind the same boundary; revisit with a binary layout.
