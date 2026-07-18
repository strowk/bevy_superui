# bevy_superui — Phase 1 plan series

Phase 1 delivers a runnable, hot-reloadable **TodoMVC** authored in plain HTML/CSS/JS,
running on native + `wasm32-unknown-unknown`. See the design spec:
[`../specs/2026-07-18-bevy-superui-design.md`](../specs/2026-07-18-bevy-superui-design.md).

Phase 1 is decomposed into 6 sequential plans, each shipping an independently-testable crate.
Later plans are written just-in-time (informed by the actual Boa / flair-0.6 APIs pinned down
while building), not all up front.

| # | Crate / deliverable | Scope | Status |
|---|---|---|---|
| 1 | `superui_dom` | Headless arena DOM (nodes, mutation, attributes/classList/textContent, W3C capture→target→bubble event dispatch). No Bevy/JS deps. | ✅ Done — merged to `main` ([plan](./2026-07-18-superui-phase1-01-dom.md)) |
| 2 | `superui_html` | HTML subset → DOM, via `html5ever`. | ⬜ Not started |
| 3 | `superui_js` + `superui_api` | Boa engine behind a `JsEngine` trait; broad DOM/Web API bindings (document, Node/Element, events, classList, style, console, timers, `fetch` warn-stub) + the `window.bevy` bridge. | ⬜ Not started |
| 4 | `superui_css` | Fork of `bevy_flair` 0.6 (targets Bevy 0.17), extended for real HTML element/attribute selectors and `:hover`/`:focus`/`:checked`. | ⬜ Not started |
| 5 | `superui_bridge` + `superui` | Reconciler (DOM diff → Bevy ECS commands; picking/input → DOM events), `SuperUiPlugin`, asset loaders, hot reload via `AssetEvent::Modified`, observer-based `window.bevy`. | ⬜ Not started |
| 6 | `examples/todomvc` + `docs/support/` | The runnable TodoMVC example (native + wasm) and the capability ledger (`html.md` / `css.md` / `js-dom.md`, status ✅/🟡 Roadmap/⛔). | ⬜ Not started |

## Conventions

- **Bevy 0.17**, edition 2021. Only Bevy-facing crates (`superui_bridge`, `superui`,
  `superui_css`) touch Bevy; `superui_dom` / `superui_html` / `superui_js` stay
  version-agnostic and wasm-clean.
- Every plan is executed via subagent-driven development (fresh implementer per task,
  task review + final whole-branch review), TDD throughout, on a feature branch merged to
  `main`.
- Naming principle: public surface mirrors web standards (`append_child`→appendChild, etc.);
  no bespoke markup or widget API. The only non-web surface is the `window.bevy` bridge.

## Resuming in a fresh session

> Read `docs/superpowers/specs/2026-07-18-bevy-superui-design.md` and this file. Plan 1
> (`superui_dom`) is done and merged to `main`. Write and execute the next unstarted plan
> in the table above.

## Performance strategy

There is a separate design about performance testing strategy, this is planned and implemented separately from the above Phase 1 plan series. 
Do not attempt to mix this with this described plan.