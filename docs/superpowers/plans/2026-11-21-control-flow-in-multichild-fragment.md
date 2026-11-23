# Control-flow components in a multi-child fragment — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make a control-flow component (`Show`/`For`/`Index`/`Switch`/`Match`) render, and react to its condition/source changing, at any position in a multi-child fragment — not only when it is the sole child of an insert point.

**Architecture:** The reactive insert path (`insert` → `reconcile` → `reconcileArray` → `normalizeArray`/`flattenInto`) currently pushes a control-flow memo (a function) into the flattened node array as if it were a DOM node, so the memo is never invoked and its `when`/`each` getter is never read. Fix `flattenInto` to resolve function values (via the existing `resolve()` helper) before classifying them — mirroring SolidJS's `normalizeIncomingArray(unwrap=true)`. The resolve call happens inside the enclosing `insert` effect, so the effect subscribes to whatever the accessor reads and re-reconciles the fragment when it changes. The resolved children flatten into the tracked node array, so `reconcileArrays` handles their add/remove on each change.

**Tech Stack:** JS runtime embedded in a Rust crate (`crates/supersolid_runtime/src/render.js`), run via `superui_js::V8Engine` against the shadow DOM. Tests are Rust `#[cfg(test)]` functions in `crates/supersolid_runtime/src/lib.rs` (`render_tests` module) that `eval` JS and read values back through the `test_read` helpers (`num`, `text`).

**Spec:** This document's Background section (derived from the task's bug report). Bounded fix; no separate spec file.

## Background (spec)

A control-flow component returns a `createMemo` accessor (a function): `Show` returns `createMemo(() => props.when ? props.children : props.fallback)`; `For`/`Index`/`Switch` likewise return memos; `Match` returns a descriptor consumed by `Switch`'s memo.

When such a component is the SOLE value an insert point receives, `insert`'s two-level branch (`typeof val === "function"`) calls the accessor via `resolve()` and it renders. When it is one of several children, the transpiler collects the children into `$ss.frag([...])` (an array) and routes them through a single `insert(el, () => children)`. The array reaches `reconcile` → `reconcileArray` → `normalizeArray` → `flattenInto`, which walks each element; a function element falls through to `out.push(value)` and is handed to `parent.insertBefore(...)` as if it were a node. The memo is never called, so its `when`/`each` getter is never read and its children never render.

Required behavior: a control-flow component renders, and reacts to its condition/source changing, at ANY position in a multi-child fragment, at any nesting depth.

**Out of scope:** the `Keyed` controller (`__ssKeyed`). Unlike the memo-returning control flow, `Keyed` owns its DOM (its `update` returns `null`, so its rows are not tracked by `reconcileArrays`). A multi-child fragment is rebuilt by its insert accessor on every re-run (`children` is a getter), which would rebuild the `Keyed` controller and leak its previous rows' DOM. Supporting `Keyed` outside the sole-child (two-level) insert path needs a larger change; it stays sole-child-only. This fix does not regress sole-child `Keyed` (handled by `reconcile()`'s top-level `__ssKeyed` branch, not by `flattenInto`).

## Global Constraints

- Target file for the fix: `crates/supersolid_runtime/src/render.js` only. Tests live in `crates/supersolid_runtime/src/lib.rs` (`render_tests` module).
- Wasm-clean: use only the Phase-1 DOM API and the reactive globals already present; no new dependencies.
- Match the file's existing ES5 style (`var`, `function`, no arrow functions / `let` / `const`).
- Do not regress any existing `tests` or `render_tests` case; all must stay green — in particular the sole-child `show_*`/`for_*`/`index_*`/`switch_*`/`keyed_*` cases.

## Review Focus

- **Control-flow NOT in the first slot** (middle/last of a fragment): the resolved children must splice at the correct position relative to static siblings. — covered by `show_renders_in_middle_of_fragment`.
- **Reactive re-toggle after first render** (`when` flips true→false→true while embedded in a fragment): the branch must appear, disappear, and reappear. — covered by `show_renders_when_non_sole_child_of_fragment`.
- **A list control-flow (`For`) embedded in a fragment** (not just `Show`): its mapped rows must flatten into the parent and track add/remove. — covered by `for_renders_when_non_sole_child_of_fragment`.
- **Sole-child control-flow unchanged**: the two-level insert path (and sole-child `Keyed`) must behave exactly as before. — covered by re-running the full existing suite in Step 4.
- **A non-control-flow function never appears in a frag array** (so unconditional resolve is safe): the transpiler only ever places accessors/memos (control flow) as raw function members; dynamic holes are routed through their own `insert` thunks, not placed raw in a frag. No new test needed — asserted by the existing suite staying green plus this reasoning.

---

### Task 1: Resolve control-flow accessors when flattening fragment children

**Files:**
- Modify: `crates/supersolid_runtime/src/render.js` (`flattenInto`, ~line 246)
- Test: `crates/supersolid_runtime/src/lib.rs` (`render_tests` module)

**Interfaces:**
- Consumes: existing `resolve(value)` helper in `render.js` (`while (typeof value === "function") value = value();`), defined earlier in the same IIFE.
- Produces: no signature change. `flattenInto(out, value)` keeps its shape; it gains a leading resolve step so any function member is invoked and its result classified.

- [ ] **Step 1: Write the failing tests** in `crates/supersolid_runtime/src/lib.rs`, `render_tests` module.

```rust
#[test]
fn show_renders_when_non_sole_child_of_fragment() {
    // Repro: Show is the LAST of several children collected into a frag array
    // routed through one insert (children is rebuilt per run, as the transpiler
    // emits via a getter). A truthy `when` must render its children, and the
    // branch must toggle off and back on with the signal.
    let mut e = render_engine();
    e.eval(r#"
        var got = createSignal("true");
        globalThis.set = got[1];
        globalThis.p = $ss.el("div");
        $ss.insert(p, function () {
            return $ss.frag([
                (function(){ var t=$ss.el("h1"); $ss.child(t,$ss.txt("Title")); return t; })(),
                (function(){ var o=$ss.el("p"); $ss.child(o,$ss.txt("Output")); return o; })(),
                $ss.cmp(Show, {
                    get when() { return got[0]() === "true"; },
                    get children() { var s=$ss.el("i"); $ss.child(s,$ss.txt("Stone")); return s; }
                })
            ]);
        });
        globalThis.t0 = p.textContent;   // "TitleOutputStone"
        globalThis.set("false");
        globalThis.t1 = p.textContent;   // "TitleOutput"
        globalThis.set("true");
        globalThis.t2 = p.textContent;   // "TitleOutputStone"
    "#).unwrap();
    assert_eq!(text(&mut e, "globalThis.t0"), "TitleOutputStone");
    assert_eq!(text(&mut e, "globalThis.t1"), "TitleOutput");
    assert_eq!(text(&mut e, "globalThis.t2"), "TitleOutputStone");
}

#[test]
fn show_renders_in_middle_of_fragment() {
    // Control-flow between two static siblings: order must be preserved on toggle.
    let mut e = render_engine();
    e.eval(r#"
        var on = createSignal(true);
        globalThis.set = on[1];
        globalThis.p = $ss.el("div");
        $ss.insert(p, function () {
            return $ss.frag([
                (function(){ var a=$ss.el("i"); a.setAttribute("k","A"); return a; })(),
                $ss.cmp(Show, {
                    get when() { return on[0](); },
                    get children() { var m=$ss.el("i"); m.setAttribute("k","M"); return m; }
                }),
                (function(){ var z=$ss.el("i"); z.setAttribute("k","Z"); return z; })()
            ]);
        });
        function order(){var s="";for(var i=0;i<p.childNodes.length;i++){var n=p.childNodes[i];if(n.getAttribute){var k=n.getAttribute("k");if(k)s+=k;}}return s;}
        globalThis.o0 = order();   // "AMZ"
        globalThis.set(false);
        globalThis.o1 = order();   // "AZ"
        globalThis.set(true);
        globalThis.o2 = order();   // "AMZ"
    "#).unwrap();
    assert_eq!(text(&mut e, "globalThis.o0"), "AMZ");
    assert_eq!(text(&mut e, "globalThis.o1"), "AZ");
    assert_eq!(text(&mut e, "globalThis.o2"), "AMZ");
}

#[test]
fn for_renders_when_non_sole_child_of_fragment() {
    // A list control-flow embedded among siblings: its rows flatten into parent
    // and track removal when the source array shrinks.
    let mut e = render_engine();
    e.eval(r#"
        globalThis.a={n:"a"}; globalThis.b={n:"b"};
        var pair = createSignal([globalThis.a, globalThis.b]);
        globalThis.set = pair[1];
        globalThis.p = $ss.el("ul");
        $ss.insert(p, function () {
            return $ss.frag([
                (function(){ var h=$ss.el("li"); $ss.child(h,$ss.txt("H")); return h; })(),
                $ss.cmp(For, {
                    get each(){ return pair[0](); },
                    get children(){ return function(item){ var li=$ss.el("li"); $ss.child(li,$ss.txt(item.n)); return li; }; }
                })
            ]);
        });
        function join(){var s="";for(var i=0;i<p.childNodes.length;i++){var n=p.childNodes[i];if(n.nodeType===1)s+=n.textContent;}return s;}
        globalThis.o0 = join();   // "Hab"
        globalThis.set([globalThis.b]);
        globalThis.o1 = join();   // "Hb"
    "#).unwrap();
    assert_eq!(text(&mut e, "globalThis.o0"), "Hab");
    assert_eq!(text(&mut e, "globalThis.o1"), "Hb");
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p supersolid_runtime render_tests::show_renders_when_non_sole_child_of_fragment render_tests::show_renders_in_middle_of_fragment render_tests::for_renders_when_non_sole_child_of_fragment`
Expected: FAIL (the memo is pushed as a node; `textContent`/`order` omits the control-flow output, or `insertBefore` errors on a non-node).

- [ ] **Step 3: Implement the fix in `crates/supersolid_runtime/src/render.js`, `flattenInto`.**

As the FIRST statement of `flattenInto(out, value)` (before the null/true/false guard), add `if (typeof value === "function") value = resolve(value);` so a function member (a control-flow memo) is invoked and its result — node, array, string, or null — is classified by the branches that follow. Add a short comment: the call runs inside the enclosing `insert` effect, so the effect subscribes to whatever the accessor reads; resolved children flatten into the tracked array and are reconciled on change.

- [ ] **Step 4: Run the full crate test suite to verify the new tests pass and nothing regressed**

Run: `cargo test -p supersolid_runtime`
Expected: PASS — all `tests` and `render_tests`, including the three new cases and every existing sole-child `show_*`/`for_*`/`index_*`/`switch_*`/`keyed_*`.

- [ ] **Step 5: Commit**

```bash
git add crates/supersolid_runtime/src/render.js crates/supersolid_runtime/src/lib.rs
git commit -m "fix(supersolid): evaluate control-flow nested in a multi-child fragment"
```
