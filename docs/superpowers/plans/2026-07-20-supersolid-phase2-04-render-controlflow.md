# Supersolid Phase 2 — Plan 4: render + control-flow layer — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the `$ss.*` JSX runtime ABI (`el`/`txt`/`attr`/`child`/`insert`/`bind`/`on`/`cmp`/`frag`) as build-once arena-DOM nodes with surgical reactive bindings, plus the root `render()` and the Solid-style control-flow components `<Show>`/`<For>`/`<Index>`/`<Switch>`/`<Match>` — and fix the transpiler's dropped fragment-in-element case.

**Architecture:** A new wasm-clean JS module `crates/supersolid_runtime/src/render.js`, embedded via `include_str!` and eval'd by the existing `supersolid_runtime::install` right after `runtime.js`. It calls **only** the DOM API already installed by `superui_api` (`document.createElement`/`createTextNode`, `appendChild`/`insertBefore`/`removeChild`, text-node `.data`, `setAttribute`, `addEventListener`) and composes on the Plan 3 reactive globals (`createEffect`/`createRoot`/`createMemo`/`untrack`/`onCleanup`). Dynamic children reconcile **around an empty anchor text node** (Plan 1 deferred `createDocumentFragment`). Control-flow components return **memoized accessors** that `$ss.insert` resolves by calling-until-non-function (the Solid model); branch/list disposal falls out of memo recomputation. Everything downstream (reconciler → ECS) is untouched Phase-1 machinery (direction §7). The design lives in [`../specs/2026-07-20-supersolid-render-controlflow-design.md`](../specs/2026-07-20-supersolid-render-controlflow-design.md).

**Tech Stack:** JS (hand-written, run in Boa 0.21), Rust edition 2021, the existing `superui_*` crates + `supersolid` (transpiler). `supersolid_runtime` stays Bevy-free and wasm-clean.

## Global Constraints

- **Bevy 0.17**, edition 2021.
- `supersolid_runtime` is **Bevy-free and wasm-clean**: pure JS + a thin Boa install. It depends only on `superui_js` (and, for tests, `superui_dom` + `superui_api` + `boa_engine`). It must **never** depend on `oxc` or Bevy.
- **TDD** throughout; **frequent commits**; work on `main` (per project CLAUDE.md — no feature branch needed).
- **Graceful degradation (design §1):** author-script errors already log-and-swallow through `UiRuntime::run_script`. The render layer's own `render.js` is vetted by this plan's tests, so `install` may treat a failed eval of *its own* source as a hard bug (as it already does for `runtime.js`).
- The render layer calls **only** the Phase-1 DOM API and the Plan 3 reactive globals — it never touches Bevy, the reconciler, or `oxc`.
- **Node identity is stable:** `superui_js::wrap_node` caches one JS wrapper per `NodeId` (`state.rs:59`), so `===` on two references to the same DOM node is true. The reconcile algorithms rely on this.
- **DOM mutations settle before reconcile:** Plan 3's scheduler is synchronous; author scripts (`run_script`), event callbacks (`dispatch_event`), and timers (`run_timers`) each set `dirty` after running, before `reconcile_system` clears it. This layer adds no per-frame pump.

### The `$ss` ABI this plan implements (emitted by Plan 2's transpiler)

| Helper | Contract |
|---|---|
| `$ss.el(tag)` | `document.createElement(tag)` → element node |
| `$ss.txt(data)` | `document.createTextNode(data)` → text node |
| `$ss.attr(el, name, value)` | static attribute/property set (value already a string) |
| `$ss.child(parent, node)` | append a static child; array/fragment-aware (flattens) |
| `$ss.insert(parent, thunk)` | dynamic child: `thunk = () => expr` (text/node/component/list), reconciled around an anchor |
| `$ss.bind(el, name, thunk)` | dynamic attribute: `createEffect(() => set(el, name, thunk()))` |
| `$ss.on(el, type, handler)` | `el.addEventListener(type, handler)` (handler as-is) |
| `$ss.cmp(Comp, props)` | `untrack(() => Comp(props))` — component runs once |
| `$ss.frag(children)` | returns the `children` array (flattened by `insert`/`child`) |

Author-facing globals published here: `render`, `Show`, `For`, `Index`, `Switch`, `Match`.

---

## Task 1: `render.js` scaffold + static builders + install wiring

Establish `render.js`, wire it into `supersolid_runtime::install`, add the `superui_api` dev-dependency + a headless test harness, and implement the static builders `el` / `txt` / `attr` / `child` (with the property-vs-attribute rule). No reactivity yet.

**Files:**
- Modify: `crates/supersolid_runtime/Cargo.toml` (add `superui_api` dev-dependency)
- Modify: `crates/supersolid_runtime/src/lib.rs` (embed + eval `render.js`; add render test harness + tests)
- Create: `crates/supersolid_runtime/src/render.js`

**Interfaces:**
- Consumes: the DOM globals from `superui_api::install` (`document.createElement`/`createTextNode`, `element.setAttribute`, `element.value`/`checked`, `parent.appendChild`, `childNodes`, `tagName`, `textContent`); the reactive globals from `runtime.js` (later tasks).
- Produces:
  - `render.js` publishes `globalThis.$ss = { el, txt, attr, child }` (extended by later tasks).
  - `install(&mut BoaEngine)` now evals `runtime.js` **then** `render.js`.
  - Test harness `fn render_engine() -> BoaEngine` (installs `superui_api` + `supersolid_runtime`) and the `num`/`text` read-back helpers (reused from the existing tests module).

- [ ] **Step 1: Add the `superui_api` dev-dependency**

In `crates/supersolid_runtime/Cargo.toml`, add to `[dev-dependencies]` (next to `superui_dom` / `boa_engine`):

```toml
superui_api = { path = "../superui_api" }
```

- [ ] **Step 2: Write the failing tests**

Add a new `#[cfg(test)] mod render_tests` at the bottom of `crates/supersolid_runtime/src/lib.rs`:

```rust
#[cfg(test)]
mod render_tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;
    use superui_dom::Dom;

    /// A BoaEngine with the DOM/Web API (superui_api) AND the reactive+render
    /// runtime installed — the full surface author `.tsx` runs against.
    fn render_engine() -> BoaEngine {
        let dom = Rc::new(RefCell::new(Dom::new()));
        let mut e = BoaEngine::new(dom);
        superui_api::install(&mut e);
        install(&mut e);
        e
    }

    fn num(e: &mut BoaEngine, expr: &str) -> f64 {
        e.context_mut()
            .eval(boa_engine::Source::from_bytes(expr))
            .unwrap()
            .as_number()
            .unwrap_or(f64::NAN)
    }

    fn text(e: &mut BoaEngine, expr: &str) -> String {
        let v = e
            .context_mut()
            .eval(boa_engine::Source::from_bytes(expr))
            .unwrap();
        v.to_string(e.context_mut()).unwrap().to_std_string_escaped()
    }

    #[test]
    fn el_and_txt_and_child_build_dom() {
        let mut e = render_engine();
        e.eval(
            r#"
            globalThis.p = $ss.el("div");
            $ss.child(p, $ss.el("span"));
            $ss.child(p, $ss.txt("hi"));
            globalThis.count = p.childNodes.length;   // 2
            globalThis.tag0 = p.childNodes[0].tagName; // "SPAN"
            globalThis.txt1 = p.childNodes[1].data;    // "hi"
            "#,
        )
        .unwrap();
        assert_eq!(num(&mut e, "globalThis.count"), 2.0);
        assert_eq!(text(&mut e, "globalThis.tag0"), "SPAN");
        assert_eq!(text(&mut e, "globalThis.txt1"), "hi");
    }

    #[test]
    fn attr_sets_attribute_and_value_property() {
        let mut e = render_engine();
        e.eval(
            r#"
            globalThis.a = $ss.el("div");
            $ss.attr(a, "class", "box");
            globalThis.b = $ss.el("input");
            $ss.attr(b, "value", "typed");
            globalThis.cls = a.getAttribute ? a.getAttribute("class") : a.className;
            globalThis.val = b.value;   // property path
            "#,
        )
        .unwrap();
        // `class` reaches the class attribute (read back via className accessor).
        assert_eq!(text(&mut e, "globalThis.a.className"), "box");
        assert_eq!(text(&mut e, "globalThis.val"), "typed");
    }

    #[test]
    fn child_flattens_arrays() {
        let mut e = render_engine();
        e.eval(
            r#"
            globalThis.p = $ss.el("div");
            $ss.child(p, [ $ss.el("span"), $ss.txt("x") ]);
            globalThis.count = p.childNodes.length;   // 2
            "#,
        )
        .unwrap();
        assert_eq!(num(&mut e, "globalThis.count"), 2.0);
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p supersolid_runtime render_tests`
Expected: FAIL to compile — `render.js` does not exist yet (`include_str!` in Step 5 not added) and `$ss` is undefined. (First add an empty `render.js` if the compile error blocks you; then the tests fail at runtime with `$ss is not defined`.)

- [ ] **Step 4: Create `render.js` with the static builders**

Create `crates/supersolid_runtime/src/render.js`:

```js
// Supersolid render + control-flow layer — implements the $ss.* ABI the Plan 2
// transpiler emits, plus render() and control flow. Wasm-clean: calls only the
// Phase-1 DOM API and the Plan 3 reactive globals (already on globalThis).
(function () {
  "use strict";

  // Names that map to live element STATE — set as a property, not an attribute.
  var PROPERTY_NAMES = { value: true, checked: true };

  // Apply a name/value to an element via the property-vs-attribute rule.
  function setProp(el, name, value) {
    if (PROPERTY_NAMES[name]) {
      el[name] = value;
    } else if (value == null || value === false) {
      // No removeAttribute in the DOM subset; empty string is the degradation.
      el.setAttribute(name, "");
    } else {
      el.setAttribute(name, "" + value);
    }
  }

  function el(tag) { return document.createElement(tag); }
  function txt(data) { return document.createTextNode(data); }
  function attr(element, name, value) { setProp(element, name, value); }

  // Append a static child. Array/fragment-aware: arrays flatten; primitives
  // become text nodes; null/booleans are skipped (nothing rendered).
  function child(parent, node) { appendFlat(parent, node); }

  function appendFlat(parent, node) {
    if (node == null || node === true || node === false) return;
    if (Array.isArray(node)) {
      for (var i = 0; i < node.length; i++) appendFlat(parent, node[i]);
      return;
    }
    if (typeof node === "string" || typeof node === "number") {
      parent.appendChild(txt("" + node));
      return;
    }
    parent.appendChild(node);
  }

  // ---- Publish the ABI (extended by later tasks) ----
  globalThis.$ss = {
    el: el,
    txt: txt,
    attr: attr,
    child: child,
  };
})();
```

- [ ] **Step 5: Wire `render.js` into `install`**

In `crates/supersolid_runtime/src/lib.rs`, add the embed constant next to `RUNTIME_JS` and eval it in `install`:

```rust
/// The reactive core, embedded at build time.
const RUNTIME_JS: &str = include_str!("runtime.js");
/// The render + control-flow layer, embedded at build time.
const RENDER_JS: &str = include_str!("render.js");

pub fn install(engine: &mut BoaEngine) {
    engine
        .eval(RUNTIME_JS)
        .expect("supersolid_runtime: runtime.js must evaluate (internal invariant)");
    engine
        .eval(RENDER_JS)
        .expect("supersolid_runtime: render.js must evaluate (internal invariant)");
}
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p supersolid_runtime`
Expected: PASS (all existing reactive-core tests + the three new render tests).

- [ ] **Step 7: Commit**

```bash
git add crates/supersolid_runtime/Cargo.toml crates/supersolid_runtime/src/lib.rs crates/supersolid_runtime/src/render.js
git commit -m "feat(supersolid_runtime): render layer scaffold + static builders (el/txt/attr/child)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 2: Reactive holes — `$ss.on` and `$ss.bind`

Add event listeners (`on`) and reactive attributes (`bind`, an effect over the property/attribute rule). Both use the machinery from Task 1.

**Files:**
- Modify: `crates/supersolid_runtime/src/render.js`
- Test: `crates/supersolid_runtime/src/lib.rs` (`render_tests`)

**Interfaces:**
- Consumes: `setProp` (Task 1); `element.addEventListener`; `createEffect` (runtime.js global); `BoaEngine::dispatch_event(node, type, key, bubbles, cancelable)` (Rust-side event dispatch, same call the input systems use).
- Produces: `$ss.on(el, type, handler)`, `$ss.bind(el, name, thunk)` added to the published `$ss` object; a `render_engine_with_dom() -> (BoaEngine, Rc<RefCell<Dom>>)` test helper (exposes the DOM so a test can resolve a `NodeId` and dispatch to it).

- [ ] **Step 1: Write the failing tests**

Add the DOM-exposing harness next to `render_engine` in `render_tests`:

```rust
fn render_engine_with_dom() -> (BoaEngine, Rc<RefCell<Dom>>) {
    let dom = Rc::new(RefCell::new(Dom::new()));
    let mut e = BoaEngine::new(dom.clone());
    superui_api::install(&mut e);
    install(&mut e);
    (e, dom)
}
```

Add to `render_tests`:

```rust
#[test]
fn bind_updates_attribute_reactively() {
    let mut e = render_engine();
    e.eval(
        r#"
        var pair = createSignal("a");
        globalThis.get = pair[0]; globalThis.set = pair[1];
        globalThis.el = $ss.el("div");
        $ss.bind(el, "class", function () { return globalThis.get(); });
        globalThis.c0 = el.className;   // "a" — effect ran once on bind
        globalThis.set("b");
        globalThis.c1 = el.className;   // "b" — surgical re-run
        "#,
    )
    .unwrap();
    assert_eq!(text(&mut e, "globalThis.c0"), "a");
    assert_eq!(text(&mut e, "globalThis.c1"), "b");
}

#[test]
fn on_fires_a_registered_click_handler() {
    // Events dispatch from the Rust side (BoaEngine::dispatch_event), not from JS.
    // Attach the button to the document so we can resolve its NodeId and dispatch.
    let (mut e, dom) = render_engine_with_dom();
    e.eval(
        r#"
        globalThis.clicks = 0;
        var b = $ss.el("button");
        b.setAttribute("id", "btn");
        document.appendChild(b);
        $ss.on(b, "click", function () { globalThis.clicks++; });
        "#,
    )
    .unwrap();
    let btn = { let d = dom.borrow(); d.get_element_by_id("btn").unwrap() };
    e.dispatch_event(btn, "click", None, true, true);
    assert_eq!(num(&mut e, "globalThis.clicks"), 1.0);
}
```

> **Guidance:** confirm `BoaEngine::dispatch_event`'s exact signature against `crates/superui_api/src/lib.rs` (its tests call `e.dispatch_event(leaf, "click", None, true, true)`); adjust the `None` (key) arg to whatever the signature expects. `document.appendChild` works because the document proto carries `appendChild` (`node.rs:152`). This proves `$ss.on` registers a *working* listener; Task 10 proves the same path end-to-end through the bridge.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p supersolid_runtime bind_updates_attribute_reactively on_registers`
Expected: FAIL — `$ss.bind`/`$ss.on` are `undefined`.

- [ ] **Step 3: Implement `on` and `bind`**

In `crates/supersolid_runtime/src/render.js`, add these functions before the publish block:

```js
  function on(element, type, handler) {
    element.addEventListener(type, handler);
  }

  function bind(element, name, thunk) {
    // One effect per dynamic attribute: re-applies surgically on dep change.
    createEffect(function () { setProp(element, name, thunk()); });
  }
```

And extend the published object:

```js
  globalThis.$ss = {
    el: el,
    txt: txt,
    attr: attr,
    child: child,
    on: on,
    bind: bind,
  };
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p supersolid_runtime`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/supersolid_runtime/src/render.js crates/supersolid_runtime/src/lib.rs
git commit -m "feat(supersolid_runtime): reactive holes — \$ss.on + \$ss.bind

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 3: `$ss.insert` core — text / node / null around an anchor

The reconciling dynamic child for single (non-array) values: surgical text-in-place, single node, and empty. This introduces the anchor model and the `reconcile` dispatcher (array branch is Task 5).

**Files:**
- Modify: `crates/supersolid_runtime/src/render.js`
- Test: `crates/supersolid_runtime/src/lib.rs` (`render_tests`)

**Interfaces:**
- Consumes: `txt` (Task 1); `createEffect` (runtime.js).
- Produces: `$ss.insert(parent, accessor)` — appends an empty text-node anchor, then `createEffect` reconciling `accessor()` before the anchor. Internal helpers `resolve(value)`, `reconcile(parent, anchor, current, value)`, `clearNodes(parent, current)`, `removeNode(parent, node)` (the array branch of `reconcile` is a no-op stub until Task 5).

- [ ] **Step 1: Write the failing tests**

Add to `render_tests`:

```rust
#[test]
fn insert_text_updates_in_place() {
    let mut e = render_engine();
    e.eval(
        r#"
        var pair = createSignal(1);
        globalThis.get = pair[0]; globalThis.set = pair[1];
        globalThis.p = $ss.el("div");
        $ss.child(p, $ss.txt("n="));
        $ss.insert(p, function () { return globalThis.get(); });
        globalThis.t0 = p.textContent;               // "n=1"
        globalThis.firstText = p.childNodes[1];      // the inserted text node
        globalThis.set(2);
        globalThis.t1 = p.textContent;               // "n=2"
        globalThis.sameNode = (p.childNodes[1] === globalThis.firstText); // true — surgical
        "#,
    )
    .unwrap();
    assert_eq!(text(&mut e, "globalThis.t0"), "n=1");
    assert_eq!(text(&mut e, "globalThis.t1"), "n=2");
    assert_eq!(text(&mut e, "globalThis.sameNode"), "true");
}

#[test]
fn insert_null_renders_nothing_and_toggles_a_node() {
    let mut e = render_engine();
    e.eval(
        r#"
        var pair = createSignal(false);
        globalThis.get = pair[0]; globalThis.set = pair[1];
        globalThis.span = $ss.el("span");
        $ss.child(span, $ss.txt("shown"));
        globalThis.p = $ss.el("div");
        $ss.insert(p, function () { return globalThis.get() ? globalThis.span : null; });
        globalThis.t0 = p.textContent;   // "" — false renders nothing
        globalThis.set(true);
        globalThis.t1 = p.textContent;   // "shown"
        globalThis.set(false);
        globalThis.t2 = p.textContent;   // "" again
        "#,
    )
    .unwrap();
    assert_eq!(text(&mut e, "globalThis.t0"), "");
    assert_eq!(text(&mut e, "globalThis.t1"), "shown");
    assert_eq!(text(&mut e, "globalThis.t2"), "");
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p supersolid_runtime insert_text_updates_in_place insert_null_renders`
Expected: FAIL — `$ss.insert` is `undefined`.

- [ ] **Step 3: Implement `insert` + `reconcile` (single-value branches)**

In `crates/supersolid_runtime/src/render.js`, add before the publish block:

```js
  // Resolve control-flow accessors/memos: call until non-function. Runs INSIDE
  // the insert effect, so the effect subscribes to whatever the accessor reads.
  function resolve(value) {
    while (typeof value === "function") value = value();
    return value;
  }

  function removeNode(parent, node) {
    if (node && node.parentNode === parent) parent.removeChild(node);
  }

  function clearNodes(parent, current) {
    if (current == null) return;
    if (Array.isArray(current)) {
      for (var i = 0; i < current.length; i++) removeNode(parent, current[i]);
    } else {
      removeNode(parent, current);
    }
  }

  // Reconcile the DOM before `anchor` from `current` to represent `value`.
  // Returns the new `current` (null | Node | Node[]). Array branch: Task 5.
  function reconcile(parent, anchor, current, value) {
    if (value == null || value === true || value === false) {
      clearNodes(parent, current);
      return null;
    }
    var t = typeof value;
    if (t === "string" || t === "number") {
      // Surgical: reuse an existing single text node.
      if (current && !Array.isArray(current) && current.nodeType === 3) {
        current.data = "" + value;
        return current;
      }
      clearNodes(parent, current);
      var node = txt("" + value);
      parent.insertBefore(node, anchor);
      return node;
    }
    if (Array.isArray(value)) {
      return reconcileArray(parent, anchor, current, value); // Task 5
    }
    // Single DOM node.
    if (current === value) return current;
    clearNodes(parent, current);
    parent.insertBefore(value, anchor);
    return value;
  }

  // Placeholder until Task 5 — a single-element list still works via replace.
  function reconcileArray(parent, anchor, current, value) {
    clearNodes(parent, current);
    var out = [];
    for (var i = 0; i < value.length; i++) {
      var v = value[i];
      if (v == null || v === true || v === false) continue;
      if (typeof v === "string" || typeof v === "number") v = txt("" + v);
      parent.insertBefore(v, anchor);
      out.push(v);
    }
    return out.length ? out : null;
  }

  function insert(parent, accessor) {
    var anchor = txt("");
    parent.appendChild(anchor);
    var current = null;
    createEffect(function () {
      current = reconcile(parent, anchor, current, resolve(accessor()));
    });
  }
```

Extend the published object with `insert: insert`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p supersolid_runtime`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/supersolid_runtime/src/render.js crates/supersolid_runtime/src/lib.rs
git commit -m "feat(supersolid_runtime): \$ss.insert core — surgical text, node, null around an anchor

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 4: `$ss.cmp` + `$ss.frag` + array normalization in `insert`

Components (run-once, `untrack`) and fragments (arrays), and make `insert` flatten arbitrary array/fragment/primitive shapes into a flat node list (still using the Task-3 replace-based `reconcileArray`).

**Files:**
- Modify: `crates/supersolid_runtime/src/render.js`
- Test: `crates/supersolid_runtime/src/lib.rs` (`render_tests`)

**Interfaces:**
- Consumes: `untrack` (runtime.js); `txt` (Task 1).
- Produces: `$ss.cmp(Comp, props)`, `$ss.frag(children)`; a `normalizeArray(value)` helper flattening nested arrays/fragments and converting primitives to text nodes, used by `reconcileArray`.

- [ ] **Step 1: Write the failing tests**

Add to `render_tests`:

```rust
#[test]
fn cmp_runs_component_once_and_inserts_its_nodes() {
    let mut e = render_engine();
    e.eval(
        r#"
        globalThis.calls = 0;
        function Box(props) {
            globalThis.calls++;
            var d = $ss.el("div");
            $ss.child(d, $ss.txt("boxed:"));
            $ss.insert(d, function () { return props.label; });
            return d;
        }
        var pair = createSignal("x");
        globalThis.set = pair[1];
        globalThis.p = $ss.el("main");
        $ss.insert(p, function () {
            return $ss.cmp(Box, { get label() { return pair[0](); } });
        });
        globalThis.t0 = p.textContent;   // "boxed:x"
        globalThis.callsAfter = globalThis.calls; // 1
        globalThis.set("y");
        globalThis.t1 = p.textContent;   // "boxed:y" — inner insert re-ran, body did not
        globalThis.callsAfter2 = globalThis.calls; // still 1 (runs once)
        "#,
    )
    .unwrap();
    assert_eq!(text(&mut e, "globalThis.t0"), "boxed:x");
    assert_eq!(num(&mut e, "globalThis.callsAfter"), 1.0);
    assert_eq!(text(&mut e, "globalThis.t1"), "boxed:y");
    assert_eq!(num(&mut e, "globalThis.callsAfter2"), 1.0);
}

#[test]
fn frag_and_insert_flatten_into_the_parent() {
    let mut e = render_engine();
    e.eval(
        r#"
        globalThis.p = $ss.el("div");
        $ss.insert(p, function () {
            return $ss.frag([ $ss.el("a"), $ss.txt("mid"), $ss.el("b") ]);
        });
        globalThis.count = p.childNodes.length;  // 3 content + 1 anchor = 4
        globalThis.first = p.childNodes[0].tagName;  // "A"
        globalThis.mid = p.childNodes[1].data;       // "mid"
        globalThis.last = p.childNodes[2].tagName;   // "B"
        "#,
    )
    .unwrap();
    assert_eq!(num(&mut e, "globalThis.count"), 4.0);
    assert_eq!(text(&mut e, "globalThis.first"), "A");
    assert_eq!(text(&mut e, "globalThis.mid"), "mid");
    assert_eq!(text(&mut e, "globalThis.last"), "B");
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p supersolid_runtime cmp_runs_component frag_and_insert`
Expected: FAIL — `$ss.cmp`/`$ss.frag` undefined; nested fragments not flattened.

- [ ] **Step 3: Implement `cmp`, `frag`, and `normalizeArray`**

In `crates/supersolid_runtime/src/render.js`, add before the publish block:

```js
  function cmp(Comp, props) {
    // Components run ONCE, untracked (fine-grained model). props carries getters
    // for dynamic props (transpiler); dynamic bits become inner effects.
    return untrack(function () { return Comp(props); });
  }

  function frag(children) { return children; }

  // Flatten nested arrays/fragments; drop null/booleans; primitives -> text.
  function normalizeArray(value) {
    var out = [];
    flattenInto(out, value);
    return out;
  }

  function flattenInto(out, value) {
    if (value == null || value === true || value === false) return;
    if (Array.isArray(value)) {
      for (var i = 0; i < value.length; i++) flattenInto(out, value[i]);
      return;
    }
    if (typeof value === "string" || typeof value === "number") {
      out.push(txt("" + value));
      return;
    }
    out.push(value);
  }
```

Replace the Task-3 `reconcileArray` stub's normalization with `normalizeArray` (still replace-based for now — Task 5 makes it keyed):

```js
  function reconcileArray(parent, anchor, current, value) {
    var next = normalizeArray(value);
    if (next.length === 0) {
      clearNodes(parent, current);
      return null;
    }
    clearNodes(parent, current);
    for (var i = 0; i < next.length; i++) parent.insertBefore(next[i], anchor);
    return next;
  }
```

Extend the published object with `cmp: cmp` and `frag: frag`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p supersolid_runtime`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/supersolid_runtime/src/render.js crates/supersolid_runtime/src/lib.rs
git commit -m "feat(supersolid_runtime): \$ss.cmp + \$ss.frag + insert array normalization

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 5: `$ss.insert` keyed minimal-move array reconcile

Replace the replace-based `reconcileArray` with an identity-keyed minimal-move algorithm (adapted from Solid's `reconcileArrays`), so retained nodes are reused and reordered with the fewest moves. This is what makes `<For>` (Task 7) surgical.

**Files:**
- Modify: `crates/supersolid_runtime/src/render.js`
- Test: `crates/supersolid_runtime/src/lib.rs` (`render_tests`)

**Interfaces:**
- Consumes: `normalizeArray`, `clearNodes`, `removeNode` (Task 4/3); `node.nextSibling`, `parent.insertBefore`, `parent.removeChild`, `parent.replaceChild`.
- Produces: `reconcileArray(parent, anchor, current, value)` doing an identity-keyed diff; contract: after it runs, `parent`'s children strictly before `anchor` equal `normalizeArray(value)` in order; nodes shared with `current` are reused (identity preserved); moves are bounded by items actually displaced.

- [ ] **Step 1: Write the failing tests**

Add to `render_tests`. Each drives an array signal and asserts order + node reuse by identity:

```rust
#[test]
fn insert_array_reorders_reusing_nodes() {
    let mut e = render_engine();
    e.eval(
        r#"
        // Build three stable element nodes keyed by identity.
        globalThis.A = $ss.el("i"); A.setAttribute("k", "A");
        globalThis.B = $ss.el("i"); B.setAttribute("k", "B");
        globalThis.C = $ss.el("i"); C.setAttribute("k", "C");
        var pair = createSignal([A, B, C]);
        globalThis.set = pair[1];
        globalThis.p = $ss.el("div");
        $ss.insert(p, function () { return pair[0](); });
        function order() {
            var s = "";
            for (var i = 0; i < p.childNodes.length; i++) {
                var n = p.childNodes[i];
                if (n.getAttribute) { var k = n.getAttribute("k"); if (k) s += k; }
            }
            return s;
        }
        globalThis.o0 = order();          // "ABC"
        globalThis.set([C, A, B]);        // rotate
        globalThis.o1 = order();          // "CAB"
        globalThis.reusedA = (p.childNodes[1] === A); // A reused (identity)
        globalThis.set([B]);              // shrink, drop A and C
        globalThis.o2 = order();          // "B"
        "#,
    )
    .unwrap();
    assert_eq!(text(&mut e, "globalThis.o0"), "ABC");
    assert_eq!(text(&mut e, "globalThis.o1"), "CAB");
    assert_eq!(text(&mut e, "globalThis.reusedA"), "true");
    assert_eq!(text(&mut e, "globalThis.o2"), "B");
}

#[test]
fn insert_array_appends_and_prepends() {
    let mut e = render_engine();
    e.eval(
        r#"
        globalThis.A = $ss.el("i"); A.setAttribute("k","A");
        globalThis.B = $ss.el("i"); B.setAttribute("k","B");
        globalThis.C = $ss.el("i"); C.setAttribute("k","C");
        var pair = createSignal([B]);
        globalThis.set = pair[1];
        globalThis.p = $ss.el("div");
        $ss.insert(p, function () { return pair[0](); });
        function order() {
            var s = "";
            for (var i=0;i<p.childNodes.length;i++){var n=p.childNodes[i];if(n.getAttribute){var k=n.getAttribute("k");if(k)s+=k;}}
            return s;
        }
        globalThis.o0 = order();      // "B"
        globalThis.set([A, B]);       // prepend A
        globalThis.o1 = order();      // "AB"
        globalThis.set([A, B, C]);    // append C
        globalThis.o2 = order();      // "ABC"
        "#,
    )
    .unwrap();
    assert_eq!(text(&mut e, "globalThis.o0"), "B");
    assert_eq!(text(&mut e, "globalThis.o1"), "AB");
    assert_eq!(text(&mut e, "globalThis.o2"), "ABC");
}

// THE RED DRIVER for this task. Node wrappers are identity-stable, so the Task-4
// replace-based stub yields the SAME final DOM as minimal-move (the two order
// tests above pass under both). What distinguishes them is HOW MANY DOM ops a
// reorder costs. Spy on the parent's insertBefore/removeChild and assert a single
// item moving in a list of four costs only a couple of ops — a full rebuild would
// be 4 removes + 4 inserts = 8.
#[test]
fn insert_array_reorder_is_minimal_moves() {
    let mut e = render_engine();
    e.eval(
        r#"
        globalThis.A=$ss.el("i"); globalThis.B=$ss.el("i");
        globalThis.C=$ss.el("i"); globalThis.D=$ss.el("i");
        var pair = createSignal([A, B, C, D]);
        globalThis.set = pair[1];
        globalThis.p = $ss.el("div");
        $ss.insert(p, function () { return pair[0](); });

        // Install op-count spies AFTER the initial render (shadow the proto methods).
        globalThis.ops = 0;
        var proto = Object.getPrototypeOf(p);
        p.insertBefore = function (n, r) { globalThis.ops++; return proto.insertBefore.call(p, n, r); };
        p.removeChild  = function (n)    { globalThis.ops++; return proto.removeChild.call(p, n); };

        globalThis.set([A, C, D, B]);   // move B from index 1 to the end
        globalThis.opsAfter = globalThis.ops;
        "#,
    )
    .unwrap();
    // Minimal-move handles this in <= 2 ops; the replace-based stub would use 8.
    let ops = num(&mut e, "globalThis.opsAfter");
    assert!(ops <= 2.0, "expected minimal moves (<=2 ops), got {ops}");
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p supersolid_runtime insert_array_reorder_is_minimal_moves insert_array_reorders insert_array_appends`
Expected: `insert_array_reorder_is_minimal_moves` FAILS against the Task-4 stub (it clears + re-inserts the whole list → ~8 ops, not ≤ 2). The two order/identity tests (`insert_array_reorders_reusing_nodes`, `insert_array_appends_and_prepends`) already PASS under the stub — they characterize ordering correctness, which the new algorithm must preserve. The op-count test is the red driver; keep all three.

- [ ] **Step 3: Implement the keyed reconcile**

In `crates/supersolid_runtime/src/render.js`, replace `reconcileArray` with the adapted Solid algorithm (uses `anchor` as the trailing marker; `.remove()` → `parent.removeChild`):

```js
  function reconcileArray(parent, anchor, current, value) {
    var next = normalizeArray(value);
    var a = current == null ? [] : (Array.isArray(current) ? current : [current]);
    if (next.length === 0) { clearNodes(parent, a); return null; }
    if (a.length === 0) {
      for (var i = 0; i < next.length; i++) parent.insertBefore(next[i], anchor);
      return next;
    }
    reconcileArrays(parent, anchor, a, next);
    return next;
  }

  // Identity-keyed minimal-move diff (adapted from Solid's reconcileArrays).
  // `after` is the trailing marker (our anchor). Nodes are identity-stable, so
  // === and Map keys work. `.remove()` is expressed as parent.removeChild.
  function reconcileArrays(parentNode, after, a, b) {
    var bLength = b.length,
      aEnd = a.length,
      bEnd = bLength,
      aStart = 0,
      bStart = 0,
      map = null;

    while (aStart < aEnd || bStart < bEnd) {
      if (a[aStart] === b[bStart]) { aStart++; bStart++; continue; }
      while (aEnd > aStart && bEnd > bStart && a[aEnd - 1] === b[bEnd - 1]) { aEnd--; bEnd--; }
      if (aEnd === aStart) {
        // pure insert: reference is the node after the insertion window
        var node = bEnd < bLength ? b[bEnd] : after;
        while (bStart < bEnd) parentNode.insertBefore(b[bStart++], node);
      } else if (bEnd === bStart) {
        // pure remove
        while (aStart < aEnd) {
          if (!map || !map.has(a[aStart])) parentNode.removeChild(a[aStart]);
          aStart++;
        }
      } else if (a[aStart] === b[bEnd - 1] && b[bStart] === a[aEnd - 1]) {
        // reversal at the ends: swap
        var mnode = a[--aEnd].nextSibling;
        parentNode.insertBefore(b[bStart++], a[aStart++].nextSibling);
        parentNode.insertBefore(b[--bEnd], mnode);
        a[aEnd] = b[bEnd];
      } else {
        if (!map) {
          map = new Map();
          var i = bStart;
          while (i < bEnd) map.set(b[i], i++);
        }
        var index = map.get(a[aStart]);
        if (index != null) {
          if (bStart < index && index < bEnd) {
            var i2 = aStart, sequence = 1, t;
            while (++i2 < aEnd && i2 < bEnd) {
              t = map.get(a[i2]);
              if (t == null || t !== index + sequence) break;
              sequence++;
            }
            if (sequence > index - bStart) {
              var refNode = a[aStart];
              while (bStart < index) parentNode.insertBefore(b[bStart++], refNode);
            } else {
              parentNode.replaceChild(b[bStart++], a[aStart++]);
            }
          } else aStart++;
        } else parentNode.removeChild(a[aStart++]);
      }
    }
  }
```

> **Guidance:** the invariant that makes `after` correct is that this hole's content sits contiguously immediately before its anchor (each `$ss.insert` owns its own anchor). `b[bEnd]` in the insert branch is a node already positioned later in `b` (identity-stable), so inserting before it is valid. Let the two tests drive correctness; if a shuffle case fails, compare against Solid's `reconcileArrays` reference (same variable names) for the exact branch.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p supersolid_runtime`
Expected: PASS (both new array tests + all prior).

- [ ] **Step 5: Commit**

```bash
git add crates/supersolid_runtime/src/render.js crates/supersolid_runtime/src/lib.rs
git commit -m "feat(supersolid_runtime): keyed minimal-move array reconcile in \$ss.insert

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 6: `<Show>`

The conditional control-flow component: a memoized accessor returning `children` when `when` is truthy, else `fallback`. Branch disposal falls out of memo recomputation.

**Files:**
- Modify: `crates/supersolid_runtime/src/render.js`
- Test: `crates/supersolid_runtime/src/lib.rs` (`render_tests`)

**Interfaces:**
- Consumes: `createMemo` (runtime.js).
- Produces: `globalThis.Show` — `Show(props)` returns `createMemo(() => props.when ? props.children : props.fallback)`. `props.when` / `props.children` / `props.fallback` are getters (transpiler); `children` builds nodes on access.

- [ ] **Step 1: Write the failing tests**

Add to `render_tests`:

```rust
#[test]
fn show_toggles_children_and_fallback() {
    let mut e = render_engine();
    e.eval(
        r#"
        var pair = createSignal(false);
        globalThis.set = pair[1];
        globalThis.p = $ss.el("div");
        $ss.insert(p, function () {
            return $ss.cmp(Show, {
                get when() { return pair[0](); },
                get children() { var s = $ss.el("span"); $ss.child(s, $ss.txt("yes")); return s; },
                get fallback() { var f = $ss.el("em"); $ss.child(f, $ss.txt("no")); return f; },
            });
        });
        globalThis.t0 = p.textContent;   // "no"
        globalThis.set(true);
        globalThis.t1 = p.textContent;   // "yes"
        globalThis.set(false);
        globalThis.t2 = p.textContent;   // "no"
        "#,
    )
    .unwrap();
    assert_eq!(text(&mut e, "globalThis.t0"), "no");
    assert_eq!(text(&mut e, "globalThis.t1"), "yes");
    assert_eq!(text(&mut e, "globalThis.t2"), "no");
}

#[test]
fn show_disposes_hidden_branch_effects() {
    let mut e = render_engine();
    e.eval(
        r#"
        globalThis.binds = 0;
        var when = createSignal(true);
        var label = createSignal("a");
        globalThis.setWhen = when[1]; globalThis.setLabel = label[1];
        globalThis.p = $ss.el("div");
        $ss.insert(p, function () {
            return $ss.cmp(Show, {
                get when() { return when[0](); },
                get children() {
                    var s = $ss.el("span");
                    $ss.bind(s, "class", function () { globalThis.binds++; return label[0](); });
                    return s;
                },
            });
        });
        globalThis.b0 = globalThis.binds;     // 1 — bind ran once while shown
        globalThis.setWhen(false);            // hide: branch (and its effect) disposed
        globalThis.setLabel("b");             // must NOT re-run the disposed bind
        globalThis.b1 = globalThis.binds;     // still 1
        "#,
    )
    .unwrap();
    assert_eq!(num(&mut e, "globalThis.b0"), 1.0);
    assert_eq!(num(&mut e, "globalThis.b1"), 1.0);
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p supersolid_runtime show_toggles show_disposes`
Expected: FAIL — `Show` is `undefined`.

- [ ] **Step 3: Implement `Show`**

In `crates/supersolid_runtime/src/render.js`, add before the publish block:

```js
  // Conditional: a memoized accessor. When `when` flips, the memo recomputes and
  // its own cleanNode disposes the previously-built branch's owned effects.
  function Show(props) {
    return createMemo(function () {
      return props.when ? props.children : props.fallback;
    });
  }
```

Publish it as a top-level author global (not on `$ss`):

```js
  globalThis.Show = Show;
```

> **Guidance:** the disposal contract in the second test works because the `$ss.bind` effect built inside `props.children` is created while `Owner` is the `Show` memo node (the memo runs `props.children` during its computation). When `when` flips false, the memo recomputes; `runtime.js`'s `update` → `cleanNode` disposes the memo's `owned`, which includes that bind effect. No bespoke teardown is needed. This is exactly why `<For>` (Task 7) needs the opposite (roots that survive recompute).

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p supersolid_runtime`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/supersolid_runtime/src/render.js crates/supersolid_runtime/src/lib.rs
git commit -m "feat(supersolid_runtime): <Show> control-flow component

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 7: `<For>` — keyed list with per-row disposable roots

Add `getOwner` + an optional owner argument to `createRoot` in `runtime.js` (both Solid-standard, backward-compatible), then implement `mapArray` and `<For>`. Rows are keyed by **item identity**, each built under its own `createRoot` attached to the **For's stable owner** (so a list change that re-runs the map memo does not dispose retained rows).

**Files:**
- Modify: `crates/supersolid_runtime/src/runtime.js` (`getOwner`, `createRoot` owner arg, publish `$ssGetOwner`)
- Modify: `crates/supersolid_runtime/src/render.js` (`mapArray`, `For`)
- Test: `crates/supersolid_runtime/src/lib.rs` (both `tests` and `render_tests`)

**Interfaces:**
- Consumes: `createRoot`, `onCleanup`, `untrack`, `createMemo` (runtime.js).
- Produces:
  - `runtime.js`: `createRoot(fn, detachedOwner?)` — when `detachedOwner` is passed, the new root attaches to it (and inherits its context) instead of the current `Owner`; `globalThis.$ssGetOwner()` returns the current owner node (opaque handle).
  - `render.js`: `globalThis.For` — `For(props)` returns `createMemo(mapArray(() => props.each, props.children))`; `mapArray(listFn, mapFn)` returns an accessor giving the ordered `Node[]`, reusing a node per item identity and disposing removed items' roots.

- [ ] **Step 1: Write the failing test for the `createRoot` owner arg**

Add to the existing `#[cfg(test)] mod tests` (the reactive-core module) in `crates/supersolid_runtime/src/lib.rs`:

```rust
#[test]
fn create_root_with_explicit_owner_survives_a_sibling_scope_disposal() {
    let mut e = engine();
    e.eval(
        r#"
        globalThis.cleaned = 0;
        globalThis.host = null;
        // An outer root whose owner we capture and reuse for a detached child.
        createRoot(function (disposeOuter) {
            globalThis.host = $ssGetOwner();     // capture the outer owner
            globalThis.disposeOuter = disposeOuter;
        });
        // A throwaway scope: create a child root attached to `host`, NOT to this scope.
        createRoot(function (disposeThrow) {
            createRoot(function () {
                onCleanup(function () { globalThis.cleaned++; });
            }, globalThis.host);                 // detached owner = host
            globalThis.disposeThrow = disposeThrow;
        });
        globalThis.disposeThrow();               // dispose the throwaway scope
        globalThis.afterThrow = globalThis.cleaned;   // 0 — child is owned by host, not the throwaway
        globalThis.disposeOuter();               // dispose host
        globalThis.afterOuter = globalThis.cleaned;   // 1
        "#,
    )
    .unwrap();
    assert_eq!(num(&mut e, "globalThis.afterThrow"), 0.0);
    assert_eq!(num(&mut e, "globalThis.afterOuter"), 1.0);
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p supersolid_runtime create_root_with_explicit_owner`
Expected: FAIL — `$ssGetOwner` is undefined, and `createRoot`'s second arg is ignored (the child attaches to the throwaway scope, so `afterThrow` is `1`).

- [ ] **Step 3: Extend `createRoot` and add `getOwner` in `runtime.js`**

In `crates/supersolid_runtime/src/runtime.js`, replace the `createRoot` function with:

```js
  function createRoot(fn, detachedOwner) {
    // A disposable, non-tracking owner. Children attach to it and tear down
    // together. When `detachedOwner` is provided, attach the root THERE (and
    // inherit its context) instead of the current Owner — used by mapArray so
    // per-item roots survive the list memo's recomputation.
    var base = detachedOwner !== undefined ? detachedOwner : Owner;
    var root = {
      fn: null, owned: null, cleanups: null, sources: null,
      context: base ? base.context : null, owner: base,
      disposed: false, state: CLEAN,
    };
    if (base) (base.owned || (base.owned = [])).push(root);
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

  function getOwner() { return Owner; }
```

Then in the publish block, after `globalThis.$ssProvideContext = provideContext;`, add:

```js
  // Runtime-internal owner handle accessor; the render layer's mapArray uses it
  // to root per-item scopes on the list's stable owner.
  globalThis.$ssGetOwner = getOwner;
```

- [ ] **Step 4: Run to verify the owner test passes (and no regressions)**

Run: `cargo test -p supersolid_runtime`
Expected: PASS (the new owner test + all existing reactive-core + render tests). The `createRoot` change is backward-compatible: `detachedOwner === undefined` ⇒ original behavior.

- [ ] **Step 5: Write the failing `<For>` tests**

Add to `render_tests`:

```rust
#[test]
fn for_renders_and_reorders_keyed_rows() {
    let mut e = render_engine();
    e.eval(
        r#"
        // Items are objects (identity keys).
        globalThis.a = { n: "a" }; globalThis.b = { n: "b" }; globalThis.c = { n: "c" };
        var pair = createSignal([globalThis.a, globalThis.b, globalThis.c]);
        globalThis.set = pair[1];
        globalThis.p = $ss.el("ul");
        $ss.insert(p, function () {
            return $ss.cmp(For, {
                get each() { return pair[0](); },
                get children() {
                    return function (item) {
                        var li = $ss.el("li");
                        $ss.child(li, $ss.txt(item.n));
                        return li;
                    };
                },
            });
        });
        function order() {
            var s = "";
            for (var i=0;i<p.childNodes.length;i++){var n=p.childNodes[i];if(n.nodeType===1)s+=n.textContent;}
            return s;
        }
        globalThis.rowA = p.childNodes[0]; // <li>a</li>
        globalThis.o0 = order();           // "abc"
        globalThis.set([globalThis.c, globalThis.a, globalThis.b]);
        globalThis.o1 = order();           // "cab"
        globalThis.reusedA = (p.childNodes[1] === globalThis.rowA); // true — same <li> reused
        "#,
    )
    .unwrap();
    assert_eq!(text(&mut e, "globalThis.o0"), "abc");
    assert_eq!(text(&mut e, "globalThis.o1"), "cab");
    assert_eq!(text(&mut e, "globalThis.reusedA"), "true");
}

#[test]
fn for_preserves_per_row_state_across_list_change() {
    let mut e = render_engine();
    e.eval(
        r#"
        globalThis.a = { n: "a" }; globalThis.b = { n: "b" };
        var pair = createSignal([globalThis.a, globalThis.b]);
        globalThis.set = pair[1];
        globalThis.p = $ss.el("ul");
        // Each row owns a private counter signal; reused rows must keep it.
        $ss.insert(p, function () {
            return $ss.cmp(For, {
                get each() { return pair[0](); },
                get children() {
                    return function (item) {
                        var c = createSignal(0);
                        item.inc = c[1]; item.read = c[0];
                        var li = $ss.el("li");
                        $ss.insert(li, function () { return c[0](); });
                        return li;
                    };
                },
            });
        });
        globalThis.a.inc(5);                 // bump row a's private state
        globalThis.set([globalThis.b, globalThis.a]); // reorder (row a retained)
        globalThis.aState = globalThis.a.read();      // 5 — state preserved
        "#,
    )
    .unwrap();
    assert_eq!(num(&mut e, "globalThis.aState"), 5.0);
}
```

- [ ] **Step 6: Run to verify the `<For>` tests fail**

Run: `cargo test -p supersolid_runtime for_renders for_preserves`
Expected: FAIL — `For` is `undefined`.

- [ ] **Step 7: Implement `mapArray` and `For`**

In `crates/supersolid_runtime/src/render.js`, add before the publish block:

```js
  // Keyed map: reuse a mapped node per item IDENTITY across list changes; build
  // new items under their own createRoot rooted on `owner` (the For's stable
  // owner, captured now) so the list memo's recomputation never disposes them;
  // dispose removed items' roots. Returns an accessor giving the ordered nodes.
  function mapArray(listFn, mapFn) {
    var owner = globalThis.$ssGetOwner();
    var items = [];       // previous item values (identity keys)
    var mapped = [];      // mapped nodes, parallel to items
    var disposers = [];   // dispose fn per item
    onCleanup(function () {
      for (var i = 0; i < disposers.length; i++) disposers[i]();
    });
    return function () {
      var list = listFn() || [];
      return untrack(function () {
        var newMapped = new Array(list.length);
        var newDisposers = new Array(list.length);
        var prevIndex = new Map();
        for (var i = 0; i < items.length; i++) prevIndex.set(items[i], i);
        var used = new Array(items.length);
        for (var j = 0; j < list.length; j++) {
          var it = list[j];
          if (prevIndex.has(it)) {
            var oldi = prevIndex.get(it);
            newMapped[j] = mapped[oldi];
            newDisposers[j] = disposers[oldi];
            used[oldi] = true;
          } else {
            makeRow(it, j, newMapped, newDisposers, owner, mapFn);
          }
        }
        for (var k = 0; k < items.length; k++) {
          if (!used[k]) disposers[k]();
        }
        items = list.slice();
        mapped = newMapped;
        disposers = newDisposers;
        return mapped.slice();
      });
    };
  }

  // Build one row under its own root attached to the list's stable owner.
  function makeRow(item, index, outMapped, outDisposers, owner, mapFn) {
    createRoot(function (dispose) {
      outDisposers[index] = dispose;
      outMapped[index] = mapFn(item, index);
    }, owner);
  }

  function For(props) {
    return createMemo(mapArray(function () { return props.each; }, props.children));
  }
```

Publish `For` as an author global: `globalThis.For = For;`

> **Guidance:**
> - `mapFn(item, index)` receives the item **value** and a plain numeric index (a keyed list rarely displays its own index; `<Index>` in Task 8 is the position-keyed variant that exposes a reactive item). This is a deliberate, documented simplification of Solid's index-as-accessor.
> - `owner` is captured when `mapArray` is first *called* — i.e. while `For` runs inside `$ss.cmp`'s `untrack`, where `Owner` is the scope that instantiated `<For>` (stable across list changes), NOT the yet-to-be-created map memo. That is the whole point.
> - Disposed roots remain in `owner.owned` (not spliced out) — a known, bounded leak at TodoMVC scale; note it, don't fix it here.

- [ ] **Step 8: Run to verify all pass**

Run: `cargo test -p supersolid_runtime`
Expected: PASS (owner test + both `<For>` tests + all prior).

- [ ] **Step 9: Commit**

```bash
git add crates/supersolid_runtime/src/runtime.js crates/supersolid_runtime/src/render.js crates/supersolid_runtime/src/lib.rs
git commit -m "feat(supersolid_runtime): <For> keyed list + createRoot owner arg / getOwner

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 8: `<Index>` + `<Switch>`/`<Match>`

Position-keyed `<Index>` (item exposed as an in-place-updated signal) and `<Switch>`/`<Match>` (first truthy branch).

**Files:**
- Modify: `crates/supersolid_runtime/src/render.js`
- Test: `crates/supersolid_runtime/src/lib.rs` (`render_tests`)

**Interfaces:**
- Consumes: `createSignal`, `createRoot`, `createMemo`, `onCleanup`, `untrack`, `$ssGetOwner` (runtime.js).
- Produces: `globalThis.Index`, `globalThis.Switch`, `globalThis.Match`.
  - `Index(props)` → `createMemo(indexArray(() => props.each, props.children))`; `mapFn(itemAccessor, i)` receives a **signal getter** for the item at position `i`.
  - `Match(props)` → returns `props` (a live descriptor with `when`/`children` getters).
  - `Switch(props)` → `createMemo` returning the first `Match` whose `when` is truthy (its `children`), else `props.fallback`.

- [ ] **Step 1: Write the failing tests**

Add to `render_tests`:

```rust
#[test]
fn index_keys_by_position_and_updates_item_in_place() {
    let mut e = render_engine();
    e.eval(
        r#"
        var pair = createSignal(["x", "y"]);
        globalThis.set = pair[1];
        globalThis.p = $ss.el("ul");
        $ss.insert(p, function () {
            return $ss.cmp(Index, {
                get each() { return pair[0](); },
                get children() {
                    return function (item) {   // item is a SIGNAL getter
                        var li = $ss.el("li");
                        $ss.insert(li, function () { return item(); });
                        return li;
                    };
                },
            });
        });
        function order(){var s="";for(var i=0;i<p.childNodes.length;i++){var n=p.childNodes[i];if(n.nodeType===1)s+=n.textContent;}return s;}
        globalThis.row0 = p.childNodes[0];
        globalThis.o0 = order();            // "xy"
        globalThis.set(["z", "y"]);         // position 0 value changes x->z
        globalThis.o1 = order();            // "zy"
        globalThis.sameRow0 = (p.childNodes[0] === globalThis.row0); // true — position reused
        "#,
    )
    .unwrap();
    assert_eq!(text(&mut e, "globalThis.o0"), "xy");
    assert_eq!(text(&mut e, "globalThis.o1"), "zy");
    assert_eq!(text(&mut e, "globalThis.sameRow0"), "true");
}

#[test]
fn switch_picks_first_matching_branch() {
    let mut e = render_engine();
    e.eval(
        r#"
        var pair = createSignal(1);
        globalThis.set = pair[1];
        globalThis.p = $ss.el("div");
        $ss.insert(p, function () {
            return $ss.cmp(Switch, {
                get fallback() { var f=$ss.el("em"); $ss.child(f,$ss.txt("none")); return f; },
                get children() {
                    return [
                        $ss.cmp(Match, { get when(){ return pair[0]() === 1; },
                            get children(){ var s=$ss.el("span"); $ss.child(s,$ss.txt("one")); return s; } }),
                        $ss.cmp(Match, { get when(){ return pair[0]() === 2; },
                            get children(){ var s=$ss.el("span"); $ss.child(s,$ss.txt("two")); return s; } }),
                    ];
                },
            });
        });
        globalThis.t0 = p.textContent;   // "one"
        globalThis.set(2);
        globalThis.t1 = p.textContent;   // "two"
        globalThis.set(9);
        globalThis.t2 = p.textContent;   // "none" (fallback)
        "#,
    )
    .unwrap();
    assert_eq!(text(&mut e, "globalThis.t0"), "one");
    assert_eq!(text(&mut e, "globalThis.t1"), "two");
    assert_eq!(text(&mut e, "globalThis.t2"), "none");
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p supersolid_runtime index_keys switch_picks`
Expected: FAIL — `Index`/`Switch`/`Match` are `undefined`.

- [ ] **Step 3: Implement `Index`, `Switch`, `Match`**

In `crates/supersolid_runtime/src/render.js`, add before the publish block:

```js
  // Position-keyed map: one row per index, reused across changes; the item is a
  // signal updated in place when the value at that position changes.
  function indexArray(listFn, mapFn) {
    var owner = globalThis.$ssGetOwner();
    var mapped = [];      // node per position
    var setters = [];     // item signal setter per position
    var disposers = [];   // dispose fn per position
    onCleanup(function () {
      for (var i = 0; i < disposers.length; i++) disposers[i]();
    });
    return function () {
      var list = listFn() || [];
      return untrack(function () {
        // Grow: build new positions.
        for (var j = mapped.length; j < list.length; j++) {
          makeIndexRow(list[j], j, mapped, setters, disposers, owner, mapFn);
        }
        // Update existing positions in place.
        for (var k = 0; k < mapped.length && k < list.length; k++) {
          setters[k](function () { return list[k]; }); // set to the new value
        }
        // Shrink: dispose trailing positions.
        for (var d = list.length; d < mapped.length; d++) disposers[d]();
        if (list.length < mapped.length) {
          mapped.length = list.length;
          setters.length = list.length;
          disposers.length = list.length;
        }
        return mapped.slice();
      });
    };
  }

  function makeIndexRow(value, index, outMapped, outSetters, outDisposers, owner, mapFn) {
    createRoot(function (dispose) {
      var sig = createSignal(value);
      outSetters[index] = sig[1];
      outDisposers[index] = dispose;
      outMapped[index] = mapFn(sig[0], index); // item is the signal GETTER
    }, owner);
  }

  function Index(props) {
    return createMemo(indexArray(function () { return props.each; }, props.children));
  }

  // Match is a plain descriptor carrying live `when`/`children` getters.
  function Match(props) { return props; }

  function Switch(props) {
    return createMemo(function () {
      var kids = props.children;
      var arr = Array.isArray(kids) ? kids : (kids == null ? [] : [kids]);
      for (var i = 0; i < arr.length; i++) {
        var m = arr[i];
        if (m && m.when) return m.children;
      }
      return props.fallback;
    });
  }
```

Publish them: `globalThis.Index = Index; globalThis.Switch = Switch; globalThis.Match = Match;`

> **Guidance:** the `setters[k](function () { return list[k]; })` form uses the signal's updater-function API to set the new value; because `createSignal`'s default `equals` is `Object.is`, a genuinely-changed value notifies the row's `item()` reads and an unchanged value is a no-op — exactly the in-place update `<Index>` promises. If passing a function as a value (rather than an updater) is ever needed, wrap it; for `<Index>` list values are data, so the updater form is correct.

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test -p supersolid_runtime`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/supersolid_runtime/src/render.js crates/supersolid_runtime/src/lib.rs
git commit -m "feat(supersolid_runtime): <Index> + <Switch>/<Match> control-flow

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 9: Transpiler fix — fragment directly inside a plain element

`lower_element` drops a `JSXChild::Fragment` child (`crates/supersolid/src/jsx.rs`, the `child_data` collection falls through `_ => None`). Route it through `$ss.insert(parent, () => <fragExpr>)` so `<div><>…</></div>` renders (and dynamic fragment children stay reactive).

**Files:**
- Modify: `crates/supersolid/src/jsx.rs`
- Test: `crates/supersolid/src/lib.rs` (tests)

**Interfaces:**
- Consumes: existing `lower_fragment`, `thunk`, `insert_stmt`, `ChildKind` (in `jsx.rs`).
- Produces: a new `ChildKind::Fragment(Expression)` (or equivalent) whose lowering emits `$ss.insert(_el, () => <frag expr>)`.

- [ ] **Step 1: Write the failing test**

Add to `crates/supersolid/src/lib.rs` tests:

```rust
#[test]
fn fragment_child_of_element_is_inserted() {
    let out = code("const a = <div><><span/><em/></></div>;");
    assert!(out.contains(r#"$ss.el("div")"#), "{out}");
    // The fragment must survive as a $ss.frag routed through insert (not dropped).
    assert!(out.contains("$ss.frag("), "fragment child must be lowered, not dropped:\n{out}");
    assert!(out.contains("$ss.insert("), "fragment child inserted around anchor:\n{out}");
    assert!(out.contains(r#"$ss.el("span")"#) && out.contains(r#"$ss.el("em")"#), "{out}");
    assert!(reparses_as_plain_js(&out), "{out}");
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p supersolid fragment_child_of_element_is_inserted`
Expected: FAIL — the fragment child is dropped; `$ss.frag(` / `$ss.insert(` absent.

- [ ] **Step 3: Handle `JSXChild::Fragment` in `lower_element`**

In `crates/supersolid/src/jsx.rs`, extend the `ChildKind` enum used by `lower_element` with a fragment variant, collect it, and emit an `insert` of a thunk over the lowered fragment. Concretely:

Add to the `ChildKind` enum (in `lower_element`):

```rust
            DynamicExpr(Expression<'ast>),
            // A fragment placed directly inside an element: lower to $ss.frag([...])
            // and route through $ss.insert (reuses insert's array handling; keeps
            // any dynamic fragment children reactive).
            Fragment(&'b JSXFragment<'ast>),
```

In the `child_data` `filter_map`, replace the fragment-dropping arm. Change:

```rust
                // Fragments, spreads: later tasks.
                _ => None,
```

to:

```rust
                JSXChild::Fragment(frag) => Some(ChildKind::Fragment(frag.as_ref())),
                // Spreads: later tasks.
                _ => None,
```

Then in the child-lowering loop (where `ChildKind` variants become statements), add the `Fragment` arm alongside the others:

```rust
                ChildKind::Fragment(frag) => {
                    let frag_expr = self.lower_fragment(frag);
                    let thunk = self.thunk(frag_expr);
                    self.insert_stmt(&local, thunk)
                }
```

> **Guidance:** `lower_fragment(&mut self, &JSXFragment) -> Expression` and `insert_stmt(&self, parent, thunk)` and `thunk(&self, expr)` already exist in `jsx.rs`. The element must use the IIFE form when it has any child (it already does once `child_data` is non-empty). Confirm the `JSXFragment` import is in scope (it is — `jsx.rs` already imports it).

- [ ] **Step 4: Run to verify it passes (and no transpiler regressions)**

Run: `cargo test -p supersolid`
Expected: PASS (the new test + all existing lowering tests).

- [ ] **Step 5: Commit**

```bash
git add crates/supersolid/src/jsx.rs crates/supersolid/src/lib.rs
git commit -m "fix(supersolid): lower a fragment placed directly inside an element (route via \$ss.insert)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 10: `render()` root + bridge integration round-trip + ledger

Add the author entry `render(code, mountEl)`, prove the full sync-scheduler → DOM → reconciler → ECS path with a `superui_bridge` integration test (a counter incremented by a dispatched click updates the ECS `Text`), and record the capability ledger.

**Files:**
- Modify: `crates/supersolid_runtime/src/render.js` (`render`)
- Test: `crates/supersolid_runtime/src/lib.rs` (`render_tests` — a headless `render` test)
- Test: `crates/superui_bridge/tests/` (new integration test, or append to an existing reconcile/runtime test file)
- Modify: `docs/support/js-dom.md` (ledger section)

**Interfaces:**
- Consumes: `createRoot` (runtime.js); `$ss.insert` (Task 3); the bridge test harness (`test_app`/`mount` in `superui_bridge/tests/support` — mirror the existing reconcile tests).
- Produces: `globalThis.render(code, mountEl)` → establishes a root, inserts `code` into `mountEl`, returns `dispose`.

- [ ] **Step 1: Write the failing headless `render` test**

Add to `render_tests` in `crates/supersolid_runtime/src/lib.rs`:

```rust
#[test]
fn render_mounts_a_component_into_a_target() {
    let mut e = render_engine();
    e.eval(
        r#"
        function App() {
            var d = $ss.el("h1");
            $ss.child(d, $ss.txt("hello"));
            return d;
        }
        globalThis.root = $ss.el("main");
        globalThis.dispose = render(function () { return $ss.cmp(App, {}); }, globalThis.root);
        globalThis.t = root.textContent;             // "hello"
        globalThis.isFn = (typeof globalThis.dispose === "function"); // true
        "#,
    )
    .unwrap();
    assert_eq!(text(&mut e, "globalThis.t"), "hello");
    assert_eq!(text(&mut e, "globalThis.isFn"), "true");
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p supersolid_runtime render_mounts_a_component`
Expected: FAIL — `render` is `undefined`.

- [ ] **Step 3: Implement `render`**

In `crates/supersolid_runtime/src/render.js`, add before the publish block:

```js
  // Root entry: establish a disposable reactive scope and mount `code` into
  // `mountEl`. Returns `dispose` (Plan 5 HMR tears the tree down with it).
  function render(code, mountEl) {
    var dispose;
    createRoot(function (d) {
      dispose = d;
      insert(mountEl, code);
    });
    return dispose;
  }
```

Publish it: `globalThis.render = render;`

- [ ] **Step 4: Run to verify the headless test passes**

Run: `cargo test -p supersolid_runtime`
Expected: PASS.

- [ ] **Step 5: Write the failing bridge integration test**

Look at the existing bridge test harness first (`crates/superui_bridge/tests/reconcile.rs` and its `support` module) to reuse `test_app` / `mount` / `child_count` and the `Text` / `Children` query pattern. Create `crates/superui_bridge/tests/supersolid_render.rs`:

```rust
//! End-to-end: a Supersolid counter rendered from JS, incremented by a dispatched
//! DOM click, updates the reconciled ECS `Text`. Locks the sync-scheduler ->
//! DOM-mutation -> reconciler -> ECS path (design §9).

use std::cell::RefCell;
use std::rc::Rc;

use bevy::prelude::*;
use superui_bridge::UiRuntime;

mod support;
use support::{mount, test_app};

#[test]
fn supersolid_click_updates_reconciled_text() {
    // A minimal shell with a mount point; the script renders into #root.
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<div id='root'></div>",
    )));
    let mut app = test_app();
    let _root = mount(&mut app, dom.clone());

    // Render a counter with Supersolid. The button's click handler bumps a signal;
    // the label is a reactive text hole.
    app.world_mut()
        .non_send_resource_mut::<UiRuntime>()
        .run_script(
            r#"
            function Counter() {
                var c = createSignal(0);
                globalThis.__count = c;
                var wrap = $ss.el("div");
                var label = $ss.el("span");
                $ss.insert(label, function () { return c[0](); });
                var btn = $ss.el("button");
                $ss.on(btn, "click", function () { c[1](function (n) { return n + 1; }); });
                $ss.child(btn, $ss.txt("+"));
                $ss.child(wrap, label);
                $ss.child(wrap, btn);
                return wrap;
            }
            render(function () { return $ss.cmp(Counter, {}); },
                   document.getElementById("root"));
            "#,
        );
    app.update(); // reconcile the initial render

    // The label text reconciled to "0".
    let label_text_0 = current_label_text(&mut app, &dom);
    assert_eq!(label_text_0, "0", "initial reactive text should reconcile");

    // Dispatch a click on the button through the engine (as the input system does).
    {
        let mut rt = app.world_mut().non_send_resource_mut::<UiRuntime>();
        let btn = {
            let d = dom.borrow();
            d.query_selector(d.document(), "button").unwrap()
        };
        rt.engine.dispatch_event(btn, "click", None, true, true);
        rt.dirty = true; // mirror the input system's post-dispatch dirtying
    }
    app.update(); // reconcile the post-click DOM

    let label_text_1 = current_label_text(&mut app, &dom);
    assert_eq!(label_text_1, "1", "click -> signal -> effect -> DOM -> ECS text");
}

/// Read the reconciled `Text` of the first `<span>`'s text child entity.
fn current_label_text(app: &mut App, dom: &Rc<RefCell<superui_dom::Dom>>) -> String {
    let span = {
        let d = dom.borrow();
        d.query_selector(d.document(), "span").unwrap()
    };
    let span_entity = app
        .world()
        .non_send_resource::<UiRuntime>()
        .entity_for(span)
        .unwrap();
    let text_entity = app.world().get::<Children>(span_entity).unwrap()[0];
    app.world().get::<Text>(text_entity).unwrap().0.clone()
}
```

> **Guidance:**
> - Copy the `support` module usage from `crates/superui_bridge/tests/reconcile.rs` (same `mod support; use support::{…};`). If `test_app`/`mount`/the `support` path differ, match whatever that file does — do not invent a new harness.
> - `BoaEngine::dispatch_event(node, type, key, bubbles, cancelable)` is the same signature the input systems call (`events.rs:191`). Confirm the exact arg types against `events.rs`; adjust the `None` (key) argument to whatever the signature expects.
> - The `render_script` must run **through `UiRuntime::run_script`** so `document`, the reactive globals, and `$ss` are all present (they are installed in `UiRuntime::new`).

- [ ] **Step 6: Run to verify the integration test fails, then passes**

Run: `cargo test -p superui_bridge --test supersolid_render`
Expected: initially FAIL only if something is mis-wired; since all runtime pieces exist by now, this test should **pass** once written correctly. If it FAILS: check (a) the script ran without a swallowed JS error (temporarily log `run_script` errors), (b) the mount `#root` exists in the shell, (c) `dispatch_event`'s signature. Fix wiring until green. Do **not** weaken the assertions.

- [ ] **Step 7: Update the capability ledger**

In `docs/support/js-dom.md`, under the existing `## Supersolid runtime (framework globals)` section (added in Plan 3), append a subsection:

```markdown
### Render + control flow (`supersolid_runtime` render layer)

Compiler-internal `$ss.*` helpers (emitted by the transpiler) and author-facing
control-flow globals. All build/mutate the arena DOM; downstream reconcile is unchanged.

| Global | Status | Since | Notes |
|---|---|---|---|
| `$ss.el` / `$ss.txt` / `$ss.attr` / `$ss.child` | ✅ | T0 | build-once nodes; `value`/`checked` set as properties, else attributes |
| `$ss.on` | ✅ | T0 | `addEventListener` (handler as-is) |
| `$ss.bind` | ✅ | T0 | reactive attribute (effect); surgical re-apply |
| `$ss.insert` | ✅ | T0 | reactive child around an anchor; surgical text; keyed minimal-move list reconcile |
| `$ss.cmp` / `$ss.frag` | ✅ | T0 | run-once component (`untrack`); fragment array |
| `render(code, mountEl)` | ✅ | T0 | root entry; returns `dispose` |
| `<Show>` | ✅ | T0 | conditional; branch disposal via memo recompute |
| `<For>` | ✅ | T0 | keyed by item identity; per-row disposable roots; state preserved on reorder |
| `<Index>` | ✅ | T0 | keyed by position; item is an in-place-updated signal |
| `<Switch>` / `<Match>` | ✅ | T0 | first truthy branch, else fallback |
```

- [ ] **Step 8: Verify the whole affected surface and commit**

Run: `cargo test -p supersolid_runtime && cargo test -p supersolid && cargo test -p superui_bridge`
Expected: PASS.

Optional (if the wasm target is installed): `cargo build -p supersolid_runtime --target wasm32-unknown-unknown` — builds (still wasm-clean; no new native deps).

```bash
git add crates/supersolid_runtime/src/render.js crates/supersolid_runtime/src/lib.rs crates/superui_bridge/tests/supersolid_render.rs docs/support/js-dom.md
git commit -m "feat(supersolid_runtime): render() root + bridge round-trip test + ledger

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Done-when

- `cargo test -p supersolid_runtime`, `cargo test -p supersolid`, and `cargo test -p superui_bridge` all green.
- The `$ss.*` ABI is fully implemented: build-once `el`/`txt`/`attr`/`child`; reactive `on`/`bind`; anchor-based `insert` with surgical text-in-place, single-node, null, and keyed minimal-move lists; run-once `cmp`; `frag`.
- Control flow works: `<Show>` (branch disposal via memo recompute), `<For>` (identity-keyed, per-row state preserved across reorder), `<Index>` (position-keyed, in-place item signal), `<Switch>`/`<Match>`.
- `render(code, mountEl)` mounts a component tree and returns `dispose`.
- The transpiler no longer drops a fragment placed directly inside an element.
- A `superui_bridge` integration test proves click → signal → effect → DOM → reconciler → ECS `Text`.
- `supersolid_runtime` stays wasm-clean (no Bevy/`oxc`); `runtime.js`'s `createRoot` gained a backward-compatible optional owner arg + `$ssGetOwner`.
- `docs/support/js-dom.md` records the render + control-flow surface.

## Self-review (author)

- **Spec coverage:** implements design §2 static builders (Task 1), §3 reactive holes (Task 2), §4 `insert` anchor model (Task 3/5), §5 array reconcile (Task 5), §6 components/fragments/control-flow (Tasks 4, 6, 7, 8), §7 `render()` (Task 10), §8 transpiler fragment fix (Task 9), §9 wiring + round-trip + ledger (Task 10). Direction-spec §5 (build-once + surgical bindings), §7 (DOM-only mutations, reconciler untouched), and §11.1 (full control-flow + `render()`) are covered. Deferred with rationale (design §11): HMR `dispose` hook is provided but unused (Plan 5); React-ism lints (later transpiler plan); `createDocumentFragment` stays deferred (anchor model is the substitute); TodoMVC example (Plan 6).
- **No placeholders:** every task ships concrete failing tests (the contracts) and real `render.js`/`runtime.js`/`jsx.rs`/bridge code. The two version-sensitive spots are (a) whether `dispatchEvent` is JS-exposed — handled by making the JS `on` test a smoke test and proving click end-to-end through `BoaEngine::dispatch_event` in the bridge test — and (b) the exact `dispatch_event` arg types, flagged to confirm against `events.rs`. The Task-3→5 `reconcileArray` is deliberately introduced as a correct replace-based stub then upgraded to minimal-move, so every intermediate state is green.
- **Type/name consistency:** `$ss` method names (`el`/`txt`/`attr`/`child`/`insert`/`bind`/`on`/`cmp`/`frag`) match the Plan 2 ABI table and the transpiler's emitted calls (`jsx.rs`); `resolve`/`reconcile`/`clearNodes`/`removeNode`/`normalizeArray`/`reconcileArray`/`reconcileArrays`/`mapArray`/`makeRow`/`indexArray`/`makeIndexRow` are introduced once and reused unchanged; `createRoot(fn, detachedOwner?)` and `$ssGetOwner` are defined in Task 7 and consumed by `mapArray`/`indexArray`; author globals `render`/`Show`/`For`/`Index`/`Switch`/`Match` match the design and direction §11.1. `install` evals `runtime.js` then `render.js` consistently.
