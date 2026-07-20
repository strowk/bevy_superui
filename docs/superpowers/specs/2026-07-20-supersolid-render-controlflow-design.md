# Supersolid Phase 2 — Plan 4: render + control-flow layer — Design

Date: 2026-07-20
Status: Design agreed. Implementation plan lives in `../plans/` (written next via the
writing-plans skill).

## 0. What this is

The design for **Plan 4** of the Supersolid Phase-2 series: the JSX **runtime** that
*implements* the `$ss.*` ABI the Plan 2 transpiler already emits, plus the root `render()`
and the Solid-style control-flow components `<Show>` / `<For>` / `<Index>` /
`<Switch>` / `<Match>`. It composes on Plan 3's reactive core and drives the Phase-1 arena
DOM through ordinary DOM mutations. It also fixes one Plan 2 follow-up (a fragment placed
directly inside a plain element is currently dropped).

References: direction spec `2026-07-19-superui-component-framework-direction.md` (§5 build-once
+ surgical bindings, §7 boundary discipline, §11.1 API surface); Plan 2 (transpiler, the ABI
this satisfies); Plan 3 (reactive core this composes on); Plan 1 (text-node `.data` mutation +
proven keyed reorder/reparent; `createDocumentFragment` deferred → we insert around anchor
text nodes).

## 1. Boundary & placement

The render layer is **wasm-clean JS**. It lives in the existing `supersolid_runtime` crate as a
second embedded module, `src/render.js`, eval'd by the same `install()` immediately after
`runtime.js`. It publishes:

- one compiler-internal global **`$ss`** — `{ el, txt, attr, child, insert, bind, on, cmp, frag }`;
- author-facing globals **`render`**, **`Show`**, **`For`**, **`Index`**, **`Switch`**, **`Match`**
  (the transpiler strips the matching imports).

It calls **only** the DOM API already installed by `superui_api` — `document.createElement` /
`createTextNode`, `appendChild` / `insertBefore` / `removeChild`, text-node `.data`,
`setAttribute`, `addEventListener`, and the navigation getters — and composes on the reactive
globals `createEffect` / `createRoot` / `createMemo` / `untrack` / `onCleanup`. It never touches
Bevy or the reconciler. Per direction §7, the render layer produces ordinary DOM mutations;
everything downstream (reconciler → taffy → `bevy_ui`, picking, `window.bevy`) is untouched
Phase-1 machinery.

**Why `supersolid_runtime` and not a new crate:** the render layer is *part of the runtime* and
composes on its public API; a second `include_str!` module keeps files focused without a new
manifest or a second install call. Tests add `superui_api` as a **dev-dependency** to install the
headless DOM, then install the reactive+render runtime and assert `$ss.*` calls mutate the arena
`Dom`.

**Why the DOM is settled by reconcile time.** Plan 3's scheduler is synchronous: a signal write
outside `batch()` propagates and flushes effects before returning to Rust. Author scripts run in
`UiRuntime::run_script` (sets `dirty`); event callbacks run inside `dispatch_event` and timers
inside `run_timers`, each followed by `dirty = true` (`events.rs:120/224/299`,
`bevy_bridge.rs:194`), all before `reconcile_system` clears `dirty` (`reconcile.rs:23`). So a
click that writes a signal re-runs the bound effects, mutates the DOM, and is reconciled the same
frame — no per-frame Rust pump is added by this layer.

## 2. Static builders

| Helper | Behavior |
|---|---|
| `$ss.el(tag)` | `document.createElement(tag)` |
| `$ss.txt(data)` | `document.createTextNode(data)` |
| `$ss.child(parent, node)` | append a static child; **array-aware** — a fragment/array appends each flattened node (this is also what makes a static nested fragment render) |
| `$ss.attr(el, name, value)` | static attribute set, via the property/attribute rule below |

**Property vs attribute rule (shared by `attr` and `bind`).** A small allowlist —
`value`, `checked` — sets the DOM **property** (`el[name] = v`), because those are live element
state (the `superui_api` element proto exposes them as accessors and the reconciler reads them
for `<input>`). Every other name uses `setAttribute(name, String(v))`. `class` stays an attribute
(flair's selectors read the `class` attribute). This keeps the ABI stable — the transpiler only
ever emits `(name, value)` — while producing correct DOM semantics.

## 3. Reactive holes

- **`$ss.on(el, type, handler)`** → `el.addEventListener(type, handler)`. The handler is passed
  as-is (the transpiler does not thunk it). Listeners live with the element; when control flow
  removes and despawns the element, the listener goes with it.
- **`$ss.bind(el, name, thunk)`** → `createEffect(() => setProp(el, name, thunk()))`, using the
  §2 property/attribute rule. One effect per dynamic attribute; re-runs surgically when a tracked
  dependency of `thunk` changes.

## 4. `$ss.insert` — the reconciling dynamic child (the core)

We have no `DocumentFragment` (Plan 1 deferred it), so dynamic content is managed **around an
anchor text node**, as Plan 1 anticipated.

**Setup.** `$ss.insert(parent, accessor)` appends an **empty text-node anchor** to `parent` and
initializes `current = null` (the content this hole owns: `null | Text | Node | Node[]`). It then
wraps the work in `createEffect(() => { current = reconcile(parent, anchor, current, accessor()); })`.
Content is always (re)inserted **before the anchor**, so multiple holes and any static siblings
keep stable positions regardless of build order.

**`reconcile(parent, anchor, current, value)`** dispatches on the *resolved* value and returns the
new `current`:

- **function** → resolve by calling until non-function (`while (typeof value === "function") value = value()`),
  *inside* the insert effect so the effect subscribes to whatever a control-flow accessor/memo
  reads. This is how `<Show>`/`<For>`/… (which return memoized accessors, §6) drive the hole.
- **`null` / `undefined` / `false` / `true`** → remove `current`'s nodes; leave the bare anchor
  (renders nothing); return `null`.
- **string / number** → if `current` is a single `Text` node, **set `current.data = String(value)`
  in place** (the surgical `{count()}` update — no node churn); otherwise remove `current`, create a
  text node, `insertBefore(anchor)`, return it.
- **DOM node** → if `current === value`, no-op; else remove `current`, `insertBefore(value, anchor)`,
  return it.
- **array** → **normalize** (flatten nested arrays/fragments; convert primitives to text nodes;
  drop `null`/booleans) to a flat `Node[]`, then run the **keyed minimal-move reconcile** (§5)
  against `current` before the anchor; return the new `Node[]`.

Removal detaches nodes via `parent.removeChild(node)` for every node in `current` not retained.

## 5. Minimal-move array reconcile

Given the previous `Node[]` (`current`) and the new `Node[]` (already node-identity-stable because
`<For>`'s `mapArray` reuses a node per item, §6), place the new list before the anchor with the
fewest DOM moves — Solid's `reconcileArrays` shape:

1. Trim the common **prefix** and common **suffix** (nodes equal by identity in the same position).
2. For the differing middle: remove nodes present in `current` but absent from `next`; then walk
   `next` left→right, `insertBefore` each node that is new or out of position, using the next
   retained node (or the anchor) as reference.

Contract locked by tests: after reconcile, `parent`'s children strictly before the anchor equal
`next` in order; nodes shared between `current` and `next` are **reused** (never re-created); the
number of `insertBefore` calls is bounded by the number of actually-moved items. This is exactly
the reorder/reparent behavior Plan 1 proved the reconciler mirrors into ECS.

## 6. Components, fragments, and control flow

**`$ss.cmp(Comp, props)`** → `untrack(() => Comp(props))`. The component body runs **once**
(fine-grained model); `props` already carries getters for dynamic props (transpiler). The return
value (nodes or an accessor) is consumed by an enclosing `insert`/`child`.

**`$ss.frag(children)`** → returns the `children` array as-is; `insert`/`child` flatten it (§2/§4).

Control-flow components are ordinary components that **return memoized accessors** — a
`createMemo` getter that `$ss.insert` resolves by calling-until-non-function (§4). Returning an
accessor (not plain nodes) is what makes them reactive and is the Solid model.

- **`<Show>`** — `createMemo(() => props.when ? props.children : props.fallback)`. Reading
  `props.when` (a getter over the condition signal) subscribes the memo. When `when` flips, the
  memo recomputes; its `cleanNode` disposes the previously-built branch's owned effects, and the
  other branch builds — **branch disposal falls out of memo recomputation**, no bespoke teardown.
  (Default rebuild-on-toggle, matching Solid's non-`keyed` `<Show>`.)
- **`<For each={…}>{(item) => …}</For>`** — keyed by **item identity**. An internal `mapArray(each, mapFn)`:
  maps each item under its **own `createRoot`** (isolated, disposable per-row reactive state);
  caches the mapped node by item identity across list changes; reuses nodes for retained items,
  builds only for new items, and disposes the roots of removed items. Wrapped in `createMemo` so it
  caches across insert-effect re-runs and is itself reactive on `each`. Returns the ordered
  `Node[]`; `insert`'s array path (§5) does the minimal-move DOM placement.
- **`<Index each={…}>{(item, i) => …}</Index>`** — keyed by **position**. One node per index,
  reused across list changes; the item is exposed to the row as a **signal** updated in place when
  the value at that index changes (so a row's structure is stable and only its content updates).
- **`<Switch fallback={…}><Match when={…}>…</Match>…</Switch>`** — `Match` is a plain descriptor
  `{ when, children }` (its `$ss.cmp` returns the object, not nodes). `Switch` is a
  `createMemo` that scans its `Match` children for the first truthy `when` and returns that Match's
  `children` (else `props.fallback`).

## 7. `render()` root

`render(code, mountEl)` establishes the root reactive scope and mounts:

```js
function render(code, mountEl) {
  let dispose;
  createRoot((d) => { dispose = d; $ss.insert(mountEl, code); });
  return dispose;
}
```

The author's `.tsx` top-level runs once (`run_script`) and calls
`render(() => <App/>, document.getElementById("root"))`; `<App/>` lowers to `$ss.cmp(App, {})`, so
`code` is `() => $ss.cmp(App, {})`. The built nodes become children of the mount element in the
arena DOM; the reconciler syncs them to ECS. `render` returns `dispose` (unused now; Plan 5 HMR
tears the tree down with it).

## 8. Transpiler fix — fragment directly inside a plain element

`lower_element`'s child collection (`crates/supersolid/src/jsx.rs:360`) handles `JSXChild::Text`,
`JSXChild::Element`, and `JSXChild::ExpressionContainer` but falls through `JSXChild::Fragment` to
`_ => None`, silently dropping it — so `<div><>…</></div>` loses the fragment. Fix: add a
`JSXChild::Fragment` arm that lowers via the existing `lower_fragment` and routes the result
through `$ss.insert(parent, () => <fragExpr>)` (reusing insert's array handling), keeping any
dynamic fragment children reactive. A new transpiler unit test (`fragment_child_of_element_is_inserted`)
locks it; this is a transpiler-crate change that ships in this plan alongside the runtime work.

## 9. Wiring & verification

- **Install:** no change to `UiRuntime::new` — `render.js` rides the existing
  `supersolid_runtime::install`, which now evals `runtime.js` then `render.js`.
- **Round-trip test (bridge):** a `superui_bridge` integration test mounts a `.tsx`-style script
  that `render`s a counter, dispatches a `click`, and asserts the ECS `Text` entity's string
  updated — locking the sync-scheduler → DOM-mutation → reconciler → ECS path end to end.
- **Ledger:** append the `$ss` + control-flow surface to `docs/support/js-dom.md`.

## 10. Task breakdown (each TDD, its own commit)

1. `render.js` scaffold + static builders (`el`/`txt`/`attr`/`child`, property/attribute rule) +
   install wiring; headless test harness (`superui_api` dev-dep).
2. `$ss.on` + `$ss.bind` (reactive attributes).
3. `$ss.insert` — text / node / null, anchor model, surgical text-in-place.
4. `$ss.cmp` + `$ss.frag` + insert array normalization.
5. `$ss.insert` keyed minimal-move array reconcile.
6. `<Show>`.
7. `<For>` (mapArray, per-row `createRoot`, identity keying).
8. `<Index>` + `<Switch>`/`<Match>`.
9. Transpiler fragment-in-element fix.
10. `render()` root + bridge integration round-trip test + ledger update.

## 11. Non-goals / deferred

- **State-preserving HMR** (signal-cell rehydration) → Plan 5; `render` returning `dispose` is the
  only hook added here.
- **React-ism lints** → the transpiler's later plan.
- **`createDocumentFragment`** stays deferred — the anchor model (§4) is the chosen substitute;
  revisit only if measurements demand it (Plan 1 note).
- **Delegated events / `removeEventListener` on cleanup** — listeners live and die with their
  element; no separate teardown until a measured need appears.
- **The `examples/todomvc_supersolid`** deliverable → Plan 6.

## 12. Self-review

- **Spec coverage:** implements direction §5 (build-once nodes + surgical bindings on the dynamic
  holes — `el`/`child` build once, `bind`/`insert` are the only reactive attachments; the
  arena-DOM analog of Solid's template-clone) and §11.1's full control-flow + `render()` surface;
  honors §7 (DOM-only mutations, reconciler untouched). Composes strictly on Plan 3's public API.
- **Boundaries:** `render.js` is one focused module with a narrow seam (the DOM API + reactive
  globals in, DOM mutations out); each `$ss.*` helper and each control-flow component is
  independently testable headlessly.
- **Open judgment calls (accepted):** (a) `value`/`checked` set as properties, other names as
  attributes — keeps the `(name,value)` ABI while matching DOM semantics; (b) control-flow returns
  memoized accessors that `insert` resolves by calling-until-non-function — the Solid model, and
  the reason branch/list disposal falls out of memo recomputation instead of bespoke teardown.
