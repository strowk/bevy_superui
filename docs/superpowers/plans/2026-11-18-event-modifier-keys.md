# Event modifier-key properties Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Set `shiftKey`/`ctrlKey`/`altKey`/`metaKey` on every mouse and keyboard event dispatched into author JS, sourced from live Bevy key state.

**Architecture:** A bevy-free `Modifiers` struct and an `EventInit { mouse, modifiers }` bundle replace the current `mouse: Option<MousePayload>` argument of the JS-dispatch seam. Engines serialize the bundle into one init object (flat pointer fields + a nested `modifiers` sub-object); `dom.js` sets the four `*Key` properties from it. The bridge snapshots modifier state from `ButtonInput<KeyCode>` at the moment each event is enqueued and attaches it to mouse, keyboard, and click events.

**Tech Stack:** Rust (Bevy 0.20, `superui_bridge`/`superui_js`/`supersolid_runtime`), deno_core V8 + wasm `WebEngine`, framework-free ES in `js/dom.js`, supersolid TSX for the example.

**Spec:** `docs/superpowers/specs/2026-11-18-event-modifier-keys-design.md`

## Global Constraints

- `superui_js` is engine-agnostic and wasm-clean: `Modifiers` and `EventInit` carry no Bevy types.
- Meta/command key maps to Bevy `KeyCode::SuperLeft`/`SuperRight`. Confirm those variant names compile against the pinned Bevy; the other three modifier pairs are stable.
- This branch (`shot/mouse-events-and-dragging`) is under active development and these files move. Before editing any dispatch seam, re-read the current signatures of `JsEngine::dispatch_event` (`superui_js/src/lib.rs`), `UiRuntime::dispatch_dom_event` (`superui_bridge/src/runtime.rs`), and `dispatch` (`superui_js/js/dom.js`) — the line numbers below are a snapshot.
- Commit messages: summary says what was done; body says why (project `.claude/CLAUDE.md`).
- Non-input events (`change`, `input`, `focus`, `blur`) carry no modifiers — their `*Key` stay `undefined`.

## Review Focus

- **Plain click, no modifiers held:** a mouse event always carries a `modifiers` object, so `e.shiftKey` must read `false` (not `undefined`). Pinned in Task 3's mouse-capture test.
- **Non-input event:** `focus`/`change` dispatched with no init -> `e.shiftKey === undefined`. Pinned in Task 1's dispatch test (null init) and Task 2's drain (both fields `None`).
- **Right-hand modifier keys:** `ShiftRight`/`ControlRight`/`AltRight`/`SuperRight` set their modifier. Pinned in Task 3's `modifiers_now` unit test.
- **Meta via `Super`:** `SuperLeft`/`SuperRight` -> `metaKey`. Pinned in Task 3's `modifiers_now` unit test.
- **Backward-compatible pointer ABI:** the existing six-argument `__ss_dispatch` pointer calls and the flat `clientX`/`clientY`/`button`/`buttons` reads keep working. Pinned by Task 1 leaving `mouse_event_exposes_coordinates` green.

---

### Task 1: `dom.js` sets `*Key` from `init.modifiers`

JS-only; no Rust signature changes, so the workspace stays green. The `dispatch` sixth argument is renamed `init`; pointer reads are unchanged, four `*Key` reads are added from a nested `modifiers` sub-object.

**Files:**
- Modify: `crates/superui_js/js/dom.js` (`dispatch`, ~622-690)
- Test: `crates/superui_js/tests/dispatch.rs`

**Interfaces:**
- Consumes: `__ss_dispatch(jsId, type, key, bubbles, cancelable, init)` where `init` is `null`, `{clientX, clientY, button, buttons}`, `{modifiers: {shiftKey, ctrlKey, altKey, metaKey}}`, or both merged.
- Produces: event object with `shiftKey`/`ctrlKey`/`altKey`/`metaKey` set from `init.modifiers` when present, `undefined` otherwise.

- [ ] **Step 1: Write the failing test**

Add to `crates/superui_js/tests/dispatch.rs`:

```rust
#[test]
fn event_exposes_modifier_keys() {
    let mut ctx = ctx();
    eval(
        &mut ctx,
        r#"
        globalThis.a = document.createElement('div');
        __ss_root.appendChild(a);
        globalThis.seen = {};
        a.addEventListener('mousedown', function (e) {
            seen.shift = e.shiftKey;
            seen.ctrl = e.ctrlKey;
            seen.alt = e.altKey;
            seen.meta = e.metaKey;
        });
        a.addEventListener('click', function (e) { seen.clickShift = e.shiftKey; });
        "#,
    );
    let _ = eval_bool(
        &mut ctx,
        "__ss_dispatch(a._nid, 'mousedown', null, true, true, \
         {modifiers: {shiftKey: true, ctrlKey: false, altKey: false, metaKey: true}})",
    );
    assert!(eval_bool(&mut ctx, "seen.shift === true"));
    assert!(eval_bool(&mut ctx, "seen.ctrl === false"));
    assert!(eval_bool(&mut ctx, "seen.meta === true"));
    // No modifiers on the init -> the property is undefined, not false.
    let _ = eval_bool(&mut ctx, "__ss_dispatch(a._nid, 'click', null, true, true, null)");
    assert!(eval_bool(&mut ctx, "seen.clickShift === undefined"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p superui_js --features engine-v8 --test dispatch event_exposes_modifier_keys`
Expected: FAIL — `seen.shift === true` is false because `e.shiftKey` is `undefined`.

- [ ] **Step 3: Implement**

In `crates/superui_js/js/dom.js`, rename the `dispatch` sixth parameter `mouse` to `init`, add a `mods` lookup, update the four pointer reads to read from `init`, and add the four modifier reads:

```js
  function dispatch(jsId, type, key, bubbles, cancelable, init) {
    var target = nodesById.get(jsId >>> 0);
    if (!target) return false;

    var mods = init && init.modifiers;
```

In the `event` object literal, replace the pointer lines and append the modifier lines:

```js
      clientX: init ? init.clientX : undefined,
      clientY: init ? init.clientY : undefined,
      button: init ? init.button : undefined,
      buttons: init ? init.buttons : undefined,
      shiftKey: mods ? !!mods.shiftKey : undefined,
      ctrlKey: mods ? !!mods.ctrlKey : undefined,
      altKey: mods ? !!mods.altKey : undefined,
      metaKey: mods ? !!mods.metaKey : undefined,
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p superui_js --features engine-v8 --test dispatch`
Expected: PASS — including `mouse_event_exposes_coordinates` (flat pointer reads still work with `init` named differently).

- [ ] **Step 5: Commit**

```bash
git add crates/superui_js/js/dom.js crates/superui_js/tests/dispatch.rs
git commit -m "feat(superui_js): expose modifier keys on dispatched events

dom.js read the dispatch init object's nested modifiers sub-object so
author listeners can branch on shiftKey/ctrlKey/altKey/metaKey. Pointer
fields stay flat on the same object, so existing callers are unaffected."
```

---

### Task 2: `Modifiers`/`EventInit` types + dispatch-ABI widening

Replaces the `mouse: Option<MousePayload>` argument of the dispatch seam with `EventInit`, carries modifiers through `PendingDomEvent`, and updates every caller so the workspace compiles. New behavior: a manually-attached `with_modifiers` reaches the listener.

**Files:**
- Modify: `crates/superui_js/src/lib.rs` (add types; `JsEngine::dispatch_event` signature, ~77-85)
- Modify: `crates/superui_js/src/engine_v8.rs` (~196-231)
- Modify: `crates/superui_js/src/engine_web.rs` (~129-164)
- Modify: `crates/superui_bridge/src/events.rs` (`PendingDomEvent`, drain ~434; keyboard calls ~324/353/359)
- Modify: `crates/superui_bridge/src/runtime.rs` (`dispatch_dom_event`, ~265-281)
- Modify: `crates/superui_bridge/src/lib.rs` (re-export)
- Modify: `crates/supersolid_runtime/src/lib.rs` (~795)
- Modify tests: `crates/superui_js/tests/v8_engine.rs` (~72), `crates/superui_bridge/tests/frame_loop.rs` (~52), `crates/superui_bridge/tests/supersolid_render.rs` (~58)
- Test: `crates/superui_bridge/tests/input_events.rs`

**Interfaces:**
- Produces:
  - `superui_js::Modifiers { pub shift: bool, pub ctrl: bool, pub alt: bool, pub meta: bool }` (`Clone, Copy, Debug, Default, PartialEq`)
  - `superui_js::EventInit { pub mouse: Option<MousePayload>, pub modifiers: Option<Modifiers> }` (`Clone, Copy, Debug, Default, PartialEq`)
  - `JsEngine::dispatch_event(&mut self, target, ty: &str, key: Option<&str>, init: EventInit, bubbles: bool, cancelable: bool) -> bool`
  - `UiRuntime::dispatch_dom_event(&mut self, node, ty: &str, key: Option<&str>, init: EventInit, bubbles: bool, cancelable: bool) -> bool`
  - `PendingDomEvent::with_modifiers(self, m: Modifiers) -> Self`; field `pub modifiers: Option<Modifiers>`
- Both re-exported from `superui_bridge`.

- [ ] **Step 1: Write the failing test**

Add to `crates/superui_bridge/tests/input_events.rs` (import `Modifiers` from the crate's use-list at the top):

```rust
/// A `PendingDomEvent` carrying `Modifiers` reaches the JS listener's
/// `MouseEvent.shiftKey`/`metaKey`.
#[test]
fn modifiers_on_pending_event_reach_the_js_listener() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<div id='b'>hi</div>",
    )));
    let mut app = test_app();
    let _root = mount_with_input(&mut app, dom.clone());

    app.world_mut().non_send_mut::<UiRuntime>().run_script(
        "globalThis.seen = {}; \
         document.getElementById('b').addEventListener('mousedown', function(e) { \
           globalThis.seen.shift = e.shiftKey; globalThis.seen.meta = e.metaKey; \
         });",
    );
    app.update();

    let node = dom.borrow().get_element_by_id("b").unwrap();
    app.world_mut().resource_mut::<PendingDomEvents>().0.push(
        PendingDomEvent::new(node, "mousedown").with_modifiers(Modifiers {
            shift: true,
            meta: true,
            ..Default::default()
        }),
    );
    app.update();

    app.world_mut().non_send_mut::<UiRuntime>().run_script(
        "document.getElementById('b').setAttribute('data-s', String(globalThis.seen.shift)); \
         document.getElementById('b').setAttribute('data-m', String(globalThis.seen.meta));",
    );
    assert_eq!(dom.borrow().get_attribute(node, "data-s"), Some("true"));
    assert_eq!(dom.borrow().get_attribute(node, "data-m"), Some("true"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p superui_bridge --test input_events modifiers_on_pending_event`
Expected: FAIL — does not compile: `Modifiers` and `with_modifiers` do not exist.

- [ ] **Step 3: Add the types (`superui_js/src/lib.rs`)**

After the `MousePayload` definition:

```rust
/// W3C modifier-key state (`MouseEvent`/`KeyboardEvent` `shiftKey` etc.).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub meta: bool,
}

/// Extra event-object data for `JsEngine::dispatch_event`: pointer geometry for
/// mouse events, modifier state for mouse and keyboard events. Both `None` for
/// events that carry neither (focus/change/input).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EventInit {
    pub mouse: Option<MousePayload>,
    pub modifiers: Option<Modifiers>,
}
```

Change the trait method signature:

```rust
    fn dispatch_event(
        &mut self,
        target: JsNodeId,
        ty: &str,
        key: Option<&str>,
        init: EventInit,
        bubbles: bool,
        cancelable: bool,
    ) -> bool;
```

- [ ] **Step 4: Serialize `EventInit` in both engines**

`crates/superui_js/src/engine_v8.rs` `dispatch_event` — replace the `mouse_arg` block and signature:

```rust
    fn dispatch_event(
        &mut self,
        target: crate::JsNodeId,
        ty: &str,
        key: Option<&str>,
        init: crate::EventInit,
        bubbles: bool,
        cancelable: bool,
    ) -> bool {
        let handle = self.tokio.handle().clone();
        let _rt = handle.enter();
        let init_arg = if init.mouse.is_none() && init.modifiers.is_none() {
            Value::Null
        } else {
            let mut o = serde_json::Map::new();
            if let Some(m) = init.mouse {
                o.insert("clientX".into(), json!(m.client_x));
                o.insert("clientY".into(), json!(m.client_y));
                o.insert("button".into(), json!(m.button));
                o.insert("buttons".into(), json!(m.buttons));
            }
            if let Some(md) = init.modifiers {
                o.insert(
                    "modifiers".into(),
                    json!({
                        "shiftKey": md.shift, "ctrlKey": md.ctrl,
                        "altKey": md.alt, "metaKey": md.meta,
                    }),
                );
            }
            Value::Object(o)
        };
        let args = [
            json!(target),
            json!(ty),
            key.map_or(Value::Null, |k| json!(k)),
            json!(bubbles),
            json!(cancelable),
            init_arg,
        ];
        match self.call_global::<bool>("__ss_dispatch", &args) {
            Ok(Some(prevented)) => prevented,
            Ok(None) => false,
            Err(e) => {
                eprintln!("__ss_dispatch() failed: {e}");
                false
            }
        }
    }
```

`crates/superui_js/src/engine_web.rs` `dispatch_event` — same shape via `Reflect`:

```rust
    fn dispatch_event(
        &mut self,
        target: crate::JsNodeId,
        ty: &str,
        key: Option<&str>,
        init: crate::EventInit,
        bubbles: bool,
        cancelable: bool,
    ) -> bool {
        let Some(dispatch) = scope_fn(&self.scope, "__ss_dispatch") else {
            return false;
        };
        let init_arg = if init.mouse.is_none() && init.modifiers.is_none() {
            JsValue::null()
        } else {
            let obj = Object::new();
            if let Some(m) = init.mouse {
                let _ = Reflect::set(&obj, &JsValue::from_str("clientX"), &JsValue::from_f64(m.client_x as f64));
                let _ = Reflect::set(&obj, &JsValue::from_str("clientY"), &JsValue::from_f64(m.client_y as f64));
                let _ = Reflect::set(&obj, &JsValue::from_str("button"), &JsValue::from_f64(m.button as f64));
                let _ = Reflect::set(&obj, &JsValue::from_str("buttons"), &JsValue::from_f64(m.buttons as f64));
            }
            if let Some(md) = init.modifiers {
                let mods = Object::new();
                let _ = Reflect::set(&mods, &JsValue::from_str("shiftKey"), &JsValue::from_bool(md.shift));
                let _ = Reflect::set(&mods, &JsValue::from_str("ctrlKey"), &JsValue::from_bool(md.ctrl));
                let _ = Reflect::set(&mods, &JsValue::from_str("altKey"), &JsValue::from_bool(md.alt));
                let _ = Reflect::set(&mods, &JsValue::from_str("metaKey"), &JsValue::from_bool(md.meta));
                let _ = Reflect::set(&obj, &JsValue::from_str("modifiers"), &mods.into());
            }
            obj.into()
        };
        let args = Array::new();
        args.push(&JsValue::from(target));
        args.push(&JsValue::from_str(ty));
        args.push(&key.map_or(JsValue::null(), JsValue::from_str));
        args.push(&JsValue::from_bool(bubbles));
        args.push(&JsValue::from_bool(cancelable));
        args.push(&init_arg);
        match Reflect::apply(&dispatch, &JsValue::undefined(), &args) {
            Ok(ret) => ret.as_bool().unwrap_or(false),
            Err(e) => {
                web_error("__ss_dispatch() failed", &e);
                false
            }
        }
    }
```

- [ ] **Step 5: Carry modifiers through `PendingDomEvent` and the runtime**

`crates/superui_bridge/src/events.rs` — add the field, builder, and default; import `Modifiers` (extend `use superui_js::MousePayload;` to `use superui_js::{EventInit, Modifiers, MousePayload};`):

```rust
    /// Modifier-key state for a mouse or keyboard event; `None` carries no
    /// `*Key` properties (focus/change/input).
    pub modifiers: Option<Modifiers>,
```

In `PendingDomEvent::new`, add `modifiers: None,`. Add the builder next to `with_mouse`:

```rust
    /// Attach [`Modifiers`] (`shiftKey`/`ctrlKey`/`altKey`/`metaKey`) to a mouse
    /// or keyboard event.
    pub fn with_modifiers(mut self, m: Modifiers) -> Self {
        self.modifiers = Some(m);
        self
    }
```

Rewrite the drain call (~434) to build an `EventInit`:

```rust
        rt.dispatch_dom_event(
            e.target,
            &e.type_,
            e.key.as_deref(),
            EventInit { mouse: e.mouse, modifiers: e.modifiers },
            e.bubbles,
            e.cancelable,
        );
```

`crates/superui_bridge/src/runtime.rs` — change `dispatch_dom_event` to take `init: EventInit` and forward it:

```rust
    pub fn dispatch_dom_event(
        &mut self,
        node: NodeId,
        ty: &str,
        key: Option<&str>,
        init: superui_js::EventInit,
        bubbles: bool,
        cancelable: bool,
    ) -> bool {
        let Some(js) = self.applier.js(node) else {
            return false;
        };
        self.sync_live_props_to_shadow();
        let prevented = self.engine.dispatch_event(js, ty, key, init, bubbles, cancelable);
        self.pump();
        prevented
    }
```

- [ ] **Step 6: Update the remaining callers to compile (no behavior change)**

In `crates/superui_bridge/src/events.rs` `keyboard_events_system`, change the three `None` arguments to `EventInit::default()` (lines ~324, ~353, ~359). Task 3 replaces the keydown/keyup/click ones with real modifiers.

In `crates/supersolid_runtime/src/lib.rs` (~795): `e.dispatch_event(btn, "click", None, superui_js::EventInit::default(), true, true);`

In tests `crates/superui_js/tests/v8_engine.rs` (~72), `crates/superui_bridge/tests/frame_loop.rs` (~52), `crates/superui_bridge/tests/supersolid_render.rs` (~58): replace the `None` mouse argument with `superui_js::EventInit::default()` (or `EventInit::default()` with an import).

`crates/superui_bridge/src/lib.rs` — re-export the new types next to `MousePayload`:

```rust
pub use superui_js::{EventInit, Modifiers, MousePayload};
```

- [ ] **Step 7: Run the new test and the suite**

Run: `cargo test -p superui_bridge --test input_events modifiers_on_pending_event`
Expected: PASS.
Run: `cargo test -p superui_js -p superui_bridge -p supersolid_runtime`
Expected: PASS — all existing tests green.

- [ ] **Step 8: Commit**

```bash
git add crates/superui_js crates/superui_bridge crates/supersolid_runtime
git commit -m "feat: carry modifier-key state through the DOM-event dispatch seam

Bundle pointer and modifier data into EventInit so keyboard events can
carry modifiers too without a mouse payload, and the JS dispatch ABI
stays one object wide. PendingDomEvent gains an optional Modifiers that
the drain forwards into the event object."
```

---

### Task 3: Capture live modifier state at enqueue sites

Adds `modifiers_now` and wires it into `mouse_events_system`, `keyboard_events_system`, and the click path, so real input produces modifier-bearing events.

**Files:**
- Modify: `crates/superui_bridge/src/events.rs` (`modifiers_now`; `click_effect`, `focus_and_click`, `apply_pointer_click`, `on_pointer_click`; keyboard calls)
- Modify: `crates/superui_bridge/src/mouse.rs` (`mouse_events_system`)
- Modify: `crates/superui_bridge/src/lib.rs` (re-export `modifiers_now` if not already via glob)
- Modify callers: `crates/superui_test_engine/src/driver.rs` (~590), `crates/superui_test_engine/src/ui_driver.rs` (~863), `crates/superui_bridge/tests/input_events.rs` (~284)
- Test: `crates/superui_bridge/src/events.rs` (inline unit), `crates/superui_bridge/tests/mouse_events.rs`, `crates/superui_bridge/tests/input_events.rs`

**Interfaces:**
- Consumes: `Modifiers` (Task 2), `PendingDomEvent::with_modifiers` (Task 2).
- Produces: `pub fn modifiers_now(keys: &ButtonInput<KeyCode>) -> Modifiers`; `click_effect(rt, node, modifiers: Modifiers, pending)`; `apply_pointer_click(..., modifiers: Modifiers, ...)`.

- [ ] **Step 1: Write the failing unit test for `modifiers_now`**

Append to `crates/superui_bridge/src/events.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use bevy::input::ButtonInput;

    #[test]
    fn modifiers_now_maps_each_side_of_every_modifier() {
        let keys = ButtonInput::<KeyCode>::default();
        assert_eq!(modifiers_now(&keys), Modifiers::default());

        // Each modifier reads true when either its left or right key is held.
        for (code, expect) in [
            (KeyCode::ShiftLeft, Modifiers { shift: true, ..Default::default() }),
            (KeyCode::ShiftRight, Modifiers { shift: true, ..Default::default() }),
            (KeyCode::ControlLeft, Modifiers { ctrl: true, ..Default::default() }),
            (KeyCode::ControlRight, Modifiers { ctrl: true, ..Default::default() }),
            (KeyCode::AltLeft, Modifiers { alt: true, ..Default::default() }),
            (KeyCode::AltRight, Modifiers { alt: true, ..Default::default() }),
            (KeyCode::SuperLeft, Modifiers { meta: true, ..Default::default() }),
            (KeyCode::SuperRight, Modifiers { meta: true, ..Default::default() }),
        ] {
            let mut keys = ButtonInput::<KeyCode>::default();
            keys.press(code);
            assert_eq!(modifiers_now(&keys), expect, "modifier for {code:?}");
        }
    }
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p superui_bridge --lib events::tests::modifiers_now`
Expected: FAIL — does not compile: `modifiers_now` undefined.

- [ ] **Step 3: Implement `modifiers_now`**

In `crates/superui_bridge/src/events.rs` (near `key_name`):

```rust
/// W3C modifier state from the live Bevy keyboard input — each modifier reads
/// true when either its left or right key is held.
pub fn modifiers_now(keys: &ButtonInput<KeyCode>) -> Modifiers {
    Modifiers {
        shift: keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight),
        ctrl: keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight),
        alt: keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight),
        meta: keys.pressed(KeyCode::SuperLeft) || keys.pressed(KeyCode::SuperRight),
    }
}
```

`ButtonInput` is in `bevy::prelude::*` (already imported). Run Step 2's command again; expect PASS.

- [ ] **Step 4: Wire the click path**

In `crates/superui_bridge/src/events.rs`:

`click_effect` — add a `modifiers` parameter and attach it to the click only (the checkbox `change` carries none):

```rust
pub fn click_effect(
    rt: &UiRuntime,
    node: NodeId,
    modifiers: Modifiers,
    pending: &mut PendingDomEvents,
) {
    let is_checkbox = {
        let d = rt.dom.borrow();
        matches!(tag_of(&d, node).as_deref(), Some("input"))
            && d.get_attribute(node, "type") == Some("checkbox")
    };

    pending.0.push(PendingDomEvent::new(node, "click").with_modifiers(modifiers));
    if is_checkbox {
        let now = !rt.dom.borrow().checked(node);
        rt.dom.borrow_mut().set_checked(node, now);
        pending.0.push(PendingDomEvent::new(node, "change"));
    }
}
```

`focus_and_click` — thread `modifiers` through to `click_effect`:

```rust
fn focus_and_click(
    node: NodeId,
    rt: &mut UiRuntime,
    pending: &mut PendingDomEvents,
    input_focus: &mut InputFocus,
    modifiers: Modifiers,
) {
    rt.set_focus(Some(node));
    if let Some(entity) = rt.entity_for(node) {
        input_focus.set(entity, FocusCause::Pressed);
    }
    click_effect(rt, node, modifiers, pending);
}
```

`on_pointer_click` — add `keys: Res<ButtonInput<KeyCode>>` to the system params and pass the snapshot:

```rust
    let mods = modifiers_now(&keys);
    focus_and_click(node, &mut dom, &mut pending, &mut input_focus, mods);
```

`apply_pointer_click` — add a `modifiers: Modifiers` parameter and forward it to `focus_and_click`.

- [ ] **Step 5: Update `click_effect`/`apply_pointer_click` callers**

- `crates/superui_bridge/tests/input_events.rs` (~284): `click_effect(rt, node, Modifiers::default(), &mut pending);`
- `crates/superui_test_engine/src/driver.rs` (~590): `click_effect(&rt, node, Modifiers::default(), &mut pending);` (import `Modifiers` from `superui_bridge`)
- `crates/superui_test_engine/src/ui_driver.rs` (~863): same change (import `Modifiers`)

- [ ] **Step 6: Wire `keyboard_events_system`**

In `crates/superui_bridge/src/events.rs`, add `keys: Res<ButtonInput<KeyCode>>` to the params, snapshot once after the empty-check, and pass modifiers to keydown/keyup and the activation click (the checkbox `change` keeps `EventInit::default()`):

```rust
pub fn keyboard_events_system(
    mut reader: MessageReader<KeyboardInput>,
    mut rt: NonSendMut<UiRuntime>,
    mut input_focus: ResMut<InputFocus>,
    keys: Res<ButtonInput<KeyCode>>,
) {
```

```rust
    if presses.is_empty() {
        return;
    }
    let mods = modifiers_now(&keys);
    let mut any = false;
```

keydown/keyup (~324):

```rust
        rt.dispatch_dom_event(
            focused,
            type_,
            Some(&kn),
            EventInit { modifiers: Some(mods), ..Default::default() },
            true,
            true,
        );
```

activation click (~353):

```rust
            rt.dispatch_dom_event(
                focused,
                "click",
                None,
                EventInit { modifiers: Some(mods), ..Default::default() },
                true,
                true,
            );
```

- [ ] **Step 7: Wire `mouse_events_system`**

In `crates/superui_bridge/src/mouse.rs`, import `KeyCode` and `modifiers_now`, add `keys: Res<ButtonInput<KeyCode>>` to the params, snapshot once, and attach `.with_modifiers(mods)` to every `PendingDomEvent` it builds.

Imports:

```rust
use bevy::input::keyboard::KeyCode;
use crate::events::{modifiers_now, resolve_dom_node, PendingDomEvent, PendingDomEvents};
```

Param (add to the system signature) and snapshot after `let mask = current_mask(&buttons);`:

```rust
    keys: Res<ButtonInput<KeyCode>>,
```
```rust
    let mods = modifiers_now(&keys);
```

Attach to each enqueue, e.g. the `mousedown`/`mouseup`:

```rust
                        pending.0.push(
                            PendingDomEvent::new(top, ty)
                                .with_mouse(payload(position, mask, code))
                                .with_modifiers(mods),
                        );
```

Apply the same `.with_modifiers(mods)` to the `mouseout`, `mouseleave`, `mouseover`, `mouseenter`, and `mousemove` events.

- [ ] **Step 8: Write the capture tests**

Mouse capture — add to `crates/superui_bridge/tests/mouse_events.rs`, modeled on the existing button-down test (press `ShiftLeft` in `ButtonInput<KeyCode>`, drive a `mousedown` on a hovered node carrying a `mousedown` listener, then read `e.shiftKey` back through a DOM attribute). Assert an unmodified mousedown yields `e.shiftKey === false` and a Shift-held one yields `true`:

```rust
#[test]
fn mousedown_carries_modifier_state() {
    // Build the standard mouse harness (see the neighboring tests): a hovered
    // node with a `mousedown` listener that records `e.shiftKey` into a global,
    // mirrored to a DOM attribute after the tick.
    // 1. tick with no keys held -> attribute reads "false"
    // 2. app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::ShiftLeft);
    //    re-fire the mousedown, tick -> attribute reads "true"
    // Assertions (fill in with the harness's helpers):
    //   assert_eq!(dom.borrow().get_attribute(node, "data-shift"), Some("false"));
    //   ... press ShiftLeft, re-fire ...
    //   assert_eq!(dom.borrow().get_attribute(node, "data-shift"), Some("true"));
}
```

Keyboard capture — add to `crates/superui_bridge/tests/input_events.rs`, modeled on `typing_into_focused_input_updates_value_and_fires_input`: focus an element with a `keydown` listener, `press(KeyCode::ShiftLeft)` in `ButtonInput<KeyCode>`, send a `KeyboardInput`, tick, and assert the listener saw `e.shiftKey === true`.

> The two capture tests reuse each file's existing harness helpers verbatim; follow the nearest neighboring test for plugin/resource setup. The inline comment above marks the only lines that differ (pressing `ShiftLeft` and asserting the mirrored attribute). There is no e2e modifier test through the `superui_test_engine` synthetic `.click()`: that path has no real `ButtonInput`, so it carries no modifiers — call that out rather than assert it.

- [ ] **Step 9: Run tests**

Run: `cargo test -p superui_bridge`
Expected: PASS — `modifiers_now` unit, the two capture tests, and all existing bridge tests.

- [ ] **Step 10: Commit**

```bash
git add crates/superui_bridge crates/superui_test_engine
git commit -m "feat(superui_bridge): capture modifier keys from live input

Snapshot ButtonInput<KeyCode> when each mouse, keyboard, and click event
is enqueued so e.shiftKey and friends reflect the state at input time,
not a frame later at drain."
```

---

### Task 4: Changelog entry

**Files:**
- Modify: `CHANGELOG.md` (`## [Unreleased]` -> `### Added`)

- [ ] **Step 1: Add the entry**

Under `## [Unreleased]` / `### Added`, after the mouse-events bullet:

```markdown
- Modifier keys on events: mouse events (`click`, `mousedown`/`up`/`move`,
  `mouseover`/`out`/`enter`/`leave`) and keyboard events (`keydown`/`keyup`)
  now carry `shiftKey`/`ctrlKey`/`altKey`/`metaKey`, sourced from live key
  state at input time — so a handler can branch on Shift/Ctrl/Alt/Cmd, e.g.
  `transfer(e.shiftKey ? 100 : 1)` or Shift-click to quick-move. Non-input
  events (`change`/`input`/`focus`/`blur`) carry none, matching the browser.
  The `chest_inventory` example gains Shift-click quick-transfer.
```

- [ ] **Step 2: Commit**

```bash
git add CHANGELOG.md
git commit -m "docs(changelog): record modifier keys on events"
```

---

### Task 5: Reference docs (website + skill)

**REQUIRED SUB-SKILL:** load `reference-docs` before writing; it governs capability/support tables. The website table also follows `documenting-new-features` for the "since version" marker (`main` for unreleased).

**Files:**
- Modify: `website/src/docs/reference/js-dom.md` (event-properties table)
- Modify: `plugins/bevy_superui/skills/supersolid/references/html-dom.md` (events section)

- [ ] **Step 1: Website reference rows**

In `website/src/docs/reference/js-dom.md`, in the event-properties table (the one with the `event.clientX/Y` row, ~67), add after the coordinate rows, matching the existing `| property | status | tier | since | notes |` column shape:

```markdown
| `event.shiftKey` / `ctrlKey` / `altKey` / `metaKey` | ✅ | T1 | main | on mouse + keyboard events; `metaKey` is Cmd (macOS) / Super (Linux) / Win key; non-input events (focus/change/input) carry none |
```

- [ ] **Step 2: Skill reference**

Mirror the same capability into `plugins/bevy_superui/skills/supersolid/references/html-dom.md`'s events section, in that file's established format (read its surrounding entries first; do not invent a new shape). State only what is supported: the four `*Key` booleans on mouse and keyboard events, `metaKey` = Cmd/Super/Win, non-input events carry none.

- [ ] **Step 3: Commit**

```bash
git add website/src/docs/reference/js-dom.md plugins/bevy_superui/skills/supersolid/references/html-dom.md
git commit -m "docs(reference): document modifier-key event properties"
```

---

### Task 6: Demonstrate in the inventory example

Shift-click a tile to instantly quick-transfer it to the other panel (reads `e.shiftKey` on the existing `onMouseDown`); plain drag is unchanged.

**Files:**
- Modify: `examples/inventory/assets/ui/inventory/app.tsx`

- [ ] **Step 1: Add quick-move on Shift-mousedown**

In `App()` (after `setterFor`), add a quick-move helper:

```tsx
  // Shift-click the opposite of a drag: move the item straight to the other
  // panel without a drag, reading the modifier off the mousedown event.
  const quickMove = (id: string, from: SlotName) => {
    const to: SlotName = from === "chest" ? "inv" : "chest";
    const fromItems = storeFor(from)();
    const item = fromItems.find((it) => it.id === id);
    if (!item) return;
    setterFor(from)(fromItems.filter((it) => it.id !== id));
    setterFor(to)([...storeFor(to)(), item]);
  };
```

In `startDrag`, short-circuit when Shift is held:

```tsx
  const startDrag = (e: any, id: string, from: SlotName) => {
    if (e.shiftKey) {
      quickMove(id, from);
      return;
    }
    setDragging({ id, from });
    setFloatPos({ x: e.clientX, y: e.clientY });
    document.addEventListener("mousemove", onMove);
    document.addEventListener("mouseup", onUp);
  };
```

- [ ] **Step 2: Rebuild and verify by hand**

Run: `cargo run -p chest_inventory`
Expected: a plain drag still moves a tile; Shift-clicking a tile moves it to the other panel immediately with no drag. (The example's `build.rs` pre-transpiles the `.tsx`; the dev run uses the HMR watcher.)

Note: no automated assertion — the `superui_test_engine` synthetic click carries no modifier state (Task 3), so a Shift-click e2e test is not possible without separate test-engine support.

- [ ] **Step 3: Commit**

```bash
git add examples/inventory/assets/ui/inventory/app.tsx
git commit -m "feat(examples): shift-click quick-transfer in chest_inventory

Demonstrate reading e.shiftKey off a mousedown to branch behavior."
```

---

## Self-review notes

- **Spec coverage:** Problem/Goal (Tasks 1-3), `Modifiers`/`EventInit` + ABI (Task 2), engine serialization (Task 2), `dom.js` mapping (Task 1), capture timing + sites (Task 3), fidelity for non-input events (Tasks 1-2), the non-goals are honored (no unfocused keyboard dispatch, no `key_name` changes). Extras the user asked for: changelog (Task 4), reference docs website+skill (Task 5), example (Task 6).
- **Type consistency:** `EventInit`/`Modifiers`/`with_modifiers`/`modifiers_now`/`click_effect(…, modifiers, …)` signatures are defined in Task 2/3 Interfaces and used consistently downstream.
- **Review Focus:** each of the five lines maps to a named test or a kept-green existing test above.
