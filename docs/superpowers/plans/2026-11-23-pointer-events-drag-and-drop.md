# Pointer Events + Capture for Mouse/Touch Drag-and-Drop Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the W3C `PointerEvent` family (`pointerdown`/`move`/`up`/`cancel` + hover + capture) as a parallel, additive event stream so one code path drives drag-and-drop on both mouse and touch, with `setPointerCapture` + `document.elementFromPoint`, proven by migrating `examples/inventory`.

**Architecture:** Three staged layers, all additive (the existing `mouse_events_system` and stage-1 touch tap synthesis are untouched):
1. **JS + engine surface** — `dom.js`'s `dispatch` grows a PointerEvent shape (`pointerId`/`pointerType`/`isPrimary`/`width`/`height`/`pressure`), gated on pointer event types; the Rust `MousePayload` carrier gains those pointer fields; both engine backends (V8, web) marshal them.
2. **Uncaptured synthesis** — six global `bevy_picking` observers in `crates/superui_bridge/src/events.rs` map `PointerOver`/`PointerOut`/`PointerPress`/`PointerRelease`/`PointerMove`/`PointerCancel` to the eight pointer events for ALL pointer types, deriving `pointerType`/`isPrimary` from the pointer id (reusing stage-1's `PrimaryTouch`). All flow through the existing `PendingDomEvents` FIFO.
3. **Capture + geometry** — `setPointerCapture` keeps a JS `{pointerId -> node}` map, retargets events W3C-style, and syncs a Rust-side `PointerCaptures` map via host-binding ops; a raw-location delivery system feeds a captured pointer `pointermove`/`pointerup` even off the UI. `document.elementFromPoint(x,y)` calls a host binding answered by an AABB hit-test over a per-frame geometry snapshot (bevy_ui `ComputedNode` + `GlobalTransform`) pushed into the engine.

**Tech Stack:** Rust, Bevy 0.20, `bevy_picking` 0.20.0, `deno_core`/V8 (native) + browser engine (wasm), the superui DOM/JS op-wire bridge, supersolid/TSX for the example.

**Spec:** `docs/superpowers/specs/2026-11-23-pointer-events-drag-and-drop-design.md`

## Global Constraints

- Shell: use the Bash tool (Git Bash / POSIX sh), never PowerShell.
- No git worktrees (the `target` folder is huge).
- **Pointer events are additive.** Do NOT modify `mouse_events_system` (`crates/superui_bridge/src/mouse.rs`), the stage-1 `on_touch_press`/`on_touch_release` observers, or the compatibility mouse-event stream. The pointer family is a separate parallel stream.
- **Picking event types in this Bevy version are flat `Pointer*` structs** (NOT `Pointer<E>`). Confirmed in the repo today: `bevy::picking::events::{PointerPress, PointerRelease, PointerClick}` (see `events.rs`). The remaining events this plan subscribes to follow the same flat convention — expected names `PointerOver`, `PointerOut`, `PointerMove`, `PointerCancel` (0.19 inner event names were `Over`/`Out`/`Move`/`Cancel`). The implementer MUST confirm the exact names by writing the `On<PointerOver>` etc. handler and letting `cargo build` resolve them; adjust if a name differs. Every such event has `ev.entity: Entity`, `ev.pointer.id: PointerId`, `ev.pointer.position: Vec2`, `ev.hit: HitData`. `PointerPress`/`PointerRelease`/`PointerMove` additionally carry `button`/`delta` fields (unused here).
- **Enter/leave are derived, not subscribed.** Mirror `mouse_events_system`: `PointerOver` enqueues `pointerover` (bubbles) + `pointerenter` (no bubble); `PointerOut` enqueues `pointerout` (bubbles) + `pointerleave` (no bubble). Do NOT subscribe to `PointerEnter`/`PointerLeave` picking events.
- **pointerType/isPrimary derivation.** `PointerId::Mouse` → `pointerType = "mouse"`, `isPrimary = true` always. `PointerId::Touch(_)` → `pointerType = "touch"`, `isPrimary` per the `PrimaryTouch` rule: a touch is primary iff `PrimaryTouch.0` is `None` OR equals this pointer (`None` means "the first finger, about to be/just adopted"; this rule is order-independent w.r.t. the stage-1 observers).
- **JS pointerId numbering.** `PointerId::Mouse` ↔ JS pointerId `1`. `PointerId::Touch(n)` ↔ JS pointerId `(n as u32).wrapping_add(2)` (so touch ids never collide with the mouse's `1`). Reverse: `1` → `Mouse`, else → `Touch((id - 2) as u64)`. Pin both directions in one place.
- **W3C ordering** is preserved by the `PendingDomEvents` FIFO shared with the mouse/compat stream.
- **Coordinates** (`clientX`/`clientY`) are logical window pixels — the same space as `PointerLocation.position`/`ev.pointer.position` and the geometry snapshot. `clientX`/`clientY` on a captured event stay the geometric pointer coordinates (capture retargets the node, never the coordinates).
- **PointerEvent constant defaults** (W3C faithful enough for drag): `width = 1`, `height = 1`, `pressure = (buttons != 0) ? 0.5 : 0` (spec: "0.5 on down / 0 otherwise"; keying off a held button makes a dragging `pointermove` read `0.5` too).
- **New systems touching `UiRuntime` are NonSend** and must run on the main thread (the existing bridge chain pattern). Register pointer picking observers alongside the existing input observers; order the geometry-snapshot push `.before(drain_dom_events_system)` so `elementFromPoint` sees fresh geometry the same frame.

## Review Focus

- **`elementFromPoint` over no UI returns null, not a stale/garbage node.** A drop released over empty background must resolve to `null` so `findSlot` bails cleanly. Covered by Task 3's `outside_all_nodes_is_none` unit test + the hit-test op returning `0`/`null`.
- **A second concurrent finger is a non-primary pointer, not suppressed.** Unlike the stage-1 mouse-compat stream (which ignores secondary fingers), the pointer stream must still fire pointer events for a second finger with `isPrimary = false`. Covered by Task 2's `secondary_touch_is_not_primary`.
- **A captured drag dragged off the UI keeps delivering.** When the pointer is over no UI entity, `bevy_picking` emits nothing; the raw-location system must still deliver `pointermove`/`pointerup` to the captured node. Covered by Task 5's off-UI delivery unit test.
- **`pointerup` implicitly releases capture exactly once.** After a captured `pointerup`, `lostpointercapture` fires and the capture is cleared on both the JS and Rust sides, and a second `pointerup` does nothing. Covered by Task 4's implicit-release test.
- **Pointer fields never leak onto compatibility mouse/keyboard events.** `MousePayload` now carries pointer fields, but a `mousedown`/`mousemove` dispatched by the mouse/compat stream must NOT expose `pointerId`/`pointerType` (a browser `MouseEvent` has none). `dispatch` gates pointer-prop attachment on the event type string. Covered by Task 1's `mouse_event_has_no_pointer_props`.

---

## Stage A — PointerEvent shape + carriers (JS + engines)

### Task 1: PointerEvent shape in `dispatch`, carrier fields, engine marshalling

**Files:**
- Modify: `crates/superui_js/src/lib.rs` — add `PointerKind` enum; add pointer fields to `MousePayload`; derive `Default`.
- Modify: `crates/superui_js/js/dom.js` — `dispatch` attaches pointer props on pointer event types.
- Modify: `crates/superui_js/src/engine_v8.rs` — `dispatch_event` marshals the pointer fields into the init object.
- Modify: `crates/superui_js/src/engine_web.rs` — same for the browser backend.
- Modify (mechanical, add `..Default::default()` to each `MousePayload { .. }` literal): `crates/superui_bridge/src/mouse.rs:174`, `crates/superui_bridge/src/events.rs:153`, `crates/superui_bridge/tests/input_events.rs:88`, `crates/superui_test_engine/src/driver.rs:533`, `crates/superui_test_engine/src/ui_driver.rs:817`.
- Test: `crates/superui_bridge/tests/pointer_events.rs` (new).

**Interfaces:**
- Produces (relied on by all later tasks):
  - `#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)] pub enum PointerKind { #[default] Mouse, Touch }` with `pub fn as_str(self) -> &'static str` (`"mouse"`/`"touch"`), exported from the `superui_js` crate root.
  - `MousePayload` gains `pub pointer_id: u32`, `pub pointer_type: PointerKind`, `pub is_primary: bool`, and `#[derive(Default)]`. Existing fields (`client_x`, `client_y`, `button`, `buttons`) unchanged.
  - `dispatch(jsId, type, key, bubbles, cancelable, init)` attaches `pointerId`/`pointerType`/`isPrimary`/`width`/`height`/`pressure` to the event object **only when `type` is a pointer event** (`type` begins with `"pointer"`, or is `"gotpointercapture"`/`"lostpointercapture"`), reading them from `init`.

- [ ] **Step 1: Write the failing tests** in `crates/superui_bridge/tests/pointer_events.rs`. Mirror `input_events.rs`'s `mouse_payload_on_pending_event_reaches_the_js_listener`: a shared `mount_with_input` harness (copy from `input_events.rs`, plus `app.init_resource::<PendingDomEvents>()` and `drain_dom_events_system.before(reconcile_system)`), push a `PendingDomEvent` carrying a `MousePayload`, `app.update()`, then mirror JS globals into DOM attributes via `run_script` and read them back.

```rust
#[test]
fn pointerdown_exposes_pointer_props_to_the_listener() {
    // DOM "<div id='b'></div>"; listener on "pointerdown" records
    //   e.pointerId, e.pointerType, e.isPrimary, e.width, e.height, e.pressure, e.clientX.
    // Push PendingDomEvent::new(node,"pointerdown").with_mouse(MousePayload{
    //   client_x: 7.0, client_y: 9.0, button: 0, buttons: 1,
    //   pointer_id: 1, pointer_type: PointerKind::Mouse, is_primary: true });
    // Assert via mirrored attributes: pointerId==1, pointerType=="mouse",
    //   isPrimary=="true", width=="1", height=="1", pressure=="0.5" (buttons!=0),
    //   clientX=="7".
}

#[test]
fn touch_pointermove_reports_touch_type_and_zero_pressure() {
    // pointer_type: Touch, is_primary: false, buttons: 0, type "pointermove".
    // Assert pointerType=="touch", isPrimary=="false", pressure=="0".
}

#[test]
fn mouse_event_has_no_pointer_props() {
    // Listener on "mousedown" records (typeof e.pointerId) and (typeof e.pointerType).
    // Push a "mousedown" with a MousePayload whose pointer fields are set.
    // Assert both read "undefined" — pointer props must not leak onto a MouseEvent.
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p superui_bridge --test pointer_events`
Expected: compile error — `PointerKind` not found / `MousePayload` missing fields.

- [ ] **Step 3: Add `PointerKind` + the `MousePayload` fields** in `crates/superui_js/src/lib.rs`, then add `..Default::default()` to every `MousePayload { .. }` literal listed under Files so the workspace compiles.

- [ ] **Step 4: Extend `dispatch` in `dom.js`.** Compute `isPointer` from the `type` string (per Interfaces). When `isPointer`, set on the event object: `pointerId = init ? init.pointerId : undefined`, `pointerType = init ? init.pointerType : undefined`, `isPrimary = init ? !!init.isPrimary : undefined`, `width = 1`, `height = 1`, `pressure = (event.buttons & 0xff) !== 0 ? 0.5 : 0`. Leave them off entirely for non-pointer events (current behavior).

- [ ] **Step 5: Marshal pointer fields in both engines.** In `engine_v8.rs::dispatch_event`, inside the `init.mouse` block, insert `pointerId`/`pointerType`(`m.pointer_type.as_str()`)/`isPrimary` into the init map. In `engine_web.rs::dispatch_event`, `Reflect::set` the same three onto the init object. (Both keep emitting `clientX`/`clientY`/`button`/`buttons` as today.)

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p superui_bridge --test pointer_events`
Expected: PASS (all three).

- [ ] **Step 7: Build affected crates**

Run: `cargo build -p superui_js -p superui_bridge -p superui_test_engine`
Expected: success.

- [ ] **Step 8: Commit**

```bash
git add crates/superui_js crates/superui_bridge crates/superui_test_engine
git commit -m "feat(js): PointerEvent shape and carrier fields for the pointer stream"
```

---

### Task 2: Uncaptured pointer-event synthesis observers

**Files:**
- Modify: `crates/superui_bridge/src/events.rs` — add the pointer id/type/primary helpers, the `PointerCaptures` resource (empty for now), a `pointer_payload` builder, and the six observers.
- Modify: `crates/superui_bridge/src/lib.rs` — export the six observers + `PointerCaptures`.
- Modify: `crates/superui/src/mount.rs` — `init_resource::<PointerCaptures>()` and `add_observer` for each of the six.
- Test: `crates/superui_bridge/tests/pointer_events.rs` (extend).

**Interfaces:**
- Consumes: `resolve_dom_node`, `PendingDomEvent`/`PendingDomEvents`, `PrimaryTouch` (from stage-1), `MousePayload`/`PointerKind` (Task 1), `DomNode`.
- Produces (relied on by Tasks 4/5):
  - `#[derive(Resource, Default)] pub struct PointerCaptures(pub std::collections::HashMap<u32, superui_dom::NodeId>);` (JS pointerId → captured node). Empty until Stage B populates it.
  - `pub fn js_pointer_id(id: PointerId) -> u32` and `pub fn bevy_pointer_id(js: u32) -> PointerId` (the numbering from Global Constraints).
  - `pub fn pointer_kind(id: PointerId) -> PointerKind`.
  - `pub fn pointer_is_primary(id: PointerId, primary: &PrimaryTouch) -> bool`.
  - `pub fn on_pointer_over`, `on_pointer_out`, `on_pointer_press`, `on_pointer_release`, `on_pointer_move`, `on_pointer_cancel` — each `pub fn on_pointer_x(mut ev: On<PointerX>, nodes: Query<&DomNode>, parents: Query<&ChildOf>, pending: ResMut<PendingDomEvents>, primary: Res<PrimaryTouch>, captures: Res<PointerCaptures>)`.

- [ ] **Step 1: Write the failing tests** (extend `pointer_events.rs`). Register the six observers + `PrimaryTouch` + `PointerCaptures` in the harness; trigger real picking events the way `input_events.rs`/`touch_tap` do (spawn `Camera2d`, normalize `PrimaryWindow`, `app.world_mut().trigger(PointerPress { entity, pointer: Pointer::new(id, Location { target, position }), button: PointerButton::Primary, hit: HitData::new(camera, 0.0, None, None), count: 1 })`, and the matching constructors for `PointerOver`/`PointerOut`/`PointerMove`/`PointerRelease`/`PointerCancel` — resolve their exact field sets at compile time). Assert against `app.world().resource::<PendingDomEvents>().0`.

```rust
#[test]
fn mouse_press_enqueues_pointerdown_primary_mouse() {
    // PointerPress PointerId::Mouse on #a at (3,4).
    // Queue == [pointerdown]: pointer_type Mouse, is_primary true, pointer_id 1,
    //   client (3,4), button 0, buttons 1, bubbles true.
}

#[test]
fn mouse_over_enqueues_pointerover_then_pointerenter() {
    // PointerOver Mouse on #a. Queue == [pointerover (bubbles), pointerenter (no bubble)].
    // PointerOut -> [pointerout (bubbles), pointerleave (no bubble)].
}

#[test]
fn touch_press_is_primary_for_the_first_finger() {
    // PrimaryTouch default None. PointerPress Touch(5) on #a.
    // pointerdown: pointer_type Touch, pointer_id == js_pointer_id(Touch(5)) == 7, is_primary true.
}

#[test]
fn secondary_touch_is_not_primary() {
    // Set PrimaryTouch.0 = Some(PointerId::Touch(5)). PointerPress Touch(6) on #a.
    // pointerdown: is_primary false (a second finger still fires, unlike the compat stream).
}

#[test]
fn pointer_release_and_cancel_map_through() {
    // PointerRelease Mouse -> pointerup (button 0, buttons 0, bubbles).
    // PointerCancel Touch(5) -> pointercancel (bubbles).
}

#[test]
fn captured_pointer_is_skipped_by_the_observers() {
    // Insert PointerCaptures { 1 -> some node }. PointerMove Mouse on #a.
    // Queue is empty — the raw-location path (Stage B) owns a captured pointer.
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p superui_bridge --test pointer_events`
Expected: FAIL — the observers/helpers/`PointerCaptures` don't exist.

- [ ] **Step 3: Implement the helpers, resource, and observers** in `events.rs`. Each observer: resolve the hit entity to its `DomNode` (`resolve_dom_node`) — on `None`, return without `ev.propagate(false)` (leave foreign entities alone, like `on_pointer_click`); if `captures.0.contains_key(&js_pointer_id(ev.pointer.id))`, return; else `ev.propagate(false)` and enqueue. Build the payload with `pointer_payload(pos, button, buttons, js_pointer_id(id), pointer_kind(id), pointer_is_primary(id, &primary))`. `button`/`buttons`: press → `(0, 1)`, release → `(0, 0)`, over/out/move/cancel → `(0, 0)`. `pointerenter`/`pointerleave` set `bubbles = false`; everything else keeps the default `true`. Over enqueues `pointerover` then `pointerenter`; Out enqueues `pointerout` then `pointerleave`.

- [ ] **Step 4: Export + register.** Add the six observers, `PointerCaptures`, and the helpers to `pub use events::{...}` in `lib.rs`; in `mount.rs` add `.init_resource::<PointerCaptures>()` and an `.add_observer(...)` for each of the six (alongside `on_touch_press`/`on_touch_release`); extend the `use superui_bridge::{...}` import.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p superui_bridge --test pointer_events`
Expected: PASS.

- [ ] **Step 6: Build the workspace wiring**

Run: `cargo build -p superui`
Expected: success.

- [ ] **Step 7: Commit**

```bash
git add crates/superui_bridge crates/superui/src/mount.rs
git commit -m "feat(bridge): synthesize the uncaptured pointer-event family from picking"
```

---

## Stage B — Capture + elementFromPoint

### Task 3: `elementFromPoint` hit-test + geometry snapshot

**Files:**
- Modify: `crates/superui_js/src/lib.rs` — add `HitRect`, the `element_from_point` pure fn, and `JsEngine::set_hit_rects`.
- Modify: `crates/superui_js/src/engine_v8.rs` — store rects in `V8HostState`; `set_hit_rects`; the `op_ss_element_from_point` op + bootstrap wiring.
- Modify: `crates/superui_js/src/engine_web.rs` — store rects behind an `Rc<RefCell<..>>`; `set_hit_rects`; the `__ss_element_from_point` closure.
- Modify: `crates/superui_js/js/dom.js` — `document.elementFromPoint`.
- Modify: `crates/superui_bridge/src/runtime.rs` — `UiRuntime::set_hit_rects` passthrough (or call the engine directly from the system).
- Modify: `crates/superui_bridge/src/events.rs` (or a small new `pointer.rs`) — `sync_hit_rects_system`.
- Modify: `crates/superui_bridge/src/lib.rs` + `crates/superui/src/mount.rs` — export + register the system ordered `.before(drain_dom_events_system)`.
- Test: `crates/superui_js` unit tests for `element_from_point`; `crates/superui_bridge/tests/pointer_events.rs` for the end-to-end `elementFromPoint`.

**Interfaces:**
- Produces:
  - `#[derive(Clone, Copy, Debug, PartialEq)] pub struct HitRect { pub js_id: JsNodeId, pub min: (f32, f32), pub max: (f32, f32), pub depth: u32 }`.
  - `pub fn element_from_point(rects: &[HitRect], x: f32, y: f32) -> Option<JsNodeId>` — returns the `js_id` of the containing rect with the greatest `depth` (ties: last one wins); `None` if no rect contains `(x, y)`. A rect contains `(x, y)` when `min.0 <= x <= max.0 && min.1 <= y <= max.1`.
  - `JsEngine::set_hit_rects(&mut self, rects: Vec<HitRect>)` — replaces the engine's current snapshot.
  - Host binding `__ss_element_from_point(x, y) -> u32` (0 = no hit).
  - `sync_hit_rects_system` — pushes a fresh snapshot each frame.

- [ ] **Step 1: Write the failing unit tests** for `element_from_point` (in `lib.rs` under `#[cfg(test)]`, or `crates/superui_js/tests/`).

```rust
#[test] fn point_inside_single_node_returns_it() {
    // rects: [{js_id:2, min:(0,0), max:(10,10), depth:0}]
    // element_from_point(&rects, 5.0, 5.0) == Some(2)
}
#[test] fn overlapping_nodes_topmost_by_depth_wins() {
    // parent depth 0 covering (0,0)-(100,100), child depth 1 covering (10,10)-(20,20).
    // point (15,15) -> child (Some(child_js_id)); point (50,50) -> parent.
}
#[test] fn point_outside_all_nodes_is_none() {
    // point (500,500) over a (0,0)-(10,10) rect -> None.
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p superui_js element_from_point`
Expected: FAIL — `element_from_point`/`HitRect` not found.

- [ ] **Step 3: Implement `HitRect` + `element_from_point`** in `lib.rs`.

- [ ] **Step 4: Run the unit tests to verify they pass**

Run: `cargo test -p superui_js element_from_point`
Expected: PASS.

- [ ] **Step 5: Wire the host binding + engine storage.**
  - `V8Engine`: add `hit_rects: Vec<HitRect>` to `V8HostState`; implement `set_hit_rects` (write via `op_state`); add `op_ss_element_from_point(state, x: f64, y: f64) -> u32` reading `host.hit_rects` and returning `element_from_point(...).unwrap_or(0)`; register it in the `extension!` ops list and forward it in `BOOTSTRAP_JS` (`globalThis.__ss_element_from_point = function (x, y) { return ops.op_ss_element_from_point(+x, +y); };`).
  - `WebEngine`: add a `hit_rects: Rc<RefCell<Vec<HitRect>>>` field + an `_element_from_point` held closure that reads it and returns the js id as an `f64`/number (0 = none); `set_hit_rects` replaces the `RefCell` contents.
  - `dom.js`: `document.elementFromPoint = function (x, y) { var fn = globalThis.__ss_element_from_point; if (typeof fn !== "function") return null; var id = fn(+x, +y) >>> 0; return id ? (nodesById.get(id) || null) : null; };`.

- [ ] **Step 6: Implement `sync_hit_rects_system`** (NonSend). Query `(&DomNode, &ComputedNode, &GlobalTransform)`; for each, compute the AABB in the same logical-window-pixel space as `clientX`/`clientY` — center from `GlobalTransform` translation (`.translation().truncate()`), half-extent from `ComputedNode` size (`.size()`), `min = center - size/2`, `max = center + size/2` (confirm the exact `ComputedNode` size accessor at compile time); resolve `js_id` via `rt.applier.js(node)` (skip nodes with no js binding); compute `depth` as the node's DOM-tree depth (`rt.dom` walk `parent()` to the document root, counting). Call `rt.set_hit_rects(rects)` (add the passthrough to `runtime.rs`). Register in `mount.rs` under `runtime_exists`, ordered `.after(reconcile dependencies as the other bridge systems)` and `.before(drain_dom_events_system)`.

- [ ] **Step 7: Write + run the end-to-end `elementFromPoint` test** (extend `pointer_events.rs`): mount `<div id='a'></div>`, `app.update()` so it lays out and `sync_hit_rects_system` runs, then `run_script` a listener-free probe — `run_script("globalThis.hit = (function(){ var n = document.elementFromPoint(X, Y); return n ? n.id : null; })();")` using a point inside `#a`'s laid-out rect — and read `globalThis.hit` back via a mirrored attribute; assert it equals `"a"`. Also assert a far-off point reads `null`. (If headless layout makes real geometry awkward, instead push a known snapshot by invoking `sync_hit_rects_system`'s effect through a direct `rt.set_hit_rects(vec![HitRect{..}])` in the test before the probe.)

Run: `cargo test -p superui_js element_from_point && cargo test -p superui_bridge --test pointer_events`
Expected: PASS.

- [ ] **Step 8: Build the workspace**

Run: `cargo build -p superui`
Expected: success (confirms the `JsEngine` trait change compiles for both backends; web is behind a wasm cfg — if a local wasm build is impractical, note it and rely on CI).

- [ ] **Step 9: Commit**

```bash
git add crates/superui_js crates/superui_bridge crates/superui/src/mount.rs
git commit -m "feat(js): document.elementFromPoint via a host-binding AABB hit-test"
```

---

### Task 4: `setPointerCapture` registry, retarget, and implicit release

**Files:**
- Modify: `crates/superui_js/src/lib.rs` — add `CaptureOp` + `JsEngine::drain_capture_ops`.
- Modify: `crates/superui_js/src/engine_v8.rs` — `capture_ops` in `V8HostState`; `drain_capture_ops`; `op_ss_set_pointer_capture` / `op_ss_release_pointer_capture` + bootstrap wiring.
- Modify: `crates/superui_js/src/engine_web.rs` — `Rc<RefCell<Vec<CaptureOp>>>` + two held closures + `drain_capture_ops`.
- Modify: `crates/superui_js/js/dom.js` — `captureMap`, `pointerMeta`, `Node.prototype.setPointerCapture`/`releasePointerCapture`, retarget + implicit-release in `dispatch`, `gotpointercapture`/`lostpointercapture`.
- Modify: `crates/superui_bridge/src/events.rs` (or `pointer.rs`) — `drain_pointer_captures_system` populating `PointerCaptures`.
- Modify: `crates/superui_bridge/src/lib.rs` + `crates/superui/src/mount.rs` — export + register, ordered right after `drain_dom_events_system`.
- Test: `crates/superui_bridge/tests/pointer_events.rs` (extend).

**Interfaces:**
- Consumes: `PointerCaptures` (Task 2), `OpApplier::node`/`js` (map js↔mirror NodeId), `drain_dom_events_system` ordering.
- Produces (relied on by Task 5):
  - `#[derive(Clone, Copy, Debug, PartialEq)] pub struct CaptureOp { pub pointer_id: u32, pub node: Option<JsNodeId> }` (`Some` = set capture to that js node; `None` = release).
  - `JsEngine::drain_capture_ops(&mut self) -> Vec<CaptureOp>`.
  - `drain_pointer_captures_system` — applies drained ops to `PointerCaptures` (`Some` → insert `pointer_id -> applier.node(js)`; `None` → remove `pointer_id`).

- [ ] **Step 1: Write the failing tests** (extend `pointer_events.rs`; register the six observers, `drain_dom_events_system`, and `drain_pointer_captures_system`). Author JS uses `setPointerCapture` in a `pointerdown` listener.

```rust
#[test]
fn pointerdown_setpointercapture_registers_on_both_sides() {
    // DOM "<div id='a'></div><div id='b'></div>". Listener on #a 'pointerdown' calls
    //   e.target.setPointerCapture(e.pointerId). Dispatch a pointerdown (pointer_id 1) at #a,
    //   app.update() (drain + capture-drain). Assert PointerCaptures.0[&1] == node(#a).
}

#[test]
fn captured_pointer_event_retargets_to_the_capturing_node() {
    // After capturing pointer 1 on #a, dispatch a pointermove (pointer_id 1) whose target node
    //   is #b, with clientX/Y (40,50). A 'pointermove' listener on #a records e.target.id and
    //   e.clientX. Assert it fired with target "a" (retargeted) and clientX 40 (coords unchanged).
    //   A 'pointermove' listener on #b must NOT fire.
}

#[test]
fn pointerup_implicitly_releases_capture_and_fires_lostpointercapture() {
    // Capture pointer 1 on #a. Listener on #a 'lostpointercapture' sets a global flag.
    // Dispatch pointerup (pointer_id 1). app.update(). Assert the flag is set, PointerCaptures
    //   no longer contains key 1, and a second pointerup does nothing (flag count stays 1).
}

#[test]
fn explicit_releasepointercapture_clears_it() {
    // Capture pointer 1 on #a, then run_script e_target.releasePointerCapture(1) (via a stored
    //   handle or document.getElementById('a')). app.update(). Assert PointerCaptures loses key 1.
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p superui_bridge --test pointer_events`
Expected: FAIL — `setPointerCapture`/`drain_capture_ops`/`drain_pointer_captures_system` missing.

- [ ] **Step 3: Implement the JS capture surface** in `dom.js`:
  - Module-level `var captureMap = new Map();` (pointerId → node) and `var pointerMeta = new Map();` (pointerId → `{pointerType, isPrimary}`).
  - `proto.setPointerCapture = function (id) { id = id >>> 0; captureMap.set(id, this); if (typeof globalThis.__ss_set_pointer_capture === "function") globalThis.__ss_set_pointer_capture(id, this._nid); fireCapture(this, "gotpointercapture", id); };`
  - `proto.releasePointerCapture = function (id) { id = id >>> 0; if (captureMap.get(id) === this) releaseCapture(this, id); };` where `releaseCapture(node, id)` deletes the map entry, calls `__ss_release_pointer_capture(id)` (guarded), and `fireCapture(node, "lostpointercapture", id)`.
  - `function fireCapture(node, type, id)` dispatches `type` at `node._nid` through the same `dispatch` (bubbles true, not cancelable), passing an `init` carrying `pointerId: id` and the cached `pointerMeta` (pointerType/isPrimary) if present.
  - In `dispatch`, when `isPointer`: before building the ancestor walk, if `init && init.pointerId != null` record `pointerMeta.set(init.pointerId>>>0, {pointerType: init.pointerType, isPrimary: !!init.isPrimary})`; then if `captureMap.has(init.pointerId>>>0)` set `target = captureMap.get(init.pointerId>>>0)` (retarget; `clientX`/`clientY` untouched). After the dispatch loop, if `type === "pointerup"` and `captureMap.get(init.pointerId>>>0) === target` (post-retarget), call `releaseCapture(target, init.pointerId>>>0)` (implicit release → `lostpointercapture`).
  - Guard: `setPointerCapture` must be on the element prototype AND resolve on `document`/`window` safely (they share `proto` methods only where defined — capture is only ever called on elements in practice; no extra wiring needed).

- [ ] **Step 4: Implement the engine capture ops + trait method.** `CaptureOp` + `drain_capture_ops` in `lib.rs` (default trait body not needed — implement in both). V8: `capture_ops: Vec<CaptureOp>` in `V8HostState`; two ops pushing `CaptureOp`; `drain_capture_ops` takes them; bootstrap forwards `__ss_set_pointer_capture`/`__ss_release_pointer_capture`. Web: two closures pushing into an `Rc<RefCell<Vec<CaptureOp>>>`; `drain_capture_ops` takes them.

- [ ] **Step 5: Implement `drain_pointer_captures_system`** (NonSend): pull `rt.engine.drain_capture_ops()`, then for each op update `PointerCaptures` (map `js` → mirror `NodeId` via `rt.applier.node(js)`; skip unmapped). Export from `lib.rs`; register in `mount.rs` inside the `runtime_exists` chain immediately `.after(drain_dom_events_system)` so a capture set during this frame's dispatch lands in `PointerCaptures` the same frame.

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p superui_bridge --test pointer_events`
Expected: PASS.

- [ ] **Step 7: Build the workspace**

Run: `cargo build -p superui`
Expected: success.

- [ ] **Step 8: Commit**

```bash
git add crates/superui_js crates/superui_bridge crates/superui/src/mount.rs
git commit -m "feat(js): setPointerCapture registry, event retarget, implicit release"
```

---

### Task 5: Off-UI captured delivery via the raw-location system

**Files:**
- Modify: `crates/superui_bridge/src/events.rs` (or `pointer.rs`) — `CapturedPointerState` resource, the pure `captured_delivery` helper, and `captured_pointer_delivery_system`.
- Modify: `crates/superui_bridge/src/lib.rs` + `crates/superui/src/mount.rs` — export + register, ordered `.before(drain_dom_events_system)`.
- Test: `crates/superui_bridge/tests/pointer_events.rs` (extend).

**Interfaces:**
- Consumes: `PointerCaptures` (Task 2/4), `bevy_pointer_id`/`pointer_kind`/`pointer_is_primary`/`pointer_payload` (Task 2), `PrimaryTouch`, `PendingDomEvent`/`PendingDomEvents`, raw `PointerLocation` (mouse) + `Touches` (touch) + `ButtonInput<MouseButton>`.
- Produces:
  - `#[derive(Resource, Default)] pub struct CapturedPointerState(pub HashMap<u32, Vec2>);` — last-delivered position per captured JS pointerId (move-dedup).
  - `pub fn captured_delivery(captures: &PointerCaptures, positions: &HashMap<u32, Vec2>, released: &HashSet<u32>, canceled: &HashSet<u32>, last: &mut HashMap<u32, Vec2>, primary: &PrimaryTouch) -> Vec<PendingDomEvent>` — pure. For each captured `(pointer_id -> node)`: if `positions[pointer_id]` differs from `last[pointer_id]`, push a `pointermove` at `node` (buttons `1`, raw coords); if `pointer_id ∈ canceled` push `pointercancel`; if `pointer_id ∈ released` push `pointerup` (buttons `0`) and drop it from `last`; update `last`. Payload pointer fields via the Task-2 helpers (`bevy_pointer_id` → kind/primary).
  - `captured_pointer_delivery_system` — gathers the raw positions / released / canceled sets from `PointerLocation`+`Touches`+`ButtonInput<MouseButton>` and calls `captured_delivery`, extending `PendingDomEvents`.

- [ ] **Step 1: Write the failing tests** (extend `pointer_events.rs`), driving the pure helper directly (mirrors how `input_events.rs` tests `click_effect`).

```rust
#[test]
fn captured_move_off_ui_delivers_pointermove() {
    // captures {1 -> nodeA}. positions {1 -> (99,99)} (off any UI). last empty.
    // captured_delivery(...) yields one pointermove at nodeA, buttons 1, client (99,99).
    // last[1] == (99,99). A second call with the same position yields nothing (dedup).
}

#[test]
fn captured_release_delivers_pointerup_and_forgets_the_pointer() {
    // captures {1 -> nodeA}, positions {1 -> (99,99)}, released {1}.
    // Yields a pointermove (if moved) then a pointerup at nodeA (buttons 0); last loses key 1.
}

#[test]
fn uncaptured_pointer_yields_nothing() {
    // captures empty -> captured_delivery returns [].
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p superui_bridge --test pointer_events`
Expected: FAIL — `captured_delivery`/`CapturedPointerState` missing.

- [ ] **Step 3: Implement `CapturedPointerState`, `captured_delivery`, and `captured_pointer_delivery_system`.** The system builds the inputs: mouse pointerId `1`'s position from the `PointerId::Mouse` `PointerLocation` (as `mouse.rs::mouse_position` does) and `released`-membership from `ButtonInput<MouseButton>::just_released(MouseButton::Left)`; each touch's position/released/canceled from `Touches` (`iter`/`just_released`/`just_canceled` — confirm the exact API at compile time), keyed by `js_pointer_id(PointerId::Touch(id))`. Only pointers present in `PointerCaptures` are considered.

- [ ] **Step 4: Export + register.** Add to `lib.rs`; in `mount.rs` `.init_resource::<CapturedPointerState>()` and register `captured_pointer_delivery_system` in the `runtime_exists` chain `.before(drain_dom_events_system)` (so a raw `pointerup` dispatches — and implicitly releases capture — the same frame).

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p superui_bridge --test pointer_events`
Expected: PASS.

- [ ] **Step 6: Full bridge suite + build**

Run: `cargo test -p superui_bridge && cargo build -p superui`
Expected: success — no regression in existing mouse/input/touch tests.

- [ ] **Step 7: Commit**

```bash
git add crates/superui_bridge crates/superui/src/mount.rs
git commit -m "feat(bridge): deliver captured pointermove/up from the raw pointer location"
```

---

## Stage C — Example migration + integration test

### Task 6: Migrate `examples/inventory` to pointer events

**Files:**
- Modify: `examples/inventory/assets/ui/inventory/app.tsx` — pointer-event drag protocol + header/comment rewrite.
- Modify: `examples/inventory/assets/ui/inventory/style.css` — update the floating-tile `pointer-events` note comment (text only; no `pointer-events` CSS property is added — flair doesn't support it, and capture replaces the need).
- Rewrite: `examples/inventory/tests/inventory.rs` — pointer-driven headless drag tests (mouse + touch), replacing the mouse/`event.target` tests (the migrated app no longer listens to mouse, so the old `dragTo` tests cannot pass).
- Possibly regenerate: `examples/inventory/assets/ui/inventory/.superui/build/app.js` (the generated JS). Confirm whether the test path transpiles the TSX at runtime (`HostProject { tsx: true }`) — the existing `inventory.rs` passes `tsx: true` and transpiles in-process, so the checked-in `.superui/build/app.js` is NOT what the test runs; leave it or regenerate via the example's `build.rs` if a native `cargo run` path needs it.

**Interfaces:**
- Consumes: the full pointer stack (Tasks 1–5), `document.elementFromPoint`, `setPointerCapture`.

- [ ] **Step 1: Write the failing integration tests** in `examples/inventory/tests/inventory.rs`. Drive the drag **directly** with pointer events + real coordinates (do NOT use the test engine's mouse `dragTo`, which the migrated app ignores). Build the headless app from the real assets (`build_headless_app(&project())` as today), `app.update()` to lay out, then a helper flow per test:
  1. Resolve the `#inv` panel's node → its ECS entity → its `ComputedNode`+`GlobalTransform` center `(cx, cy)` (query the world).
  2. Resolve `#tile-sword`'s node and its center `(tx, ty)`.
  3. Enqueue a `pointerdown` (`PendingDomEvent::new(sword, "pointerdown").with_mouse(MousePayload{ client (tx,ty), button 0, buttons 1, pointer_id, pointer_type, is_primary:true })`) — the mouse case uses `pointer_id 1`/`Mouse`, the touch case uses `js_pointer_id(Touch(0))`/`Touch`. `app.update()` (the app's `onPointerDown` calls `setPointerCapture`; `drain_pointer_captures_system` registers it; `sync_hit_rects_system` has the geometry).
  4. Enqueue a `pointermove` at `(cx, cy)` then a `pointerup` at `(cx, cy)` (same pointer id/type). `app.update()` after each.
  5. Assert the sword moved from `#chest` to `#inv` by reading the mirror DOM (`rt.dom` / `document.getElementById` via `run_script` + mirrored attribute, or the existing locator helpers if reused read-only).

```rust
#[test]
fn mouse_drag_moves_sword_from_chest_to_inventory() { /* pointer_id 1 / Mouse */ }

#[test]
fn touch_drag_moves_sword_from_chest_to_inventory() { /* js_pointer_id(Touch(0)) / Touch; set PrimaryTouch so is_primary holds */ }

#[test]
fn drop_within_same_panel_is_a_noop() { /* drop over #chest -> sword stays in chest */ }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p inventory --test inventory`
Expected: FAIL — the app still uses `onMouseDown`/mouse listeners (pre-migration) and/or the new test harness references not-yet-written drag flow.

- [ ] **Step 3: Migrate `app.tsx`.** Replace the drag protocol:
  - `Tile`'s `onMouseDown` prop → `onPointerDown`; `startDrag` becomes an `onPointerDown` handler that, for a non-shift press, calls `e.target.setPointerCapture(e.pointerId)` and records `{ id, from, pointerId: e.pointerId }`. Keep the shift-click `quickMove` shortcut (read `e.shiftKey`).
  - `document.addEventListener("mousemove", onMove)`/`("mouseup", onUp)` → `("pointermove", onMove)`/`("pointerup", onUp)`; `removeEventListener` likewise. Keep the stable function identities.
  - `onUp` resolves the drop slot from `findSlot(document.elementFromPoint(e.clientX, e.clientY))` (NOT `e.target` — capture makes `e.target` the dragged tile). Keep the existing `findSlot` ancestor walk unchanged.
  - Rewrite the file header comment to describe the pointer-capture protocol (capture on `pointerdown`, document-level `pointermove`/`pointerup`, geometric drop resolution via `elementFromPoint`), and update the floating-tile comment in both `app.tsx` and `style.css` to explain that capture (not a `pointer-events` offset) is what keeps the drop target resolvable — but keep the positive `+12px` offset so the preview never sits under the pointer.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p inventory --test inventory`
Expected: PASS (mouse drag, touch drag, same-panel no-op).

- [ ] **Step 5: Build the example**

Run: `cargo build -p inventory`
Expected: success.

- [ ] **Step 6: Commit**

```bash
git add examples/inventory
git commit -m "feat(inventory): drive drag-and-drop through pointer events + capture"
```

---

### Task 7: Whole-feature verification

**Files:** none (verification only).

- [ ] **Step 1: Run the full affected test surface**

Run: `cargo test -p superui_js -p superui_bridge -p inventory`
Expected: PASS across pointer_events, element_from_point, the existing mouse/input/touch suites, and the inventory integration tests.

- [ ] **Step 2: Build the workspace**

Run: `cargo build`
Expected: success.

- [ ] **Step 3: Request a whole-branch code review** per superpowers:requesting-code-review, then address findings before the factory opens the PR. (Do NOT open or merge the PR yourself.)
