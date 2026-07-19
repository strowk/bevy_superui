# bevy_superui — implementation plans

Plans are grouped by roadmap phase. Each plan ships an independently-testable deliverable and is
executed via subagent-driven development (fresh implementer per task, task review + final
whole-branch review), TDD throughout, on a feature branch merged to `main`. Later plans in a
series are written **just-in-time** (informed by the actual APIs pinned down while building), not
all up front.

Design specs: [base design](../specs/2026-07-18-bevy-superui-design.md) ·
[Supersolid direction](../specs/2026-07-19-superui-component-framework-direction.md).

**Roadmap:**
- **Phase 1 — browser core** — plain HTML/CSS/JS TodoMVC, native + `wasm32-unknown-unknown`. ✅ Done.
- **Phase 2 — Supersolid** — the Solid-like fine-grained TSX component framework, its lower-level
  DOM/JS prerequisites, and a Supersolid TodoMVC example. ◀ **current**.
- **Phase 3 — browser compatibility** — fuller browser fidelity, scoped to what doesn't require
  large underlying-crate changes. Later.

---

## Phase 1 — browser core (✅ complete)

Phase 1 delivered a runnable, hot-reloadable **TodoMVC** authored in plain HTML/CSS/JS, running on
native + `wasm32-unknown-unknown`. Decomposed into 6 sequential plans, each shipping an
independently-testable crate.

| # | Crate / deliverable | Scope | Status |
|---|---|---|---|
| 1 | `superui_dom` | Headless arena DOM (nodes, mutation, attributes/classList/textContent, W3C capture→target→bubble event dispatch). No Bevy/JS deps. | ✅ Done — merged to `main` ([plan](./2026-07-18-superui-phase1-01-dom.md)) |
| 2 | `superui_html` | HTML subset → DOM, via `html5ever`. | ✅ Done — merged to `main` ([plan](./2026-07-18-superui-phase1-02-html.md)) |
| 3 | `superui_js` + `superui_api` | Boa engine behind a `JsEngine` trait; broad DOM/Web API bindings (document, Node/Element, events, classList, style, console, timers, `fetch` warn-stub). **`window.bevy` moved to Plan 5.** | ✅ Done — merged to `main` ([plan](./2026-07-18-superui-phase1-03-js-api.md)) |
| 4 | `superui_css` | Fork of `bevy_flair` 0.6 (targets Bevy 0.17), extended for real HTML element/attribute selectors and `:hover`/`:focus`/`:checked`. | ✅ Done — merged to `main` ([plan](./2026-07-19-superui-phase1-04-css.md)) |
| 5 | `superui_bridge` + `superui` | Reconciler (DOM diff → Bevy ECS commands; picking/input → DOM events), `SuperUiPlugin`, asset loaders, hot reload via `AssetEvent::Modified`, **the full `window.bevy` bridge**. | ✅ Done — merged to `main` ([plan](./2026-07-19-superui-phase1-05-bridge.md)) |
| 6 | `examples/todomvc` + `docs/support/` | The runnable TodoMVC example (native + wasm) and the capability ledger (`html.md` / `css.md` / `js-dom.md`). | ✅ Done — merged to `main` ([plan](./2026-07-19-superui-phase1-06-todomvc.md)) |

Run it: `cargo run -p todomvc`.

---

## Phase 2 — Supersolid (current)

**Supersolid** is a single-model, Solid-like fine-grained component framework authored in
`.tsx`, transpiled in Rust and run in Boa above the existing arena DOM. Direction spec:
[`../specs/2026-07-19-superui-component-framework-direction.md`](../specs/2026-07-19-superui-component-framework-direction.md).

Decomposed into a plan series. **Plan 1 (lower-level prerequisites) is detailed now**; Plans 2–6
are written just-in-time as the runtime/compiler APIs firm up (Phase 1's approach). The phase's
deliverable is a runnable **Supersolid TodoMVC** in a new `examples/todomvc_supersolid/` folder —
the existing `examples/todomvc` is kept as-is.

| # | Deliverable | Scope | Status |
|---|---|---|---|
| 1 | Lower-level DOM/JS prerequisites | The small additions Supersolid's runtime needs, added to existing crates: per-**text-node** value mutation (`superui_dom` + `superui_api`), reconciler **node move / reorder** for keyed lists (`superui_bridge`), `document.createDocumentFragment` (`superui_api`). Headless-testable; ledger updated. | 📝 In progress ([plan](./2026-07-19-supersolid-phase2-01-prereqs.md)) |
| 2 | `supersolid` transpiler | `.tsx`/`.ts` → JS via `oxc` (type-strip + Solid-style JSX lowering) inside a Bevy `AssetLoader`; build-time pre-transpile path for wasm. | 📝 Planned ([plan](./2026-07-19-supersolid-phase2-02-transpiler.md)) |
| 3 | `supersolid` reactive core | JS runtime module in Boa: `createSignal`/`createEffect`/`createMemo`/`onMount`/`onCleanup`/`createContext`/`useContext` + scheduler. Headless. | ⏳ Just-in-time |
| 4 | `supersolid` render + control-flow | JSX runtime (build-once nodes + surgical reactive bindings via the DOM API) and control-flow `<Show>`/`<For>`/`<Index>`/`<Switch>`. | ⏳ Just-in-time |
| 5 | State-preserving hot reload | `.tsx` HMR: signal-cell rehydration keyed by module × instance × creation-order; remount-on-shape-change fallback (spec §11.2). | ⏳ Just-in-time |
| 6 | `examples/todomvc_supersolid` | The Phase 2 deliverable: runnable Supersolid TodoMVC (native + wasm, hot reload), authored in `.tsx`. Existing `examples/todomvc` retained. | ⏳ Just-in-time |

The series is provisional — crate splits and plan boundaries may shift as Plans 2–4 are designed
just-in-time (each may get a short focused brainstorm first, since the direction spec is
architectural, not implementation-level).

---

## Conventions

- **Bevy 0.17**, edition 2021. Only Bevy-facing crates (`superui_bridge`, `superui`,
  `superui_css`, and the Bevy-facing parts of `supersolid`) touch Bevy; `superui_dom` /
  `superui_html` / `superui_js` stay version-agnostic and wasm-clean.
- Every plan is executed via subagent-driven development (fresh implementer per task, task review
  + final whole-branch review), TDD throughout, on a feature branch merged to `main`.
- Naming principle: the browser's public surface mirrors web standards
  (`append_child`→appendChild, etc.); no bespoke markup or widget API. The only non-web surface is
  the `window.bevy` bridge. Supersolid's authoring surface is Solid-shaped TSX.

## Performance strategy

There is a separate design about performance testing strategy, planned and implemented separately
from the above plan series. Do not mix it with these plans.
