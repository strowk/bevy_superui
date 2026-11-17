# Mouse events & dragging — design

**Date:** 2026-11-17
**Status:** Draft — awaiting spec review (design approved conversationally)
**Ships in:** superui `0.4` line (exact patch set at release)

## Goal

Give superui a browser-faithful **mouse-event foundation** so TSX/JS apps can
implement hover effects and dragging themselves, without superui shipping a
bespoke drag widget. Concretely, expose to author JS:

- `mousedown` / `mouseup` / `mousemove`
- `mouseover` / `mouseout` (bubbling)
- `mouseenter` / `mouseleave` (non-bubbling)

each carrying `clientX`, `clientY`, `button`, `buttons`. While a mouse button is
held, `mousemove` and `mouseup` **also** dispatch at `document` / `window`, so a
drag survives the pointer outrunning the dragged element (classic web idiom:
`mousedown` on the handle, then `document.addEventListener('mousemove'/'mouseup')`
for the duration).

The driving use cases (all three wanted): draggable UI chrome, drag-and-drop
between elements, and dragging on a canvas/world surface. The low-level mouse
primitives cover all three; nothing higher-level is built in.

### Hard constraint — zero cost when nobody listens

An app that only uses `click` and CSS `:hover` (the common case) must pay
**nothing** for this feature — the same cost as today. Cost may appear only in
apps that actually register mouse listeners, and must scale to the specific
event families they use.

### Out of scope

- HTML5 Drag-and-Drop API (`draggable` attribute, `dragstart`/`dragover`/`drop`,
  `dataTransfer`). All target use cases are served by the mouse primitives.
- Pointer Events API (`pointer*`, `setPointerCapture`). Capture is handled by the
  document-level dispatch rule instead.
- Touch events.
- `movementX` / `movementY` — JS derives deltas from successive `clientX/Y`.
- `offsetX` / `offsetY` (element-relative coords) — would need per-node rect
  lookups at dispatch time; not needed by the target use cases.

## Background — what exists today

The input → DOM-event seam lives in `crates/superui_bridge/src/events.rs`. The
established pattern: a `bevy_picking` observer resolves the hit entity to a
`NodeId`, pushes a `PendingDomEvent` onto the `PendingDomEvents` queue, and
`drain_dom_events_system` dispatches each into JS with full W3C
capture → target → bubble semantics, then reconciles.

Key facts that shape this design:

- **DOM dispatch is already generic over event-type strings.** `dom.js`'s
  `dispatch()` and `addEventListener` handle any type; W3C ordering is already
  implemented in JS (the Rust-side `superui_dom::build_dispatch_plan` is retired,
  kept only for reference/tests). So `addEventListener('mouseover', …)` already
  dispatches correctly — the gap is the *source* of such events and the *payload*.
- **Only `PointerClick` is wired as a pointer source.** There is no
  mouseover/out/down/up/move generation.
- **The cross-engine `dispatch_event` ABI carries only `key`.** No coordinates or
  button info. Both `V8Engine` and `WebEngine` implement the trait in
  `crates/superui_js`.
- **`document` is a plain object literal** in `dom.js` (no `addEventListener`, no
  node id). `window` is `globalThis`. Neither is an event target today.
- **`OP_SET_LISTENER` carries only `[nid, 0|1]`** — "this node has *some* listener"
  — which is enough for the picking/`interactive` marker but not per-type.
- **Picking already keeps every element `is_hoverable: true`** (it drives flair's
  `:hover`); `should_block_lower` is gated on a node (or ancestor) being
  `interactive` = having any listener. A node with a mouse listener is therefore
  already `interactive` and blocks correctly — no change needed to the picking
  policy.
- **`HoverMap` and `PointerLocation` are available.** `scroll.rs` already reads
  `Res<HoverMap>` directly (`hover_map.get(&PointerId::Mouse)`), establishing the
  house style of reading picking state in a per-frame system.
- **The transpiler already lowers `on*` props generically.** `supersolid/src/jsx.rs`
  turns any `on<X>` prop (X ≥ 1 char) into `$ss.on(el, "<x>".to_ascii_lowercase(),
  handler)`. So JSX `onMouseDown`/`onMouseOver`/`onMouseEnter`/`onMouseMove`/… all
  become `addEventListener("mousedown"/"mouseover"/…)` with **no transpiler change**.
  The only listeners that need new support are `document`/`window` ones (written
  directly in JS during a drag), handled by Component 4.

## Architecture

One new per-frame system, `mouse_events_system`, in `superui_bridge`, plus three
cross-cutting pieces: a listener index, an extended dispatch payload, and
`document`/`window` as event targets.

### Component 1 — `MouseListenerIndex` (the zero-cost gate)

**What it does.** A small resource holding global counts of how many listeners
exist per mouse event type across the whole document (including `document` and
`window`). Exposes cheap derived predicates:

- `any()` — any mouse listener at all
- `any_hover()` — any of `mouseover`/`mouseout`/`mouseenter`/`mouseleave`
- `any_button()` — any `mousedown`/`mouseup`
- `any_move()` — any `mousemove`

**How it's maintained.** `OP_SET_LISTENER` is extended to carry the **event type**
(interned) alongside the present/absent flag, and JS emits a transition op
whenever a *specific type's* listener count on a node crosses 0↔1 (today it only
tracks the node's overall has-any). The op applier updates, from the same op:

1. **Per-node listener bookkeeping** → feeds the existing picking/`interactive`
   check unchanged ("node has any listener"). This MUST stay correct — the
   picking tests in `superui_bridge/tests/picking.rs` pin it.
2. **The global per-type counts** in `MouseListenerIndex`.

`document`/`window` listeners feed the same index (see Component 4).

**How it gates (two levels):**

- **Run condition on the whole system:** `mouse_events_system.run_if(idx.any())`.
  When no mouse listener exists anywhere, Bevy does not even fetch the system's
  params — no `HoverMap` read, no queries, no diffing. This is the hard
  zero-cost guarantee.
- **Per-family gating inside the system:** skip the hover-set diff unless
  `any_hover()`; skip button-edge handling unless `any_button()`; skip the
  per-frame `mousemove` dispatch unless `any_move()`.

### Component 2 — `PointerState` (frame-to-frame memory)

A resource holding what the previous frame saw, so the system can diff:

- `hover: Vec<NodeId>` (or small set) — the resolved hovered nodes last frame
- `position: Option<Vec2>` — last pointer position (logical window px)
- `buttons: MouseButtonMask` — last frame's held-button mask

**Seeding rule:** the first frame the system activates (transition from "no mouse
listeners" to "some"), `PointerState` is seeded to the *current* frame's hover
set and position without emitting any events. This prevents a spurious
`mouseover` for nodes already under the cursor when a listener is added mid-hover
— matching browser behavior.

### Component 3 — extended event payload & dispatch ABI

- **`MousePayload`** — a small `Copy` struct: `{ client_x: f32, client_y: f32,
  button: u8, buttons: u8 }`.
- **`PendingDomEvent`** gains `mouse: Option<MousePayload>`, independent of the
  existing `key: Option<String>` (keyboard path untouched).
- **`JsEngine::dispatch_event`** gains one new parameter: `mouse:
  Option<MousePayload>`. Both `V8Engine` and `WebEngine` implement it identically.
- **`UiRuntime::dispatch_dom_event`** threads the payload through.
- **`dom.js` `dispatch()`** sets `clientX`/`clientY`/`button`/`buttons` on the
  event object when a mouse payload is present; they are `undefined` otherwise.

`button` / `buttons` follow the W3C convention (`button`: 0 = left, 1 = middle,
2 = right; `buttons`: bitmask of currently-pressed buttons).

### Component 4 — `document` / `window` as bubble-chain targets

The drag-capture requirement ("`mousemove`/`mouseup` keep reaching the drag's
handler after the pointer leaves the dragged element") is met by putting
`document`/`window` **at the top of the bubble chain**, exactly as the browser
does — *not* by dispatching a second, separate event at `document`.

- Give `document` and `window` real listener storage plus
  `add`/`removeEventListener`, backed by **reserved sentinel js node ids** so the
  existing dispatch and `OP_SET_LISTENER` machinery addresses them like any node.
  Their listeners feed `MouseListenerIndex`.
- **Extend `dom.js`'s `dispatch()` ancestor walk** so the chain above the root is
  `… → body(root) → document → window`. Today the walk stops at the root (body's
  `parentNode` is null); it will continue through the two sentinels.
- **Consequence — `event.target` is preserved.** A `mousemove`/`mouseup`
  dispatched at the node under the pointer bubbles up to any `document`/`window`
  listener with `target` still equal to that node. So a drag handler on
  `document` reads `event.target` to find the slot under the cursor — no separate
  dispatch, no lost identity.
- **This is a general bubbling change, intended.** Any bubbling event (`click`,
  `keydown`, …) now also reaches `document`/`window` listeners — the browser-
  faithful behavior (e.g. a global `document` click handler works). Non-bubbling
  events (`mouseenter`/`mouseleave`, `focus`/`blur`) are unaffected, since
  `dispatch()` already doesn't walk ancestors when `bubbles` is false. Existing
  dispatch tests don't register `document`/`window` listeners, so they're
  unchanged; a regression test pins the new reach.
- `document`/`window` have no backing ECS entity, which is fine — only element
  nodes do.

### Component 5 — `mouse_events_system` (the generator)

Runs each frame (under the run condition). Reads `HoverMap`, `PointerLocation`,
`ButtonInput<MouseButton>`, resolves nodes via the existing `resolve_dom_node`
ancestor-walk (so hovering a text child counts as hovering its element), and
produces `PendingDomEvent`s:

1. **Hover diff** (if `any_hover()`): compare resolved hover set to
   `PointerState.hover`.
   - For each node left: `mouseout` (bubbles) + `mouseleave` (no bubble).
   - For each node entered: `mouseover` (bubbles) + `mouseenter` (no bubble).
   - `out`/`leave` on old targets are enqueued before `over`/`enter` on new ones.
2. **Button edges** (if `any_button()`): for each button whose state changed vs
   `PointerState.buttons`, enqueue `mousedown`/`mouseup` at the node under the
   pointer. These bubble up through `document`/`window` (Component 4), so a
   drag handler on `document` sees them with `target` = the real node.
3. **Move** (if `any_move()`): if the pointer position changed since last frame,
   enqueue a single `mousemove` at the node under the pointer. Because the root
   is a full-viewport hoverable node, "under the pointer" is at least the root
   whenever the cursor is inside the UI, so the move bubbles to `document`/`window`
   and a drag's `document`-level `mousemove` handler keeps firing even as the
   cursor crosses the gap between panels — the drag-capture guarantee, achieved
   by bubbling rather than a second dispatch. (If no node resolves at all — cursor
   outside the UI viewport — and a button is held, enqueue the `mousemove`/`mouseup`
   directly at `document` so an in-progress drag still completes.)
4. Update `PointerState` to the current frame.

**Ordering within a frame** mirrors the browser where observable:
`mouseout`/`mouseleave` before `mouseover`/`mouseenter`; `mousedown` before the
`click` the existing `PointerClick` path later synthesizes. `click` generation is
unchanged — this feature does not touch the click path.

All enqueued events drain through the existing `drain_dom_events_system`, so
dispatch, reconcile, and the dirty-flag flow are unchanged.

### System scheduling

`mouse_events_system` is registered in `crates/superui/src/mount.rs` alongside the
other input systems, ordered so its events are enqueued before
`drain_dom_events_system` runs in the same frame (same contract the keyboard path
already follows). `MouseListenerIndex` and `PointerState` are initialized as
resources there.

## Data flow (drag, end to end)

1. App does `tile.addEventListener('mousedown', …)` and, on mousedown,
   `document.addEventListener('mousemove', onMove)` +
   `addEventListener('mouseup', onUp)`.
2. `addEventListener` for each type emits an extended `OP_SET_LISTENER`;
   `MouseListenerIndex` now reports `any()`, `any_button()`, `any_move()`.
3. The run condition flips on; `mouse_events_system` begins running.
4. Press over the tile → `mousedown` at the tile node (payload with coords +
   button). App's handler records the grabbed item and starts following.
5. Pointer moves, button still held → `mousemove` at the node under the cursor,
   bubbling up to `document` where `onMove` lives (`event.target` is whatever's
   under the cursor; `onMove` ignores it and just reads `clientX/clientY` to move
   the floating tile). The tile follows across the gap between panels because the
   root is always under the cursor inside the UI, so `document` keeps receiving
   moves.
6. Release over a target slot → `mouseup` at the slot, bubbling to `document`
   (`onUp`). `onUp` reads `event.target` to resolve the drop slot, moves the item
   between backing stores, and removes the document listeners.
7. With the document listeners gone, `MouseListenerIndex` drops those counts; if no
   mouse listeners remain, the run condition turns the system back off.

## Worked example — chest ↔ inventory drag-and-drop

New crate `examples/inventory/` (supersolid TSX, matching `game_menu`/`todomvc`):

- Two panels side by side: **Chest** and **Player inventory**, each a grid of
  slots. Slots hold item tiles backed by a reactive store.
- `mousedown` on a tile begins a drag: a floating tile element follows `document`
  `mousemove` (positioned via inline `style` from `clientX/clientY`).
- `mouseup` **resolves the drop slot from `event.target`** (the element under the
  cursor), not from coordinates — moving the item between the two backing stores;
  release over no valid slot returns it to its origin. Resolving by element
  identity (rather than hit-testing coordinates) is what keeps the example
  E2E-testable: the test engine can target an element but cannot supply real
  layout coordinates (`getBoundingClientRect` returns zeros — see Risks).
- Exercises: the full mouse-event family, document-level capture across the gap
  between panels, the coordinate payload (for the visual follow), and the reactive
  layer.

It doubles as the manual demo and the end-to-end automated test target, driven
through the test engine's new drag actions (below).

## Test-engine integration (`superui_test_engine`)

The test engine synthesizes input by pushing `PendingDomEvent`s at a located node
(`driver.rs::dispatch`), rather than moving a real pointer. The existing `hover`
action does `dispatch(…, "mouseover")` — it keeps compiling (the new `mouse` field
defaults to `None`), but it's low-fidelity against the new model and can't drive a
drag at all. This feature makes it faithful and adds drag actions. **Both drivers
change identically: `driver.rs` (headless) and `ui_driver.rs` (egui UI mode).**

- **Faithful `hover`.** Instead of a lone `mouseover`, the `hover` action emits the
  full browser sequence against a small driver-side `last_hovered: Option<NodeId>`:
  `mouseout` + `mouseleave` on the previous target (if any and different), then
  `mouseover` + `mouseenter` on the new target. Each carries a synthetic
  `MousePayload` (coordinates are placeholder — see below).
- **New drag actions**, mirroring Playwright shape:
  - `mouseDown(locator)` → `mousedown` at the node (bubbles to `document`/`window`).
  - `moveTo(locator)` → `mousemove` at the node (bubbles), plus the same hover
    transition bookkeeping as `hover` so `mouseover`/`out` fire during the move.
  - `mouseUp(locator)` → `mouseup` at the node (bubbles).
  - `dragTo(from, to)` — the composite: `mouseDown(from)` → `moveTo(to)` →
    `mouseUp(to)`. This is the action the inventory E2E spec uses.
  Each enqueues through `PendingDomEvents` exactly like the existing actions, so
  auto-wait/settle semantics are unchanged. New `Command`/`PendingActionKind`
  variants are added alongside `Hover`/`Click`/`Press`/`Fill`.
- **Coordinates are placeholder.** `getBoundingClientRect` returns zeros today
  (ledger, T2 roadmap), so the test engine cannot compute a node's real screen
  position. Synthetic events therefore carry a fixed/zero `clientX/clientY`. This
  is why drop resolution in the example (and any drag spec) keys off `event.target`
  (element identity, which the locator controls), **not** coordinates. If/when
  post-layout read-back lands, `moveTo`/`dragTo` can be upgraded to real coords
  without changing specs.
- **ABI note.** These actions go through `PendingDomEvent` → `drain_dom_events_system`
  → the extended `dispatch_event`; the test engine does not call `dispatch_event`
  directly, so the ABI change doesn't ripple into it beyond constructing the
  payload.

## Testing

- **`superui_dom` / `dom.js`:** per-type listener transitions emit the extended
  `OP_SET_LISTENER`; adding a second listener of the same type on a node does not
  re-emit; `document`/`window` accept listeners and fire them on dispatch.
- **`superui_bridge` (new `tests/mouse_events.rs`, in the style of
  `tests/input_events.rs`):** drive `HoverMap` / `PointerLocation` /
  `ButtonInput<MouseButton>` and assert:
  - enter/exit produce `mouseover`/`mouseout` + `mouseenter`/`mouseleave` in the
    right order, with correct bubbling flags;
  - button edges produce `mousedown`/`mouseup` at the right node;
  - a `mousemove`/`mouseup` on an element **bubbles to a `document` listener with
    `event.target` = the element** (the capture-by-bubbling guarantee);
  - **the run condition keeps the system fully off with no mouse listeners**
    (assert no work / no events);
  - per-family gating: a `mousedown`-only app produces no `mousemove` and does no
    hover diffing;
  - mid-hover listener addition does not retro-fire `mouseover` (seeding rule).
- **`superui_bridge/tests/picking.rs`:** existing assertions still hold — a mouse
  listener makes a node `interactive`/blocking exactly like a click listener
  (regression guard on the `OP_SET_LISTENER` change).
- **`superui_js` dispatch tests:** the event object exposes
  `clientX`/`clientY`/`button`/`buttons` when a mouse payload is present; a bubbling
  event reaches a `document`/`window` listener (new bubble-chain reach), while a
  non-bubbling one (`mouseenter`) does not.
- **`superui_test_engine`:** `hover` emits the full out/leave → over/enter
  sequence; `dragTo(from, to)` produces `mousedown`→`mousemove`→`mouseup` with the
  drop resolvable from `event.target`. Cover in both driver paths.
- **Example-level:** an end-to-end spec that `dragTo`s a tile from chest to
  inventory and asserts the item moved between stores.

## Documentation updates (part of "done")

- **`website/src/docs/reference/js-dom.md`** (the ledger — authoritative surface):
  in the **Events** table, flip `mouseover`/`mouseout` 🟡→✅; add ✅ rows for
  `mouseenter`/`mouseleave`, `mousedown`/`mouseup`, `mousemove`; flip
  `event.clientX/Y` 🟡→✅ (leave `offsetX/Y` 🟡 — out of scope); in **Globals**,
  note on the `window` row that `document`/`window` accept mouse listeners and that
  `mousemove`/`mouseup` dispatch there while a button is held (the drag-capture
  contract). New/flipped rows get `main` in the **Since** column per the
  `documenting-new-features` skill (swapped to the version at release).
- **`CHANGELOG.md`** → `[Unreleased] → Added`: one entry covering the mouse-event
  family + document-level drag capture.
- **Examples gallery:** register `examples/inventory/` under `website/src/examples/`
  and add a card to `website/src/examples/README.md`, the way `game_menu`/`todomvc`
  are registered.
- **`website/src/docs/concepts/picking.md`:** a short subsection giving mouse
  events and the drag-capture rule a conceptual home (picking already keeps nodes
  hoverable; this is where that connects to JS events). Kept brief; the ledger is
  the detailed reference.
- **`plugins/bevy_superui/skills/supersolid/references/html-dom.md`** (the
  supersolid skill's capability reference — a tracked, condensed duplicate of the
  website ledger): mirror the Events-table changes (flip `mouseover`/`mouseout`,
  add the new rows, flip `event.clientX/Y`), and extend the JSX `on*` handler
  prose (currently lists `onClick`/`onInput`/`onChange`/`onKeyDown`/`onKeyUp`) to
  include `onMouseDown`/`onMouseUp`/`onMouseMove`/`onMouseOver`/`onMouseOut`/
  `onMouseEnter`/`onMouseLeave` and note the event now exposes `clientX`/`clientY`/
  `button`/`buttons`.
### Open doc-process note

The DOM/event capability surface is documented in **two** tracked ledgers that
duplicate and will drift: `website/src/docs/reference/js-dom.md` and
`plugins/bevy_superui/skills/supersolid/references/html-dom.md`. This spec updates
both by hand; collapsing the duplication is out of scope here but worth noting.

## Risks & notes

- **`OP_SET_LISTENER` change is the riskiest edit** because picking depends on it.
  Mitigation: the applier keeps the per-node has-any derivation intact, and the
  picking tests are the regression guard.
- **`mousemove` frequency** is the main perf concern; the `any_move()` gate plus
  the position-changed check keep it off entirely unless an app opts in, and even
  then it's at most one dispatch to the hovered node plus one to `document` per
  frame of motion.
- **Coordinate space:** `PointerLocation` is in logical window pixels, which is the
  correct meaning of `clientX/clientY`. If a UI is rendered to a non-fullscreen
  `UiTargetCamera` viewport, client coords remain window-relative (browser-faithful);
  revisit only if a concrete example needs viewport-relative coords.
- **Bubble-chain expansion is broader than mouse events.** Putting
  `document`/`window` atop the bubble chain (Component 4) means *every* bubbling
  event (`click`, `keydown`, …) can now reach a `document`/`window` listener. This
  is intended and browser-faithful, but it is a behavior change beyond the mouse
  scope. Mitigation: non-bubbling events are unaffected, and existing dispatch
  tests (which never add `document`/`window` listeners) pin that nothing fires
  that didn't before; a new test pins the added reach.
- **The test engine cannot supply real coordinates.** `getBoundingClientRect`
  returns zeros (no post-layout read-back), so synthetic drag events carry
  placeholder coords. Drag specs — and the inventory example — must therefore make
  drop decisions off `event.target`, not coordinates. Documented as a constraint,
  not a blocker; revisit if/when layout read-back ships.
- **Capture depends on a full-viewport hoverable root being in the `HoverMap`.**
  The drag-across-the-gap guarantee assumes the root node is reported as hovered
  whenever the cursor is inside the UI. Verify this holds for a `PassThrough`
  (transparent, non-blocking but hoverable) root during implementation; if the
  root is ever absent from the hover map, fall back to dispatching held-button
  moves directly at `document`.
