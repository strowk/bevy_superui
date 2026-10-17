# Preserve Module-Scope Signals Across HMR Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Preserve module-top-level signals across `--features hmr` re-exec, keyed per module path, on by default whenever `__ssHmr` is on.

**Architecture:** Add a render.js-retained per-module store (`moduleStack` + `moduleFrames`) parallel to the existing component `frameStack`/`roots`. The `$ssmod` prelude brackets each module's `m.factory(...)` with `$ss.modEnter(id)`/`$ss.modExit(id)`; `onSignal` collects into the open module frame only when no component frame is open; `modExit` rehydrates the new frame from the previous one when their signal counts match, then anchors the new frame for the next reload.

**Tech Stack:** Rust (`supersolid`, `supersolid_runtime` crates), ES5-target JavaScript (Boa/V8), `cargo test`.

**Spec:** `docs/superpowers/specs/2026-10-16-module-scope-signal-hmr-design.md`

## Global Constraints

- **ES5-only JS.** No optional chaining, arrow functions, `let`/`const` in render.js or the prelude string — match the surrounding `var`/`function` ES5 style (Boa/ES5 target).
- **Separate stacks.** `moduleStack`/`moduleFrames` are kept distinct from `frameStack`/`roots` so component rehydration (`withInstance`, `indexArray`, instance keys) is provably undisturbed.
- **Everything keys off `hmrOn()`** (`!!globalThis.__ssHmr`). `modEnter`/`modExit` return immediately when the gate is off; the gate-off and wasm/shipped paths behave exactly as today. No new cargo feature or flag.
- **`frameStack`-first order in `onSignal`.** A component frame, when present, always wins over the module frame, so component-body signals never land in the module frame.
- **Prelude guard is defensive and ES5.** `if (typeof $ss !== "undefined" && $ss.modEnter)` — a bundle run without the render layer skips the bracket instead of throwing.
- **Commits:** summary says what was done; body says why (and maybe how), not a restatement of the diff.
- **Do NOT use git worktrees** (huge `target/` folder).

## Review Focus

- **Nested imported module** — a module whose factory `require`s a dep opens a nested `modEnter`/`modExit`; each module's top-level signals must key to its *own* path, not the importer's. Covered by Task 1 test `module_nested_require_keys_by_own_path`.
- **Function-valued top-level signal** — rehydration must restore a signal whose value is itself a function (updater-form write `cell.write(function(){return val;})`). Covered by Task 1 test `module_function_valued_signal_rehydrates`.
- **Top-level signal created via helper/IIFE indirection** — preserved iff `createSignal` runs during top-level execution, regardless of call indirection. Covered by Task 1 test `module_signal_via_helper_is_preserved`.
- **Deferred signal from a handler** — a `createSignal` that runs after `modExit` is created with both stacks empty, so it is not tracked and resets. Covered by Task 1 test `module_deferred_signal_not_preserved`.
- **Throwing factory** — a module whose body throws must still run `modExit` (via the prelude's `try/finally`) so the next module collects correctly. Covered by Task 1 test `module_throwing_factory_keeps_stack_balanced` and Task 2's `try/finally` prelude assertion.

---

### Task 1: Module-frame mechanism in `render.js`

**Files:**
- Modify: `crates/supersolid_runtime/src/render.js` (add `moduleStack`, `moduleFrames`, `modEnter`, `modExit`; extend `onSignal`; publish on `$ss`)
- Test: `crates/supersolid_runtime/src/lib.rs` (new `#[test]` fns in `mod render_tests`)

**Interfaces:**
- Consumes: existing render.js globals — `hmrOn()`, `frameStack`, `untrack`, the `$ssOnSignal` hook, `createSignal`, `render`, `$ss.cmp`, `$ss.el`, `$ss.insert`.
- Produces (new `$ss` members, used by Task 2's prelude):
  - `$ss.modEnter(path: string): void` — push `{ path, cells: [] }` onto `moduleStack` (no-op when gate off).
  - `$ss.modExit(path: string): void` — pop the frame; if `moduleFrames.get(path)` exists and its `cells.length` equals the popped frame's, copy each old value into the new cell via `untrack` + updater-form write; then `moduleFrames.set(path, frame)` (no-op when gate off).

- [ ] **Step 1: Write the failing repro test**

In `crates/supersolid_runtime/src/lib.rs`, inside `mod render_tests`, add:

```rust
#[test]
fn module_top_level_signal_survives_reexec() {
    let mut e = render_engine();
    e.eval(
        r#"
        globalThis.__ssHmr = true;
        function exec() {
            $ss.modEnter("app.tsx");
            try {
                var s = createSignal("menu");
                globalThis.__s = s;
            } finally { $ss.modExit("app.tsx"); }
        }
        exec();                              // fresh mount
        globalThis.v0 = globalThis.__s[0](); // "menu"
        globalThis.__s[1]("game");           // user navigates
        globalThis.v1 = globalThis.__s[0](); // "game"
        exec();                              // reload: re-exec module top level
        globalThis.v2 = globalThis.__s[0](); // "game" — preserved on the fresh signal
        "#,
    )
    .unwrap();
    assert_eq!(text(&mut e, "globalThis.v0"), "menu");
    assert_eq!(text(&mut e, "globalThis.v1"), "game");
    assert_eq!(text(&mut e, "globalThis.v2"), "game");
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p supersolid_runtime module_top_level_signal_survives_reexec`
Expected: FAIL — `$ss.modEnter is not a function` (not yet published).

- [ ] **Step 3: Implement the module-frame mechanism in `render.js`**

In the HMR state block (near `var frameStack = [];`), add two persistent vars:

```js
var moduleStack = [];          // module frames open during the current (re-)exec
var moduleFrames = new Map();  // modulePath -> last committed frame
```

Extend `onSignal` with a module-stack fallback, checked only when no component frame is open (keep the `frameStack`-first early return):

```js
function onSignal(read, write) {
  var f = frameStack[frameStack.length - 1];
  if (f) { f.cells.push({ read: read, write: write }); return; }
  var m = moduleStack[moduleStack.length - 1];
  if (m) m.cells.push({ read: read, write: write });
}
```

Add `modEnter`/`modExit` (both gate-guarded), mirroring `commitRehydration`'s `untrack` + updater-form write:

```js
function modEnter(path) {
  if (!hmrOn()) return;
  moduleStack.push({ path: path, cells: [] });
}

function modExit(path) {
  if (!hmrOn()) return;
  var frame = moduleStack.pop();
  var prev = moduleFrames.get(path);
  if (prev && prev.cells.length === frame.cells.length) {
    for (var i = 0; i < prev.cells.length; i++) {
      var v = untrack(prev.cells[i].read);
      (function (val, cell) { cell.write(function () { return val; }); })(v, frame.cells[i]);
    }
  }
  moduleFrames.set(path, frame);
}
```

Publish both on the `$ss` object alongside `hot`:

```js
    hot: hot,
    modEnter: modEnter,
    modExit: modExit,
```

- [ ] **Step 4: Run the repro test to verify it passes**

Run: `cargo test -p supersolid_runtime module_top_level_signal_survives_reexec`
Expected: PASS.

- [ ] **Step 5: Add the remaining runtime tests**

Add these `#[test]` fns in `mod render_tests`. Each re-uses the `render_engine()`, `num`, `text` helpers already in scope.

```rust
#[test]
fn module_fresh_mount_uses_initial_values() {
    // Empty moduleFrames on first exec => no rehydration => initial value.
    let mut e = render_engine();
    e.eval(
        r#"
        globalThis.__ssHmr = true;
        $ss.modEnter("app.tsx");
        try { globalThis.__s = createSignal("menu"); } finally { $ss.modExit("app.tsx"); }
        globalThis.v0 = globalThis.__s[0]();
        "#,
    )
    .unwrap();
    assert_eq!(text(&mut e, "globalThis.v0"), "menu");
}

#[test]
fn module_shape_change_resets_only_edited_module() {
    let mut e = render_engine();
    e.eval(
        r#"
        globalThis.__ssHmr = true;
        function execA(extra) {
            $ss.modEnter("a.tsx");
            try {
                if (extra) createSignal(0);          // added top-level signal on edit
                globalThis.__a = createSignal("a0");
            } finally { $ss.modExit("a.tsx"); }
        }
        function execB() {
            $ss.modEnter("b.tsx");
            try { globalThis.__b = createSignal("b0"); } finally { $ss.modExit("b.tsx"); }
        }
        execA(false); execB();
        globalThis.__a[1]("a1"); globalThis.__b[1]("b1");
        execA(true);  execB();                       // A changes shape, B unchanged
        globalThis.va = globalThis.__a[0]();         // "a0" — reset (count mismatch)
        globalThis.vb = globalThis.__b[0]();         // "b1" — preserved
        "#,
    )
    .unwrap();
    assert_eq!(text(&mut e, "globalThis.va"), "a0");
    assert_eq!(text(&mut e, "globalThis.vb"), "b1");
}

#[test]
fn module_multi_module_preserves_per_path() {
    let mut e = render_engine();
    e.eval(
        r#"
        globalThis.__ssHmr = true;
        function execA() { $ss.modEnter("a.tsx"); try { globalThis.__a = createSignal("a0"); } finally { $ss.modExit("a.tsx"); } }
        function execB() { $ss.modEnter("b.tsx"); try { globalThis.__b = createSignal("b0"); } finally { $ss.modExit("b.tsx"); } }
        execA(); execB();
        globalThis.__a[1]("a1"); globalThis.__b[1]("b1");
        execA(); execB();
        globalThis.va = globalThis.__a[0]();   // "a1"
        globalThis.vb = globalThis.__b[0]();   // "b1"
        "#,
    )
    .unwrap();
    assert_eq!(text(&mut e, "globalThis.va"), "a1");
    assert_eq!(text(&mut e, "globalThis.vb"), "b1");
}

#[test]
fn module_nested_require_keys_by_own_path() {
    // Importer factory runs a nested modEnter/modExit for its dep; each keyed by
    // its own path, so both top-level signals preserve independently.
    let mut e = render_engine();
    e.eval(
        r#"
        globalThis.__ssHmr = true;
        function exec() {
            $ss.modEnter("app.tsx");
            try {
                $ss.modEnter("dep.tsx");
                try { globalThis.__dep = createSignal("d0"); } finally { $ss.modExit("dep.tsx"); }
                globalThis.__app = createSignal("a0");
            } finally { $ss.modExit("app.tsx"); }
        }
        exec();
        globalThis.__dep[1]("d1"); globalThis.__app[1]("a1");
        exec();
        globalThis.vd = globalThis.__dep[0]();   // "d1"
        globalThis.va = globalThis.__app[0]();   // "a1"
        "#,
    )
    .unwrap();
    assert_eq!(text(&mut e, "globalThis.vd"), "d1");
    assert_eq!(text(&mut e, "globalThis.va"), "a1");
}

#[test]
fn module_function_valued_signal_rehydrates() {
    // Updater-form write restores a signal whose value is itself a function.
    let mut e = render_engine();
    e.eval(
        r#"
        globalThis.__ssHmr = true;
        function exec() {
            $ss.modEnter("app.tsx");
            try { globalThis.__s = createSignal(function () { return 1; }); } finally { $ss.modExit("app.tsx"); }
        }
        exec();
        globalThis.__s[1](function () { return 42; });   // updater-form setter
        exec();
        globalThis.v = globalThis.__s[0]()();            // call the restored function
        "#,
    )
    .unwrap();
    assert_eq!(num(&mut e, "globalThis.v"), 42.0);
}

#[test]
fn module_signal_via_helper_is_preserved() {
    // A signal minted through a top-level helper call is still created during
    // top-level execution, so it is tracked and preserved.
    let mut e = render_engine();
    e.eval(
        r#"
        globalThis.__ssHmr = true;
        function exec() {
            $ss.modEnter("app.tsx");
            try {
                function make() { return createSignal("init"); }
                globalThis.__s = make();
            } finally { $ss.modExit("app.tsx"); }
        }
        exec();
        globalThis.__s[1]("changed");
        exec();
        globalThis.v = globalThis.__s[0]();   // "changed"
        "#,
    )
    .unwrap();
    assert_eq!(text(&mut e, "globalThis.v"), "changed");
}

#[test]
fn module_and_component_signals_both_preserved() {
    // Top-level signal collects into the module frame; the component's signal
    // collects into the component frame (frameStack-first). Both survive reload,
    // and the separate stacks don't interfere.
    let mut e = render_engine();
    e.eval(
        r#"
        globalThis.__ssHmr = true;
        globalThis.root = $ss.el("main");
        function exec() {
            $ss.modEnter("app.tsx");
            try {
                globalThis.__top = createSignal("T0");
                function Counter() {
                    var c = createSignal(0);
                    globalThis.__c = c;
                    var d = $ss.el("div");
                    $ss.insert(d, function () { return c[0](); });
                    return d;
                }
                Counter.__ssId = "app#Counter";
                render(function () { return $ss.cmp(Counter, {}); }, root);
            } finally { $ss.modExit("app.tsx"); }
        }
        exec();
        globalThis.__top[1]("T1"); globalThis.__c[1](5);
        exec();
        globalThis.vtop = globalThis.__top[0]();   // "T1"
        globalThis.vc = globalThis.__c[0]();       // 5
        "#,
    )
    .unwrap();
    assert_eq!(text(&mut e, "globalThis.vtop"), "T1");
    assert_eq!(num(&mut e, "globalThis.vc"), 5.0);
}

#[test]
fn module_gate_off_does_not_preserve() {
    let mut e = render_engine();
    e.eval(
        r#"
        globalThis.__ssHmr = false;                 // gate OFF
        function exec() {
            $ss.modEnter("app.tsx");
            try { globalThis.__s = createSignal("menu"); } finally { $ss.modExit("app.tsx"); }
        }
        exec();
        globalThis.__s[1]("game");
        exec();
        globalThis.v = globalThis.__s[0]();         // "menu" — no preservation
        "#,
    )
    .unwrap();
    assert_eq!(text(&mut e, "globalThis.v"), "menu");
}

#[test]
fn module_deferred_signal_not_preserved() {
    // A signal minted after modExit (both stacks empty) is untracked => resets.
    let mut e = render_engine();
    e.eval(
        r#"
        globalThis.__ssHmr = true;
        function exec() {
            $ss.modEnter("app.tsx");
            try { globalThis.__make = function () { globalThis.__d = createSignal("init"); }; }
            finally { $ss.modExit("app.tsx"); }
        }
        exec();
        globalThis.__make();                 // deferred: after modExit
        globalThis.__d[1]("changed");
        exec();
        globalThis.__make();                 // defer again on reload
        globalThis.v = globalThis.__d[0]();  // "init" — not preserved
        "#,
    )
    .unwrap();
    assert_eq!(text(&mut e, "globalThis.v"), "init");
}

#[test]
fn module_throwing_factory_keeps_stack_balanced() {
    // A module body that throws still runs modExit (prelude try/finally, mirrored
    // here), so a later module collects and rehydrates correctly.
    let mut e = render_engine();
    e.eval(
        r#"
        globalThis.__ssHmr = true;
        function execBad() { $ss.modEnter("bad.tsx"); try { throw new Error("boom"); } finally { $ss.modExit("bad.tsx"); } }
        function execGood() { $ss.modEnter("good.tsx"); try { globalThis.__g = createSignal("g0"); } finally { $ss.modExit("good.tsx"); } }
        try { execBad(); } catch (e) {}
        execGood();
        globalThis.__g[1]("g1");
        try { execBad(); } catch (e) {}
        execGood();
        globalThis.v = globalThis.__g[0]();   // "g1" — good module unaffected
        "#,
    )
    .unwrap();
    assert_eq!(text(&mut e, "globalThis.v"), "g1");
}
```

- [ ] **Step 6: Run the full runtime suite to verify all pass**

Run: `cargo test -p supersolid_runtime`
Expected: PASS — all new module tests plus the existing HMR/component tests (regression guard for the separate-stack invariant).

- [ ] **Step 7: Commit**

```bash
git add crates/supersolid_runtime/src/render.js crates/supersolid_runtime/src/lib.rs
git commit -m "feat(supersolid): preserve module-scope signals across HMR"
```

---

### Task 2: `$ssmod` prelude brackets `m.factory(...)`

**Files:**
- Modify: `crates/supersolid/src/module.rs:74-89` (`SSMOD_PRELUDE`)
- Test: `crates/supersolid/src/module.rs` (new `#[test]` fns in `mod tests`)

**Interfaces:**
- Consumes: `$ss.modEnter`/`$ss.modExit` from Task 1 (referenced by name in the emitted prelude string; the guard tolerates their absence).
- Produces: no new Rust symbols; the fixed prelude string now emits the guarded bracket around `m.factory(...)`.

- [ ] **Step 1: Write the failing prelude tests**

In `crates/supersolid/src/module.rs`, inside `mod tests`, add:

```rust
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
    assert!(b.code.contains(r#"typeof $ss !== "undefined""#), "{}", b.code);
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p supersolid prelude_brackets_factory_with_module_frame prelude_bracket_is_guarded`
Expected: FAIL — the prelude does not yet contain `$ss.modEnter`.

- [ ] **Step 3: Edit `SSMOD_PRELUDE` to bracket `m.factory(...)`**

In `crates/supersolid/src/module.rs`, replace the single line

```js
    m.factory(module.exports, module, require);
```

inside `req` with:

```js
    if (typeof $ss !== "undefined" && $ss.modEnter) $ss.modEnter(id);
    try {
      m.factory(module.exports, module, require);
    } finally {
      if (typeof $ss !== "undefined" && $ss.modExit) $ss.modExit(id);
    }
```

Keep ES5 style (no optional chaining) and the existing two-space prelude indentation.

- [ ] **Step 4: Run the prelude tests to verify they pass**

Run: `cargo test -p supersolid prelude_brackets_factory_with_module_frame prelude_bracket_is_guarded`
Expected: PASS.

- [ ] **Step 5: Run the full supersolid suite (regression on existing prelude tests)**

Run: `cargo test -p supersolid`
Expected: PASS — `assemble_emits_prelude_and_entry_run`, `walk_sync_*`, and all `reparses_as_plain_js` assertions still hold with the new prelude.

- [ ] **Step 6: Commit**

```bash
git add crates/supersolid/src/module.rs
git commit -m "feat(supersolid): bracket module factory exec for signal HMR"
```
