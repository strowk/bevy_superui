# Keyboard modifier properties on DOM events

Date: 2026-11-18
Branch: `shot/mouse-events-and-dragging`

## Problem

Author JS cannot read modifier-key state on a superui event. The web carries
`shiftKey`, `ctrlKey`, `altKey`, and `metaKey` on every `MouseEvent` and
`KeyboardEvent`, so a shift-click handler reads `e.shiftKey` directly:

```js
button.addEventListener("click", (e) => transfer(e.shiftKey ? 100 : 1));
```

superui dispatches these events — `dom.js`'s `dispatch` builds the event object
— but never sets the four properties, so the reading is always `undefined`.
There is no way to branch on Shift/Ctrl/Alt/Meta from a handler.

## Goal

Set `shiftKey`/`ctrlKey`/`altKey`/`metaKey` on the event object for every
mouse event (`click`, `mousedown`, `mouseup`, `mousemove`, `mouseover`,
`mouseout`, `mouseenter`, `mouseleave`) and every keyboard event (`keydown`,
`keyup`), sourced from live Bevy key state at the moment the event is enqueued.

This is the modifier-on-event capability. It makes shift-click, ctrl/cmd-click,
and Shift+Enter work, which is the common need. `document` and `window` are
already event targets at the top of the bubble chain (`dom.js`), so these
modifiers reach a `document`/`window` listener for any bubbling event — and
because mouse events dispatch from picking regardless of focus, a global
`document.addEventListener("mousedown", …)` can read `e.shiftKey` too.

## Non-goals

- **Making a bare modifier `keydown` reach a global listener while nothing is
  focused.** Keyboard events dispatch only to the focused node
  (`keyboard_events_system` gates on `rt.focused`) and `key_name` does not map
  modifier keys, so `e.key === "Shift"` never holds. A `shiftHeld` flag tracked
  via a global `keydown`/`keyup` listener therefore still will not work. Fixing
  that needs unfocused keyboard dispatch plus modifier-key naming — a separate
  change. This feature only adds the `*Key` booleans to events that already
  dispatch.
- **`getModifierState()`** and other `UIEvent` methods.
- **Modifiers on non-input events** (`change`, `input`, `focus`, `blur`). In the
  browser these are not `MouseEvent`/`KeyboardEvent` and have no modifier
  properties; they stay `undefined` here too.

## Design

### Data types (`superui_js`)

`superui_js` is engine-agnostic and wasm-clean, so these carry no Bevy types.

```rust
/// W3C modifier-key state (`MouseEvent`/`KeyboardEvent` `shiftKey` etc.).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub meta: bool,
}

/// Extra event-object data carried through `JsEngine::dispatch_event`:
/// pointer geometry for mouse events, modifier state for mouse and keyboard
/// events. Both `None` for events that carry neither (focus/change/input).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EventInit {
    pub mouse: Option<MousePayload>,
    pub modifiers: Option<Modifiers>,
}
```

`EventInit` replaces the current `mouse: Option<MousePayload>` parameter of
`JsEngine::dispatch_event` (the engine serializes it into the single init object
that `__ss_dispatch` receives last). Bundling the two independent pieces of
event data keeps that JS ABI at its current six arguments rather than growing a
seventh.

### Dispatch ABI (`__ss_dispatch` / `dom.js`)

The sixth argument to `__ss_dispatch(jsId, type, key, bubbles, cancelable, init)`
is the init object. Pointer fields stay flat on it (unchanged); modifiers ride
in a nested `modifiers` sub-object:

```js
init = {
  clientX, clientY, button, buttons,   // present only for mouse events
  modifiers: { shiftKey, ctrlKey, altKey, metaKey },  // present for mouse+keyboard
}
```

`dispatch()` sets the event's four `*Key` properties from `init.modifiers` when
present, leaving them `undefined` when absent. Keeping pointer fields flat means
the existing `clientX`/`clientY`/`button`/`buttons` reads and their tests are
untouched; a keyboard event passes an init whose only key is `modifiers`, so its
`clientX` reads back `undefined` as before.

A missing sixth argument (the existing five-argument `__ss_dispatch` test calls)
yields all four `*Key` as `undefined`.

### Engine serialization

`V8Engine` and `WebEngine` each build the init object from `EventInit`:

- Both fields `None` -> pass JS `null` (focus/change/input).
- `mouse` `Some` -> set the four flat pointer keys (as today).
- `modifiers` `Some` -> set a nested `modifiers` object with the four `*Key`
  booleans.

### Bridge plumbing (`superui_bridge`)

- `PendingDomEvent` gains `modifiers: Option<Modifiers>` and a `with_modifiers`
  builder, alongside the existing `mouse`/`with_mouse`.
- `UiRuntime::dispatch_dom_event` and the `JsEngine::dispatch_event` trait take
  `init: EventInit` in the slot currently holding `mouse`.
- `drain_dom_events_system` builds `EventInit { mouse: e.mouse, modifiers:
  e.modifiers }` per queued event.

### Capturing modifier state

A helper reads live key state into `Modifiers`:

```rust
pub fn modifiers_now(keys: &ButtonInput<KeyCode>) -> Modifiers {
    Modifiers {
        shift: keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight),
        ctrl:  keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight),
        alt:   keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight),
        meta:  keys.pressed(KeyCode::SuperLeft) || keys.pressed(KeyCode::SuperRight),
    }
}
```

Bevy names the meta/command key `Super`. The other three variant names are
stable across input-crate versions; `Super` is the one to confirm against the
pinned Bevy.

Modifier state is snapshotted **when the event is enqueued**, not when it is
drained. The drain runs a frame later, by which point a key may have released;
capturing at enqueue records the state that was live when the input happened.

Enqueue sites that attach modifiers:

- `mouse_events_system` reads `Res<ButtonInput<KeyCode>>`, computes `Modifiers`
  once per frame, and attaches it to every mouse `PendingDomEvent` (which
  already carry a `MousePayload`).
- `keyboard_events_system` reads `Res<ButtonInput<KeyCode>>` and attaches
  modifiers to the `keydown`/`keyup` it dispatches and to the synthetic
  Enter/Space activation `click` (a keyboard-activated click carries modifier
  state in the browser). The checkbox `change` it emits carries none.
- The pointer-click path (`on_pointer_click` -> `focus_and_click` ->
  `click_effect`, and `apply_pointer_click`) threads a `Modifiers` so the
  `click` event carries it. The checkbox `change` enqueued alongside carries
  none.

Known limitation: `keyboard_events_system` processes a frame's buffered
`KeyboardInput` messages against the end-of-frame aggregate key state, so a
modifier pressed and released within a single frame is attributed to every key
message that frame. This is acceptable — held modifiers span frames.

## Affected code

| File | Change |
| --- | --- |
| `superui_js/src/lib.rs` | add `Modifiers`, `EventInit`; `dispatch_event` takes `init: EventInit` |
| `superui_js/src/engine_v8.rs` | serialize `EventInit` (flat pointer + nested modifiers) |
| `superui_js/src/engine_web.rs` | same via `Reflect` |
| `superui_js/js/dom.js` | `dispatch()` sets `*Key` from `init.modifiers` |
| `superui_bridge/src/events.rs` | `PendingDomEvent.modifiers`, `with_modifiers`, `modifiers_now`; click path + keyboard modifiers; drain builds `EventInit` |
| `superui_bridge/src/mouse.rs` | `mouse_events_system` attaches modifiers |
| `superui_bridge/src/runtime.rs` | `dispatch_dom_event` takes `EventInit` |
| `superui_bridge/src/lib.rs` | re-export `Modifiers`, `EventInit` |

Callers updated for the signature change (all pass empty/forwarded init):
`supersolid_runtime/src/lib.rs` (synthetic click), `superui_test_engine`
(`driver.rs`, `ui_driver.rs` route clicks through `click_effect`), and tests
`frame_loop.rs`, `supersolid_render.rs`, `v8_engine.rs`. The existing
`dispatch.rs` six-argument pointer test keeps its flat shape; one new case adds
`modifiers`.

## Testing

- **`modifiers_now`** unit test: each left/right key sets its modifier; none ->
  all false.
- **`dom.js` dispatch** (`dispatch.rs`): an init with `modifiers` sets the four
  `*Key`; an init without it (and a missing sixth arg) leaves them `undefined`.
- **Integration** (`input_events.rs` pattern, V8): a `PendingDomEvent` with
  `with_modifiers(...)` reaches a listener's `e.shiftKey`/`e.metaKey`.
- **Fidelity**: a non-input event (`focus`) has `e.shiftKey === undefined`.
- **System capture**: with `ShiftLeft` pressed in `ButtonInput<KeyCode>`,
  `mouse_events_system` and `keyboard_events_system` produce events whose
  listener sees `e.shiftKey === true`.
