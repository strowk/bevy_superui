# bevy_superui capability ledger

This directory is the **authoritative, machine-loadable record** of what the
`bevy_superui` HTML/CSS/JS subset supports. It is both the **AI-context document**
(load it so generated UI stays inside the supported surface) and the **roadmap
tracker**. See the design: `../superpowers/specs/2026-07-18-bevy-superui-design.md` §7.

## Legend

**Status**
- ✅ **Supported** — implemented and covered by tests.
- 🟡 **Roadmap** — achievable in theory (regardless of what `bevy_ui` can do
  today) and planned; currently degrades gracefully (no-op / skipped / warn).
- ⛔ **Won't support** — fundamentally out of scope (network, cookies, real
  navigation, multi-document).

**Priority tier** (by game-UI usefulness, for ordering work)
- **T0** essential — layout, text, click, class toggling.
- **T1** common — inputs, lists, hover/focus, transitions.
- **T2** advanced — SVG, canvas, animations, transforms.
- **T3** niche.

Rows are ordered T0 first, so the top of each file is the highest-value surface.

## Files
- `html.md` — HTML elements & attributes.
- `css.md` — CSS properties, selectors, pseudo-classes, at-rules.
- `js-dom.md` — DOM/Web API objects, methods, events.

## Graceful degradation

Unsupported features never hard-crash (design §1): unknown tags render as plain
boxes, unknown CSS is skipped, unimplemented JS methods no-op/warn, `fetch` warns
and rejects. AI-generated code that touches an unimplemented corner keeps running.

## Sync policy

The ledger is kept in step with the implementation. `examples/todomvc/tests/ledger.rs`
is a best-effort check that a sample of ✅ DOM rows have a live binding (executing a
snippet using them does not throw). Tighten over time; it is a smoke test, not a proof.

## Phase 1 status

Phase 1 (TodoMVC) is complete. The ✅ rows below are what shipped; 🟡 rows are the
Phase 2/3 roadmap. TodoMVC itself exercises the T0/T1 ✅ core.
