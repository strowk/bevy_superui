# Future optimization — clone-based template lowering for Supersolid JSX

Date captured: 2026-07-19
Status: **Backlog / not scheduled.** Deferred out of Supersolid Phase-2 Plan 2 (the transpiler),
which ships the simpler **element-walk** lowering. This document records everything understood at
decision time so the optimization can be picked up later without re-deriving it.

## 0. TL;DR

Supersolid's transpiler (Plan 2) lowers JSX to **build-once nodes + surgical reactive bindings**.
There are two ways to emit the *build-once* half:

- **Element-walk (shipped in Plan 2):** each JSX element lowers to a sequence of
  `createElement` / `setAttribute` / `appendChild` runtime calls. Simple, no new DOM primitive.
- **Clone-based templates (this doc, deferred):** the compiler emits one HTML **template string**
  per component; the runtime parses it **once** (cached) and **deep-clones** it per instance via a
  single native Rust call, then binds only the dynamic holes. This is what SolidJS/`dom-expressions`
  actually does in the browser.

The **reactive-binding half is identical** in both (`_insert`, `_bindAttr`, `_setEvent` against the
dynamic holes), so **per-update performance is byte-for-byte the same** — and per-update is the
whole fine-grained win over a VDOM (direction spec §4). The clone approach only changes
**creation-time** cost (component mount, and `<For>` row insertion). This is why it is an
*optimization*, not a correctness change, and why it can be added later behind an unchanged
authoring surface and an unchanged reactive core.

## 1. Why this is worth revisiting (the real mechanism)

The thing that makes browser `cloneNode(true)` fast is not memory magic — it **collapses N
JS→native boundary crossings into one**. Building a 10-node subtree with `createElement`/
`appendChild` is 20+ calls that each cross from JS into the engine; `cloneNode` is one native call
that builds all 10.

Our arena has the exact same boundary. In `bevy_superui`, every `createElement` / `appendChild` /
`setAttribute` issued from **Boa** is: a native-function call → argument marshalling (`NodeId`
as `u64`/`f64`) → borrow `Rc<RefCell<Dom>>` → mutate the `SlotMap`. On **Boa** — a non-JIT
interpreter where native calls and interpretation are relatively expensive — that per-call tax is
significant and is paid *per node* by element-walk.

A single `clone_subtree(template) -> NodeId` Rust primitive, bound into Boa once, collapses an
N-node instantiation into **one** boundary crossing. That is a genuine, Solid-shaped win, largest
exactly where it hurts most on Boa: **list-heavy UIs** (e.g. TodoMVC's `<For>` over todos).

## 2. Why it is feasible in our arena (grounded in the code)

Verified against `crates/superui_dom/src/node.rs` and `tree.rs` on 2026-07-19:

- Nodes live in a `SlotMap<NodeId, NodeData>` (`tree.rs`).
- `NodeData`, `NodeKind`, and `ElementData` all already `#[derive(Clone)]` (`node.rs:38-64`).
- So a deep clone is straightforward: recursively insert cloned `NodeData` into the slotmap, remap
  each child `NodeId`, and fix `parent` links. No structural redesign required.
- We already have `superui_html::parse_document`, so the **template itself** can be built the
  literal `dom-expressions` way: compiler emits a template **HTML string**; runtime parses it once
  into a detached subtree, caches it, and clones per instance.

Conclusion: a fast clone is not hypothetical. The arena can have a real one, and the template can
be produced by the HTML parser we already ship.

## 3. What the optimization consists of (implementation sketch)

Three pieces. Only the compiler prologue and the DOM primitive are new; the reactive-binding
helpers are shared with the element-walk ABI and do not change.

### 3.1 New `superui_dom` primitive — deep clone (a Plan-1-style prerequisite)

```rust
// crates/superui_dom/src/tree.rs (sketch)
impl Dom {
    /// Deep-clone a detached subtree, returning the new root. Clones NodeKind/
    /// ElementData by value; remaps children; sets parents; the returned root is
    /// detached (parent = None). Listeners are NOT cloned (templates are inert;
    /// events are wired per-instance by the surgical bindings).
    pub fn clone_subtree(&mut self, root: NodeId) -> Option<NodeId> { /* recursive insert + remap */ }
}
```

- **Listeners:** do **not** clone them. The template is inert; `_setEvent` wires handlers on the
  cloned instance. (Element `value`/`checked` IDL fields: clone or reset — decide by what the
  template can legally carry; templates are static markup, so resetting to `None` is safe.)
- **JS binding:** expose as an internal runtime helper (e.g. `_clone(templateId)`), *not*
  necessarily as public `Node.prototype.cloneNode` — keep it a Supersolid-runtime primitive unless
  a browser-fidelity need for `cloneNode` appears (that would be a Phase-3 browser-compat item).
- **Ledger:** add a `docs/support/js-dom.md` row if it becomes a public DOM surface.

### 3.2 Compiler change — emit template string + static path-to-hole navigation

This is the real added complexity, and it is **independent of clone speed**. `dom-expressions`
does not just clone; for each dynamic hole it statically computes a **navigation path** from the
clone root and emits code to reach it:

```js
// <div class="todo"><span>{label()}</span><button onClick={remove}>x</button></div>
const _tmpl = _template(`<div class="todo"><span></span><button>x</button></div>`);
(() => {
  const _root = _clone(_tmpl);
  const _span = _firstChild(_root);            // path: root -> child 0
  const _btn  = _nextSibling(_span);           // path: -> child 1
  _insert(_span, () => label());               // surgical (SHARED with element-walk)
  _setEvent(_btn, "click", remove);            // surgical (SHARED)
  return _root;
})()
```

Compiler responsibilities that element-walk does **not** have:
- Serialize the static skeleton to an HTML string (attributes with literal values inlined; dynamic
  attrs/children left as empty holes).
- Compute, for every dynamic hole, a stable navigation path from the clone root, expressed in
  child-index / `firstChild`+`nextSibling` terms.
- Emit navigation locals + the (shared) surgical binding calls.

**Text-hole markers.** `dom-expressions` inserts comment/marker nodes to anchor dynamic text and
list ranges. We currently have **no comment nodes** and deliberately deferred
`createDocumentFragment` (see Plan 1 scope notes). Two ways to handle this without new node kinds:
1. **Child-index navigation** to an empty text node placeholder baked into the template, replaced
   on bind. Works for single dynamic text holes.
2. **Anchor text nodes** for `<For>`/`<Show>` ranges (the render-layer plan already plans to insert
   around anchor text nodes rather than fragments). Reuse that scheme so lists clone cleanly.

Decide markers before implementing; this is the sharpest design edge of the optimization.

### 3.3 Runtime — template cache

- `_template(htmlString)` parses once and memoizes by identity (module-level const call site), then
  returns a cached detached root that `_clone` copies. Cache keyed per compiled call site (Solid
  hoists `_tmpl$` to module scope — do the same: emit each template as a module-level const so it
  parses exactly once).

## 4. The ABI seam that makes this a drop-in later

Element-walk (Plan 2) and clone-based both emit the **same surgical helpers** for dynamic holes:
`_insert(el, thunk)`, `_bindAttr(el, name, thunk)`, `_setEvent(el, type, handler)`,
`_createComponent(Comp, props)`. Those are implemented by Plan 3 (reactive core) + Plan 4 (render).

Only the **node-construction prologue** differs:

| | Element-walk (Plan 2) | Clone-based (this doc) |
|---|---|---|
| Static structure | `createElement`/`setAttribute`/`appendChild` per node | `_template(str)` once + `_clone` per instance |
| Reach a dynamic hole | you already hold the local from the create call | navigate `_firstChild`/`_nextSibling` from clone root |
| Dynamic holes | `_insert`/`_bindAttr`/`_setEvent` | **same** `_insert`/`_bindAttr`/`_setEvent` |
| Per-update cost | identical | identical |
| Creation-time boundary crossings | O(nodes) | O(1) template parse + O(1) clone + O(holes) |

Because the author's `.tsx` and the reactive core never change, switching prologues is an internal
compiler + one-DOM-primitive change. **Keep the surgical-helper ABI stable in Plan 2 with this swap
in mind** (e.g. don't design `_insert` to assume the parent was just created by element-walk).

## 5. How to measure whether it actually helps (do this BEFORE committing)

The optimization is only worth its complexity if Boa creation-time is a real bottleneck. Measure,
don't assume. Concretely:

### 5.1 Microbenchmark — the boundary-crossing hypothesis
- Build the same N-node subtree two ways from Boa: (a) N `createElement`/`appendChild` calls, vs
  (b) one `_clone` of a pre-parsed template. Sweep N ∈ {5, 20, 100, 500}.
- Measure wall-clock per instantiation on the **native Boa** engine and record the crossover N and
  the ratio. Expectation: (b) wins and the gap widens with N; if it doesn't, the whole premise is
  wrong — stop.

### 5.2 Macro / realistic — the actual UI workload
- Use the Supersolid TodoMVC (Phase-2 Plan 6) as the fixture. Scenarios that stress *creation*:
  - Initial mount with a seeded list of {100, 1000} todos.
  - `<For>` bulk insert: add 100 rows in one batch.
  - Filter toggle that re-creates a large visible subset.
- Metric: time from state change → reconciled Bevy entities (creation path), plus allocation count
  in Boa if obtainable. Compare element-walk build vs clone build of the *same* example.
- **Also record the non-goal metric:** steady-state per-update (toggle one todo's checkbox) should
  be **unchanged** between builds — if it isn't, a binding regressed, not the prologue.

### 5.3 Decision rule
- Ship the clone path only if the macro benchmark shows a **material** creation-time win on Boa
  (e.g. list mount / bulk-insert meaningfully faster) at 100+ items — the regime real game UIs hit.
- If the win is marginal below sizes real UIs use, keep element-walk; the compiler simplicity and
  debuggable output are worth more than a constant factor nobody feels.
- Re-evaluate automatically if/when a faster native engine (`rquickjs`) lands — a cheaper native
  boundary shrinks the clone advantage on native, though **wasm is interpreter-only permanently**
  (spec §6), so the web build keeps the most to gain.

## 6. Interactions / watch-outs
- **HMR (Phase-2 Plan 5):** templates are keyed per call site; a hot-swap that changes static
  markup must invalidate the cached template. Keep the template cache keyed so re-eval rebuilds it.
- **Capability footprint auto-derivation (spec §11.5):** deriving the footprint from a template
  string vs from `createElement` calls should yield the same set — verify the deriver reads whichever
  form the compiler emits.
- **`<For>`/`<Index>` keyed lists:** row templates are the highest-value clone target; make sure the
  keyed-move reconciliation (Plan 1 Task 2 locked reorder/reparent) composes with cloned rows.
- **Marker strategy** (§3.2) is the single most important design decision to nail before coding.

## 7. Pointers
- Direction spec: `../superpowers/specs/2026-07-19-superui-component-framework-direction.md` §3, §4, §5, §6, §11.3.
- Plan 2 (element-walk transpiler): `../superpowers/plans/` (Supersolid Phase-2 Plan 2).
- Arena DOM: `crates/superui_dom/src/{node.rs,tree.rs}`; HTML parse: `crates/superui_html`.
- Prior art: SolidJS `dom-expressions` / `babel-preset-solid` (template + clone + path bindings).
