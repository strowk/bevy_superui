# Supersolid Phase 2 — Plan 5: state-preserving hot reload — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make a `.tsx` hot reload preserve live signal values — signal-cell rehydration keyed by `module × instance × creation-order`, with a per-instance remount-on-shape-change fallback and `<For>`/`<Index>` per-row preservation — gated by a `superui/hmr` feature plus the asset watcher.

**Architecture:** All reload behavior is realized **inside `render()`** on its second invocation for the same mount node (the same-engine re-exec that `apply_hot_reload` already performs). `render.js` keeps a `roots` map keyed by the mount node; a repeat call snapshots each instance's signal cells, disposes the old reactive scope, rebuilds the DOM fresh, and rehydrates matched cells at each **instance-frame close**. Cells are collected via a single `runtime.js` hook (`$ssOnSignal`) fired at `createSignal`. Components get stable ids from a transpiler-baked `$ss.hot("<path>#<Name>", fn)`. The whole machinery is inert unless `globalThis.__ssHmr` is set, which `UiRuntime::new` does only when the `superui/hmr` feature is on **and** the `AssetServer` is watching.

**Tech Stack:** JS (hand-written, run in Boa 0.21) in `supersolid_runtime`; Rust (edition 2021) in `supersolid` (oxc transpiler), `superui` (loader + plugin + gating), `superui_bridge` (`UiRuntime`). Bevy 0.17.

Design spec: [`../specs/2026-07-20-supersolid-hmr-state-preservation-design.md`](../specs/2026-07-20-supersolid-hmr-state-preservation-design.md).

## Global Constraints

- **Bevy 0.17**, edition 2021.
- `supersolid_runtime` stays **Bevy-free and wasm-clean** (pure JS + thin Boa install; no `oxc`, no Bevy). `render.js`/`runtime.js` call only the Phase-1 DOM API and the Plan 3/4 globals.
- `supersolid`'s transpiler core stays **Bevy-free**; its `oxc` dependency never enters wasm (loader is target-gated in `superui`, spec §11.3).
- **TDD** throughout; **frequent commits**; work on `main` (project CLAUDE.md — no feature branch). Do **not** touch `.superpowers/sdd` (another session uses it).
- **Graceful degradation:** author-script errors already log-and-swallow through `UiRuntime::run_script`; `render.js`/`runtime.js` are vetted by these tests, so their own eval failing stays a hard bug (as today).
- **Node identity is stable:** `superui_js::wrap_node` caches one JS wrapper per `NodeId`, so `===` and `Map` keys on DOM nodes work. HMR keys `roots` by the mount node this way.
- **Gating invariant:** HMR is active **iff** `cfg!(feature = "hmr")` (on `superui`) **and** `AssetServer::watching_for_changes()`. Default builds (feature off) compile the flag-set out; wasm never watches. The instrumentation is construction-time only — the steady-state reactive/reconcile hot path is identical to Plan 4 in every build.

### Keys and the `$ss`/global surface this plan adds

- `runtime.js`: `createSignal` calls `globalThis.$ssOnSignal(read, write)` when it is a function (new).
- `render.js`: publishes `$ss.hot(id, fn)`; sets `globalThis.$ssOnSignal`; reads `globalThis.__ssHmr`. Internal module state: `roots` (Map mountNode→entry), `frameStack`, `currentRoot`.
- Instance key: component = `parentPath + "/" + id + "#" + ordinal` (or `+ ":" + key` when a `key` prop is present); `<For>` row = the item object (identity); `<Index>` row = `idxPath + "#i" + position`.
- `supersolid`: `TranspileOptions.module_id: Option<String>`; the JSX pass appends `$ss.hot("<module_id>#<Name>", <Name>)` right after each top-level component declaration.
- `superui`: `hmr` cargo feature; `UiRuntime::new` gains an `hmr: bool` arg.

---

## Task 1: `runtime.js` — the `$ssOnSignal` cell-collection hook

Fire an optional global hook at every `createSignal` so the render layer can collect cells. No-op when the hook is absent (all Plan 3/4 tests unaffected).

**Files:**
- Modify: `crates/supersolid_runtime/src/runtime.js` (`createSignal`)
- Test: `crates/supersolid_runtime/src/lib.rs` (`mod tests` — the reactive-core module)

**Interfaces:**
- Consumes: nothing new.
- Produces: `createSignal` invokes `globalThis.$ssOnSignal(read, write)` after building `read`/`write`, iff `typeof globalThis.$ssOnSignal === "function"`. `read`/`write` are the signal's getter/setter.

- [ ] **Step 1: Write the failing tests**

Add to `crates/supersolid_runtime/src/lib.rs` in `#[cfg(test)] mod tests`:

```rust
#[test]
fn on_signal_hook_receives_created_signals() {
    let mut e = engine();
    e.eval(
        r#"
        globalThis.collected = [];
        globalThis.$ssOnSignal = function (read, write) {
            globalThis.collected.push([read, write]);
        };
        var a = createSignal(1);
        var b = createSignal(2);
        globalThis.n = globalThis.collected.length;      // 2
        globalThis.v0 = globalThis.collected[0][0]();    // read a -> 1
        globalThis.collected[1][1](9);                   // write b -> 9
        globalThis.v1 = b[0]();                          // 9
        "#,
    )
    .unwrap();
    assert_eq!(num(&mut e, "globalThis.n"), 2.0);
    assert_eq!(num(&mut e, "globalThis.v0"), 1.0);
    assert_eq!(num(&mut e, "globalThis.v1"), 9.0);
}

#[test]
fn on_signal_hook_absent_is_a_noop() {
    let mut e = engine();
    e.eval(r#"var s = createSignal(5); globalThis.v = s[0]();"#).unwrap();
    assert_eq!(num(&mut e, "globalThis.v"), 5.0);
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p supersolid_runtime on_signal_hook`
Expected: `on_signal_hook_receives_created_signals` FAILS (`collected.length` is 0 — the hook is never called). `on_signal_hook_absent_is_a_noop` passes already.

- [ ] **Step 3: Add the hook in `createSignal`**

In `crates/supersolid_runtime/src/runtime.js`, in `createSignal`, replace:

```js
    function write(next) {
      if (typeof next === "function") next = next(node.value);
      return writeSource(node, next);
    }
    return [read, write];
  }
```

with:

```js
    function write(next) {
      if (typeof next === "function") next = next(node.value);
      return writeSource(node, next);
    }
    // Plan 5 HMR: let the render layer collect this cell (identity read/write).
    // Property access (not a bare identifier) so it is safe under "use strict".
    if (typeof globalThis.$ssOnSignal === "function") {
      globalThis.$ssOnSignal(read, write);
    }
    return [read, write];
  }
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p supersolid_runtime`
Expected: PASS (all reactive-core + render tests, plus the two new).

- [ ] **Step 5: Commit**

```bash
git add crates/supersolid_runtime/src/runtime.js crates/supersolid_runtime/src/lib.rs
git commit -m "feat(supersolid_runtime): \$ssOnSignal hook in createSignal (HMR cell collection)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 2: `render.js` — `$ss.hot` component-id tag

Add the `$ss.hot(id, fn)` helper the transpiler emits (tags a component function with a stable id). Pure tagging; no behavior yet.

**Files:**
- Modify: `crates/supersolid_runtime/src/render.js`
- Test: `crates/supersolid_runtime/src/lib.rs` (`render_tests`)

**Interfaces:**
- Produces: `$ss.hot(id, fn)` — sets `fn.__ssId = id` when `fn` is a function; returns `fn`.

- [ ] **Step 1: Write the failing test**

Add to `render_tests`:

```rust
#[test]
fn hot_tags_component_with_id() {
    let mut e = render_engine();
    e.eval(
        r#"
        function App() { return $ss.el("div"); }
        $ss.hot("app.tsx#App", App);
        globalThis.id = App.__ssId;                       // "app.tsx#App"
        globalThis.same = ($ss.hot("x#Y", App) === App);  // returns the fn
        "#,
    )
    .unwrap();
    assert_eq!(text(&mut e, "globalThis.id"), "app.tsx#App");
    assert_eq!(text(&mut e, "globalThis.same"), "true");
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p supersolid_runtime hot_tags_component`
Expected: FAIL — `$ss.hot` is `undefined`.

- [ ] **Step 3: Implement `hot`**

In `crates/supersolid_runtime/src/render.js`, add before the publish block (near `frag`):

```js
  // Plan 5 HMR: tag a component function with a stable, transpiler-supplied id
  // ("<assetpath>#<Name>"). Guarded so a non-function argument is a harmless
  // no-op (an uppercase non-component binding never breaks). Returns the arg.
  function hot(id, fn) {
    if (typeof fn === "function") fn.__ssId = id;
    return fn;
  }
```

Add `hot: hot,` to the published `$ss` object:

```js
  globalThis.$ss = {
    el: el,
    txt: txt,
    attr: attr,
    child: child,
    on: on,
    bind: bind,
    insert: insert,
    cmp: cmp,
    frag: frag,
    hot: hot,
  };
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p supersolid_runtime`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/supersolid_runtime/src/render.js crates/supersolid_runtime/src/lib.rs
git commit -m "feat(supersolid_runtime): \$ss.hot — tag component with a stable HMR id

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 3: `render.js` — HMR core for components (flag, frames, snapshot, rehydrate, `render()` reload path)

The heart of Plan 5. Add the gating flag, the instance-frame machinery, the `$ssOnSignal` collector, per-instance rehydration commit, and the `render()` reload path. `<For>`/`<Index>` rows come in Task 4.

**Files:**
- Modify: `crates/supersolid_runtime/src/render.js`
- Test: `crates/supersolid_runtime/src/lib.rs` (`render_tests`)

**Interfaces:**
- Consumes: `createRoot`, `untrack` (runtime.js); `insert` (Plan 4); `$ss.hot` tags (Task 2); `$ssOnSignal` hook (Task 1).
- Produces (module-internal): `roots` (Map), `frameStack`, `currentRoot`; `hmrOn()`; `onSignal(read, write)` published as `globalThis.$ssOnSignal`; `withInstance(id, key, run)`; `commitRehydration(frame)`; `snapshotCells(entry)`; `clearChildren(node)`. `cmp` and `render` gain HMR branches (Plan-4 behavior preserved when the flag is off).

- [ ] **Step 1: Write the failing tests**

Add to `render_tests`:

```rust
#[test]
fn hmr_preserves_component_signal_value() {
    let mut e = render_engine();
    e.eval(
        r#"
        globalThis.__ssHmr = true;
        globalThis.root = $ss.el("main");
        function makeApp() {
            function Counter() {
                var c = createSignal(0);
                globalThis.__c = c;
                var d = $ss.el("div");
                $ss.insert(d, function () { return c[0](); });
                return d;
            }
            Counter.__ssId = "app#Counter";
            return function () { return $ss.cmp(Counter, {}); };
        }
        render(makeApp(), root);
        globalThis.t0 = root.textContent;   // "0"
        globalThis.__c[1](5);
        globalThis.t1 = root.textContent;   // "5"
        render(makeApp(), root);            // hot reload: same mount node
        globalThis.t2 = root.textContent;   // "5" — preserved
        "#,
    )
    .unwrap();
    assert_eq!(text(&mut e, "globalThis.t0"), "0");
    assert_eq!(text(&mut e, "globalThis.t1"), "5");
    assert_eq!(text(&mut e, "globalThis.t2"), "5");
}

#[test]
fn hmr_resets_on_shape_change() {
    let mut e = render_engine();
    e.eval(
        r#"
        globalThis.__ssHmr = true;
        globalThis.root = $ss.el("main");
        function makeApp(twoCells) {
            function Counter() {
                if (twoCells) { createSignal(0); }   // extra leading cell -> shape change
                var c = createSignal(0);
                globalThis.__c = c;
                var d = $ss.el("div");
                $ss.insert(d, function () { return c[0](); });
                return d;
            }
            Counter.__ssId = "app#Counter";
            return function () { return $ss.cmp(Counter, {}); };
        }
        render(makeApp(false), root);
        globalThis.__c[1](5);
        globalThis.t1 = root.textContent;   // "5"
        render(makeApp(true), root);        // reload with a DIFFERENT signal count
        globalThis.t2 = root.textContent;   // "0" — shape changed -> reset
        "#,
    )
    .unwrap();
    assert_eq!(text(&mut e, "globalThis.t1"), "5");
    assert_eq!(text(&mut e, "globalThis.t2"), "0");
}

#[test]
fn hmr_keys_sibling_instances_separately() {
    let mut e = render_engine();
    e.eval(
        r#"
        globalThis.__ssHmr = true;
        globalThis.root = $ss.el("main");
        function makeApp() {
            globalThis.__cs = [];
            function Counter() {
                var c = createSignal(0);
                globalThis.__cs.push(c);
                var d = $ss.el("i");
                $ss.insert(d, function () { return c[0](); });
                return d;
            }
            Counter.__ssId = "app#Counter";
            function App() {
                var wrap = $ss.el("div");
                $ss.insert(wrap, function () { return $ss.cmp(Counter, {}); });
                $ss.insert(wrap, function () { return $ss.cmp(Counter, {}); });
                return wrap;
            }
            App.__ssId = "app#App";
            return function () { return $ss.cmp(App, {}); };
        }
        render(makeApp(), root);
        globalThis.__cs[0][1](7);           // first sibling -> 7
        globalThis.__cs[1][1](3);           // second sibling -> 3
        globalThis.t1 = root.textContent;   // "73"
        render(makeApp(), root);            // reload
        globalThis.t2 = root.textContent;   // "73" — each sibling kept its own value
        "#,
    )
    .unwrap();
    assert_eq!(text(&mut e, "globalThis.t1"), "73");
    assert_eq!(text(&mut e, "globalThis.t2"), "73");
}

#[test]
fn hmr_off_does_not_preserve() {
    let mut e = render_engine();
    e.eval(
        r#"
        globalThis.__ssHmr = false;         // gate OFF
        globalThis.root = $ss.el("main");
        function makeApp() {
            function Counter() {
                var c = createSignal(0);
                globalThis.__c = c;
                var d = $ss.el("div");
                $ss.insert(d, function () { return c[0](); });
                return d;
            }
            Counter.__ssId = "app#Counter";
            return function () { return $ss.cmp(Counter, {}); };
        }
        render(makeApp(), root);
        globalThis.__c[1](5);               // bump the first render's signal
        render(makeApp(), root);            // second render (gate off)
        globalThis.v = globalThis.__c[0](); // the SECOND render's fresh signal -> 0
        "#,
    )
    .unwrap();
    assert_eq!(num(&mut e, "globalThis.v"), 0.0);
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p supersolid_runtime hmr_`
Expected: `hmr_preserves_component_signal_value`, `hmr_resets_on_shape_change`, `hmr_keys_sibling_instances_separately` FAIL (no rehydration — `t2` reflects fresh defaults / duplicated trees). `hmr_off_does_not_preserve` may already pass (Plan-4 `render` appends a second fresh tree), but keep it — it locks the gate-off contract.

- [ ] **Step 3: Add the HMR module state + collector**

In `crates/supersolid_runtime/src/render.js`, right after `var PROPERTY_NAMES = {...};` near the top of the IIFE, add:

```js
  // ---- Plan 5: state-preserving HMR (all inert unless globalThis.__ssHmr) ----
  var roots = new Map();   // mountNode -> { dispose, instances:Map, snapshot:Map|null, ordinals:{} }
  var frameStack = [];     // active instance frames during a (re)render
  var currentRoot = null;  // the root entry currently mounted / being rebuilt

  function hmrOn() { return !!globalThis.__ssHmr; }

  // Collector for the runtime.js $ssOnSignal hook: push {read, write} onto the
  // current instance frame. No frame active => no-op (module-top-level signals,
  // plain scripts, and the gate-off path are all unaffected).
  function onSignal(read, write) {
    var f = frameStack[frameStack.length - 1];
    if (f) f.cells.push({ read: read, write: write });
  }

  // Open an instance frame keyed by tree position (+ explicit key), run the
  // component body, then commit any matched rehydration at frame close (which is
  // also where a signal-count mismatch cleanly resets the instance).
  function withInstance(id, key, run) {
    var parent = frameStack[frameStack.length - 1];
    var parentPath = parent ? parent.path : "";
    var instKey;
    if (key != null) {
      instKey = parentPath + "/" + id + ":" + key;
    } else {
      var ords = parent ? parent.ordinals : currentRoot.ordinals;
      var n = ords[id] || 0;
      ords[id] = n + 1;
      instKey = parentPath + "/" + id + "#" + n;
    }
    var frame = { path: instKey, cells: [], ordinals: {} };
    currentRoot.instances.set(instKey, frame);
    frameStack.push(frame);
    try {
      return run();
    } finally {
      frameStack.pop();
      commitRehydration(frame);
    }
  }

  // Rehydrate a just-built frame from the snapshot IFF the signal shape matches.
  // Count mismatch (add/remove) => skip => the fresh defaults stand = reset.
  function commitRehydration(frame) {
    if (!currentRoot || !currentRoot.snapshot) return;
    var saved = currentRoot.snapshot.get(frame.path);
    if (saved && saved.length === frame.cells.length) {
      for (var i = 0; i < saved.length; i++) {
        // updater form sets the value even if it is itself a function.
        (function (v, cell) { cell.write(function () { return v; }); })(saved[i], frame.cells[i]);
      }
    }
  }

  // Snapshot every tracked cell's current value (untracked reads).
  function snapshotCells(entry) {
    var snap = new Map();
    entry.instances.forEach(function (frame, key) {
      var vals = new Array(frame.cells.length);
      for (var i = 0; i < frame.cells.length; i++) {
        vals[i] = untrack(frame.cells[i].read);
      }
      snap.set(key, vals);
    });
    return snap;
  }

  // Remove all children of a node (rebuild-fresh on reload). Descending index so
  // it is correct whether childNodes is live or a snapshot.
  function clearChildren(node) {
    var kids = node.childNodes;
    for (var i = kids.length - 1; i >= 0; i--) node.removeChild(kids[i]);
  }
```

> **Guidance:** `childNodes` supports `.length` and index access (the reconcile code and Plan-4 tests rely on this); `node.removeChild` exists in the DOM subset. `untrack(fn)` and `createRoot` are runtime.js globals already used elsewhere in this file.

- [ ] **Step 4: Gate `cmp` and rewrite `render` for the reload path**

In `crates/supersolid_runtime/src/render.js`, replace `cmp`:

```js
  function cmp(Comp, props) {
    // Components run ONCE, untracked (fine-grained model). props carries getters
    // for dynamic props (transpiler); dynamic bits become inner effects.
    return untrack(function () { return Comp(props); });
  }
```

with:

```js
  function cmp(Comp, props) {
    // Components run ONCE, untracked (fine-grained model). props carries getters
    // for dynamic props (transpiler); dynamic bits become inner effects.
    if (!hmrOn() || !currentRoot) {
      return untrack(function () { return Comp(props); });
    }
    var id = (Comp && Comp.__ssId) || (Comp && Comp.name) || "?";
    var key = props && props.key;
    return untrack(function () {
      return withInstance(id, key, function () { return Comp(props); });
    });
  }
```

Replace `render`:

```js
  function render(code, mountEl) {
    var dispose;
    createRoot(function (d) {
      dispose = d;
      insert(mountEl, code);
    });
    return dispose;
  }
```

with:

```js
  // Root entry. First call for a mount node = fresh mount. A REPEAT call for the
  // same node (same engine, same cached wrapper) = hot reload: snapshot the live
  // cells, dispose the old scope, rebuild the DOM fresh, rehydrate matched cells
  // at each instance-frame close. Gate off => exact Plan-4 behavior.
  function render(code, mountEl) {
    if (!hmrOn()) {
      var d0;
      createRoot(function (d) { d0 = d; insert(mountEl, code); });
      return d0;
    }
    var prev = roots.get(mountEl);
    var snapshot = null;
    if (prev) {
      snapshot = snapshotCells(prev);
      prev.dispose();
      clearChildren(mountEl);
    }
    var entry = { dispose: null, instances: new Map(), snapshot: snapshot, ordinals: {} };
    // currentRoot stays set after render() returns so components/rows created
    // later (Show flips, list growth) still attach to this root (one mounted UI
    // per engine — the bridge's model). A reload swaps in the new entry.
    currentRoot = entry;
    createRoot(function (d) { entry.dispose = d; insert(mountEl, code); });
    roots.set(mountEl, entry);
    return entry.dispose;
  }
```

Publish the collector — add to the publish block (after the `globalThis.Match = Match;` lines, before `globalThis.$ss = {`):

```js
  // Plan 5: render layer's cell collector for runtime.js's createSignal hook.
  globalThis.$ssOnSignal = onSignal;
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p supersolid_runtime`
Expected: PASS (all four `hmr_*` tests + every prior render/reactive test — the gate-off path leaves Plan-4 behavior intact).

- [ ] **Step 6: Commit**

```bash
git add crates/supersolid_runtime/src/render.js crates/supersolid_runtime/src/lib.rs
git commit -m "feat(supersolid_runtime): HMR core — frames, snapshot, frame-close rehydrate, render() reload

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 4: `render.js` — `<For>`/`<Index>` per-row state preservation

Wrap each row's `mapFn` in an instance frame so signals created inside a row are collected and rehydrated. `<For>` rows key by **item identity** (survives reorder); `<Index>` rows key by **position**.

**Files:**
- Modify: `crates/supersolid_runtime/src/render.js` (`mapArray`/`makeRow`, `indexArray`/`makeIndexRow`, add `withRowInstance`)
- Test: `crates/supersolid_runtime/src/lib.rs` (`render_tests`)

**Interfaces:**
- Consumes: `withInstance`'s siblings — adds `withRowInstance(rowKey, run)` (same open/collect/commit lifecycle, arbitrary key type).
- Produces: `makeRow` and `makeIndexRow` build their row's mapped nodes under a row instance frame when `hmrOn() && currentRoot`.

- [ ] **Step 1: Write the failing tests**

Add to `render_tests`:

```rust
#[test]
fn hmr_preserves_for_row_state_across_reorder() {
    let mut e = render_engine();
    e.eval(
        r#"
        globalThis.__ssHmr = true;
        globalThis.a = { n: "a" };
        globalThis.b = { n: "b" };
        globalThis.root = $ss.el("div");
        function build() {
            function App() {
                var list = createSignal([globalThis.a, globalThis.b]);
                globalThis.__list = list;
                var ul = $ss.el("ul");
                $ss.insert(ul, function () {
                    return $ss.cmp(For, {
                        get each() { return list[0](); },
                        get children() {
                            return function (item) {
                                var cnt = createSignal(0);
                                item.__cnt = cnt;            // expose row signal via the item
                                var li = $ss.el("li");
                                $ss.insert(li, function () { return item.n + ":" + cnt[0](); });
                                return li;
                            };
                        },
                    });
                });
                return ul;
            }
            App.__ssId = "app#App";
            return function () { return $ss.cmp(App, {}); };
        }
        render(build(), root);
        globalThis.t0 = root.textContent;   // "a:0b:0"
        globalThis.a.__cnt[1](7);
        globalThis.b.__cnt[1](3);
        globalThis.t1 = root.textContent;   // "a:7b:3"
        globalThis.__list[1]([globalThis.b, globalThis.a]);   // runtime reorder (Plan-4 For)
        globalThis.tR = root.textContent;   // "b:3a:7"
        render(build(), root);              // hot reload
        globalThis.t2 = root.textContent;   // "b:3a:7" — list + per-row state preserved
        "#,
    )
    .unwrap();
    assert_eq!(text(&mut e, "globalThis.t0"), "a:0b:0");
    assert_eq!(text(&mut e, "globalThis.t1"), "a:7b:3");
    assert_eq!(text(&mut e, "globalThis.tR"), "b:3a:7");
    assert_eq!(text(&mut e, "globalThis.t2"), "b:3a:7");
}

#[test]
fn hmr_preserves_index_row_state_by_position() {
    let mut e = render_engine();
    e.eval(
        r#"
        globalThis.__ssHmr = true;
        globalThis.root = $ss.el("div");
        function build() {
            function App() {
                var list = createSignal(["x", "y"]);
                var ul = $ss.el("ul");
                $ss.insert(ul, function () {
                    return $ss.cmp(Index, {
                        get each() { return list[0](); },
                        get children() {
                            return function (item) {              // item: signal getter
                                var tag = createSignal("");        // private per-position state
                                globalThis.__tags = globalThis.__tags || [];
                                globalThis.__tags.push(tag);
                                var li = $ss.el("li");
                                $ss.insert(li, function () { return item() + tag[0](); });
                                return li;
                            };
                        },
                    });
                });
                return ul;
            }
            App.__ssId = "app#App";
            return function () { return $ss.cmp(App, {}); };
        }
        globalThis.__tags = [];
        render(build(), root);
        globalThis.t0 = root.textContent;   // "xy"
        globalThis.__tags[0][1]("!");        // position 0 private state
        globalThis.t1 = root.textContent;   // "x!y"
        globalThis.__tags = [];
        render(build(), root);              // hot reload
        globalThis.t2 = root.textContent;   // "x!y" — position 0 state preserved
        "#,
    )
    .unwrap();
    assert_eq!(text(&mut e, "globalThis.t0"), "xy");
    assert_eq!(text(&mut e, "globalThis.t1"), "x!y");
    assert_eq!(text(&mut e, "globalThis.t2"), "x!y");
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p supersolid_runtime hmr_preserves_for_row hmr_preserves_index_row`
Expected: FAIL — row signals are not collected yet, so `t2` shows the fresh defaults (`"a:0b:0"` / `"xy"`-with-reset).

- [ ] **Step 3: Add `withRowInstance` and wrap the row builders**

In `crates/supersolid_runtime/src/render.js`, add `withRowInstance` next to `withInstance`:

```js
  // Like withInstance but for <For>/<Index> rows. rowKey is the item object
  // (<For>, identity) or a position string (<Index>). Same collect + commit.
  function withRowInstance(rowKey, run) {
    var frame = { path: rowKey, cells: [], ordinals: {} };
    currentRoot.instances.set(rowKey, frame);
    frameStack.push(frame);
    try {
      return run();
    } finally {
      frameStack.pop();
      commitRehydration(frame);
    }
  }
```

Replace `makeRow`:

```js
  function makeRow(item, index, outMapped, outDisposers, owner, mapFn) {
    createRoot(function (dispose) {
      outDisposers[index] = dispose;
      outMapped[index] = mapFn(item, index);
    }, owner);
  }
```

with:

```js
  function makeRow(item, index, outMapped, outDisposers, owner, mapFn) {
    createRoot(function (dispose) {
      outDisposers[index] = dispose;
      if (hmrOn() && currentRoot) {
        // <For> row keyed by item identity (same identity survives reorder).
        outMapped[index] = withRowInstance(item, function () { return mapFn(item, index); });
      } else {
        outMapped[index] = mapFn(item, index);
      }
    }, owner);
  }
```

For `<Index>`, thread the enclosing list's frame path so positions are unique per list. In `indexArray`, capture it next to the `owner` capture — change:

```js
  function indexArray(listFn, mapFn) {
    var owner = globalThis.$ssGetOwner();
```

to:

```js
  function indexArray(listFn, mapFn) {
    var owner = globalThis.$ssGetOwner();
    // Capture the <Index> instance path NOW (while its frame is on the stack) so
    // per-position row keys are unique across multiple lists.
    var idxPath = frameStack.length ? frameStack[frameStack.length - 1].path : "";
```

Then pass `idxPath` through to `makeIndexRow`. Change the grow-loop call:

```js
        for (var j = mapped.length; j < list.length; j++) {
          makeIndexRow(list[j], j, mapped, setters, disposers, owner, mapFn);
        }
```

to:

```js
        for (var j = mapped.length; j < list.length; j++) {
          makeIndexRow(list[j], j, mapped, setters, disposers, owner, mapFn, idxPath);
        }
```

And replace `makeIndexRow`:

```js
  function makeIndexRow(value, index, outMapped, outSetters, outDisposers, owner, mapFn) {
    createRoot(function (dispose) {
      var sig = createSignal(value);
      outSetters[index] = sig[1];
      outDisposers[index] = dispose;
      outMapped[index] = mapFn(sig[0], index); // item is the signal GETTER
    }, owner);
  }
```

with:

```js
  function makeIndexRow(value, index, outMapped, outSetters, outDisposers, owner, mapFn, idxPath) {
    createRoot(function (dispose) {
      // The item signal is derived from list position, NOT preserved — create it
      // before opening the row frame so it is excluded from the collected cells.
      var sig = createSignal(value);
      outSetters[index] = sig[1];
      outDisposers[index] = dispose;
      if (hmrOn() && currentRoot) {
        outMapped[index] = withRowInstance(idxPath + "#i" + index, function () {
          return mapFn(sig[0], index);
        });
      } else {
        outMapped[index] = mapFn(sig[0], index); // item is the signal GETTER
      }
    }, owner);
  }
```

> **Guidance:** `<For>` rows keyed by the raw item object rely on those objects reappearing by identity after the enclosing list signal is itself rehydrated (the `__list` cell holds the same references). `<Index>` excludes its position-derived item signal from collection by creating it before the frame opens. Both row builders run inside `render()`'s synchronous createRoot (and the rehydration cascade it drives), so `currentRoot` is set.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p supersolid_runtime`
Expected: PASS (both new row tests + all prior).

- [ ] **Step 5: Commit**

```bash
git add crates/supersolid_runtime/src/render.js crates/supersolid_runtime/src/lib.rs
git commit -m "feat(supersolid_runtime): HMR per-row preservation for <For> (identity) and <Index> (position)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 5: transpiler — `module_id` option + component `$ss.hot` registration

Give `TranspileOptions` a `module_id`, and make the JSX pass emit `$ss.hot("<module_id>#<Name>", <Name>)` immediately after each top-level component declaration (so the id is set before any `render()` use).

**Files:**
- Modify: `crates/supersolid/src/lib.rs` (`TranspileOptions` + `Default` + `transpile_file`)
- Modify: `crates/supersolid/src/pipeline.rs` (pass `module_id` to `jsx::lower`)
- Modify: `crates/supersolid/src/jsx.rs` (`lower` signature + registration emission)
- Test: `crates/supersolid/src/lib.rs` (`mod tests`)

**Interfaces:**
- Produces: `TranspileOptions.module_id: Option<String>` (default `None`); `jsx::lower(allocator, program, module_id: Option<&str>)` appends per-component registrations. Id format: `"<module_id>#<Name>"`, or `"#<Name>"` when `module_id` is `None`.

- [ ] **Step 1: Write the failing tests**

Add to `crates/supersolid/src/lib.rs` `mod tests`:

```rust
#[test]
fn top_level_function_component_is_registered() {
    let out = code("function Counter(){ return <div/>; } render(() => <Counter/>, root);");
    assert!(out.contains(r#"$ss.hot("#Counter", Counter)"#), "component registered:\n{out}");
    assert!(reparses_as_plain_js(&out), "{out}");
}

#[test]
fn const_arrow_component_is_registered() {
    let out = code("const Item = () => <li/>;");
    assert!(out.contains(r#"$ss.hot("#Item", Item)"#), "const-arrow registered:\n{out}");
    assert!(reparses_as_plain_js(&out), "{out}");
}

#[test]
fn module_id_qualifies_the_hot_id() {
    let r = transpile(
        "function App(){ return <div/>; }",
        &TranspileOptions { module_id: Some("app.tsx".into()), ..Default::default() },
    );
    assert!(r.code.contains(r#"$ss.hot("app.tsx#App", App)"#), "path-qualified id:\n{}", r.code);
    assert!(reparses_as_plain_js(&r.code), "{}", r.code);
}

#[test]
fn lowercase_and_non_function_bindings_are_not_registered() {
    let out = code("function helper(){ return 1; } const value = 2; const Config = 3;");
    assert!(!out.contains("$ss.hot"), "no registration for non-components:\n{out}");
    assert!(reparses_as_plain_js(&out), "{out}");
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p supersolid top_level_function_component const_arrow_component module_id_qualifies lowercase_and_non_function`
Expected: FAIL to compile first (`module_id` field missing), then FAIL on the missing `$ss.hot` output.

- [ ] **Step 3: Add `module_id` to `TranspileOptions`**

In `crates/supersolid/src/lib.rs`, extend the struct and its `Default`:

```rust
#[derive(Debug, Clone)]
pub struct TranspileOptions {
    /// Import specifiers whose imports are stripped silently (their names are
    /// provided as runtime globals by Plans 3–4).
    pub runtime_specifiers: Vec<String>,
    /// Parse as `.tsx` (allow JSX) when true, `.ts` when false.
    pub tsx: bool,
    /// Module id baked into component HMR registrations (`"<module_id>#<Name>"`);
    /// the native loader / CLI supply the asset path. `None` => `"#<Name>"`.
    pub module_id: Option<String>,
}

impl Default for TranspileOptions {
    fn default() -> Self {
        TranspileOptions {
            runtime_specifiers: vec!["supersolid".into(), "solid-js".into()],
            tsx: true,
            module_id: None,
        }
    }
}
```

And thread the path in `transpile_file`:

```rust
pub fn transpile_file(input: &std::path::Path, output: &std::path::Path) -> std::io::Result<TranspileResult> {
    let src = std::fs::read_to_string(input)?;
    let tsx = input.extension().and_then(|e| e.to_str()) != Some("ts");
    let module_id = Some(input.to_string_lossy().into_owned());
    let result = transpile(&src, &TranspileOptions { tsx, module_id, ..Default::default() });
    std::fs::write(output, &result.code)?;
    Ok(result)
}
```

- [ ] **Step 4: Pass `module_id` into `jsx::lower`**

In `crates/supersolid/src/pipeline.rs`, change the lowering call:

```rust
        crate::jsx::lower(&allocator, &mut program);
```

to:

```rust
        crate::jsx::lower(&allocator, &mut program, options.module_id.as_deref());
```

- [ ] **Step 5: Emit registrations in `jsx.rs`**

In `crates/supersolid/src/jsx.rs`, add these helpers (place the free functions near `is_static_literal`, and the method inside the `impl<'a> Lower<'a>` block):

```rust
/// True iff `s` begins with an uppercase character (JSX component convention).
fn starts_uppercase(s: &str) -> bool {
    s.chars().next().is_some_and(|c| c.is_uppercase())
}

/// If `stmt` is a top-level component definition, return its binding name:
/// an uppercase-named function declaration, or `const/let/var NAME = (arrow|function)`
/// with an uppercase identifier binding.
fn top_level_component_name(stmt: &Statement<'_>) -> Option<String> {
    use oxc::ast::ast::BindingPatternKind;
    match stmt {
        Statement::FunctionDeclaration(f) => {
            let name = f.id.as_ref()?.name.as_str();
            starts_uppercase(name).then(|| name.to_string())
        }
        Statement::VariableDeclaration(decl) => {
            let d = decl.declarations.first()?;
            match d.init.as_ref()? {
                Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_) => {
                    if let BindingPatternKind::BindingIdentifier(id) = &d.id.kind {
                        let name = id.name.as_str();
                        return starts_uppercase(name).then(|| name.to_string());
                    }
                    None
                }
                _ => None,
            }
        }
        _ => None,
    }
}
```

Add the registration-statement builder as a method on `Lower`:

```rust
    /// `$ss.hot("<module_id>#<name>", <name>);` as a statement.
    fn hot_registration(&self, module_id: Option<&str>, name: &str) -> Statement<'a> {
        let id = match module_id {
            Some(m) => format!("{m}#{name}"),
            None => format!("#{name}"),
        };
        let callee = self.runtime_member("hot");
        let name_ref = self.ast.expression_identifier(SPAN, self.atom(name));
        let call = self.call(callee, vec![self.string(&id), name_ref]);
        self.ast.statement_expression(SPAN, call)
    }
```

Rewrite the `lower` entry point to insert a registration right after each component declaration:

```rust
/// Entry point called from the pipeline.
pub(crate) fn lower<'a>(
    allocator: &'a Allocator,
    program: &mut Program<'a>,
    module_id: Option<&str>,
) {
    let mut pass = Lower { ast: AstBuilder::new(allocator), next_local: 0 };
    pass.visit_program(program);

    // Insert each top-level component's `$ss.hot(...)` immediately AFTER its
    // declaration, so the id is tagged before any later `render()` uses it.
    let old = std::mem::replace(&mut program.body, pass.ast.vec());
    for stmt in old {
        let comp = top_level_component_name(&stmt);
        program.body.push(stmt);
        if let Some(name) = comp {
            program.body.push(pass.hot_registration(module_id, &name));
        }
    }
}
```

> **Spike note (confirm against installed oxc 0.140):** `AstBuilder::vec()` creates an empty arena `Vec`; the arena `Vec` supports `into_iter()` (consuming) and `push`; `AstBuilder::statement_expression(SPAN, expr)` and `expression_identifier(SPAN, atom)` build the statement/identifier. `Statement`/`Expression`/`Program` are already imported in `jsx.rs`. If consuming `into_iter()` is unavailable, iterate by draining indices into a `Vec<Statement>` first (collect the moved statements via `program.body.drain(..)` if `drain` exists, else swap-remove), then rebuild. Let the four tests drive the exact API.

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p supersolid`
Expected: PASS (four new + all prior transpiler tests — the appended `$ss.hot(...)` statements are plain JS, so `reparses_as_plain_js` still holds, and existing `.contains` assertions are unaffected).

- [ ] **Step 7: Commit**

```bash
git add crates/supersolid/src/lib.rs crates/supersolid/src/pipeline.rs crates/supersolid/src/jsx.rs
git commit -m "feat(supersolid): emit \$ss.hot component registrations + module_id option

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 6: `superui` — `TsxLoader` bakes the asset path as `module_id`

Thread the loaded asset's path into the transpile as `module_id`, so component ids are path-qualified end-to-end.

**Files:**
- Modify: `crates/superui/src/assets.rs` (`TsxLoader::load`)
- Test: `crates/superui/src/assets.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: `supersolid::TranspileOptions.module_id` (Task 5).
- Produces: `TsxLoader` sets `module_id = Some(lc.path().to_string())`.

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests` in `crates/superui/src/assets.rs` (native-only, mirroring `tsx_loader_transpiles_to_jssource`):

```rust
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn tsx_loader_bakes_module_path_into_hot_id() {
    let dir = Dir::new("assets".into());
    dir.insert_asset(
        "counter.tsx".as_ref(),
        b"function Counter(){ return <div/>; } render(() => <Counter/>, root);",
    );

    let mut app = App::new();
    app.register_asset_source(
        AssetSourceId::Default,
        AssetSource::build().with_reader(move || Box::new(MemoryAssetReader { root: dir.clone() })),
    );
    app.add_plugins((bevy::app::TaskPoolPlugin::default(), AssetPlugin::default()));
    app.init_asset::<JsSource>().register_asset_loader(TsxLoader);
    app.finish();

    let handle = {
        let server = app.world().resource::<AssetServer>().clone();
        server.load::<JsSource>("counter.tsx")
    };
    for _ in 0..64 {
        app.update();
        if matches!(
            app.world().resource::<AssetServer>().load_state(handle.id()),
            LoadState::Loaded
        ) {
            break;
        }
    }
    let jss = app.world().resource::<Assets<JsSource>>();
    let out = &jss.get(&handle).unwrap().0;
    assert!(
        out.contains(r#"$ss.hot("counter.tsx#Counter", Counter)"#),
        "loader must bake the asset path into the HMR id:\n{out}"
    );
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p superui tsx_loader_bakes_module_path`
Expected: FAIL — the loader currently passes no `module_id`, so the id is `"#Counter"`, not `"counter.tsx#Counter"`.

- [ ] **Step 3: Pass `module_id` in the loader**

In `crates/superui/src/assets.rs`, in `TsxLoader::load`, change:

```rust
        let tsx = lc.path().extension().and_then(|e| e.to_str()) != Some("ts");
        let opts = supersolid::TranspileOptions { tsx, ..Default::default() };
```

to:

```rust
        let tsx = lc.path().extension().and_then(|e| e.to_str()) != Some("ts");
        let module_id = Some(lc.path().to_string());
        let opts = supersolid::TranspileOptions { tsx, module_id, ..Default::default() };
```

> **Guidance:** `lc.path()` returns an `AssetPath`; `.to_string()` yields the source-relative path string (e.g. `"counter.tsx"`). If `to_string()` is unavailable on the exact type, use `lc.path().path().to_string_lossy().into_owned()`.

- [ ] **Step 4: Run the test to verify it passes + no regressions**

Run: `cargo test -p superui`
Expected: PASS (new test + `tsx_loader_transpiles_to_jssource` + `loads_html_and_js_sources`).

- [ ] **Step 5: Commit**

```bash
git add crates/superui/src/assets.rs
git commit -m "feat(superui): TsxLoader bakes asset path into component HMR ids (module_id)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 7: `superui_bridge` — `UiRuntime::new(hmr)` sets the runtime flag

Add the `hmr: bool` constructor arg that sets `globalThis.__ssHmr`, and update every caller.

**Files:**
- Modify: `crates/superui_bridge/src/runtime.rs` (`UiRuntime::new` + its two tests + a new test)
- Modify: `crates/superui_bridge/tests/support/mod.rs` (`mount` + new `mount_hmr`)
- Modify: `crates/superui/src/mount.rs`, `crates/superui/src/hot_reload.rs` (pass `false` for now — Task 8 computes the real value)

**Interfaces:**
- Produces: `UiRuntime::new(dom, root, stylesheet, hmr: bool)` — when `hmr`, evaluates `globalThis.__ssHmr = true` once after `supersolid_runtime::install`. `support::mount_hmr(app, dom) -> Entity` builds an HMR-on runtime.

- [ ] **Step 1: Write the failing tests**

In `crates/superui_bridge/src/runtime.rs` `mod tests`, add:

```rust
#[test]
fn hmr_flag_set_when_enabled() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document("<div id='a'></div>")));
    let mut rt = UiRuntime::new(dom, Entity::PLACEHOLDER, Handle::default(), true);
    let on = rt
        .engine
        .context_mut()
        .eval(boa_engine::Source::from_bytes("globalThis.__ssHmr === true"))
        .unwrap()
        .as_boolean()
        .unwrap();
    assert!(on, "hmr=true must set globalThis.__ssHmr");
}

#[test]
fn hmr_flag_absent_when_disabled() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document("<div id='a'></div>")));
    let mut rt = UiRuntime::new(dom, Entity::PLACEHOLDER, Handle::default(), false);
    let on = rt
        .engine
        .context_mut()
        .eval(boa_engine::Source::from_bytes("globalThis.__ssHmr === true"))
        .unwrap()
        .as_boolean()
        .unwrap();
    assert!(!on, "hmr=false must leave globalThis.__ssHmr unset");
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p superui_bridge hmr_flag`
Expected: FAIL to compile — `UiRuntime::new` takes 3 args, not 4.

- [ ] **Step 3: Add the `hmr` parameter**

In `crates/superui_bridge/src/runtime.rs`, change the `new` signature and body:

```rust
    pub fn new(
        dom: Rc<RefCell<Dom>>,
        root: Entity,
        stylesheet: Handle<StyleSheet>,
        hmr: bool,
    ) -> Self {
        let mut engine = BoaEngine::new(dom.clone());
        superui_api::install(&mut engine);
        supersolid_runtime::install(&mut engine);
        // Plan 5: enable state-preserving HMR collection in render.js. Must run
        // after install (so the runtime exists) and before any run_script (so the
        // first render already collects). Gate decided by the caller (feature +
        // asset watcher); off => render.js takes the Plan-4 fast paths.
        if hmr {
            let _ = engine.eval("globalThis.__ssHmr = true;");
        }
        crate::bevy_bridge::install_bevy_bridge(&mut engine);
        UiRuntime {
            dom,
            engine,
            root,
            stylesheet,
            dirty: true,
            node_to_entity: HashMap::new(),
            entity_to_node: HashMap::new(),
            focused: None,
            caret_visible: true,
            caret_accum: 0.0,
            input_texts: HashMap::new(),
        }
    }
```

Update the two existing tests in this file (`new_runtime_is_dirty_and_runs_script`, `supersolid_runtime_globals_are_available_in_the_ui_runtime`) to pass `false` as the 4th arg to `UiRuntime::new`.

- [ ] **Step 4: Update the other callers**

In `crates/superui_bridge/tests/support/mod.rs`, change `mount` to pass `false` and add `mount_hmr`:

```rust
pub fn mount(app: &mut App, dom: Rc<RefCell<Dom>>) -> Entity {
    let root = app.world_mut().spawn(Node::default()).id();
    let stylesheet: Handle<StyleSheet> = Handle::default();
    let rt = UiRuntime::new(dom, root, stylesheet, false);
    app.world_mut().insert_non_send_resource(rt);
    app.add_systems(Update, reconcile_system);
    root
}

/// Like `mount`, but with state-preserving HMR collection enabled.
pub fn mount_hmr(app: &mut App, dom: Rc<RefCell<Dom>>) -> Entity {
    let root = app.world_mut().spawn(Node::default()).id();
    let stylesheet: Handle<StyleSheet> = Handle::default();
    let rt = UiRuntime::new(dom, root, stylesheet, true);
    app.world_mut().insert_non_send_resource(rt);
    app.add_systems(Update, reconcile_system);
    root
}
```

In `crates/superui/src/mount.rs`, change `let mut rt = UiRuntime::new(dom, entity, css_handle);` to `let mut rt = UiRuntime::new(dom, entity, css_handle, false);` (Task 8 replaces the `false` with the computed gate).

In `crates/superui/src/hot_reload.rs`, change `rt = UiRuntime::new(dom, entity, stylesheet);` to `rt = UiRuntime::new(dom, entity, stylesheet, false);` (Task 8 replaces it).

- [ ] **Step 5: Run to verify they pass + no regressions**

Run: `cargo test -p superui_bridge && cargo test -p superui`
Expected: PASS (new `hmr_flag_*` tests, updated existing tests, and the Plan-4 `supersolid_render` integration test still green with `mount` passing `false`).

- [ ] **Step 6: Commit**

```bash
git add crates/superui_bridge/src/runtime.rs crates/superui_bridge/tests/support/mod.rs crates/superui/src/mount.rs crates/superui/src/hot_reload.rs
git commit -m "feat(superui_bridge): UiRuntime::new(hmr) sets globalThis.__ssHmr

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 8: `superui` — `hmr` feature + watcher gate + one-time warning

Add the `hmr` cargo feature and wire the gate: `mount_when_ready`/`apply_hot_reload` compute `cfg!(feature="hmr") && AssetServer.watching_for_changes()` and pass it to `UiRuntime::new`; `mount_when_ready` warns once when the feature is on but no watcher is active.

**Files:**
- Modify: `crates/superui/Cargo.toml` (`[features] hmr`)
- Modify: `crates/superui/src/mount.rs` (gate helper + compute + warn + pass)
- Modify: `crates/superui/src/hot_reload.rs` (compute + pass on HTML rebuild)
- Test: `crates/superui/src/mount.rs` (`#[cfg(test)]`)

**Interfaces:**
- Produces: `pub(crate) fn hmr_active(watching: bool) -> bool` — `cfg!(feature = "hmr") && watching`. Used by both systems and unit-tested.

- [ ] **Step 1: Add the feature**

In `crates/superui/Cargo.toml`, add a features table (create it if absent; place after `[dependencies]`):

```toml
[features]
# State-preserving hot reload (Plan 5). Enable ALONGSIDE `bevy/file_watcher`.
# When on, HMR still only activates while the AssetServer is watching for changes.
hmr = []
```

- [ ] **Step 2: Write the failing test**

Add to `crates/superui/src/mount.rs` a test module (or extend an existing one):

```rust
#[cfg(test)]
mod hmr_gate_tests {
    use super::hmr_active;

    #[test]
    fn hmr_active_requires_watching_and_feature() {
        // Without the `hmr` feature, the gate is always false.
        // With it, the gate follows `watching`.
        if cfg!(feature = "hmr") {
            assert!(hmr_active(true), "feature on + watching => active");
            assert!(!hmr_active(false), "feature on + not watching => inactive");
        } else {
            assert!(!hmr_active(true), "feature off => inactive even when watching");
            assert!(!hmr_active(false));
        }
    }
}
```

- [ ] **Step 3: Run to verify it fails**

Run: `cargo test -p superui hmr_active_requires_watching`
Expected: FAIL to compile — `hmr_active` does not exist.

- [ ] **Step 4: Add `hmr_active` and wire `mount_when_ready`**

In `crates/superui/src/mount.rs`, add the helper (module-level):

```rust
/// The Plan 5 HMR gate: active only when the `hmr` feature is compiled in AND the
/// asset server is watching for changes (there is no point collecting HMR state
/// when no edit can ever be observed). `cfg!` folds to `false` with the feature off.
pub(crate) fn hmr_active(watching: bool) -> bool {
    cfg!(feature = "hmr") && watching
}
```

In `mount_when_ready`, replace the runtime build:

```rust
    // Build the runtime: parse HTML -> Dom, wire engine, run author JS.
    let dom = Rc::new(RefCell::new(superui_html::parse_document(&html_src)));
    let mut rt = UiRuntime::new(dom, entity, css_handle, false);
    rt.run_script(&js_src);
```

with:

```rust
    // Plan 5 gate: feature + asset watcher. Warn once (here, the single mount
    // point) if the feature is enabled but nothing is watching, then stay off.
    let watching = world.resource::<AssetServer>().watching_for_changes();
    let hmr = hmr_active(watching);
    #[cfg(feature = "hmr")]
    if !watching {
        bevy::log::warn!(
            "superui: `hmr` feature is enabled but the AssetServer is not watching for \
             changes; state-preserving hot reload is OFF. Enable `bevy/file_watcher` (or set \
             AssetPlugin.watch_for_changes_override = Some(true)) to activate it."
        );
    }

    // Build the runtime: parse HTML -> Dom, wire engine, run author JS.
    let dom = Rc::new(RefCell::new(superui_html::parse_document(&html_src)));
    let mut rt = UiRuntime::new(dom, entity, css_handle, hmr);
    rt.run_script(&js_src);
```

- [ ] **Step 5: Wire `apply_hot_reload`**

In `crates/superui/src/hot_reload.rs`, in the `html_changed` rebuild branch, replace:

```rust
            // Rebuild the whole runtime around the fresh DOM.
            let dom = Rc::new(RefCell::new(superui_html::parse_document(&src)));
            let entity = rt.root;
            let stylesheet = rt.stylesheet.clone();
            rt = UiRuntime::new(dom, entity, stylesheet, false);
```

with:

```rust
            // Rebuild the whole runtime around the fresh DOM. Recompute the HMR
            // gate (an HTML-rebuild only happens while watching, so the feature
            // decides it here; no re-warn — mount_when_ready owns the warning).
            let watching = world.resource::<AssetServer>().watching_for_changes();
            let hmr = crate::mount::hmr_active(watching);
            let dom = Rc::new(RefCell::new(superui_html::parse_document(&src)));
            let entity = rt.root;
            let stylesheet = rt.stylesheet.clone();
            rt = UiRuntime::new(dom, entity, stylesheet, hmr);
```

> **Guidance:** `apply_hot_reload` already holds `&mut World` and has removed the `UiRuntime` NonSend resource at this point, so `world.resource::<AssetServer>()` is available. `hmr_active` must be `pub(crate)` (Step 4) so `hot_reload.rs` can call it.

- [ ] **Step 6: Run to verify it passes (both feature states)**

Run: `cargo test -p superui`
Then: `cargo test -p superui --features hmr`
Expected: PASS in both. The gate test asserts the correct branch for whichever feature state it is compiled under.

- [ ] **Step 7: Commit**

```bash
git add crates/superui/Cargo.toml crates/superui/src/mount.rs crates/superui/src/hot_reload.rs
git commit -m "feat(superui): hmr feature + asset-watcher gate + one-time misconfig warning

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 9: `superui_bridge` end-to-end reload test + capability ledger

Prove HMR through the genuine `UiRuntime` / `run_script` seam (the path `apply_hot_reload` drives): a counter's reconciled ECS `Text` keeps its value after a simulated re-exec. Record the ledger.

**Files:**
- Test: `crates/superui_bridge/tests/supersolid_render.rs` (append; reuse `current_label_text` + `support::mount_hmr`)
- Modify: `docs/support/js-dom.md` (ledger)

**Interfaces:**
- Consumes: `support::{test_app, mount_hmr}` (Task 7); `current_label_text` (existing in this file); `UiRuntime::run_script`.

- [ ] **Step 1: Write the failing test**

In `crates/superui_bridge/tests/supersolid_render.rs`, add `mount_hmr` to the `use support::{...}` line, then append:

```rust
#[test]
fn supersolid_hmr_preserves_counter_across_reexec() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<div id='root'></div>",
    )));
    let mut app = test_app();
    let _root = support::mount_hmr(&mut app, dom.clone());

    // The same transpiled-style module the loader would emit: a component tagged
    // via $ss.hot, mounted with render().
    let script = r#"
        function Counter() {
            var c = createSignal(0);
            globalThis.__c = c;
            var wrap = $ss.el("div");
            var label = $ss.el("span");
            $ss.insert(label, function () { return c[0](); });
            $ss.child(wrap, label);
            return wrap;
        }
        $ss.hot("root.tsx#Counter", Counter);
        render(function () { return $ss.cmp(Counter, {}); },
               document.getElementById("root"));
    "#;

    app.world_mut().non_send_resource_mut::<UiRuntime>().run_script(script);
    app.update();
    assert_eq!(current_label_text(&mut app, &dom), "0", "initial reconcile");

    // Bump the signal, reconcile.
    app.world_mut()
        .non_send_resource_mut::<UiRuntime>()
        .run_script("globalThis.__c[1](5);");
    app.update();
    assert_eq!(current_label_text(&mut app, &dom), "5", "runtime update reconciles");

    // Simulate a hot reload: re-exec the SAME module on the SAME runtime, exactly
    // as apply_hot_reload does for a JsSource Modified event.
    app.world_mut().non_send_resource_mut::<UiRuntime>().run_script(script);
    app.update();
    assert_eq!(
        current_label_text(&mut app, &dom),
        "5",
        "reload rehydrates the signal cell: value preserved through DOM rebuild -> reconcile"
    );
}
```

- [ ] **Step 2: Run to verify it fails, then investigate**

Run: `cargo test -p superui_bridge --test supersolid_render supersolid_hmr_preserves_counter`
Expected: PASS if Tasks 1–7 are correct (this is an integration check over already-built pieces). If it FAILS: confirm (a) `mount_hmr` set `__ssHmr` (Task 7), (b) `render()`'s reload branch cleared + rebuilt (Task 3), (c) the re-exec ran without a swallowed JS error (temporarily log `run_script` errors). Do **not** weaken the assertions.

- [ ] **Step 3: Update the capability ledger**

In `docs/support/js-dom.md`, under the existing `### Render + control flow (supersolid_runtime render layer)` subsection, add a row and a note:

```markdown
| `$ss.hot(id, fn)` | ✅ | T0 | tags a component with a stable HMR id (`"<assetpath>#<Name>"`); no-op off-HMR |

**State-preserving hot reload (Plan 5).** When the `superui/hmr` feature is enabled **and** the
asset server is watching (`bevy/file_watcher`), a `.tsx`/`.js` edit re-execs on the same engine and
`render()` rehydrates each component's signal cells (keyed by `module × instance × creation-order`),
rebuilding the DOM fresh while preserving values. A per-instance signal-shape change (add/remove)
resets that instance. `<For>` rows preserve state by item identity, `<Index>` rows by position.
Off by default (feature off, or no watcher) — then `render()`/`$ss.cmp` take the Plan-4 fast paths.
```

- [ ] **Step 4: Verify the whole affected surface and commit**

Run: `cargo test -p supersolid_runtime && cargo test -p supersolid && cargo test -p superui_bridge && cargo test -p superui --features hmr`
Expected: PASS.

Optional (if the wasm target is installed): `cargo build -p supersolid_runtime --target wasm32-unknown-unknown` and `cargo build -p superui --target wasm32-unknown-unknown` — both build (runtime stays wasm-clean; `hmr` feature is `superui`-only and the flag-set compiles out / never watches on wasm).

```bash
git add crates/superui_bridge/tests/supersolid_render.rs docs/support/js-dom.md
git commit -m "test(superui_bridge): end-to-end HMR reload preserves reconciled Text + ledger

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Done-when

- `cargo test -p supersolid_runtime`, `cargo test -p supersolid`, `cargo test -p superui_bridge`, and `cargo test -p superui --features hmr` all green.
- A `.tsx`/`.js` edit (same-engine re-exec) preserves component-setup signal values, keyed by `module × instance × creation-order`; a per-instance signal-shape change resets that instance.
- `<For>` per-row state survives a reload+reorder (item identity); `<Index>` per-position state survives (position).
- Components carry transpiler-baked ids `"<assetpath>#<Name>"` via `$ss.hot`, emitted right after each declaration; the native loader bakes the asset path.
- HMR is active iff `superui/hmr` is enabled **and** the asset server is watching; feature-on-without-watcher warns once and stays off; feature-off is compile-time inert; the steady-state hot path matches Plan 4 in every build.
- `hot_reload.rs`'s reload algorithm is unchanged (its `UiRuntime::new` call gains the gate arg); the reconciler, taffy layout, and picking are untouched.
- `supersolid_runtime` stays wasm-clean; `docs/support/js-dom.md` records `$ss.hot` + the HMR behavior/gating.

## Self-review (author)

- **Spec coverage:** §2 seam/trigger + `hot_reload.rs` untouched-logic → Tasks 7–8 pass a gate arg only; §3 keying (module/export, instance tree-position/key, cell order) → Task 5 (`$ss.hot` ids) + Task 3 (`withInstance`/ordinals) + Task 4 (row keys); §4 `$ssOnSignal` hook → Task 1; §5 frame lifecycle + frame-close rehydration + shape reset → Task 3 (`withInstance`/`commitRehydration`) and Task 4 (`withRowInstance`); §6 `render()` reload path (snapshot/dispose/clear/rebuild) → Task 3; §7 transpiler+loader → Tasks 5–6; §8 limitations → documented in spec (name/position collisions, same-count reorder); §9 gating (feature + watcher, one-time warn, `__ssHmr`) → Tasks 7–8; §10 tests → Tasks 1–9; §11 non-goals → not implemented by design (DOM identity, cross-module, wasm live reload, proxy swap).
- **No placeholders:** every task ships concrete failing tests and the exact JS/Rust edits (before→after blocks). The two genuinely version-sensitive Rust spots — oxc 0.140 arena `Vec` `into_iter`/`push` + `AstBuilder::vec()`/`statement_expression` (Task 5), and `AssetPath::to_string()` (Task 6) — are flagged as spike notes with fallbacks, driven by executable tests, consistent with Plans 2/4's convention.
- **Type/name consistency:** `$ssOnSignal(read, write)` (Task 1) matches `onSignal(read, write)` published in Task 3; `withInstance`/`withRowInstance`/`commitRehydration`/`snapshotCells`/`clearChildren`/`hmrOn`/`roots`/`frameStack`/`currentRoot` are introduced in Task 3 and reused unchanged in Task 4; `$ss.hot(id, fn)` (Task 2) matches the transpiler emission (Task 5) and the loader-baked id (Task 6); `TranspileOptions.module_id` + `jsx::lower(allocator, program, module_id)` are consistent across Tasks 5–6; `UiRuntime::new(dom, root, stylesheet, hmr)` (Task 7) matches every caller updated in Tasks 7–8 and used in Task 9; `hmr_active(watching)` (Task 8) is `pub(crate)` and called from both `mount.rs` and `hot_reload.rs`.
