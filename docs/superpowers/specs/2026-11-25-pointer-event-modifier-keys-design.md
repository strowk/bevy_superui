# Pointer/touch events carry live keyboard modifiers (+ test-engine modifier & pointer input)

Date: 2026-11-25
Branch: `main`
Ticket: `bevy-superui_9`
Follows: `bevy-superui_8` (pointer events + capture for drag-and-drop) — the
regression this fixes was introduced there.

## Problem

Shift-click quick-move in `examples/inventory` (hold shift and click a tile to
send it straight to the other panel without dragging) stopped working after the
pointer-events migration (`bevy-superui_8`, commit `bbd47f6`).

The demo moved drag from `onMouseDown` to `onPointerDown`. `startDrag` branches
on the modifier:

```tsx
const startDrag = (e, id, from) => {
  if (e.shiftKey) { quickMove(id, from); return; }
  ...
```

The JS dispatch layer populates `e.shiftKey` from `init.modifiers` for any event
type (`crates/superui_js/js/dom.js:718`), so the event shape is correct. The
break is in the bridge: the pointer-event observers hardcode
`Modifiers::default()` (all-false) and never query the keyboard —

```
crates/superui_bridge/src/events.rs:460  (on_pointer_press)
  PendingDomEvent::new(node, "pointerdown").with_mouse(m).with_modifiers(Modifiers::default())
```

— with the same hardcoding in `on_pointer_release` (:477), `on_pointer_move`
(:493), `on_pointer_cancel` (:510), `on_pointer_over` (:417), and
`on_pointer_out` (:439). The mouse/click path does it correctly
(`events.rs:687`): `let mods = modifiers_now(&keys)`.

A second, independent site has the same defect: the touch→mouse-compatibility
synthesis (`enqueue_touch_press` / `enqueue_touch_release`, invoked by
`on_touch_press` / `on_touch_release`) fabricates `mousemove`/`mouseover`/
`mouseenter`/`mousedown`/`mouseup` from a touch and hardcodes
`Modifiers::default()`, with a comment asserting "touch has no keyboard-modifier
concept of its own" (`events.rs:161-162`, `:207-208`).

So every synthesized pointer event — and every touch-compat mouse event —
reports `shiftKey`/`ctrlKey`/`altKey`/`metaKey` as `false`, and `quickMove`
never fires.

The regression also exposed two gaps in the test engine that let it slip through
(see "Why existing tests missed this"): the test engine cannot hold a keyboard
modifier during synthesized input, and it has no pointer-event actions at all.
This spec closes the bridge bug and both test-engine gaps.

## Why this is wrong

In the DOM, `PointerEvent` extends `MouseEvent`, which carries the modifier
attributes:

```
Event → UIEvent → MouseEvent (shiftKey/ctrlKey/altKey/metaKey) → PointerEvent
```

A real browser's `pointerdown`/`pointermove`/`pointerup` carry the live modifier
state regardless of pointer type — a touchscreen device with an attached
keyboard reports `shiftKey: true` when shift is held during a touch. Reading
`e.shiftKey` off a `pointerdown` is spec-sanctioned. superui's pointer events
are currently the odd ones out; the hardcoded-false is the non-conformant
behavior, and the "touch has no modifier concept" comment is simply incorrect.

## Goal

1. Every synthesized DOM event that carries a `Modifiers` field sources it from
   the live keyboard via `modifiers_now(&keys)`, so pointer and touch-compat
   events report real modifier state, matching browser semantics.
2. The test engine can drive input with held keyboard modifiers (a per-action
   `modifiers` option), and can drive the pointer-event protocol
   (`pointerDown`/`pointerMove`/`pointerUp`) that the drag migration introduced.

The acceptance proof is shift-click quick-move working again in
`examples/inventory`, guarded at two levels: a bridge-level regression test on
the observer, and an app-level end-to-end test driven through the test engine's
new pointer+modifier input.

## Scope

- **Bridge** (`crates/superui_bridge/src/events.rs`): both modifier defect sites.
- **Test engine** (`crates/superui_test_engine`): a per-action `modifiers`
  option on the existing actions, and new `pointerDown`/`pointerMove`/
  `pointerUp` actions — across the JS prelude, the `Command` enum, and both
  driver implementations (`driver.rs` and the parallel in-world-stepper
  `ui_driver.rs`).

## Non-goals

- No change to `modifiers_now`, the `Modifiers` type, the JS dispatch layer
  (`dom.js` already reads `init.modifiers` correctly), or the `EventInit`
  plumbing.
- No change to `examples/inventory/assets/ui/inventory/app.tsx`. The demo is
  already correct; it starts working again the moment the bridge delivers real
  modifiers.
- No new abstraction for sourcing modifiers in the bridge (no per-frame
  `CurrentModifiers` resource, no folding the keyboard read into the
  payload/event builders). The inline `modifiers_now(&keys)` idiom already
  exists on the mouse path and is the pattern to follow.
- No `page.keyboard.down/up` persistent held-modifier state. The per-action
  `modifiers` option covers "hold a modifier during this click/drag"; a global
  keyboard state is a larger, separate feature.
- No coordinate-based pointer-drag drop resolution *through the test engine*.
  A full drag resolves the drop via `document.elementFromPoint(x, y)`, which
  headlessly needs a seeded hit-rect snapshot; that path stays covered by the
  existing Rust `dispatch_dom_event` tests in `examples/inventory/tests/`. The
  new pointer actions carry coordinates but their own coverage uses
  position-independent flows (see Testing).

## Approach

### Part 1 — Bridge: sample the live keyboard

Inline `modifiers_now(&keys)` at each observer, mirroring the existing mouse
path. Each affected observer already takes several `Res`/`Query` params; adding
`keys: Res<ButtonInput<KeyCode>>` is consistent with the mouse/click systems
that already depend on it. All changes are in
`crates/superui_bridge/src/events.rs`.

- **Six pointer observers** — `on_pointer_press`, `on_pointer_release`,
  `on_pointer_move`, `on_pointer_cancel`, `on_pointer_over`, `on_pointer_out`:
  add `keys: Res<ButtonInput<KeyCode>>`; replace `Modifiers::default()` with
  `modifiers_now(&keys)`. These fire for every pointer type, so this also makes
  touch-*pointer* events carry live keyboard state — browser-correct, no
  per-type special-casing.
- **Two touch-compat observers** — `on_touch_press`, `on_touch_release`: add
  `keys: Res<ButtonInput<KeyCode>>`; compute `let mods = modifiers_now(&keys)`
  and pass it into the helpers.
- **Two helpers** — `enqueue_touch_press`, `enqueue_touch_release`: take a
  `mods: Modifiers` parameter instead of constructing `Modifiers::default()`,
  and apply it to every event emitted (the idle `mousemove`/`mouseover`/
  `mouseenter` preamble as well as `mousedown`/`mouseup`), so the whole
  synthesized sequence is consistent.
- **Comment fix** — rewrite the `enqueue_touch_press`/`enqueue_touch_release`
  doc comments (lines 161-162, 207-208) that claim "touch has no
  keyboard-modifier concept of its own" to state that the sequence carries the
  live keyboard modifiers.

### Part 2 — Test engine: per-action modifier option

A per-action `{ modifiers: [...] }` option, using Playwright's modifier names
(`'Shift'`, `'Control'`, `'Alt'`, `'Meta'`), threaded through every layer:

- **`prelude.js`** (`src/prelude.js:104-114`): `click`, `mouseDown`, `mouseUp`,
  `hover`, `moveTo`, and `dragTo` accept a trailing options object and serialize
  a `modifiers` array into the enqueued command.
- **`command.rs`**: those `Command` variants gain
  `#[serde(default)] modifiers: Vec<String>`.
- **Drivers** (`driver.rs` and `ui_driver.rs`): a shared
  `parse_modifiers(&[String]) -> Modifiers` helper maps the Playwright names to
  the `Modifiers` struct (unknown names ignored). `PendingActionKind` carries
  the parsed `Modifiers`. The effect functions take and apply it:
  `push_mouse_event` gains `.with_modifiers(mods)`; the `click` path passes the
  action's `mods` into `click_effect` instead of `Modifiers::default()`; `hover`
  applies it to its enter/leave events. Both driver implementations change
  identically (they are near-verbatim duplicates).

### Part 3 — Test engine: pointer-event actions

New `pointerDown` / `pointerMove` / `pointerUp` locator methods, each taking
`{ modifiers, pointerId, pointerType, x, y }` with defaults `pointerId: 1`,
`pointerType: 'mouse'`, `x/y: 0`:

- **`prelude.js`** + **`command.rs`**: three new commands carrying those fields.
- **Drivers (both)**: new `PendingActionKind`s and an effect fn that pushes a
  `PendingDomEvent` for `pointerdown`/`pointermove`/`pointerup` with a pointer
  `MousePayload` (`pointer_id`, `pointer_type`, `is_primary`, `buttons` = 1 on
  down / 0 on up), the parsed `modifiers`, and the `x`/`y` coordinates —
  mirroring how `examples/inventory/tests/inventory.rs` hand-builds these
  payloads today.
- **Capture needs no new code.** `setPointerCapture` retargeting already lives
  in the JS dispatch layer (`captureMap` in `dom.js`). The actions only need to
  fire with a consistent `pointerId`; the JS layer retargets subsequent
  `pointermove`/`pointerup` to the capturing node. The test engine implements no
  capture logic of its own.

## Data flow

```
# Bridge (Part 1)
ButtonInput<KeyCode> → modifiers_now(&keys) inside each observer, when it fires
  → PendingDomEvent::with_modifiers(mods) → EventInit.modifiers
  → JS dispatch() sets event.shiftKey/… (dom.js:718) → app.tsx reads e.shiftKey

# Test engine (Parts 2-3)
spec: locator.click({modifiers:['Shift']}) / locator.pointerDown({modifiers:['Shift']})
  → prelude.js enqueues {type, locator, modifiers, …}
  → Command deserialize → parse_modifiers → Modifiers
  → effect fn pushes PendingDomEvent::with_modifiers(mods) (+ pointer payload)
  → JS dispatch() sets event.shiftKey/… → app handler reads it
```

## Why existing tests missed this

Two reasons, one trivial and one structural — and this spec addresses the
structural one in Parts 2-3 rather than leaving it.

1. **No coverage existed.** `examples/inventory/tests/inventory.rs` has three
   tests — mouse drag, touch drag, same-panel no-op drop. None hold a modifier
   or exercise the `quickMove` branch.

2. **The test layers sat *below* the bug, and couldn't reach it.** The defect is
   in the bridge observer `on_pointer_press`, the Bevy-picking → DOM translation
   step that samples the keyboard. Both ways of driving input bypassed it:

   - The inventory e2e tests call `UiRuntime::dispatch_dom_event(...,
     modifiers: None, ...)` directly, hand-building the `pointerdown` — the
     observer never runs.
   - The test engine's human-input simulation (`page.locator().click()` /
     `.dragTo()`) builds DOM events directly (`driver.rs` / `ui_driver.rs` call
     `click_effect(&rt, node, Modifiers::default(), ...)`), hardcoding
     `Modifiers::default()` with no API to hold a modifier, and additionally
     fires only *mouse* events — it has no `pointerdown` action, which is why
     `inventory.rs` had to drop to `dispatch_dom_event` and why the old
     `dragTo`-based tests were removed.

   So the test engine imitates human input at the DOM-event layer, below the
   bridge observers that read `ButtonInput<KeyCode>`. Bugs in how Bevy input
   becomes DOM events are invisible to it by construction.

This is why coverage lands at two levels: the **bridge-level** regression test
(a real `PointerPress` observer driven with a real `ButtonInput<KeyCode>`) is
the only thing that exercises the code this fixes; the **test-engine** pointer +
modifier work (Parts 2-3) then makes an app-level inventory shift-click test
expressible for the first time, so the demo's own behavior is guarded too.

## Testing

1. **Bridge (guards the fix).** In `crates/superui_bridge/tests/`: hold
   `KeyCode::ShiftLeft` in `ButtonInput<KeyCode>`, drive a `PointerPress`, assert
   the enqueued `pointerdown` carries `shift: true`; with no key held, assert
   `false`. Mirror for a primary-touch `PointerPress` asserting the synthesized
   `mousedown` carries `shift: true`.
2. **Modifier API (Part 2).** A test-engine spec against a fixture that records
   `event.shiftKey`: `click({ modifiers: ['Shift'] })` asserts it arrives
   `true`, a plain `click()` asserts `false`.
3. **Pointer actions (Part 3).** A test-engine spec against a fixture recording
   `pointerdown`/`pointermove`/`pointerup`: a `pointerDown`/`pointerMove`/
   `pointerUp` sequence asserts the events fire in order, and
   `pointerDown({ modifiers: ['Shift'] })` asserts `shiftKey: true`. Position-
   independent (single element), so no hit-rect seeding needed.
4. **Inventory shift-click e2e (the payoff).** A test-engine spec that mounts
   the real inventory assets, fires `pointerDown({ modifiers: ['Shift'] })` on
   `#tile-sword`, and asserts the sword is now under `#inv` (e.g.
   `expect(page.locator('#inv #tile-sword')).toHaveCount(1)`). No coordinates or
   capture needed — `quickMove` fires immediately on `pointerdown`.

Existing coverage that must stay green: the `modifiers_now` unit test, the
pointer/touch/mouse event tests in `superui_bridge/tests/`, and the existing
test-engine and inventory tests (the new `modifiers` option and pointer actions
are additive — existing calls pass no modifiers and behave as before).

## Acceptance

- Shift-click a tile in `examples/inventory` → it moves to the other panel
  immediately, on both mouse and touch-with-keyboard.
- Plain click/drag is unaffected (no modifier held → `shiftKey: false` → drag
  path).
- The test engine can express `click({ modifiers: [...] })` and
  `pointerDown`/`pointerMove`/`pointerUp`, with modifiers flowing through.
- All new tests pass; all existing bridge, test-engine, and inventory tests stay
  green.
