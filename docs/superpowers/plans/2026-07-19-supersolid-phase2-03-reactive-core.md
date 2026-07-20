# Supersolid Phase 2 — Plan 3: `supersolid_runtime` reactive core — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a new **wasm-clean** crate `supersolid_runtime` holding the Solid-like fine-grained reactive core — `createSignal` / `createEffect` / `createMemo` / `onMount` / `onCleanup` / `createContext` / `useContext` (plus `createRoot` / `untrack` / `batch`) — authored as a hand-written JS module run in Boa, and wire it into the live `UiRuntime` so author scripts get the reactive globals the transpiler (Plan 2) already emits imports for.

**Architecture:** The runtime is a single hand-written `runtime.js` (embedded via `include_str!`) evaluated once into the Boa context by a thin `install(&mut BoaEngine)`. The graph is the modern **glitch-free two-color mark-and-sweep** (`reactively`/Solid-internals model): `CLEAN`/`CHECK`/`DIRTY` node states, reads subscribe the current tracking `Listener`, writes mark observers and enqueue effects, and memos are **lazy** (`updateIfNecessary` pulls them current on read). Layered on top is Solid's **Owner/Listener split**: every computation is also an owner carrying cleanups, owned children, and an inherited context map, so `onCleanup` / `createRoot` disposal / `createContext` work off the owner tree. The scheduler is **synchronous** — a write outside `batch()` propagates and flushes effects before returning to Rust, so no per-frame Rust pump is needed (event callbacks run inside `dispatch_event`, timers inside `run_timers`, both before `reconcile_system`, leaving the DOM settled by reconcile time). Internals stay closured; only the author API is published on `globalThis`.

**Tech Stack:** Rust, edition 2021, `boa_engine` 0.21 (via `superui_js::BoaEngine`), the existing `superui_*` crates. The crate is **Bevy-free and wasm-clean** (no `oxc`, unlike the `supersolid` transpiler crate — the runtime must run on every target, spec §5/§6).

## Global Constraints

- **Bevy 0.17**, edition 2021.
- `supersolid_runtime` is **Bevy-free and wasm-clean**: pure JS + a thin Boa install. It depends only on `superui_js` (and, for tests, `superui_dom` + `boa_engine`). It must **never** depend on `oxc` or `supersolid` (the transpiler).
- **TDD** throughout; **frequent commits**; work on `main` (per project CLAUDE.md — no feature branch needed).
- **Graceful degradation (design §1):** author-script errors already log-and-swallow through `UiRuntime::run_script`. The runtime's own `runtime.js` is vetted by this plan's tests, so `install` may treat a failed eval of *its own* source as a hard bug.
- The runtime **implements** the author-facing globals that Plan 2's transpiler strips imports for: `createSignal`, `createEffect`, `createMemo`, `onMount`, `onCleanup`, `createContext`, `useContext`. It additionally provides `createRoot`, `untrack`, `batch` (Solid-standard supporting primitives), and a runtime-internal `$ssProvideContext` (context provision primitive; Plan 4's `<Provider>` will wrap it). The `$ss.*` render helpers (`el`/`txt`/`attr`/`child`/`insert`/`bind`/`on`/`cmp`/`frag`) and control-flow components (`Show`/`For`/…) are **out of scope** — they are Plan 4 and will compose on this core (e.g. `$ss.bind(el, name, thunk)` becomes `createEffect(() => el.setAttribute(name, thunk()))`).

### The reactive-graph contract (what `runtime.js` guarantees)

These are the behaviors the tests lock in. They are the whole point of the crate:

- **Signals:** `createSignal(v, {equals?})` → `[read, write]`. `read()` returns the value and subscribes the current tracking scope. `write(next)` sets it; `write(fn)` calls `fn(prev)` first (updater form). No notification when the new value is `equals` to the old (`equals` defaults to `Object.is`; `equals: false` always notifies).
- **Effects:** `createEffect(fn, seed?)` runs `fn` once immediately (tracking its reads), then re-runs whenever a tracked dependency changes. `fn` receives its previous return value. Disposed with its owner.
- **Memos:** `createMemo(fn, seed?, {equals?})` returns a read getter that is **lazy** (does not compute until first read), **memoized** (recomputes only when a dependency changed), and is itself a reactive source. A memo whose recomputed value is `equals` to its previous value does **not** propagate — it gates downstream effects.
- **Glitch-free:** in a diamond (`A → B`, `A → C`, `(B,C) → D`), one change to `A` re-runs `D` **exactly once**.
- **Ownership:** `createRoot(fn => …)` establishes a disposable owner and passes a `dispose` function; disposing runs all nested cleanups and stops all nested effects. `onCleanup(fn)` registers `fn` on the current owner; it runs before the owner's next re-run and on disposal.
- **Lifecycle:** `onMount(fn)` runs `fn` exactly once, untracked, after the current setup flush.
- **Context:** `createContext(default?)` / `useContext(ctx)` resolve via the owner chain; `useContext` returns the nearest provided value or the default. `$ssProvideContext(ctx, value, fn)` runs `fn` with `ctx` set to `value` for the dynamic extent, nesting correctly; effects/memos created under a provider capture that value.
- **Batching:** `batch(fn)` coalesces all writes inside `fn` into a single effects flush. `untrack(fn)` runs `fn` without subscribing its reads.

---

## Task 1: Crate scaffold + reactive-graph core (signals, effects, scheduler, `untrack`, `batch`)

Establishes the `supersolid_runtime` crate, the `install(&mut BoaEngine)` entry point, and the complete graph engine plus `createSignal` / `createEffect` / `untrack` / `batch`. This is the smallest *functional* unit: a signal→effect reactive loop with a synchronous, glitch-free scheduler. `createMemo`, ownership, lifecycle, and context are added in later tasks (their code layers on the same internals).

**Files:**
- Modify: `Cargo.toml` (workspace) — nothing to add (`boa_engine`/`boa_gc` already in `[workspace.dependencies]`); the new crate is picked up by `members = ["crates/*"]`.
- Create: `crates/supersolid_runtime/Cargo.toml`
- Create: `crates/supersolid_runtime/src/lib.rs` (public `install` + `#[cfg(test)] mod tests` + test helpers)
- Create: `crates/supersolid_runtime/src/runtime.js` (the reactive core, embedded via `include_str!`)

**Interfaces:**
- Produces:
  - `supersolid_runtime::install(engine: &mut superui_js::BoaEngine)` — evaluates the embedded `runtime.js`, publishing the author API on `globalThis`. Idempotent per engine is **not** required (called once per `UiRuntime`).
  - `runtime.js` publishes on `globalThis`: `createSignal`, `createEffect`, `untrack`, `batch` (this task); later tasks add `createMemo`, `createRoot`, `onMount`, `onCleanup`, `createContext`, `useContext`, `$ssProvideContext`.
  - Internal (closured) engine used by every later task: `createComputation(fn, init, isEffect, options)`, `readSource(node)`, `writeSource(node, next)`, `stale(node, state)`, `updateIfNecessary(node)`, `update(node)`, `cleanNode(node)`, `disposeOwner(node)`, `runUpdates(fn)`, `runEffects(queue)`, and the module globals `Listener`, `Owner`, `Effects`.

- [ ] **Step 1: Create the crate manifest**

Create `crates/supersolid_runtime/Cargo.toml`:

```toml
[package]
name = "supersolid_runtime"
edition.workspace = true
version.workspace = true
license.workspace = true

[dependencies]
superui_js = { path = "../superui_js" }

[dev-dependencies]
superui_dom = { path = "../superui_dom" }
boa_engine.workspace = true

# Boa pulls getrandom 0.3, which needs the JS backend on wasm (same gotcha as the
# other JS crates). Pair with the repo-root `.cargo/config.toml` rustflag.
[target.'cfg(target_arch = "wasm32")'.dependencies]
getrandom = { version = "0.3", features = ["wasm_js"] }
```

- [ ] **Step 2: Write the failing tests**

Create `crates/supersolid_runtime/src/lib.rs` with the public API stub and the Task-1 tests. (The `install` body is filled in Step 4; write it as a stub first so the tests compile and fail on behavior, not on a missing symbol.)

```rust
//! `supersolid_runtime` — the Supersolid reactive core: Solid-like fine-grained
//! signals, effects, memos, lifecycle, and context, authored in JS and run in
//! Boa. Bevy-free and wasm-clean (unlike the `supersolid` transpiler crate, this
//! runs on every target — direction spec §5/§6). Only the author API is published
//! on `globalThis`; the graph internals stay closured.

use superui_js::{BoaEngine, JsEngine};

/// The reactive core, embedded at build time.
const RUNTIME_JS: &str = include_str!("runtime.js");

/// Install the Supersolid reactive core onto `engine`. Call once, after
/// `superui_api::install` and before evaluating author scripts. Publishes
/// `createSignal`/`createEffect`/`createMemo`/`onMount`/`onCleanup`/
/// `createContext`/`useContext` (+ `createRoot`/`untrack`/`batch`) as globals.
pub fn install(engine: &mut BoaEngine) {
    engine
        .eval(RUNTIME_JS)
        .expect("supersolid_runtime: runtime.js must evaluate (internal invariant)");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;
    use superui_dom::Dom;

    fn engine() -> BoaEngine {
        let dom = Rc::new(RefCell::new(Dom::new()));
        let mut e = BoaEngine::new(dom);
        install(&mut e);
        e
    }

    /// Evaluate `expr` and read it back as an f64 (reads `globalThis.*` snapshots).
    fn num(e: &mut BoaEngine, expr: &str) -> f64 {
        e.context_mut()
            .eval(boa_engine::Source::from_bytes(expr))
            .unwrap()
            .as_number()
            .unwrap_or(f64::NAN)
    }

    #[test]
    fn effect_runs_on_creation_and_reruns_on_dependency_change() {
        let mut e = engine();
        e.eval(
            r#"
            globalThis.runs = 0; var last = 0;
            var pair = createSignal(1);
            var count = pair[0], setCount = pair[1];
            createEffect(function () { globalThis.runs++; last = count(); });
            globalThis.runsAfterCreate = globalThis.runs;   // 1
            globalThis.lastAfterCreate = last;              // 1
            setCount(5);
            globalThis.runsAfterSet = globalThis.runs;      // 2
            globalThis.lastAfterSet = last;                 // 5
            "#,
        )
        .unwrap();
        assert_eq!(num(&mut e, "globalThis.runsAfterCreate"), 1.0);
        assert_eq!(num(&mut e, "globalThis.lastAfterCreate"), 1.0);
        assert_eq!(num(&mut e, "globalThis.runsAfterSet"), 2.0);
        assert_eq!(num(&mut e, "globalThis.lastAfterSet"), 5.0);
    }

    #[test]
    fn effect_does_not_rerun_for_unrelated_signal() {
        let mut e = engine();
        e.eval(
            r#"
            globalThis.runs = 0;
            var a = createSignal(0), b = createSignal(0);
            createEffect(function () { globalThis.runs++; a[0](); }); // reads a only
            b[1](99);                                                 // write b
            globalThis.runsAfterUnrelated = globalThis.runs;          // still 1
            a[1](1);                                                  // write a
            globalThis.runsAfterRelated = globalThis.runs;            // 2
            "#,
        )
        .unwrap();
        assert_eq!(num(&mut e, "globalThis.runsAfterUnrelated"), 1.0);
        assert_eq!(num(&mut e, "globalThis.runsAfterRelated"), 2.0);
    }

    #[test]
    fn signal_equals_default_and_updater_form() {
        let mut e = engine();
        e.eval(
            r#"
            globalThis.runs = 0;
            var s = createSignal(1);
            createEffect(function () { globalThis.runs++; s[0](); });
            s[1](1);                              // Object.is equal -> no notify
            globalThis.runsAfterSame = globalThis.runs;   // 1
            s[1](function (prev) { return prev + 1; });   // updater -> 2
            globalThis.updated = s[0]();                  // 2
            globalThis.runsAfterUpdate = globalThis.runs; // 2
            "#,
        )
        .unwrap();
        assert_eq!(num(&mut e, "globalThis.runsAfterSame"), 1.0);
        assert_eq!(num(&mut e, "globalThis.updated"), 2.0);
        assert_eq!(num(&mut e, "globalThis.runsAfterUpdate"), 2.0);
    }

    #[test]
    fn signal_equals_false_always_notifies() {
        let mut e = engine();
        e.eval(
            r#"
            globalThis.runs = 0;
            var s = createSignal(0, { equals: false });
            createEffect(function () { globalThis.runs++; s[0](); });
            s[1](0);   // same value, but equals:false -> notify
            globalThis.runsAfter = globalThis.runs;   // 2
            "#,
        )
        .unwrap();
        assert_eq!(num(&mut e, "globalThis.runsAfter"), 2.0);
    }

    #[test]
    fn untrack_reads_do_not_subscribe() {
        let mut e = engine();
        e.eval(
            r#"
            globalThis.runs = 0;
            var u = createSignal(1);
            createEffect(function () {
                globalThis.runs++;
                untrack(function () { u[0](); });   // read but do not subscribe
            });
            u[1](2);
            globalThis.runsAfter = globalThis.runs;   // still 1
            "#,
        )
        .unwrap();
        assert_eq!(num(&mut e, "globalThis.runsAfter"), 1.0);
    }

    #[test]
    fn batch_coalesces_writes_into_one_effect_run() {
        let mut e = engine();
        e.eval(
            r#"
            globalThis.runs = 0;
            var p = createSignal(0), q = createSignal(0);
            createEffect(function () { globalThis.runs++; p[0](); q[0](); });
            batch(function () { p[1](1); q[1](1); });   // one combined run
            globalThis.runsAfterBatch = globalThis.runs;   // 2
            p[1](2);                                       // outside batch
            globalThis.runsAfterSingle = globalThis.runs;  // 3
            "#,
        )
        .unwrap();
        assert_eq!(num(&mut e, "globalThis.runsAfterBatch"), 2.0);
        assert_eq!(num(&mut e, "globalThis.runsAfterSingle"), 3.0);
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p supersolid_runtime`
Expected: FAIL to compile — `runtime.js` does not exist yet (the `include_str!` fails). Create an empty `crates/supersolid_runtime/src/runtime.js` and re-run; then the tests compile but FAIL at runtime because `createSignal` / `createEffect` / `untrack` / `batch` are undefined (`TypeError: createSignal is not a function`).

- [ ] **Step 4: Implement the reactive core in `runtime.js`**

Write `crates/supersolid_runtime/src/runtime.js`:

```js
// Supersolid reactive core — Solid-like fine-grained signals for Boa.
//
// Glitch-free two-color (CLEAN/CHECK/DIRTY) mark-and-sweep graph with lazy
// pull-through memos, plus Solid's Owner/Listener split for ownership, cleanup,
// and context. The scheduler is synchronous: a write outside `batch()` flushes
// effects before returning. Internals stay closured; only the author API is
// published on globalThis (the Plan 2 transpiler strips the matching imports).
(function () {
  "use strict";

  var CLEAN = 0, CHECK = 1, DIRTY = 2;

  // Current dependency-tracking computation (null = untracked read).
  var Listener = null;
  // Current ownership scope (for onCleanup / owned children / context).
  var Owner = null;
  // Pending-effects queue for the active update cycle (null = no cycle running).
  var Effects = null;

  var nextContextId = 1; // used by createContext (Task 4)

  // ---- Computation / owner node ----
  function createComputation(fn, init, isEffect, options) {
    var node = {
      fn: fn,
      value: init,
      state: DIRTY,
      effect: isEffect,
      sources: null,     // Set of sources this node reads
      observers: null,   // Set of computations that read this node (memos only)
      cleanups: null,
      owner: Owner,
      owned: null,
      context: Owner ? Owner.context : null,
      equals: options && "equals" in options ? options.equals : Object.is,
      disposed: false,
    };
    if (Owner) (Owner.owned || (Owner.owned = [])).push(node);
    return node;
  }

  // ---- Reads / writes ----
  function readSource(node) {
    if (Listener) {
      (Listener.sources || (Listener.sources = new Set())).add(node);
      (node.observers || (node.observers = new Set())).add(Listener);
    }
    if (node.fn) updateIfNecessary(node); // lazy memo pull-through
    return node.value;
  }

  function writeSource(node, next) {
    var eq = node.equals;
    if (eq !== false && eq(node.value, next)) return next;
    node.value = next;
    if (node.observers && node.observers.size) {
      runUpdates(function () {
        node.observers.forEach(function (o) { stale(o, DIRTY); });
      });
    }
    return next;
  }

  // Two-color marking: direct observers go DIRTY, transitive observers CHECK.
  // An effect that leaves CLEAN is enqueued for the current flush.
  function stale(node, state) {
    if (node.state < state) {
      if (node.state === CLEAN && node.effect) Effects.push(node);
      node.state = state;
      if (node.observers) node.observers.forEach(function (o) { stale(o, CHECK); });
    }
  }

  // Pull a node current: if maybe-dirty (CHECK), refresh memo sources first; if
  // a source recomputed to a new value it will have marked us DIRTY.
  function updateIfNecessary(node) {
    if (node.state === CHECK && node.sources) {
      node.sources.forEach(function (s) { if (s.fn) updateIfNecessary(s); });
    }
    if (node.state === DIRTY) update(node);
    node.state = CLEAN;
  }

  function update(node) {
    if (node.disposed) return;
    cleanNode(node);
    var prevListener = Listener, prevOwner = Owner;
    Listener = node;
    Owner = node;
    try {
      var next = node.fn(node.value);
      if (node.effect) {
        node.value = next; // effects keep their last return for the next call
      } else {
        var eq = node.equals; // memo: propagate only if the value actually changed
        if (eq === false || !eq(node.value, next)) {
          node.value = next;
          if (node.observers) node.observers.forEach(function (o) { o.state = DIRTY; });
        }
      }
    } finally {
      Listener = prevListener;
      Owner = prevOwner;
    }
  }

  // Detach from sources, dispose owned children, run cleanups (in that order).
  function cleanNode(node) {
    if (node.sources) {
      node.sources.forEach(function (s) { if (s.observers) s.observers.delete(node); });
      node.sources.clear();
    }
    if (node.owned) {
      var owned = node.owned;
      node.owned = null;
      for (var i = 0; i < owned.length; i++) disposeOwner(owned[i]);
    }
    if (node.cleanups) {
      var cl = node.cleanups;
      node.cleanups = null;
      for (var j = 0; j < cl.length; j++) cl[j]();
    }
  }

  function disposeOwner(node) {
    node.disposed = true;
    cleanNode(node);
    node.state = CLEAN;
  }

  // ---- Scheduler (synchronous) ----
  function runUpdates(fn) {
    if (Effects) return fn(); // reentrant: an outer cycle owns the flush
    Effects = [];
    try {
      var result = fn();
      runEffects(Effects); // flush while Effects is still live so cascades enqueue here
      return result;
    } finally {
      Effects = null;
    }
  }

  function runEffects(queue) {
    for (var i = 0; i < queue.length; i++) {
      if (i > 1000000) throw new Error("supersolid: effect loop exceeded 1e6 iterations");
      var node = queue[i];
      if (!node.disposed && node.state !== CLEAN) updateIfNecessary(node);
    }
  }

  function batch(fn) { return runUpdates(fn); }

  function untrack(fn) {
    var prev = Listener;
    Listener = null;
    try { return fn(); } finally { Listener = prev; }
  }

  // ---- Public primitives ----
  function createSignal(value, options) {
    var node = {
      value: value,
      fn: null,
      observers: null,
      equals: options && "equals" in options ? options.equals : Object.is,
    };
    function read() { return readSource(node); }
    function write(next) {
      if (typeof next === "function") next = next(node.value);
      return writeSource(node, next);
    }
    return [read, write];
  }

  function createEffect(fn, value) {
    var node = createComputation(fn, value, true, undefined);
    runUpdates(function () { Effects.push(node); });
  }

  // ---- Publish author API (the transpiler strips the matching imports) ----
  var api = {
    createSignal: createSignal,
    createEffect: createEffect,
    untrack: untrack,
    batch: batch,
  };
  for (var name in api) globalThis[name] = api[name];
})();
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p supersolid_runtime`
Expected: PASS (all six Task-1 tests).

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml crates/supersolid_runtime/
git commit -m "feat(supersolid_runtime): reactive-graph core — signals, effects, scheduler

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 2: `createMemo` — lazy, memoized, glitch-free

Add derived reactive values. `createMemo` builds a pure computation that computes lazily on first read, memoizes (recomputes only when a dependency changed), and is itself a source that gates downstream. The graph engine from Task 1 already implements the memo path (`readSource` pull-through, `update`'s memo branch); this task adds the `createMemo` constructor and the tests that lock in laziness, memoization, downstream gating, and the diamond/glitch-free guarantee.

**Files:**
- Modify: `crates/supersolid_runtime/src/runtime.js` (add `createMemo`; publish it)
- Test: `crates/supersolid_runtime/src/lib.rs` tests

**Interfaces:**
- Consumes: `createComputation`, `readSource` (Task 1 internals).
- Produces: `globalThis.createMemo(fn, seed?, options?)` → a read getter (subscribes callers; lazy; memoized by `options.equals`, default `Object.is`).

- [ ] **Step 1: Write the failing tests**

Add to `crates/supersolid_runtime/src/lib.rs` tests:

```rust
#[test]
fn memo_is_lazy_then_memoized() {
    let mut e = engine();
    e.eval(
        r#"
        globalThis.memoRuns = 0;
        var x = createSignal(10);
        var m = createMemo(function () { globalThis.memoRuns++; return x[0]() * 2; });
        globalThis.beforeRead = globalThis.memoRuns;   // 0 — lazy, not computed yet
        globalThis.v1 = m();                           // 20 — computes now
        globalThis.afterRead = globalThis.memoRuns;    // 1
        globalThis.v2 = m();                           // 20 — cached
        globalThis.afterRead2 = globalThis.memoRuns;   // 1 — memoized, no recompute
        "#,
    )
    .unwrap();
    assert_eq!(num(&mut e, "globalThis.beforeRead"), 0.0);
    assert_eq!(num(&mut e, "globalThis.v1"), 20.0);
    assert_eq!(num(&mut e, "globalThis.afterRead"), 1.0);
    assert_eq!(num(&mut e, "globalThis.v2"), 20.0);
    assert_eq!(num(&mut e, "globalThis.afterRead2"), 1.0);
}

#[test]
fn memo_value_equality_gates_downstream_effects() {
    let mut e = engine();
    e.eval(
        r#"
        globalThis.effRuns = 0;
        var n = createSignal(4);
        var even = createMemo(function () { return n[0]() % 2 === 0; });
        createEffect(function () { globalThis.effRuns++; even(); });
        globalThis.e0 = globalThis.effRuns;   // 1
        n[1](6);                              // even stays true -> no downstream re-run
        globalThis.e1 = globalThis.effRuns;   // 1
        n[1](7);                              // even flips to false -> re-run
        globalThis.e2 = globalThis.effRuns;   // 2
        "#,
    )
    .unwrap();
    assert_eq!(num(&mut e, "globalThis.e0"), 1.0);
    assert_eq!(num(&mut e, "globalThis.e1"), 1.0);
    assert_eq!(num(&mut e, "globalThis.e2"), 2.0);
}

#[test]
fn diamond_dependency_reruns_effect_exactly_once() {
    let mut e = engine();
    e.eval(
        r#"
        globalThis.dRuns = 0;
        var a = createSignal(1);
        var b = createMemo(function () { return a[0]() * 2; });
        var c = createMemo(function () { return a[0]() + 1; });
        createEffect(function () { globalThis.dRuns++; return b() + c(); });
        globalThis.after1 = globalThis.dRuns;   // 1
        a[1](2);                                // one change to A ...
        globalThis.after2 = globalThis.dRuns;   // ... D runs once, not twice
        "#,
    )
    .unwrap();
    assert_eq!(num(&mut e, "globalThis.after1"), 1.0);
    assert_eq!(num(&mut e, "globalThis.after2"), 2.0);
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p supersolid_runtime memo_ diamond_`
Expected: FAIL — `createMemo is not a function`.

- [ ] **Step 3: Implement `createMemo`**

In `crates/supersolid_runtime/src/runtime.js`, add this function just above the `// ---- Publish author API` block:

```js
  function createMemo(fn, value, options) {
    // A pure computation: lazy (runs on first read via readSource), memoized by
    // `equals`, and itself a source for downstream reads.
    var node = createComputation(fn, value, false, options);
    return function () { return readSource(node); };
  }
```

Then add `createMemo` to the published `api` object:

```js
  var api = {
    createSignal: createSignal,
    createEffect: createEffect,
    createMemo: createMemo,
    untrack: untrack,
    batch: batch,
  };
```

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test -p supersolid_runtime`
Expected: PASS (all prior + three new).

- [ ] **Step 5: Commit**

```bash
git add crates/supersolid_runtime/src/
git commit -m "feat(supersolid_runtime): createMemo — lazy, memoized, glitch-free

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 3: Ownership — `createRoot`, `onCleanup`, disposal

Add the owner-tree surface: `createRoot(fn => …)` to establish a disposable scope and `onCleanup(fn)` to register teardown on the current owner. The engine already disposes owned children and runs cleanups in `cleanNode`/`disposeOwner` (Task 1); this task adds the two public constructors and locks in cleanup-on-rerun, cleanup-on-dispose, and dispose-stops-effects.

**Files:**
- Modify: `crates/supersolid_runtime/src/runtime.js` (add `createRoot`, `onCleanup`; publish them)
- Test: `crates/supersolid_runtime/src/lib.rs` tests

**Interfaces:**
- Consumes: `Owner`, `Listener`, `runUpdates`, `disposeOwner` (Task 1 internals).
- Produces: `globalThis.createRoot(fn)` — calls `fn(dispose)` under a fresh non-tracking owner, returns `fn`'s result; `dispose()` tears the scope down. `globalThis.onCleanup(fn)` — registers `fn` on the current owner (no-op if none); returns `fn`.

- [ ] **Step 1: Write the failing tests**

Add to `crates/supersolid_runtime/src/lib.rs` tests:

```rust
#[test]
fn on_cleanup_runs_before_each_effect_rerun() {
    let mut e = engine();
    e.eval(
        r#"
        globalThis.cleanups = 0;
        var a = createSignal(0);
        createEffect(function () {
            a[0]();
            onCleanup(function () { globalThis.cleanups++; });
        });
        globalThis.c0 = globalThis.cleanups;   // 0 — nothing to clean before first re-run
        a[1](1);
        globalThis.c1 = globalThis.cleanups;   // 1 — prior run's cleanup fired
        a[1](2);
        globalThis.c2 = globalThis.cleanups;   // 2
        "#,
    )
    .unwrap();
    assert_eq!(num(&mut e, "globalThis.c0"), 0.0);
    assert_eq!(num(&mut e, "globalThis.c1"), 1.0);
    assert_eq!(num(&mut e, "globalThis.c2"), 2.0);
}

#[test]
fn create_root_dispose_runs_cleanups_and_stops_effects() {
    let mut e = engine();
    e.eval(
        r#"
        globalThis.rootRuns = 0; globalThis.rootCleanups = 0;
        createRoot(function (dispose) {
            var x = createSignal(0);
            globalThis.setInner = x[1];
            globalThis.disposeRoot = dispose;
            createEffect(function () {
                globalThis.rootRuns++;
                x[0]();
                onCleanup(function () { globalThis.rootCleanups++; });
            });
        });
        globalThis.r0 = globalThis.rootRuns;         // 1
        globalThis.setInner(1);
        globalThis.r1 = globalThis.rootRuns;         // 2
        globalThis.disposeRoot();                    // tear down
        globalThis.rc = globalThis.rootCleanups;     // 2 (re-run cleanup + dispose cleanup)
        globalThis.setInner(2);                      // disposed -> effect must not run
        globalThis.r2 = globalThis.rootRuns;         // still 2
        "#,
    )
    .unwrap();
    assert_eq!(num(&mut e, "globalThis.r0"), 1.0);
    assert_eq!(num(&mut e, "globalThis.r1"), 2.0);
    assert_eq!(num(&mut e, "globalThis.rc"), 2.0);
    assert_eq!(num(&mut e, "globalThis.r2"), 2.0);
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p supersolid_runtime on_cleanup_ create_root_`
Expected: FAIL — `createRoot`/`onCleanup` are not functions.

- [ ] **Step 3: Implement `createRoot` and `onCleanup`**

In `crates/supersolid_runtime/src/runtime.js`, add both just above the `// ---- Publish author API` block:

```js
  function createRoot(fn) {
    // A disposable, non-tracking owner. Children (effects/memos) created inside
    // attach to it and are torn down together by `dispose`.
    var root = {
      fn: null, owned: null, cleanups: null, sources: null,
      context: Owner ? Owner.context : null, owner: Owner,
      disposed: false, state: CLEAN,
    };
    if (Owner) (Owner.owned || (Owner.owned = [])).push(root);
    var prevOwner = Owner, prevListener = Listener;
    Owner = root;
    Listener = null;
    try {
      return runUpdates(function () {
        return fn(function () { disposeOwner(root); });
      });
    } finally {
      Owner = prevOwner;
      Listener = prevListener;
    }
  }

  function onCleanup(fn) {
    if (Owner) (Owner.cleanups || (Owner.cleanups = [])).push(fn);
    return fn;
  }
```

Then add both to the published `api` object:

```js
  var api = {
    createSignal: createSignal,
    createEffect: createEffect,
    createMemo: createMemo,
    createRoot: createRoot,
    onCleanup: onCleanup,
    untrack: untrack,
    batch: batch,
  };
```

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test -p supersolid_runtime`
Expected: PASS (all prior + two new).

- [ ] **Step 5: Commit**

```bash
git add crates/supersolid_runtime/src/
git commit -m "feat(supersolid_runtime): ownership — createRoot, onCleanup, disposal

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 4: Lifecycle + context — `onMount`, `createContext`, `useContext`, `$ssProvideContext`

Add `onMount` (run-once-after-setup) and the context primitives. `onMount` is a one-shot untracked effect. Context rides the owner tree: `createContext` mints an id + default, `useContext` reads the nearest provided value from the current owner's context map (else the default), and `$ssProvideContext` runs a body with the context set for its dynamic extent (Plan 4's `<Provider>` will wrap this internal helper).

**Files:**
- Modify: `crates/supersolid_runtime/src/runtime.js` (add `onMount`, `createContext`, `useContext`, `provideContext`; publish `onMount`/`createContext`/`useContext` + `$ssProvideContext`)
- Test: `crates/supersolid_runtime/src/lib.rs` tests

**Interfaces:**
- Consumes: `createEffect`, `untrack`, `Owner`, `nextContextId` (Task 1 internals).
- Produces:
  - `globalThis.onMount(fn)` — runs `fn` once, untracked, during the current flush.
  - `globalThis.createContext(default?)` → `{ id, defaultValue }`.
  - `globalThis.useContext(ctx)` → provided value or `ctx.defaultValue`.
  - `globalThis.$ssProvideContext(ctx, value, fn)` — runs `fn` with `ctx` provided; returns `fn`'s result.

- [ ] **Step 1: Write the failing tests**

Add to `crates/supersolid_runtime/src/lib.rs` tests. Add a string read-back helper next to `num` (once, at the top of the tests module — if a later merge complains it already exists, keep the single copy):

```rust
/// Evaluate `expr` and read it back as a Rust String.
fn text(e: &mut BoaEngine, expr: &str) -> String {
    let v = e
        .context_mut()
        .eval(boa_engine::Source::from_bytes(expr))
        .unwrap();
    v.to_string(e.context_mut()).unwrap().to_std_string_escaped()
}

#[test]
fn on_mount_runs_exactly_once() {
    let mut e = engine();
    e.eval(
        r#"
        globalThis.mounts = 0;
        var m = createSignal(0);
        createRoot(function () {
            onMount(function () { globalThis.mounts++; m[0](); }); // reads m, but untracked
        });
        globalThis.m0 = globalThis.mounts;   // 1
        m[1](1);                             // one-shot + untracked -> no re-run
        globalThis.m1 = globalThis.mounts;   // 1
        "#,
    )
    .unwrap();
    assert_eq!(num(&mut e, "globalThis.m0"), 1.0);
    assert_eq!(num(&mut e, "globalThis.m1"), 1.0);
}

#[test]
fn context_default_provided_and_nested() {
    let mut e = engine();
    e.eval(
        r#"
        var Ctx = createContext("default");
        globalThis.d0 = useContext(Ctx);     // "default" (no provider)
        globalThis.prov = null; globalThis.nested = null; globalThis.effCtx = null;
        $ssProvideContext(Ctx, "outer", function () {
            globalThis.prov = useContext(Ctx);          // "outer"
            $ssProvideContext(Ctx, "inner", function () {
                globalThis.nested = useContext(Ctx);    // "inner"
            });
            createRoot(function () {
                createEffect(function () { globalThis.effCtx = useContext(Ctx); }); // "outer"
            });
        });
        "#,
    )
    .unwrap();
    assert_eq!(text(&mut e, "globalThis.d0"), "default");
    assert_eq!(text(&mut e, "globalThis.prov"), "outer");
    assert_eq!(text(&mut e, "globalThis.nested"), "inner");
    assert_eq!(text(&mut e, "globalThis.effCtx"), "outer");
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p supersolid_runtime on_mount_ context_`
Expected: FAIL — `onMount`/`createContext`/`useContext`/`$ssProvideContext` are not functions.

- [ ] **Step 3: Implement lifecycle + context**

In `crates/supersolid_runtime/src/runtime.js`, add these just above the `// ---- Publish author API` block:

```js
  function onMount(fn) {
    // Run once after the current setup flush, untracked (no dependencies).
    createEffect(function () { untrack(fn); });
  }

  function createContext(defaultValue) {
    return { id: nextContextId++, defaultValue: defaultValue };
  }

  function useContext(context) {
    var ctx = Owner && Owner.context;
    if (ctx && Object.prototype.hasOwnProperty.call(ctx, context.id)) {
      return ctx[context.id];
    }
    return context.defaultValue;
  }

  function provideContext(context, value, fn) {
    // Run `fn` under a child owner whose context has `context` set to `value`.
    // The map is copied down so nested reads and captured computations see it.
    var prevOwner = Owner;
    var merged = {};
    if (prevOwner && prevOwner.context) {
      for (var k in prevOwner.context) {
        if (Object.prototype.hasOwnProperty.call(prevOwner.context, k)) {
          merged[k] = prevOwner.context[k];
        }
      }
    }
    merged[context.id] = value;
    var owner = {
      fn: null, owned: null, cleanups: null, sources: null,
      context: merged, owner: prevOwner, disposed: false, state: CLEAN,
    };
    if (prevOwner) (prevOwner.owned || (prevOwner.owned = [])).push(owner);
    Owner = owner;
    try { return fn(); } finally { Owner = prevOwner; }
  }
```

Then extend the publish block (add the three author globals to `api`, plus the internal helper on `globalThis`):

```js
  var api = {
    createSignal: createSignal,
    createEffect: createEffect,
    createMemo: createMemo,
    createRoot: createRoot,
    onMount: onMount,
    onCleanup: onCleanup,
    createContext: createContext,
    useContext: useContext,
    untrack: untrack,
    batch: batch,
  };
  for (var name in api) globalThis[name] = api[name];
  // Runtime-internal context-provision primitive; Plan 4's <Provider> wraps it.
  globalThis.$ssProvideContext = provideContext;
```

(Replace the existing `var api = {…}; for (var name …)` block with the above — do not leave two publish blocks.)

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test -p supersolid_runtime`
Expected: PASS (all prior + two new). This is the full headless reactive core.

- [ ] **Step 5: Commit**

```bash
git add crates/supersolid_runtime/src/
git commit -m "feat(supersolid_runtime): lifecycle + context — onMount, createContext/useContext

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 5: Wire the runtime into the live `UiRuntime` + capability ledger

Install the reactive core into every mounted UI so author scripts (and Plan 2's transpiled `.tsx`) actually get the globals. The wiring is a single `install` call in `UiRuntime::new`, gated by nothing (the crate is wasm-clean, so it is correct on native and wasm alike). Add a bridge-level test proving the globals work end-to-end through `run_script`, and record the capability in the ledger.

**Files:**
- Modify: `crates/superui_bridge/Cargo.toml` (add `supersolid_runtime` dep)
- Modify: `crates/superui_bridge/src/runtime.rs` (call `supersolid_runtime::install` in `UiRuntime::new`; add a test)
- Modify: `docs/support/js-dom.md` (ledger section)

**Interfaces:**
- Consumes: `supersolid_runtime::install(&mut BoaEngine)` (Task 1); existing `UiRuntime::new`, `UiRuntime::run_script`, `UiRuntime::engine`.
- Produces: no new public Rust surface — the reactive globals are now present in the engine of every `UiRuntime`.

- [ ] **Step 1: Add the dependency**

In `crates/superui_bridge/Cargo.toml`, add to `[dependencies]` (after the other `superui_*` path deps):

```toml
supersolid_runtime = { path = "../supersolid_runtime" }
```

- [ ] **Step 2: Write the failing test**

Add to the `#[cfg(test)] mod tests` block in `crates/superui_bridge/src/runtime.rs`:

```rust
#[test]
fn supersolid_runtime_globals_are_available_in_the_ui_runtime() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<div id='a'></div>",
    )));
    let mut rt = UiRuntime::new(dom, Entity::PLACEHOLDER, Handle::default());
    // The reactive globals the Plan 2 transpiler emits imports for must resolve.
    rt.run_script(
        r#"
        var n = createSignal(1);
        globalThis.captured = 0;
        createEffect(function () { globalThis.captured = n[0](); });
        n[1](42);
        "#,
    );
    let got = rt
        .engine
        .context_mut()
        .eval(boa_engine::Source::from_bytes("globalThis.captured"))
        .unwrap()
        .as_number()
        .unwrap();
    assert_eq!(got, 42.0);
}
```

> **Note:** `superui_html` is already a `dev-dependency` of `superui_bridge` and `boa_engine` a normal dependency, so no manifest change beyond Step 1 is needed for this test.

- [ ] **Step 3: Run to verify it fails**

Run: `cargo test -p superui_bridge supersolid_runtime_globals_are_available_in_the_ui_runtime`
Expected: FAIL — `createSignal is not defined` (the runtime isn't installed yet), surfaced as a swallowed JS error, leaving `globalThis.captured` at `0` (or the eval of `captured` reads `0`), so the assertion `== 42.0` fails.

- [ ] **Step 4: Install the runtime in `UiRuntime::new`**

In `crates/superui_bridge/src/runtime.rs`, in `UiRuntime::new`, add the install call after `superui_api::install` and before `install_bevy_bridge`:

```rust
        let mut engine = BoaEngine::new(dom.clone());
        superui_api::install(&mut engine);
        supersolid_runtime::install(&mut engine);
        crate::bevy_bridge::install_bevy_bridge(&mut engine);
```

- [ ] **Step 5: Run to verify it passes + no regressions**

Run: `cargo test -p superui_bridge`
Expected: PASS (new test + all existing bridge tests, including `new_runtime_is_dirty_and_runs_script`).

- [ ] **Step 6: Update the capability ledger**

In `docs/support/js-dom.md`, append a new top-level section at the end of the file:

```markdown
## Supersolid runtime (framework globals)

Provided by `supersolid_runtime` (installed into every `UiRuntime`), not part of the
browser DOM/Web API surface. Available to author `.js` and to Plan 2's transpiled
`.tsx` (whose `import { … } from "solid-js"` is stripped in favour of these globals).

| Global | Status | Since | Notes |
|---|---|---|---|
| `createSignal(v, {equals?})` → `[get, set]` | ✅ | T0 | fine-grained signal; updater-form set; `equals` gates notifications |
| `createEffect(fn, seed?)` | ✅ | T0 | tracks reads, re-runs on change; disposed with owner |
| `createMemo(fn, seed?, {equals?})` | ✅ | T0 | lazy, memoized derived value; is itself a source |
| `createRoot(fn => …)` | ✅ | T0 | disposable reactive scope |
| `onMount(fn)` / `onCleanup(fn)` | ✅ | T0 | run-once-after-setup / owner teardown |
| `createContext(default?)` / `useContext(ctx)` | ✅ | T0 | context via the owner tree |
| `untrack(fn)` / `batch(fn)` | ✅ | T0 | read without subscribing / coalesce writes |
```

- [ ] **Step 7: Verify the whole workspace and commit**

Run: `cargo test -p supersolid_runtime && cargo test -p superui_bridge`
Expected: PASS.

Optional (if the wasm target is installed): `cargo build -p supersolid_runtime --target wasm32-unknown-unknown` — builds (the crate is wasm-clean; no `oxc`).

```bash
git add crates/superui_bridge/Cargo.toml crates/superui_bridge/src/runtime.rs docs/support/js-dom.md
git commit -m "feat(superui_bridge): install supersolid_runtime reactive core into UiRuntime

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Done-when

- `cargo test -p supersolid_runtime` and `cargo test -p superui_bridge` both green.
- `supersolid_runtime::install` publishes the Solid-style reactive globals in Boa: `createSignal`/`createEffect`/`createMemo`/`onMount`/`onCleanup`/`createContext`/`useContext` (+ `createRoot`/`untrack`/`batch` and internal `$ssProvideContext`), backed by a glitch-free mark-and-sweep graph with lazy memos and a synchronous scheduler.
- The reactive-graph contract holds under test: signal notify/equality/updater; effect track/re-run/isolation; memo laziness/memoization/downstream-gating; diamond runs-once; ownership cleanup on re-run and dispose; dispose stops effects; onMount-once; context default/provided/nested/captured; batch coalescing; untrack isolation.
- The runtime is installed into every `UiRuntime`, so author `.js`/transpiled `.tsx` can use the primitives (proven by the bridge end-to-end test).
- `supersolid_runtime` carries **no** Bevy or `oxc` dependency and cross-compiles for `wasm32-unknown-unknown`.
- `docs/support/js-dom.md` records the framework globals.
- **Out of scope (Plan 4):** the `$ss.*` render helpers and control-flow components (`Show`/`For`/`Index`/`Switch`/`Match`), which compose on this core; state-preserving HMR cell rehydration (Plan 5), which will inspect this graph's signal cells.

## Self-review (author)

- **Spec coverage:** implements direction-spec §5's reactivity primitives (`createSignal`/`createEffect`/`createMemo`/`onMount`/`onCleanup`/`createContext`/`useContext`) as the single fine-grained runtime "living in Boa" (§5), fine-grained-and-only-fine-grained per §4 (glitch-free mark-and-sweep, lazy memos, near-zero allocation — the interpreter-friendly property §4 argues for), wasm-clean so it satisfies §6's "Boa on every target / permanent interpreter-only wasm" (the crate carries no `oxc`, unlike `supersolid`). The getter-you-call semantic cost (§4) is inherent to `createSignal` returning `[read, write]`. Deferred with rationale: `$ss.*` render + control-flow (§5) → Plan 4, and it composes on `createEffect`/`createRoot`/`untrack` (design check in the header); HMR cell rehydration (§11.2) → Plan 5, served by keeping the whole graph as inspectable JS signal cells; React-ism lints (§11.1) → the transpiler's later plan.
- **No placeholders:** every task ships concrete failing tests (the contracts) and the real `runtime.js`/`install`/wiring code. `runtime.js` is authored in full in Task 1 and *extended by named function additions* in Tasks 2–4 (each shows the exact function and the exact publish-block edit), so a reader jumping to Task 3 sees the complete `createRoot`/`onCleanup` code, not a back-reference. The one non-obvious runtime behaviour — that the synchronous scheduler flushes effects before returning, so no Rust pump is needed — is stated in the Architecture and enforced by the batch/dispose tests.
- **Type consistency:** the internal engine names (`createComputation`, `readSource`, `writeSource`, `stale`, `updateIfNecessary`, `update`, `cleanNode`, `disposeOwner`, `runUpdates`, `runEffects`, and the `Listener`/`Owner`/`Effects` module globals, node fields `state`/`sources`/`observers`/`owned`/`cleanups`/`context`/`equals`/`disposed`) are introduced in Task 1 and used unchanged in Tasks 2–4. The published author globals match the ABI the Plan 2 memo records (`createSignal`, `createEffect`, `createMemo`, `onMount`, `onCleanup`, `createContext`, `useContext`) exactly. `install(&mut BoaEngine)` matches the sibling `superui_api::install` shape and is called identically in `UiRuntime::new`.
```
