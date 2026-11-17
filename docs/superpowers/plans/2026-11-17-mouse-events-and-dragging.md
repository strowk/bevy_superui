# Mouse Events & Dragging Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give superui a browser-faithful mouse-event foundation (`mouseover`/`mouseout`/`mouseenter`/`mouseleave`/`mousedown`/`mouseup`/`mousemove` with `clientX`/`clientY`/`button`/`buttons`) plus document-level drag capture, at zero cost when no app registers mouse listeners, validated by a chest↔inventory drag example.

**Architecture:** A per-frame `mouse_events_system` in `superui_bridge` reads `HoverMap`/`PointerLocation`/`ButtonInput<MouseButton>`, diffs against a `PointerState`, and enqueues `PendingDomEvent`s that drain into JS through the existing W3C dispatch. The whole system is gated off a `MouseListenerIndex` (maintained by extending the `SetListener` op with an event-kind code) via a Bevy run condition, so apps that don't listen pay nothing. `document`/`window` become real bubble-chain targets above the root so drag `mousemove`/`mouseup` reach a `document` handler with `event.target` intact.

**Tech Stack:** Rust, Bevy 0.20, `bevy_picking`, `deno_core`/V8 (native JS) + browser engine (wasm), the superui op-wire (`superui_js::opwire`), supersolid TSX.

**Spec:** `docs/superpowers/specs/2026-11-17-mouse-events-and-dragging-design.md` — read it alongside this plan; the plan argues from it.

## Global Constraints

- **No cost when no mouse listener exists.** The generator must be skipped by a Bevy run condition (not merely early-return) when `MouseListenerIndex` reports zero mouse listeners. Verified by a test.
- **Picking behavior must not regress.** The `SetListener` change must keep a node `interactive`/blocking exactly as today whenever it has any listener. `crates/superui_bridge/tests/picking.rs` and `crates/superui_bridge/tests/frame_loop.rs` are the guards and must stay green.
- **Both JS engines stay in lockstep.** Any change to the `JsEngine::dispatch_event` ABI is implemented identically in `engine_v8.rs` and `engine_web.rs`. `dom.js` is one file shared by both.
- **wasm-clean.** No `std`-only or native-only API in `dom.js` or anything compiled for `engine-web`.
- **Coordinates are logical window pixels** (`PointerLocation`), the meaning of `clientX/clientY`. `offsetX/offsetY`, `movementX/Y`, HTML5 DnD, pointer-capture, and touch are OUT of scope.
- **`Since` marker:** every new/flipped docs ledger row gets `main` in its `Since` column (per the `documenting-new-features` skill), swapped to the version at release.
- **Commit style (repo CLAUDE.md):** commit summary says what was done; body says why/how, not a restatement of the diff.

## Review Focus

- **Drag crossing the gap between panels** (cursor over the root background, not over any slot): `mousemove` must still reach a `document` listener so the floating tile keeps following. Pinned in Task 7 (`mousemove_over_root_reaches_document`).
- **`event.target` on a `document`-level `mouseup`**: a drop handler on `document` must see `target` = the slot under the cursor, not `document`. Pinned in Task 5 (`bubbled_event_keeps_element_target`).
- **Adding a listener mid-hover** must not retro-fire `mouseover` for already-hovered nodes (seeding rule). Pinned in Task 7 (`activation_seeds_without_firing`).
- **Only one mouse family registered** (e.g. `mousedown` only): no `mousemove` dispatch and no hover diffing should occur. Pinned in Task 7 (`mousedown_only_does_no_move_or_hover_work`).
- **Removing the last mouse listener** turns the system fully off again (run condition flips back). Pinned in Task 8 (`run_condition_toggles_with_listeners`).

---

## Task 1: Extend the `SetListener` op with an event-kind code

**Files:**
- Modify: `crates/superui_js/src/opwire/codec.rs` (the `Op` enum + opcode/arity/operand/decode arms, ~lines 10–111)
- Test: `crates/superui_js/src/opwire/codec.rs` (`#[cfg(test)]` module at bottom, has existing `set_listener_*` tests ~lines 305–320)

**Interfaces:**
- Produces: `pub enum ListenerKind` (in `codec.rs`, re-exported from `opwire/mod.rs`): `Other=0, MouseOver=1, MouseOut=2, MouseEnter=3, MouseLeave=4, MouseDown=5, MouseUp=6, MouseMove=7`, with `pub fn from_code(u8) -> ListenerKind` (unknown → `Other`) and `pub fn code(self) -> u8`.
- Produces: `Op::SetListener { id: JsNodeId, kind: ListenerKind, has_listener: bool }` — now arity 3, operands `[id, has_listener as u32, kind.code() as u32]`.

- [ ] **Step 1: Write the failing test** — append to the `#[cfg(test)]` module in `codec.rs`:

```rust
#[test]
fn set_listener_carries_kind() {
    let mut b = OpBatch::default();
    b.ops.push(Op::SetListener { id: 7, kind: ListenerKind::MouseMove, has_listener: true });
    b.ops.push(Op::SetListener { id: 7, kind: ListenerKind::Other, has_listener: false });
    let bytes = b.encode();
    let decoded = OpBatch::decode(&bytes).expect("roundtrip");
    assert_eq!(decoded.ops, b.ops);
}
```

- [ ] **Step 2: Run it to confirm it fails** — `cargo test -p superui_js set_listener_carries_kind` → FAIL (no `kind` field / `ListenerKind`).

- [ ] **Step 3: Implement.** In `codec.rs`:
  - Add the `ListenerKind` enum (derive `PartialEq, Debug, Clone, Copy`) with `from_code`/`code`.
  - Change the `SetListener` variant to `{ id, kind: ListenerKind, has_listener: bool }`.
  - `arity`: change arm `9 => Some(2)` to `9 => Some(3)`.
  - `write_operands`: `Op::SetListener { id, kind, has_listener } => [id, has_listener as u32, kind.code() as u32]`.
  - `from_operands`: `9 => Op::SetListener { id: operands[0], has_listener: operands[1] != 0, kind: ListenerKind::from_code(operands[2] as u8) }`.
  - `string_ids`: leave `SetListener => [None, None]`.
  - Update the two existing `set_listener_*` tests (~305–320) to pass a `kind` (use `ListenerKind::Other`).
  - Re-export `ListenerKind` from `crates/superui_js/src/opwire/mod.rs` (`pub use codec::{... , ListenerKind};`).

- [ ] **Step 4: Run tests** — `cargo test -p superui_js opwire::codec` → PASS.

- [ ] **Step 5: Commit** — `git add crates/superui_js/src/opwire && git commit -m "feat(opwire): carry event-kind on SetListener op"`

---

## Task 2: Emit per-type `SetListener` kinds from `dom.js`

**Files:**
- Modify: `crates/superui_js/js/dom.js` — `addEventListener`/`removeEventListener`/`hasAnyListener` (~lines 290–332), and add a `listenerKindCode(type)` helper + per-type bookkeeping.
- Test: `crates/superui_js/tests/dispatch.rs` (new test) — drive scripts through the engine and read `flush_ops()`.

**Interfaces:**
- Consumes: `Op::SetListener { id, kind, has_listener }` (Task 1).
- Produces (JS contract): `addEventListener(type)` emits `OP_SET_LISTENER [nid, 1, kindCode]` on that **type's** per-node count going 0→1; `removeEventListener` emits `[nid, 0, kindCode]` on 1→0. `kindCode` matches `ListenerKind::code` (mouseover=1 … mousemove=7, everything else 0). The node-level "has any" signal is still derivable: `has_listener=1` for a kind means that kind is present; the applier (Task 3) unions kinds per node.

- [ ] **Step 1: Write the failing test** — add to `crates/superui_js/tests/dispatch.rs` (follow the existing harness in that file for building an engine and evaluating scripts; use `flush_ops()` to read the batch):

```rust
#[test]
fn addeventlistener_emits_kind_per_type() {
    let mut eng = test_engine("<div id='a'></div>"); // helper already used in this file
    eng.eval(r#"
        var a = document.getElementById('a');
        a.addEventListener('mousemove', function(){});
        a.addEventListener('click', function(){});
    "#).unwrap();
    let batch = eng.flush_ops();
    let kinds: Vec<(u32, bool, u8)> = batch.ops.iter().filter_map(|op| match op {
        superui_js::opwire::Op::SetListener { id, kind, has_listener } =>
            Some((*id, *has_listener, kind.code())),
        _ => None,
    }).collect();
    // mousemove -> kind 7 present; click -> kind 0 (Other) present.
    assert!(kinds.contains(&(/* a's jsId */  a_id, true, 7)));
    assert!(kinds.iter().any(|&(_, present, code)| present && code == 0));
}
```

(If the file's existing harness names differ, match them; the assertion on `a_id` can instead assert "some op with kind 7, present" without pinning the id.)

- [ ] **Step 2: Run it to confirm it fails** — `cargo test -p superui_js addeventlistener_emits_kind_per_type` → FAIL (today emits only node-level has-any, kind always 0).

- [ ] **Step 3: Implement** in `dom.js`:
  - Add near the other constants:
    ```js
    function listenerKindCode(type) {
      switch (type) {
        case "mouseover": return 1;
        case "mouseout": return 2;
        case "mouseenter": return 3;
        case "mouseleave": return 4;
        case "mousedown": return 5;
        case "mouseup": return 6;
        case "mousemove": return 7;
        default: return 0;
      }
    }
    ```
  - In `addEventListener`: compute `var before = (this._listeners.get(type) || []).length;` BEFORE pushing. After pushing, `if (before === 0) emit(OP_SET_LISTENER, [this._nid, 1, listenerKindCode(type)]);` Remove the old `hasAnyListener`-based emit.
  - In `removeEventListener`: after splicing, `if (arr.length === 0) emit(OP_SET_LISTENER, [this._nid, 0, listenerKindCode(type)]);` Remove the old `hasAnyListener` emit.
  - `hasAnyListener` can stay (still used elsewhere if any) or be deleted if now unused — check references first.
  - Note: `emit(OP_SET_LISTENER, [...])` now passes a 3-element operand array; confirm the JS `emit` writes all provided operands (it should, since arity is now 3 on the Rust side).

- [ ] **Step 4: Run tests** — `cargo test -p superui_js` → PASS (new test + existing dispatch tests).

- [ ] **Step 5: Commit** — `git add crates/superui_js/js/dom.js crates/superui_js/tests/dispatch.rs && git commit -m "feat(dom): emit per-type listener kind on add/removeEventListener"`

---

## Task 3: Count mouse listeners in the applier; preserve the picking marker

**Files:**
- Modify: `crates/superui_js/src/opwire/applier.rs` — the `SetListener` arm (~128–141) and `OpApplier` struct/impl.
- Test: `crates/superui_js/src/opwire/applier.rs` `#[cfg(test)]` module.

**Interfaces:**
- Consumes: `Op::SetListener { id, kind, has_listener }`, `ListenerKind`.
- Produces: `pub struct MouseListenerCounts { counts: [u32; 7] }` (indexed by `kind.code()-1` for codes 1..=7) with `pub fn any(&self) -> bool`, `pub fn any_hover(&self) -> bool` (codes 1–4), `pub fn any_button(&self) -> bool` (codes 5–6), `pub fn any_move(&self) -> bool` (code 7). Re-export from `opwire/mod.rs`.
- Produces: `OpApplier::mouse_listener_counts(&self) -> MouseListenerCounts` (returns a copy).

- [ ] **Step 1: Write the failing test** — append to the applier test module:

```rust
#[test]
fn applier_counts_mouse_kinds_and_keeps_interactive_marker() {
    let mut dom = Dom::new();
    let root = dom.document();
    let mut applier = OpApplier::new(root);
    // Bind js 2 -> a new element under root.
    let el = StrPool-less setup... // use the same create+insert pattern as nearby tests
    // Register a mousemove listener on the element:
    let mut b = OpBatch::default();
    b.ops.push(Op::SetListener { id: 2, kind: ListenerKind::MouseMove, has_listener: true });
    applier.apply(&mut dom, &b);
    assert!(applier.mouse_listener_counts().any_move());
    assert!(applier.mouse_listener_counts().any());
    // The node must read as interactive (has a listener) for picking:
    assert!(!dom.listeners(/* node bound to js 2 */ node2).is_empty());
    // Remove it:
    let mut b2 = OpBatch::default();
    b2.ops.push(Op::SetListener { id: 2, kind: ListenerKind::MouseMove, has_listener: false });
    applier.apply(&mut dom, &b2);
    assert!(!applier.mouse_listener_counts().any());
    assert!(dom.listeners(node2).is_empty());
}
```

(Model the element creation on the existing applier tests in this file — `Op::CreateElement { id:2, .. }` + `Op::InsertBefore { parent:1, node:2, reference:0 }` — and resolve `node2 = applier.node(2).unwrap()`.)

- [ ] **Step 2: Run it to confirm it fails** — `cargo test -p superui_js applier_counts_mouse_kinds` → FAIL.

- [ ] **Step 3: Implement** in `applier.rs`:
  - Add `MouseListenerCounts` (derive `Clone, Copy, Default, Debug`) with the four predicates. Index helper: for a `ListenerKind` with `code()` in 1..=7, `counts[code-1]`.
  - Add fields to `OpApplier`: `kinds_per_node: HashMap<NodeId, std::collections::HashSet<u8>>` (present kind codes per node) and `mouse_counts: MouseListenerCounts`.
  - Rewrite the `SetListener` arm:
    ```rust
    Op::SetListener { id, kind, has_listener } => {
        let code = kind.code();
        // Global mouse counts update regardless of whether the id maps to a
        // mirror node (document/window sentinels don't, but still count).
        if (1..=7).contains(&code) {
            let slot = &mut self.mouse_counts.counts[(code - 1) as usize];
            if has_listener { *slot += 1; } else { *slot = slot.saturating_sub(1); }
        }
        let Some(node) = self.map.node(id) else { return };
        // Track the union of kinds on this node to drive the picking marker.
        let set = self.kinds_per_node.entry(node).or_default();
        if has_listener { set.insert(code); } else { set.remove(&code); }
        let interactive = !set.is_empty();
        if set.is_empty() { self.kinds_per_node.remove(&node); }
        // Keep exactly one synthetic "__ss" marker while the node has any listener.
        let has_marker = self.listeners.contains_key(&node);
        if interactive && !has_marker {
            if let Some(lid) = dom.add_event_listener(node, "__ss", false) {
                self.listeners.insert(node, lid);
            }
        } else if !interactive && has_marker {
            if let Some(lid) = self.listeners.remove(&node) { dom.remove_event_listener(node, lid); }
        }
    }
    ```
  - Add `pub fn mouse_listener_counts(&self) -> MouseListenerCounts { self.mouse_counts }`.

- [ ] **Step 4: Run tests** — `cargo test -p superui_js` → PASS.

- [ ] **Step 5: Commit** — `git add crates/superui_js/src/opwire && git commit -m "feat(opwire): track global mouse-listener counts in the applier"`

---

## Task 4: Thread a mouse payload through the dispatch ABI

**Files:**
- Modify: `crates/superui_js/src/lib.rs` (the `JsEngine::dispatch_event` trait signature ~63–70) + add `MousePayload`.
- Modify: `crates/superui_js/src/engine_v8.rs` (`dispatch_event` ~196–221).
- Modify: `crates/superui_js/src/engine_web.rs` (`dispatch_event` ~133…).
- Modify: `crates/superui_js/js/dom.js` (`dispatch(jsId, type, key, bubbles, cancelable)` ~588, and the `__ss_dispatch` glue that calls it).
- Test: `crates/superui_js/tests/dispatch.rs`.

**Interfaces:**
- Produces: `#[derive(Clone, Copy, Debug, PartialEq)] pub struct MousePayload { pub client_x: f32, pub client_y: f32, pub button: u8, pub buttons: u8 }` in `superui_js` (pub from crate root).
- Produces: `fn dispatch_event(&mut self, target: JsNodeId, ty: &str, key: Option<&str>, mouse: Option<MousePayload>, bubbles: bool, cancelable: bool) -> bool` on `JsEngine` and both impls.
- Produces (JS): the event object exposes `clientX`, `clientY`, `button`, `buttons` when a mouse payload is present; `undefined` otherwise.

- [ ] **Step 1: Write the failing test** — in `dispatch.rs`:

```rust
#[test]
fn mouse_event_exposes_coordinates() {
    let mut eng = test_engine("<div id='a'></div>");
    eng.eval(r#"
        globalThis.seen = {};
        document.getElementById('a').addEventListener('mousedown', function(e){
            seen.x = e.clientX; seen.y = e.clientY; seen.b = e.button; seen.bs = e.buttons;
        });
    "#).unwrap();
    let a = /* jsId of #a via the file's helper */;
    eng.dispatch_event(a, "mousedown",
        None,
        Some(superui_js::MousePayload { client_x: 12.0, client_y: 34.0, button: 0, buttons: 1 }),
        true, true);
    assert_eq!(eval_number(&mut eng, "seen.x"), 12.0);
    assert_eq!(eval_number(&mut eng, "seen.y"), 34.0);
    assert_eq!(eval_number(&mut eng, "seen.bs"), 1.0);
}
```

(Use the file's existing eval helpers; `eval_number` mirrors the existing `eval_string`.)

- [ ] **Step 2: Run it to confirm it fails** — `cargo test -p superui_js mouse_event_exposes_coordinates` → FAIL (signature mismatch / no coords).

- [ ] **Step 3: Implement.**
  - `lib.rs`: add `MousePayload`; add `mouse: Option<MousePayload>` to the trait method (place it right after `key`, before `bubbles`, matching the test).
  - `engine_v8.rs`: add the param; build a JS value `let mouse_arg = mouse.map_or(Value::Null, |m| json!({"clientX": m.client_x, "clientY": m.client_y, "button": m.button, "buttons": m.buttons}));` and pass it as the final arg to `__ss_dispatch` (append after `cancelable`). Update the `args` array accordingly.
  - `engine_web.rs`: mirror — construct the JS object for the browser engine’s call path and pass it through to `dispatch`.
  - `dom.js`: change `function dispatch(jsId, type, key, bubbles, cancelable, mouse)`; in the event-object literal add `clientX: mouse ? mouse.clientX : undefined, clientY: mouse ? mouse.clientY : undefined, button: mouse ? mouse.button : undefined, buttons: mouse ? mouse.buttons : undefined,`. Update the `__ss_dispatch` wrapper (search for where `dispatch(` is invoked / `__ss_dispatch` is installed) to accept and forward the `mouse` arg.
  - Fix all other callers of `dispatch_event` to pass `None` for `mouse` (there is one runtime caller — Task 6 updates it; update any test callers now so the crate compiles).

- [ ] **Step 4: Run tests** — `cargo test -p superui_js` → PASS.

- [ ] **Step 5: Commit** — `git add crates/superui_js && git commit -m "feat(js): carry mouse coordinates through dispatch_event into the event object"`

---

## Task 5: `document` / `window` as bubble-chain targets

**Files:**
- Modify: `crates/superui_js/js/dom.js` — the `document` object (~558–577), the `dispatch` ancestor walk (~592–631), the globals publish (~654–687), and listener storage.
- Test: `crates/superui_js/tests/dispatch.rs`.

**Interfaces:**
- Produces (JS): `document.addEventListener/removeEventListener` and `window.addEventListener/removeEventListener` work and feed `OP_SET_LISTENER` (with sentinel jsIds `DOC_ID`/`WIN_ID`). The dispatch ancestor chain is `target → … → root(body) → document → window`, so a bubbling event dispatched at an element reaches `document`/`window` listeners with `event.target` = the element. Non-bubbling events (`bubbles=false`) do not reach them.

- [ ] **Step 1: Write the failing tests** — in `dispatch.rs`:

```rust
#[test]
fn bubbled_event_keeps_element_target() {
    let mut eng = test_engine("<div id='a'></div>");
    eng.eval(r#"
        globalThis.hit = null;
        document.addEventListener('mouseup', function(e){ hit = e.target && e.target.getAttribute('id'); });
    "#).unwrap();
    let a = /* jsId of #a */;
    eng.dispatch_event(a, "mouseup", None,
        Some(superui_js::MousePayload{client_x:0.0,client_y:0.0,button:0,buttons:0}), true, true);
    assert_eq!(eval_string(&mut eng, "hit"), "a");
}

#[test]
fn non_bubbling_event_does_not_reach_document() {
    let mut eng = test_engine("<div id='a'></div>");
    eng.eval(r#"
        globalThis.n = 0;
        document.addEventListener('mouseenter', function(){ n++; });
    "#).unwrap();
    let a = /* jsId of #a */;
    eng.dispatch_event(a, "mouseenter", None, None, /*bubbles*/ false, true);
    assert_eq!(eval_number(&mut eng, "n"), 0.0);
}
```

- [ ] **Step 2: Run to confirm failure** — `cargo test -p superui_js bubbled_event_keeps_element_target non_bubbling_event_does_not_reach_document` → FAIL (`document.addEventListener` is not a function).

- [ ] **Step 3: Implement** in `dom.js`:
  - Reserve sentinel ids near the top: `var DOC_ID = 0xFFFFFFFE; var WIN_ID = 0xFFFFFFFF;` (far above `nextId`, which starts at 2).
  - Make `document` and `window` behave as `Node`-like listener targets: give each a `_nid` (`DOC_ID`/`WIN_ID`), a `_listeners` Map, and the shared `addEventListener`/`removeEventListener` (reuse the prototype methods — e.g. assign `document.addEventListener = Node.prototype.addEventListener` and same for `removeEventListener`, and give them `_listeners`). Register them in `nodesById` so `dispatch` can resolve them and so bubbling can look them up.
  - Set up the chain: `root.parentNode` must resolve to the `document` target, `document.parentNode` to the `window` target, `window.parentNode` to `null`. Since `root` is a real `Node` whose `parentNode` is normally null, add a getter or explicit wiring so the `dispatch` ancestor walk (`cur = target.parentNode; while (cur) {...}`) climbs through them. Keep this wiring from interfering with structural ops (the applier/root tree); it only affects the JS-side dispatch walk and `document`/`window` are never children in the op-wire.
  - In `dispatch`, the plan-building `capture`/`bubble` loops already use `ancestors`; because `document`/`window` are now in `ancestors`, their listeners fire for bubbling events and are skipped when `bubbles` is false (the bubble loop is gated on `bubbles`). Confirm the capture loop also includes them (acceptable — capture listeners on document/window are browser-faithful).
  - Ensure `window` is still `globalThis` for other code paths: `window.addEventListener` must coexist with `window` being the global. If `window === globalThis`, attach `_nid`/`_listeners` and the methods onto `globalThis`. (Check the existing `window` note in the ledger — `window (alias of globalThis)` — and the globals-publish block.)

- [ ] **Step 4: Run tests** — `cargo test -p superui_js` → PASS. Also run `cargo test -p superui_js shadow_dom` to catch tree-wiring regressions.

- [ ] **Step 5: Commit** — `git add crates/superui_js/js/dom.js crates/superui_js/tests/dispatch.rs && git commit -m "feat(dom): make document/window bubble-chain event targets"`

---

## Task 6: `PendingDomEvent.mouse` + runtime dispatch + `MouseListenerIndex` accessor

**Files:**
- Modify: `crates/superui_bridge/src/events.rs` — `PendingDomEvent` (~16–45), its uses, and `drain_dom_events_system` (~413–426) + `keyboard_events_system` call sites.
- Modify: `crates/superui_bridge/src/runtime.rs` — `dispatch_dom_event` (~265–280) + add `mouse_listener_counts()` passthrough.
- Modify: `crates/superui_bridge/src/lib.rs` — re-export `MousePayload`.
- Test: `crates/superui_bridge/tests/input_events.rs` (add a payload roundtrip) — or a focused unit test in `events.rs`.

**Interfaces:**
- Consumes: `superui_js::MousePayload`, `OpApplier::mouse_listener_counts`.
- Produces: `PendingDomEvent { target, type_, key: Option<String>, mouse: Option<MousePayload>, bubbles, cancelable }` with builder `pub fn with_mouse(mut self, m: MousePayload) -> Self`.
- Produces: `UiRuntime::dispatch_dom_event(&mut self, node, ty, key: Option<&str>, mouse: Option<MousePayload>, bubbles, cancelable) -> bool`.
- Produces: `UiRuntime::mouse_listener_counts(&self) -> superui_js::opwire::MouseListenerCounts` (delegates to `self.applier`).

- [ ] **Step 1: Write the failing test** — in `crates/superui_bridge/tests/input_events.rs`, add a test that registers a `mousedown` handler recording `e.clientX`, pushes a `PendingDomEvent::new(node,"mousedown").with_mouse(MousePayload{..})`, runs a drain tick, and asserts the handler saw the coordinate (mirror the existing input_events harness for mounting + reading a JS global).

- [ ] **Step 2: Run to confirm failure** — `cargo test -p superui_bridge mouse` → FAIL (`with_mouse` / `mouse` field absent).

- [ ] **Step 3: Implement.**
  - `events.rs`: add `pub mouse: Option<MousePayload>` to `PendingDomEvent`; default it `None` in `new`; add `with_mouse`. In `drain_dom_events_system`, pass `e.mouse` into `dispatch_dom_event`.
  - `keyboard_events_system` and any other direct `rt.dispatch_dom_event(...)` call: add `None` for the new `mouse` param.
  - `runtime.rs`: add `mouse: Option<MousePayload>` param to `dispatch_dom_event`, forward it to `self.engine.dispatch_event(js, ty, key, mouse, bubbles, cancelable)`. Add `pub fn mouse_listener_counts(&self) -> MouseListenerCounts { self.applier.mouse_listener_counts() }`.
  - `lib.rs`: `pub use superui_js::MousePayload;` (and re-export `MouseListenerCounts` if the bridge’s public API needs it).

- [ ] **Step 4: Run tests** — `cargo test -p superui_bridge` → PASS (new test + existing input_events/picking/frame_loop).

- [ ] **Step 5: Commit** — `git add crates/superui_bridge && git commit -m "feat(bridge): carry mouse payload on PendingDomEvent and through dispatch"`

---

## Task 7: `mouse_events_system` + `PointerState`

**Files:**
- Create: `crates/superui_bridge/src/mouse.rs` — the system, `PointerState` resource, `MouseButtonMask` helper, and the run condition.
- Modify: `crates/superui_bridge/src/lib.rs` — `mod mouse;` + re-exports.
- Test: Create `crates/superui_bridge/tests/mouse_events.rs`.

**Interfaces:**
- Consumes: `resolve_dom_node` (events.rs, already `pub`), `PendingDomEvents`/`PendingDomEvent::with_mouse`, `UiRuntime::mouse_listener_counts`, `MousePayload`.
- Produces: `pub fn mouse_events_system(...)`; `#[derive(Resource, Default)] pub struct PointerState { pub hover: Vec<NodeId>, pub position: Option<Vec2>, pub buttons: u8, pub seeded: bool }`; `pub fn mouse_listeners_active(rt: Option<NonSend<UiRuntime>>) -> bool`.
- Button mask bit layout: left=bit0 (`buttons & 1`), right=bit1, middle=bit2 — matching the W3C `buttons` bitfield; `button` field uses 0=left,1=middle,2=right.

**System behavior (document in a module comment, then implement):**
1. Read `HoverMap` (`Res<HoverMap>`), the mouse `PointerLocation` (`Query<&PointerLocation>` or `Res` per the bevy_picking API in this version — follow `scroll.rs`'s `HoverMap`/`PointerId::Mouse` usage and add the pointer-position source), `ButtonInput<MouseButton>` (`Res`), `Query<&DomNode>`, `Query<&ChildOf>`, `ResMut<PendingDomEvents>`, `ResMut<PointerState>`, and `NonSend<UiRuntime>` (for `mouse_listener_counts`).
2. Resolve the current hovered `NodeId` set via `resolve_dom_node` over `hover_map.get(&PointerId::Mouse)` entries; dedup, preserve deepest-first is not required (set semantics).
3. **Seeding:** if `!state.seeded`, set `state.hover`/`position`/`buttons` to current and `seeded=true`, enqueue nothing, return. (System only runs when listeners exist — see run condition — so first activation seeds.)
4. **Hover diff** (only if `counts.any_hover()`): nodes in `state.hover` not in current → `mouseout`(bubbles=true)+`mouseleave`(bubbles=false); nodes in current not in `state.hover` → `mouseover`(true)+`mouseenter`(false). Enqueue out/leave before over/enter. Each with the current `MousePayload`.
5. **Buttons** (only if `counts.any_button()`): compute current mask from `ButtonInput`; for each changed bit vs `state.buttons`, enqueue `mousedown` (press) / `mouseup` (release) at the current top hovered node (first resolved, or skip if none) with the payload. These bubble to document/window via Task 5.
6. **Move** (only if `counts.any_move()`): if `position != state.position`, enqueue one `mousemove` at the top hovered node with the payload. If no node resolves but a button is held, enqueue `mousemove` directly at the document sentinel node (resolve via `UiRuntime`/`OpApplier` mapping for `DOC_ID`, or add a helper `UiRuntime::document_node()` that returns the mirror root — acceptable fallback; note in the module comment).
7. Update `state.hover`/`position`/`buttons` to current.

Payload: `client_x/client_y` from the pointer position; `button`/`buttons` from the button state.

- [ ] **Step 1: Write the failing tests** — `crates/superui_bridge/tests/mouse_events.rs`. Model mounting on `tests/input_events.rs`/`tests/picking.rs` (`test_app()`, `mount(...)`), register listeners via `run_script`, then drive input by inserting/mutating `HoverMap`, `PointerLocation`, and `ButtonInput<MouseButton>` resources and calling `app.update()`. Tests (each asserts a JS-recorded global or the `PendingDomEvents`/dispatch effect):
  - `activation_seeds_without_firing` — with a `mouseover` listener, first update after the cursor is already over `#a` fires **no** `mouseover`.
  - `enter_then_leave_sequence` — moving hover from `#a` to `#b` fires `mouseout`+`mouseleave` on `a` then `mouseover`+`mouseenter` on `b`, in that order.
  - `mousedown_up_dispatch` — pressing then releasing the left button over `#a` fires `mousedown` then `mouseup` on `a` with `button==0`.
  - `mousemove_over_root_reaches_document` — with a `document` `mousemove` listener and the left button held, moving the pointer while hovered only on the root still delivers `mousemove` to the document handler.
  - `mousedown_only_does_no_move_or_hover_work` — with only a `mousedown` listener, moving the pointer fires no `mousemove` and does no hover-diff dispatch.

- [ ] **Step 2: Run to confirm failure** — `cargo test -p superui_bridge --test mouse_events` → FAIL (module/system absent).

- [ ] **Step 3: Implement** `mouse.rs` per the behavior spec above; add `mod mouse;` and re-export `mouse_events_system`, `PointerState`, `mouse_listeners_active` from `lib.rs`. (Resolve the exact `PointerLocation` query shape against this repo's `bevy_picking` version by reading how `scroll.rs` imports picking types; keep the import style identical.)

- [ ] **Step 4: Run tests** — `cargo test -p superui_bridge --test mouse_events` → PASS; then `cargo test -p superui_bridge` → all PASS.

- [ ] **Step 5: Commit** — `git add crates/superui_bridge && git commit -m "feat(bridge): generate mouse events from picking with per-family gating"`

---

## Task 8: Wire the system into the plugin with the run condition

**Files:**
- Modify: `crates/superui/src/mount.rs` — resource init (~204) and the bridge system chain (~256–281).
- Modify: `crates/superui/src/lib.rs` — re-export surface if needed.
- Test: `crates/superui/tests/` (new small test) or extend an existing mount test.

**Interfaces:**
- Consumes: `mouse_events_system`, `PointerState`, `mouse_listeners_active` (Task 7).

- [ ] **Step 1: Write the failing test** — `crates/superui/tests/mouse_run_condition.rs` (or add to an existing integration test): mount a UI with no mouse listeners, run several `app.update()`s, assert `PointerState` stays unseeded / the system did no work; then `run_script` an `addEventListener('mousemove', …)`, update, assert the system now runs (e.g. `PointerState.seeded == true`). Name it `run_condition_toggles_with_listeners`.

- [ ] **Step 2: Run to confirm failure** — FAIL (system not registered).

- [ ] **Step 3: Implement** in `mount.rs`:
  - `.init_resource::<PointerState>()` alongside `PendingDomEvents`.
  - Add `mouse_events_system` into the chained bridge tuple **before** `drain_dom_events_system` (so its events dispatch the same frame), i.e. insert it at the front of the `(editable_input_events_system, drain_dom_events_system, …)` chain, and attach `.run_if(mouse_listeners_active)` to that system specifically. Since the tuple is `.chain()`ed and `.run_if(runtime_exists)` is applied to the whole set, apply the mouse-specific condition on the individual system: pull `mouse_events_system` out as `mouse_events_system.run_if(mouse_listeners_active)` and order it `.before(drain_dom_events_system)` within the chain (or make it the first element of the chained tuple and gate the tuple with `runtime_exists` while gating the element with `mouse_listeners_active` — verify Bevy applies a per-system `run_if` inside a chained set; if not, register `mouse_events_system` as its own `.add_systems(Update, mouse_events_system.run_if(runtime_exists).run_if(mouse_listeners_active).before(drain_dom_events_system))`). Prefer the standalone registration for a clean per-system condition.

- [ ] **Step 4: Run tests** — `cargo test -p superui` → PASS, and `cargo test -p superui_bridge` (picking/frame_loop regression) → PASS.

- [ ] **Step 5: Commit** — `git add crates/superui && git commit -m "feat(superui): register gated mouse_events_system in the bridge chain"`

---

## Task 9: Test-engine faithful hover + drag actions

**Files:**
- Modify: `crates/superui_test_engine/src/command.rs` — add `MouseDown`/`MouseUp`/`MoveTo`/`DragTo` command variants.
- Modify: `crates/superui_test_engine/src/driver.rs` — `PendingActionKind` (~26), command match (~139), `perform_action` (~432), and a `last_hovered` state + helpers.
- Modify: `crates/superui_test_engine/src/ui_driver.rs` — mirror the same additions (~56, ~327, ~756).
- Test: `crates/superui_test_engine/tests/actions.rs` (extend) — assert the event sequences.

**Interfaces:**
- Consumes: `PendingDomEvent::with_mouse`, `MousePayload`.
- Produces: `Command::{MouseDown,MouseUp,MoveTo}{locator}` and `Command::DragTo { from: LocatorSpec, to: LocatorSpec }`; matching `PendingActionKind` variants; a driver-held `last_hovered: Option<NodeId>`.
- Synthetic payload: `MousePayload { client_x: 0.0, client_y: 0.0, button: 0, buttons }` (placeholder coords — drop resolution keys off `event.target`; see spec Risks). `buttons=1` during a drag's move/up, `0` otherwise.

- [ ] **Step 1: Write the failing tests** — in `tests/actions.rs`:
  - `hover_emits_enter_and_bubbling_over` — `hover(#a)` then `hover(#b)` produces `mouseout`+`mouseleave` on `a` then `mouseover`+`mouseenter` on `b` (assert via listeners recording into a JS global).
  - `drag_to_moves_between_targets` — `dragTo(#tileInChest, #slotInInv)` produces `mousedown` on the tile, `mousemove`, and `mouseup` resolvable from `event.target` as the inventory slot.

- [ ] **Step 2: Run to confirm failure** — `cargo test -p superui_test_engine` → FAIL.

- [ ] **Step 3: Implement.**
  - `command.rs`: add the variants (serde `rename_all = "camelCase"`, so JSON `type: "mouseDown"`, `"moveTo"`, `"dragTo"` with `from`/`to`).
  - `driver.rs`:
    - Extend `PendingActionKind` with `MouseDown`, `MouseUp`, `MoveTo`, and represent `DragTo` as a sequence (either a new kind that performs three dispatches, or expand it in the command match into three queued actions).
    - Replace the `Hover => dispatch(app, spec, "mouseover")` behavior with a `hover(app, spec)` fn that: resolves the node; if `last_hovered` is `Some(prev)` and `prev != node`, push `mouseout`(bubbles)+`mouseleave`(no bubble) on `prev`; push `mouseover`(bubbles)+`mouseenter`(no bubble) on `node`; set `last_hovered = Some(node)`. Each event carries a synthetic `MousePayload`. (Store `last_hovered` on the driver state struct that owns `inflight`.)
    - Add `mouse_dispatch(app, spec, type_, buttons)` that pushes `PendingDomEvent::new(node, type_).with_mouse(MousePayload{client_x:0.0,client_y:0.0,button:0,buttons})` with the right `bubbles` flag per type (over/out/down/up/move bubble; enter/leave don't).
    - `DragTo { from, to }`: mouseDown(from, buttons=1) → moveTo(to, buttons=1) → mouseUp(to, buttons=1). Implement by expanding into the three dispatches in order within one action's `perform_action`, or by queueing three actions.
  - `ui_driver.rs`: mirror all of the above (same helper names, same behavior) so UI mode and headless stay identical.

- [ ] **Step 4: Run tests** — `cargo test -p superui_test_engine` → PASS.

- [ ] **Step 5: Commit** — `git add crates/superui_test_engine && git commit -m "feat(test-engine): faithful hover plus mouseDown/moveTo/mouseUp/dragTo actions"`

---

## Task 10: Inventory example (`examples/inventory`)

**Files:**
- Create: `examples/inventory/Cargo.toml`, `examples/inventory/build.rs`, `examples/inventory/src/main.rs`, `examples/inventory/assets/ui/inventory/{index.html,app.tsx,style.css}`, `examples/inventory/tsconfig.json`. (Model every file on `examples/counter/` — copy its `Cargo.toml`/`build.rs`/`main.rs`/`tsconfig.json` and adapt names/paths; `SuperUiRoot::from_asset_dir("ui/inventory", &assets)`.)
- Modify: root `Cargo.toml` workspace members if examples are listed there (check; `examples/counter` is a member — add `examples/inventory` the same way).
- Test: `examples/inventory/tests/inventory.rs` — a `superui_test_engine`-driven E2E spec (model on `examples/todomvc_supersolid/tests/`).

**App design (`app.tsx`):**
- A reactive store of items per container: `chest: Item[]`, `inv: Item[]` (id, name/emoji).
- Two panels (`#chest`, `#inv`), each rendering slots; each occupied slot renders a tile `<div class="tile" data-item={id} onMouseDown={startDrag}>`.
- `startDrag(e)`: record `dragging = {id, from}`; `document.addEventListener("mousemove", onMove)`, `document.addEventListener("mouseup", onUp)`; create/show a floating tile positioned from `e.clientX/e.clientY`.
- `onMove(e)`: reposition the floating tile from `e.clientX/e.clientY`.
- `onUp(e)`: resolve the drop slot from `e.target.closest?.('[data-slot]')` or by walking `e.target`'s ancestors for a `data-slot` container (note: `closest` is roadmap — walk `parentNode` manually reading `getAttribute("data-slot")`); move the item between stores; remove the floating tile; `document.removeEventListener(...)` for both.
- Slots carry `data-slot="chest"` / `data-slot="inv"` so the drop target is resolvable by element identity (matches the test-engine's `event.target` story).

- [ ] **Step 1: Write the failing E2E test** — `tests/inventory.rs`: mount the example app, `dragTo` a chest tile onto an inventory slot, assert (via a DOM query/visible assertion) the tile now lives in the inventory panel and not the chest. Model harness on `examples/todomvc_supersolid/tests/`.

- [ ] **Step 2: Run to confirm failure** — `cargo test -p inventory` → FAIL (crate/app absent).

- [ ] **Step 3: Implement** the crate files and `app.tsx` per the design. Pre-transpile is handled by `build.rs` (copied from counter). Verify `cargo run -p inventory` shows two panels and a draggable tile manually if possible; the automated test is the gate.

- [ ] **Step 4: Run tests** — `cargo test -p inventory` → PASS. Run `cargo build -p inventory` to confirm it compiles for native.

- [ ] **Step 5: Commit** — `git add examples/inventory Cargo.toml && git commit -m "feat(examples): chest<->inventory drag-and-drop example"`

---

## Task 11: Documentation

**Files:**
- Modify: `website/src/docs/reference/js-dom.md` — Events table + Globals note.
- Modify: `plugins/bevy_superui/skills/supersolid/references/html-dom.md` — Events table + JSX `on*` prose.
- Modify: `CHANGELOG.md` — `[Unreleased] → Added`.
- Modify: `website/src/docs/concepts/picking.md` — short mouse-events subsection.
- Modify: `website/src/examples/README.md` (+ add `website/src/examples/inventory/` gallery entry the way `game_menu`/`todomvc` are set up — check how an existing example is wired into the gallery and mirror it).

**This task has no code tests; its gate is that `mdbook build website` (or the repo's docs build) succeeds and the ledgers are internally consistent.**

- [ ] **Step 1: `js-dom.md`** — in the **Events** table: change `mouseover`/`mouseout` to ✅ with `Since` `main`; add ✅ rows (`Since` `main`) for `mouseenter`/`mouseleave`, `mousedown`/`mouseup`, `mousemove`; change `event.clientX/Y` row so `clientX/Y` are ✅ (note `offsetX/Y` still 🟡). In **Globals**, on the `window` row note that `document`/`window` accept mouse listeners and that bubbling events (incl. `mousemove`/`mouseup` during a drag) reach them with `event.target` intact.

- [ ] **Step 2: `html-dom.md` (skill ref)** — mirror the Events-table changes; extend the JSX `on*` prose (currently `onClick`/`onInput`/`onChange`/`onKeyDown`/`onKeyUp`) to add `onMouseDown`/`onMouseUp`/`onMouseMove`/`onMouseOver`/`onMouseOut`/`onMouseEnter`/`onMouseLeave`, and note the event now exposes `clientX`/`clientY`/`button`/`buttons`.

- [ ] **Step 3: `CHANGELOG.md`** — under `[Unreleased] → Added`, one entry: mouse-event family (over/out/enter/leave/down/up/move) with `clientX/Y/button/buttons`, document/window listeners, and document-level drag capture; mention the `examples/inventory` demo.

- [ ] **Step 4: `picking.md`** — add a short "Mouse events & dragging" subsection: nodes stay hoverable regardless of picking policy; a listener makes a node block like a click listener; drag capture is achieved by document/window bubbling. Add the unreleased `since-note--unreleased` callout under the new `##`/`###` heading (per `documenting-new-features`).

- [ ] **Step 5: Gallery** — register `examples/inventory` in the website examples gallery mirroring an existing example, and add a card to `website/src/examples/README.md`.

- [ ] **Step 6: Build & commit** — run the docs build to confirm no broken includes; `git add website CHANGELOG.md plugins/bevy_superui/skills/supersolid/references/html-dom.md && git commit -m "docs: document mouse events, dragging, and the inventory example"`

---

## Self-Review Notes

- **Spec coverage:** Component 1 (gate) → Tasks 1–3, 7, 8; Component 2 (`PointerState`) → Task 7; Component 3 (payload/ABI) → Tasks 4, 6; Component 4 (document/window bubble chain) → Task 5; Component 5 (generator) → Task 7; scheduling → Task 8; test-engine section → Task 9; worked example → Task 10; testing → folded into each task; docs → Task 11. No uncovered spec section.
- **Type consistency:** `ListenerKind` (codes 1–7) is defined in Task 1 and used by Tasks 2, 3; `MouseListenerCounts` predicates (`any`/`any_hover`/`any_button`/`any_move`) defined in Task 3, used in Tasks 6–8; `MousePayload` defined in Task 4, used in Tasks 6, 7, 9; `PointerState` defined in Task 7, used in Task 8.
- **Known soft spots a subagent must resolve against live code (not placeholders — concrete lookups):** the exact `PointerLocation` source for this `bevy_picking` version (Task 7 — copy `scroll.rs`'s import style); the per-system `run_if` inside a `.chain()` (Task 8 — prefer standalone registration); the `test_engine`/`dispatch.rs` helper names (Tasks 2/4/5 — match the file). These are flagged inline in their tasks.
