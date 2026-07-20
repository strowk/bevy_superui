# Supersolid TodoMVC — Plan 6 Design

Date: 2026-07-20
Status: Design agreed. Capstone deliverable of the Phase-2 / Supersolid plan series
([plans README](../plans/README.md)). Implementation plan lives in
`../plans/2026-07-20-supersolid-phase2-06-todomvc.md`.

## 0. What this is

The **runnable, hot-reloadable Supersolid TodoMVC** — the Phase-2 capstone — in a new
`examples/todomvc_supersolid/` folder, authored in idiomatic Solid-style `.tsx`, running on
native (live HMR) and `wasm32-unknown-unknown` (pre-transpiled). It is a **pure composition of
Plans 1–5** (all DONE + merged): no browser/ledger changes, no underlying-crate work. The existing
plain-HTML/CSS/JS `examples/todomvc` is retained unchanged.

Grounding: direction spec [`2026-07-19-superui-component-framework-direction.md`](./2026-07-19-superui-component-framework-direction.md)
(§1 HMR goal, §5 authoring surface + control-flow, §8 authoring, §11.3 native/wasm split); the DONE
Plan docs 2–5; the plain example `examples/todomvc/`; the capability ledger `docs/support/`.

## 1. Goal & scope

A Supersolid TodoMVC that (a) reads clearly to anyone familiar with Solid, and (b) demonstrates
**state-preserving hot reload** — the Phase-2 headline (direction §1).

**Feature set — everything inside the supported DOM subset (`docs/support/js-dom.md`):**

- Add a todo — via an **Add button** (Enter-to-add needs `event.key`, which is 🟡 *not exposed*).
- Toggle a todo complete — checkbox `change` event.
- Delete a todo — destroy button (`click`).
- Toggle-all — a header checkbox that sets every todo's `done` to one value.
- Filter **all / active / completed** — three buttons, a `selected` class on the active one.
- Clear completed — a button that drops all done todos.
- "N item(s) left" count.

**Explicitly out of scope** (outside the current subset — this is Plan 6, not a browser-capability
plan; those are Phase 3):

- **Editing** (classic double-click a row → type → Enter/Escape). Needs `dblclick` (unsupported)
  and `event.key` (🟡). Omitted.
- **Persistence.** `localStorage`/`cookie` is ⛔ ("games persist via ECS"). The app is pure
  in-memory; todos reset on restart.
- **`window.bevy` ECS-seam demo.** The plain example fires `bevy.send("TodoAdded", …)`; the
  Supersolid app **omits** it to read as a clean, idiomatic Solid app end-to-end.

## 2. Component decomposition & where state lives (the HMR story)

**Core principle: all application state lives in the top-level `App` component; every child is a
stateless view driven only by props.** Plan 5 preserves `App`'s live signal cells across a hot
reload (keyed by `module × instance × creation-order`, direction §11.2), so editing any child's
markup/CSS — or `App`'s own view — keeps the todos, the active filter, **and** the half-typed
new-todo text alive. Because children hold no state, they can be freely hot-swapped.

- **`App`** — top-level, `$ss.hot`-tagged, **stable signal shape** (three `createSignal` calls, in
  a fixed order, so HMR never trips the shape-change reset). Owns:
  - `todos` — `createSignal<Todo[]>([])`, where `Todo = { id: number, title: string, done: boolean }`
    (plain data).
  - `filter` — `createSignal<"all" | "active" | "completed">("all")`.
  - `draft` — `createSignal("")` (the in-progress new-todo input text — a deliberate HMR showcase:
    start typing, edit the `.tsx`, the text survives).
  - Derived `createMemo`s: `remaining` (count of not-done), `filtered` (todos matching `filter`),
    `hasCompleted`.
  - Handlers: `addTodo`, `toggle(id)`, `remove(id)`, `toggleAll`, `clearCompleted`, `setFilter`.
- **`Header`** — the `<input>` (its `value` reactively bound to `draft`; `onInput` updates `draft`)
  and the Add `<button>`. Props: `draft` getter, `onInput`, `onAdd`.
- **`TodoItem`** — one `<li>`: the toggle `<input type=checkbox>` (`checked` bound to `todo.done`),
  the label `<span>`, the destroy `<button>`. Props: `todo`, `onToggle`, `onRemove`. Pure view.
- **`Footer`** — the count, the three filter buttons (`selected` class bound to `filter`), and the
  clear-completed button. Props: `remaining` getter, `filter` getter, `hasCompleted` getter,
  `onFilter`, `onClearCompleted`.

**State updates are immutable and identity-preserving.** Toggling/removing rebuilds the `todos`
array but keeps the identity of untouched items:
`setTodos(todos().map(t => t.id === id ? { ...t, done: !t.done } : t))`. `filtered` is a
`createMemo` feeding a single keyed `<For>`, so `<For>` (keyed by item identity) reuses unchanged
rows and only re-renders the one that changed — the fine-grained, surgical path (direction §4/§5).

Rationale for **plain-data `done` + immutable updates** over a per-item `createSignal`: it keeps
`App`'s signal shape trivially stable, makes the whole app state a clean snapshot/rehydrate unit for
HMR, and keeps filtering reactive through the `filtered` memo — the clearest correct demonstration.
(Per-item fine-grained signals are a valid Solid variation but add HMR subtleties around
signal-closure survival that don't serve the "clear Solid app" goal.)

## 3. File layout

```
examples/todomvc_supersolid/
  Cargo.toml            # deps: superui, superui_css, superui_bridge, bevy; build-dep: supersolid;
                        # features: hmr (-> superui/hmr + bevy/file_watcher), mcp_debug, debug-ui
  build.rs              # HOST-ONLY: transpile app.tsx -> app.generated.js (rerun-if-changed)
  src/main.rs           # SuperUiPlugin wiring; setup() picks .tsx vs .generated.js by target/feature
  assets/ui/todomvc_supersolid/
    index.html          # minimal shell: <body><div id="root"></div></body>
    style.css           # adapted from examples/todomvc/assets/ui/todomvc/style.css
    app.tsx             # the Supersolid app (App/Header/TodoItem/Footer + render(App, #root))
    app.generated.js    # GITIGNORED — build.rs output; loaded on wasm / no-HMR native
  tests/
    todomvc.rs          # headless: add/toggle/delete/filter/clear via dispatched DOM events
    hmr.rs              # headless: re-exec module preserves todos + filter + draft
    support/mod.rs      # shared harness (mirrors examples/todomvc/tests/support)
```

`app.tsx` imports its runtime names from `"supersolid"`
(`import { createSignal, createMemo, For, render } from "supersolid";`). The transpiler **strips**
that import (the specifier is in `runtime_specifiers`) and the names resolve to the injected
globals; the import exists purely so the IDE/TS tooling resolves them (direction §3). Co-located CSS
is loaded via the app's `SuperUiRoot.css` handle, not a JS `import`.

The shell `index.html` is intentionally minimal — a single `<div id="root">` mount point inside
`<body>`. `App` builds the entire UI (including the "todos" title chrome), mounted by
`render(App, document.getElementById("root"))`, so the `.tsx` reads as a self-contained Solid app.

## 4. Native vs wasm build story

`TsxLoader` (oxc) is compiled **native-only** (`#[cfg(not(target_arch = "wasm32"))]`, direction
§11.3), so live `.tsx` is used **only** where a file-watcher HMR build can actually benefit;
everything else loads **pre-transpiled `.js`**. The rule: *ship pre-transpiled `.js` whenever it is
needed (wasm, or a native build without HMR); use live `.tsx` only for the HMR dev build.*

- **`build.rs`** — declares `supersolid` as a **build-dependency**. Build scripts and their deps
  always compile for the **host**, never the target, so oxc is never linked into the wasm binary. It
  transpiles `assets/ui/todomvc_supersolid/app.tsx → app.generated.js` and emits
  `cargo:rerun-if-changed=assets/ui/todomvc_supersolid/app.tsx`. It runs on every build (transpiling
  one file is cheap and deterministic), guaranteeing `app.generated.js` is always present and fresh
  whenever a build path needs it.
- **`setup()` asset choice** (both arms produce `Handle<JsSource>`, so the rest of the pipeline is
  identical):
  - `cfg(all(not(target_arch = "wasm32"), feature = "hmr"))` → load **`app.tsx`** through the live
    `TsxLoader` → state-preserving HMR.
  - otherwise (wasm **or** native release / no `hmr` feature) → load **`app.generated.js`**.
  - The wasm arm is selected by `target_arch` regardless of the `hmr` feature, so a stray
    `--features hmr` on wasm still loads the `.js` (there is no `.tsx` loader on wasm).
- The example's **`hmr` feature** = `["superui/hmr", "bevy/file_watcher"]`; the
  `AssetPlugin.watch_for_changes_override = Some(true)` is set only under that feature (it is inert
  without `file_watcher` anyway).
- **`mcp_debug` feature** (mirrors the plain example): pulls `bevy_brp_extras` + `bevy/bevy_remote`
  and registers a `DebugClick` injector + the BRP screenshot/keys plumbing, so the `bevy_brp_mcp`
  server can drive/inspect the running app for manual issue-spotting. **`debug-ui`** (optional text
  dump + click/key logging) is carried over too.

**Run matrix:**

| Command | Target | Path | HMR |
|---|---|---|---|
| `cargo run -p todomvc_supersolid --features hmr` | native | `app.tsx` (live) | ✅ state-preserving |
| `cargo run -p todomvc_supersolid` | native | `app.generated.js` | — |
| `cargo build -p todomvc_supersolid --target wasm32-unknown-unknown` | wasm | `app.generated.js` | — |

## 5. Testing strategy (TDD)

Mirror the plain example's headless harness (`examples/todomvc/tests/`): parse the shell HTML into a
`Dom`, transpile `app.tsx` via `supersolid` and run it in a headless `UiRuntime` (no window),
dispatch DOM events (`click` / `change` / `input`) through the engine exactly as the input systems
do, and assert the **reconciled ECS state** (`Text` content, node presence, classes).

- `tests/todomvc.rs` — add, toggle complete, delete, filter switching (all/active/completed),
  clear-completed, toggle-all, and the "N items left" count.
- `tests/hmr.rs` — build the module, mutate state (add todos, set a filter, type a draft), re-exec
  the **same** module on the **same** runtime (as `apply_hot_reload` does for a `JsSource` Modified
  event), and assert todos + filter + draft are preserved through the DOM rebuild → reconcile
  (pattern from `superui_bridge/tests/supersolid_render.rs::supersolid_hmr_preserves_counter_across_reexec`).

Executed via **subagent-driven development**, task-by-task, TDD throughout (write the failing test,
implement, green, commit), on `main` per project CLAUDE.md.

## 6. Ledger

No new capabilities: the app stays entirely within the ✅ subset (direction §7 — the component layer
is above the arena DOM; everything downstream is untouched Phase-1 machinery). `docs/support/`'s
status note is updated to record that a Supersolid TodoMVC now exercises the Supersolid ✅ rows
(`createSignal`/`createMemo`/`<For>`/`render`/`$ss.*`/HMR). A light ledger smoke-test analogous to
`examples/todomvc/tests/ledger.rs` is included if it adds signal beyond the functional tests.

## 7. Non-goals (this plan)

- No editing, no persistence, no `window.bevy` demo (see §1).
- No browser/ledger capability additions (`event.key`, `dblclick`, `localStorage`) — Phase 3.
- No `cargo superui` cargo-metadata projector / component-distribution work (direction §9) — later.
- No per-item fine-grained `done` signals (see §2 rationale) — a possible future variation.
