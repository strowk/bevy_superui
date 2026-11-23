# Pointer events + capture for mouse/touch drag-and-drop

Date: 2026-11-23
Branch: `main`
Ticket: `bevy-superui_8`
Depends on: `bevy-superui_7` (stage-1 touch tap mouse-event synthesis; shares `PrimaryTouch`)

## Problem

Drag-and-drop in superui is mouse-only. `examples/inventory` wires `startDrag`
to `onMouseDown` and tracks the drag with `document`-level `mousemove`/`mouseup`,
resolving the drop slot from `event.target`. Mouse events come only from
`mouse_events_system` (`crates/superui_bridge/src/mouse.rs`), which reads the
mouse pointer alone, so none of this reacts to touch. Stage 1
(`bevy-superui_7`) makes a touch *tap* fire compatibility mouse events, but a
drag needs a continuous move stream and a robust capture mechanism that a tap
seam does not provide.

The web solves this with Pointer Events: `pointerdown`/`pointermove`/
`pointerup`/`pointercancel` unify mouse, touch, and pen behind one `PointerEvent`
interface, and `element.setPointerCapture(pointerId)` redirects all subsequent
events for a pointer to the capturing element until release — so a drag keeps
tracking even when the pointer leaves the element or the surface entirely.
superui implements none of this.

## Goal

Add the `PointerEvent` family so one code path drives drag-and-drop on both
mouse and touch, using `setPointerCapture` as a browser does. The acceptance
proof is `examples/inventory` migrated to pointer events and working with both
a mouse and a touchscreen.

Pointer events are **additive**: a browser dispatches both pointer events and
(compatibility) mouse events, and superui keeps doing the same. The existing
`mouse_events_system` and the stage-1 tap synthesis are untouched; the pointer
family is a parallel stream.

## Event types in scope

`pointerdown`, `pointermove`, `pointerup`, `pointercancel`, plus the hover
family `pointerover`, `pointerout`, `pointerenter`, `pointerleave`, for both
`pointerType: "mouse"` and `pointerType: "touch"`. Capture adds
`gotpointercapture` / `lostpointercapture`.

## Design

### 1. JS-facing surface (`crates/superui_js`: `js/dom.js` + `src/lib.rs`)

- **PointerEvent shape.** `dispatch` builds an event object that inherits
  `clientX`/`clientY`/`button`/`buttons`/`shiftKey`/… and adds `pointerId`,
  `pointerType` (`"mouse"` | `"touch"`), and `isPrimary`, plus sane constant
  defaults `width = height = 1`, `pressure = 0.5` on down / `0` otherwise. The
  Rust-side carriers (`MousePayload` / `EventInit`) gain `pointer_id`,
  `pointer_type`, and `is_primary`.
- **Capture registry.** `element.setPointerCapture(id)` and
  `releasePointerCapture(id)` maintain a JS map `{pointerId -> node}`. In
  `dispatch`, while a pointer is captured, pointer-type events **retarget** to
  the capturing node (W3C capture semantics) before the capture/target/bubble
  walk; `clientX`/`clientY` remain the geometric pointer coordinates.
  `pointerup` implicitly releases the capture. `gotpointercapture` fires on set,
  `lostpointercapture` on release (explicit or implicit-on-up).
- **`document.elementFromPoint(x, y)`.** Returns the topmost element at a screen
  point, or `null`. This is what the migrated example uses to find the drop
  target, because capture makes `pointerup.target` the dragged element rather
  than the element underneath.

### 2. Two host-backed pieces (JS alone cannot answer these)

- **`elementFromPoint` needs real geometry.** The JS shadow DOM has no layout.
  So `document.elementFromPoint` calls a host binding,
  `__ss_element_from_point(x, y)`, answered Rust-side by an axis-aligned
  bounding-box hit-test over mounted DOM-node entities (bevy_ui `ComputedNode`
  size + `GlobalTransform` position), returning the deepest node whose rect
  contains the point (children on top) as a `JsNodeId`, or null when the point
  is over no UI. Z-ordering is approximated by tree depth; exact stacking-context
  resolution is out of scope.
- **Capture must survive leaving the UI.** bevy_picking emits no `Move`/`Released`
  when the pointer is over no UI entity, so a captured drag dragged off the UI
  would go silent (the edge gap noted in `mouse.rs`). `setPointerCapture` syncs
  a Rust-side capture map (JS -> bridge via an op / the outbox). While a pointer
  is captured, a dedicated system reads the **raw** pointer location
  (`PointerLocation` for mouse, `Touches` for touch) each frame and delivers
  `pointermove`/`pointerup` to the captured node even with no pick, using raw
  window coordinates for `clientX`/`clientY`.

### 3. Pointer-event synthesis (`crates/superui_bridge`)

- **Uncaptured family from picking observers.** Source the normal stream from
  bevy_picking's per-pointer observer events — `Pointer<Over>`, `Pointer<Out>`,
  `Pointer<Pressed>`, `Pointer<Released>`, `Pointer<Move>`, `Pointer<Cancel>` —
  mapped to `pointerover`/`pointerout`/`pointerenter`/`pointerleave`/
  `pointerdown`/`pointerup`/`pointermove`/`pointercancel` via `resolve_dom_node`,
  for **all** pointer types (no mouse filter). `pointerType` and `isPrimary`
  derive from the event's `pointer_id`, reusing stage-1's `PrimaryTouch` for the
  touch primary; the mouse pointer is always primary.
- **Captured delivery** is handled by the raw-location system in §2.
- All pointer events flow through the existing `PendingDomEvents` FIFO, so W3C
  ordering is preserved relative to each other and to the compatibility mouse
  events.
- `mouse_events_system` is not modified; mouse events continue to originate
  there.

### 4. Registration (`crates/superui/src/mount.rs`)

Register the new picking observers and systems alongside the existing input
wiring, and initialize the capture-map resource.

### 5. Example migration (`examples/inventory`)

Rewrite the drag-and-drop to one pointer path that works on mouse and touch:

- `onMouseDown` -> `onPointerDown`, which calls
  `e.target.setPointerCapture(e.pointerId)` and records the drag.
- `document` `mousemove`/`mouseup` listeners -> `pointermove`/`pointerup`.
- The drop slot is resolved from `document.elementFromPoint(e.clientX,
  e.clientY)` fed into the existing `findSlot` ancestor walk (capture makes
  `event.target` the dragged tile, so the geometric lookup replaces reading
  `event.target`).
- Update the file header comment and the floating-tile `pointer-events` note to
  describe the pointer-capture protocol.

## Testing

Headless `crates/superui_bridge/tests/` modeled on `input_events.rs`:

- `pointerdown`/`move`/`up`/`cancel` and `pointerover`/`out` for both mouse and
  touch, asserting `pointerType`, `isPrimary`, and payload fields.
- Secondary-touch suppression of `isPrimary`.
- Capture: events retarget to the capturing node while captured; off-UI
  delivery via the raw-location path; implicit release + `lostpointercapture`
  on `pointerup`.
- `elementFromPoint` hit-test unit tests: point inside a node, inside
  overlapping nodes (topmost wins), and outside all nodes (null).
- An `examples/inventory` integration test exercising a touch-driven drag from
  one panel to the other end to end.

## Non-goals

- Pen pressure/tilt beyond the constant defaults; `getCoalescedEvents` /
  predicted events.
- `touch-action` CSS and gesture disambiguation.
- Pointer events for scroll/wheel.
- Simultaneous multi-pointer drags: a single primary capture at a time.

## Scope note

This is one cohesive feature but a large one. It is written as a single spec;
the implementation plan is expected to stage it — the `PointerEvent` family and
synthesis first, then capture + `elementFromPoint`, then the example migration.
