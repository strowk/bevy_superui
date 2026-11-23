# Synthesize mouse events on taps from touch devices

Date: 2026-11-23
Branch: `main`
Ticket: `bevy-superui_7`

## Problem

Mouse-driven UIs do nothing on touch devices. `examples/inventory`'s
drag-and-drop is the worked case: it wires `startDrag` to `onMouseDown` and
installs `document`-level `mousemove`/`mouseup` listeners, so a finger produces
no interaction at all.

The mouse-event seam, `mouse_events_system`
(`crates/superui_bridge/src/mouse.rs`), reads only `HoverMap[PointerId::Mouse]`,
the mouse pointer's `PointerLocation`, and `ButtonInput<MouseButton>`. Touch
input flows through a different channel (`Touches` / `PointerId::Touch`) and is
never consulted, and nothing anywhere synthesizes mouse events from touch. So
`mousedown`/`mouseup`/`mousemove`/`mouseover`/`mouseenter` never fire for a tap.

This matches how real browsers behave for *drag* gestures (a pan suppresses the
compatibility mouse events), but superui is further behind: it emits nothing
mouse-shaped from touch even for a plain tap.

What already works, and is not re-touched: `click` + focus already fire on a
touch tap. `on_pointer_click` (`crates/superui_bridge/src/events.rs`) observes
`bevy_picking`'s pointer-agnostic `PointerClick`, which picking emits for a
press-then-release of any pointer, mouse or touch.

## Goal

On a tap from the **primary** touch, synthesize the browser compatibility
mouse-event sequence so mouse-based handlers and `:hover`/`onMouseOver` styling
respond to touch:

```
touchstart -> mousemove
           -> mouseover  (bubbles)
           -> mouseenter (no bubble)
           -> mousedown  (button 0, buttons 1)
touchend   -> mouseup    (button 0, buttons 0)
           -> click + focus   (already emitted by the existing PointerClick observer)
```

`clientX`/`clientY` come from the touch's pick location; modifiers are empty
(touch carries no keyboard modifiers). The `PendingDomEvents` FIFO drain
preserves W3C order: `mousedown` lands on the `touchstart` frame, `mouseup` +
`click` on the `touchend` frame, so JS observes `mousedown ... mouseup, click`.

"Primary touch only" is the browser `isPrimary` rule: compatibility mouse
events are generated solely from the first/primary active touch. Secondary
fingers emit no mouse events.

## Non-goals (later stages)

- **`mousemove` tracking during a drag/pan.** This stage covers taps
  (press+release resolving to a node), not continuous move. The inventory
  drag-and-drop needs move synthesis and is a subsequent stage.
- **`mouseout`/`mouseleave` as the virtual cursor migrates between taps.** A
  real browser fires `mouseout`/`mouseleave` on the previously-tapped element
  when a later tap lands elsewhere; tracking that across taps is deferred with
  the move stage. This stage emits per-tap `mouseover`/`mouseenter` only.
- **`touchcancel` handling, multi-touch mouse events, real Pointer Events.**

## Approach

Mirror the existing click seam and leave `mouse_events_system` untouched.

### Observers (`crates/superui_bridge/src/events.rs`)

Two global observers on `bevy_picking`'s press/release events
(`Pointer<Pressed>` / `Pointer<Released>` — exact type-alias spelling confirmed
against this Bevy version during implementation), each **filtered to
`PointerId::Touch(_)`** so the mouse path is never doubled with a second
`mousedown`/`mouseup`.

- **Primary `Pressed`:** `resolve_dom_node(ev.entity, &nodes, &parents)` to the
  nearest `DomNode` ancestor (same resolution as `on_pointer_click`), then
  enqueue, in order, `mousemove`, `mouseover` (bubbles), `mouseenter` (no
  bubble), `mousedown` (`button` 0, `buttons` 1) into `PendingDomEvents`, each
  carrying the touch location as `clientX`/`clientY`.
- **Primary `Released`:** enqueue `mouseup` (`button` 0, `buttons` 0) at the
  resolved node.

When `resolve_dom_node` returns `None` (the pick belongs to no mounted UI --
a host game's own entity), the observer does nothing, exactly like
`on_pointer_click`.

### Primary-touch tracking

A small `Resource`, `PrimaryTouch(Option<PointerId>)`, records the primary
finger:

- On a touch `Pressed` while `PrimaryTouch` is `None`: adopt this pointer as
  primary and synthesize its press sequence.
- On a touch `Pressed` while a primary is already active: ignore (secondary
  finger).
- On the primary's `Released`: synthesize `mouseup`, then clear `PrimaryTouch`
  to `None`.
- A non-primary `Released` is ignored.

### Registration (`crates/superui/src/mount.rs`)

Register the two observers alongside the existing `on_pointer_click` /
`on_slider_value_change` / focus observers, and initialize the `PrimaryTouch`
resource.

## Testing

TDD, headless, modeled on `crates/superui_bridge/tests/input_events.rs` and
`mouse_events.rs`. Construct the picking press/release events with
`PointerId::Touch` and assert against the drained `PendingDomEvents`:

- A primary tap enqueues exactly `mousemove`, `mouseover`, `mouseenter`,
  `mousedown` on press and `mouseup` on release, at the resolved node, with the
  expected `button`/`buttons`/`clientX`/`clientY`/bubbles flags.
- A second concurrent touch (while a primary is active) enqueues nothing.
- After the primary releases, a following touch becomes the new primary and
  synthesizes again.
- A mouse `Pressed`/`Released` enqueues nothing from the new observers (no
  double `mousedown`/`mouseup`; the mouse path remains the sole mouse source).

## Files

- `crates/superui_bridge/src/events.rs` -- the two observers, the
  `PrimaryTouch` resource, and a shared helper that enqueues the press/release
  sequences.
- `crates/superui/src/mount.rs` -- register the observers and init the
  resource.
- `crates/superui_bridge/tests/` -- a new test module for the touch-tap
  synthesis.
