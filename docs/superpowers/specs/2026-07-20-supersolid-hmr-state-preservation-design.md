# Supersolid Phase 2 — Plan 5: state-preserving hot reload — design

Date: 2026-07-20
Status: Design agreed (forward-looking). Feeds the Plan 5 implementation plan.

## 0. What this document is

The agreed design for **Supersolid Plan 5** — state-preserving HMR for `.tsx`: signal-cell
rehydration keyed by `module × instance × creation-order`, with a per-instance
remount-on-shape-change fallback (direction spec §11.2). Plans 1–4 are done and merged; this
composes directly on the Plan 4 render layer (`render.js`) and the Plan 3 reactive core
(`runtime.js`), and on the existing native hot-reload seam (`AssetEvent::Modified<JsSource>` →
`apply_hot_reload` → `run_script`).

The implementation plan lives in `../plans/2026-07-20-supersolid-phase2-05-hmr.md`.

## 1. Goal

Direction spec §1: "hot reload on par with React+Vite / Svelte — including *state-preserving*
HMR, not just file re-execution." Concretely: when an author edits a `.tsx` file, the running UI
reloads with the **new code** but the **live signal values persist** (the counter stays at 5, the
todo list keeps its items and their checked state), except where a component's signal *shape*
changed — there it cleanly remounts (state resets), the accepted degradation.

## 2. The seam it upgrades (and what stays untouched)

A native `.tsx` edit already flows: `file_watcher` → asset reload → `TsxLoader` re-transpile →
**`AssetEvent::Modified<JsSource>`** → `detect_hot_reload` sets `flags.js` → `apply_hot_reload`
calls `rt.run_script(&js)` on the **existing** `UiRuntime` (same Boa engine). Today that re-exec
re-defines the component functions and re-invokes `render(code, mountEl)`, which naïvely appends a
second tree and resets all state.

**The reload behavior itself is realized inside `render()` on that second invocation.** The Rust
reload *algorithm* is unchanged; the only Rust additions are the §9 gating wiring (mount /
apply_hot_reload compute the `hmr` bool and pass it to `UiRuntime::new`; mount emits the one-time
misconfiguration warning):

- `hot_reload.rs`'s reload *logic* is untouched — it stays Supersolid-agnostic (its one
  `UiRuntime::new` call gains the `hmr` arg, nothing else). A plain imperative `.js` app re-execs
  exactly as today (it never calls `render()`, so none of the new behavior engages).
- The trigger is the **same asset-change source** — there is no second change-detection path.
- **HTML edits** rebuild the whole `UiRuntime` (fresh engine) → `render.js`'s module state is gone
  → clean full remount (state resets). Accepted, no special-casing.
- **CSS edits** fire only the `css` flag → reconcile re-applies styles; `run_script` is not called;
  no rehydration churn.

The linchpin: because a JS-only reload reuses the same engine, `render.js`'s module-level state
survives across reloads, and `document.getElementById("root")` returns the **same** cached node
wrapper (`superui_js::wrap_node` caches one wrapper per `NodeId`). So `render()` keeps a `roots`
map keyed by that node; a second `render()` for the same node **is** the hot-reload signal.

## 3. Keying (direction spec §11.2)

`(module = asset path + export name) × (instance = tree position + explicit key) × (cell =
createSignal order at setup)`.

- **module × export name** — each component carries a transpiler-baked id string
  `"<assetpath>#<Name>"`, set via a new `$ss.hot(id, fn)` helper (`fn.__ssId = id`). The transpiler
  emits the id with the module path supplied by the loader (`TranspileOptions.module_id`).
- **instance = tree position + explicit key** — an *instance frame* is opened at each `$ss.cmp`
  and at each `<For>`/`<Index>` row:
  - Component instance key: `parentPath + "/" + id + "#" + ordinal`, where `ordinal` is a
    per-parent, per-id sibling counter — or `parentPath + "/" + id + ":" + key` when a `key` prop
    is present.
  - `<For>` row key: the row's **item identity** (the same reference identity `<For>` already keys
    on), so a row survives reorder.
  - `<Index>` row key: the row's **position** (matching `<Index>`'s position-keyed semantics).
- **cell = creation order** — signals created within a frame, in order, indexed `0..n`.

## 4. Cell collection — one `runtime.js` hook

`createSignal` gains, after building `read`/`write` and before returning:

```js
if (typeof $ssOnSignal === "function") $ssOnSignal(read, write);
```

`render.js` sets `globalThis.$ssOnSignal`. It pushes `{ read, write }` onto the current instance
frame's `cells` array, and is a **no-op when no frame is active** — so module-top-level signals,
plain-`.js` scripts, and every Plan 3/4 test are unaffected (in the Plan 3 engine, `$ssOnSignal`
is simply undefined). Only `createSignal` is hooked. Memos, effects, and `onMount` re-run on
reload by construction; they are not rehydrated.

## 5. Instance-frame lifecycle & rehydration commit (`render.js`)

Module-level state in `render.js`: `roots = new Map()` (mountNode → entry), `frameStack`,
`currentRoot`, and a flag `hmrEnabled` (see §9).

- `$ss.cmp(Comp, props)`:
  - `hmrEnabled === false` → the **exact Plan-4 path**: `untrack(() => Comp(props))`. No frame,
    nothing retained.
  - `hmrEnabled === true` → `untrack(() => withInstance(Comp.__ssId || Comp.name, props && props.key,
    () => Comp(props)))`.
- `withInstance(id, key, run)`: compute `instKey` from the parent frame's path + ordinals (§3),
  create `frame = { path: instKey, cells: [], ordinals: {} }`, register it in
  `currentRoot.instances` (a `Map` keyed by `instKey`), push it, `run()`, **commit rehydration**,
  pop.
- `<For>`/`<Index>` rows: `makeRow` / `makeIndexRow` (Plan 4) wrap their `mapFn` call in
  `withRowInstance(itemOrPosition, …)` inside the per-row `createRoot`, when `hmrEnabled`. Row
  frames register in the same `currentRoot.instances` map (component keys are strings; `<For>` row
  keys are item objects; `<Index>` row keys are position strings — a `Map` handles mixed key types
  by identity/value equality).

**Rehydration commits at frame close** — not in a global post-render pass. At close, the new cell
count is known, giving both correct timing and per-instance shape detection in one place:

```js
var saved = currentRoot.snapshot && currentRoot.snapshot.get(instKey);
if (saved && saved.length === frame.cells.length) {          // shape matches → rehydrate
  for (var i = 0; i < saved.length; i++) {
    (function (v) { frame.cells[i].write(function () { return v; }); })(saved[i]);
  }
}
// else: leave the freshly-created defaults untouched → state resets = per-instance remount
```

Why frame-close (not global):
- A `<For>` row's `createSignal`s run lazily inside the list memo, *after* the enclosing `<For>`
  component's frame has popped, and often *during* the reactive cascade that a parent
  rehydration write triggers. Committing at each frame's close handles synchronously-built
  components and later-built rows uniformly.
- It is the **per-instance shape-change fallback**: `saved.length !== frame.cells.length` (a signal
  added/removed) → skip the commit → the instance keeps its fresh defaults = clean reset, exactly
  the "remount that component" degradation.

The `write(function () { return v; })` updater form sets `v` even when `v` is itself a function
(the updater is invoked, its result stored), so there is no value-type caveat. The commit writes
land inside the active update cycle (the render's `createRoot` → `runUpdates`, or the cascade's
`batch`), so dependent effects re-run and the DOM updates to reflect the preserved state.

## 6. `render()` reload path

```js
function render(code, mountEl) {
  var prev = roots.get(mountEl);
  var snapshot = null;
  if (prev) {                         // second call for this node = hot reload
    snapshot = snapshotCells(prev);   // Map(instKey -> cells.map(c => untrack(c.read)))
    prev.dispose();                   // tear down old reactive scope (effects stop)
    clearChildren(mountEl);           // remove old DOM children (rebuild-fresh)
  }
  var entry = { dispose: null, instances: new Map(), snapshot: snapshot };
  var savedRoot = currentRoot;
  currentRoot = entry;
  try {
    createRoot(function (d) { entry.dispose = d; insert(mountEl, code); });
  } finally {
    currentRoot = savedRoot;
  }
  roots.set(mountEl, entry);
  return entry.dispose;
}
```

When `hmrEnabled === false`, `render()` is the Plan-4 version (no `roots` bookkeeping, no
snapshot) — byte-for-byte the current behavior.

Because the App's `todos` array is itself a preserved cell, its rehydration commit (at App's
frame close) re-supplies the **same item objects**; `<For>` then rebuilds rows whose
`withRowInstance(item)` keys match the snapshot → per-row signal state is preserved across reload
and reorder. `snapshotCells` reads every cell via `untrack(read)` so snapshotting never creates
dependencies.

## 7. Transpiler + loader (`supersolid` / `superui`)

- `TranspileOptions` gains `module_id: Option<String>` (default `None`).
- The JSX pass detects top-level components — uppercase-named `function` declarations and
  `const NAME = (arrow|function)` bindings — and appends `$ss.hot("<module_id>#<Name>", <Name>);`
  registration statements (plain JS; the output still re-parses as plain JS). With `module_id ==
  None` the id is `"#<Name>"` (single-module apps still get a unique-per-name id).
- `TsxLoader` (native) passes `lc.path()` as `module_id`; the build-time `transpile_file` (wasm
  pre-transpile) passes the input path. The registration calls stay in the wasm output — they are
  a single property-set per component at module eval, negligible, and harmless when the flag is off
  (the id is never read).
- `$ss.hot(id, fn) { fn.__ssId = id; return fn; }` is added to `render.js`'s published `$ss`.

## 8. Component-identity limitations (documented)

- Two components sharing both a name and a tree position would collide on `instKey`; distinct
  positions never collide. Path-qualified ids plus tree position make this vanishingly rare at
  app scale.
- Reorder of **same-count** component signals cannot be distinguished from a stable shape and will
  mis-map — the same limitation React Fast Refresh carries for same-count hook reorders. Add /
  remove (count change) is detected and resets cleanly (§5).

## 9. Production gating — `superui/hmr` feature + asset-watcher check

All Plan 5 instrumentation is **construction-time only** — a frame push/pop per `$ss.cmp` and per
row, a `{read, write}` push per `createSignal`. `$ssOnSignal` fires only at signal *creation*,
never on writes / effect re-runs / reconcile, so the **steady-state hot path is identical to
Plan 4 in every build**. The two costs worth avoiding in production are the retained `instances`
map (a permanent per-instance/per-row memory cost that exists only to enable snapshots) and doing
any of the setup work in the shipped wasm build, where reload can never fire.

Gate: a runtime flag `globalThis.__ssHmr`, default **off**, read by `render.js` as `hmrEnabled`.

- **Off** → `$ss.cmp`, the row builders, and `render()` take the exact Plan-4 fast paths; no frame
  is ever pushed, so `$ssOnSignal` short-circuits and nothing is retained.
- **On** → full instrumentation.

**Enablement rule: HMR is active if and only if the `superui/hmr` cargo feature is enabled AND the
Bevy `AssetServer` is watching for changes** (`watching_for_changes()`, public in `bevy_asset`
0.17.3 — set by `bevy/file_watcher` or `AssetPlugin { watch_for_changes_override: Some(true) }`).
The feature is the explicit, `Cargo.toml`-visible opt-in (documented as "enable alongside
`bevy/file_watcher`"); the watcher check reflects the reality that HMR is meaningless without a
watcher (no `AssetEvent::Modified`, so `render()` is never re-invoked, so there is nothing to
rehydrate).

Fail-loud on misconfiguration: if the feature is enabled but the AssetServer is **not** watching,
`mount_when_ready` emits a **one-time `warn!`** ("`superui/hmr` is enabled but the AssetServer is
not watching for changes; state-preserving hot reload is OFF — enable `bevy/file_watcher`") and
HMR stays off. So the only effect of enabling the feature without a watcher is that one log line —
no collection overhead, no half-working state. The warning lives under `#[cfg(feature = "hmr")]`,
so a feature-off build neither warns nor checks. (An HTML-reload rebuild in `apply_hot_reload` can
only occur while watching, so it cannot re-fire the warning; the mount point is the single
one-time site.)

Mechanism: the `hmr` feature lives on **`superui`** (the crate users configure and where the
flag-setting systems have `AssetServer` access). `mount_when_ready` and `apply_hot_reload` (both
exclusive `&mut World` systems) compute
`let hmr = cfg!(feature = "hmr") && world.resource::<AssetServer>().watching_for_changes();` and
thread it into `UiRuntime::new(dom, entity, stylesheet, hmr)`, which sets `globalThis.__ssHmr =
true` only when `hmr` — once, right after `supersolid_runtime::install` and before the first
`run_script` (collection must be live at the first render to have anything to snapshot on the first
reload). `superui_bridge` merely receives the bool (no feature there). `hot_reload.rs`'s reload
logic is unchanged — its single `UiRuntime::new` call (HTML rebuild) gains the same arg as mount's.

Resulting matrix:

- **feature off (default, all release/wasm builds):** compile-time off. No warn, no check, no
  overhead. wasm production also never watches, so it is doubly inert.
- **feature on, no watcher:** one-time `warn!`, HMR off, no collection overhead.
- **feature on + `file_watcher` (native dev):** full HMR. Even here the only cost is setup-time
  plus the instances map — never the update loop.

## 10. Testing strategy

- **`supersolid_runtime` headless** (extends `render_tests`, driving `render()` twice on one
  engine with `__ssHmr = true` to simulate a reload):
  - rehydrate-preserves-value: render a counter, bump it, re-render same-shape code → value
    preserved, DOM rebuilt.
  - shape-change-resets: re-render with a different signal count → state resets.
  - sibling instances keyed distinctly (two same-component siblings keep separate state).
  - `<For>` per-row state preserved across a reorder reload.
  - module-top-level signal resets (locks in the documented limitation).
  - flag off → no `instances` retained, Plan-4 behavior intact.
- **`supersolid` transpiler:** `$ss.hot("<path>#Counter", Counter)` emitted and re-parses as plain
  JS; `module_id` threads from options into the id; components via `const NAME = () => …` are also
  registered.
- **`superui_bridge` integration:** two `run_script`s on one `UiRuntime` built with `hmr = true`,
  simulating the reload re-exec, of a click-incremented counter → the reconciled ECS `Text` keeps
  its value after the "reload"; and a `UiRuntime` built with `hmr = false` proves the Plan-4 fast
  path (no instance retention, no `__ssHmr`).
- **`superui` gating** (native, `#[cfg(feature = "hmr")]`): mounting with the `hmr` feature but a
  non-watching `AssetServer` emits the one-time `warn!` and leaves `__ssHmr` off; mounting while
  watching sets it on. (The feature-off path needs no test — it compiles the flag-set out.)
- **Ledger** (`docs/support/js-dom.md`): add `$ss.hot` and the HMR behavior + `superui/hmr` +
  watcher gating note under the Supersolid render-layer section.

## 11. Non-goals

- DOM node identity / focus / scroll / input caret across a reload (rebuild-fresh preserves signal
  *values*, not DOM).
- Reorder of same-count component signals (mis-maps; see §8).
- Cross-module keying beyond the single per-runtime module (cross-module imports are still Plan-2
  warn-only).
- wasm live reload (out of scope by direction spec §11.3 / base design).
- In-place component-implementation proxy swapping (solid-refresh style); Plan 5 does full re-exec
  + rebuild-fresh with value rehydration, which meets the goal without a proxy layer.
