# Synthesize Mouse Events on Taps From Touch Devices — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** On a tap from the primary touch, synthesize the browser compatibility mouse-event sequence (`mousemove`/`mouseover`/`mouseenter`/`mousedown` on press, `mouseup` on release) so mouse-based handlers and `:hover`/`onMouseOver` styling respond to touch.

**Architecture:** Mirror the existing click seam. Two global `bevy_picking` observers in `crates/superui_bridge/src/events.rs` — one on `PointerPress`, one on `PointerRelease` — each filtered to `PointerId::Touch(_)`, resolve the hit entity to its nearest `DomNode` ancestor (same `resolve_dom_node` as `on_pointer_click`) and enqueue the synthesized events into the existing `PendingDomEvents` FIFO. A `PrimaryTouch(Option<PointerId>)` resource enforces the browser `isPrimary` rule so only the first active finger generates mouse events. `mouse_events_system` is left untouched.

**Tech Stack:** Rust, Bevy 0.20, `bevy_picking` 0.20.0, the superui DOM/JS bridge.

**Spec:** `docs/superpowers/specs/2026-11-23-touch-tap-mouse-events-design.md`

## Global Constraints

- Shell: use the Bash tool (Git Bash / POSIX sh), never PowerShell.
- No git worktrees (the `target` folder is huge).
- Picking event types in this Bevy version: `bevy::picking::events::PointerPress` and `bevy::picking::events::PointerRelease` (flat structs, NOT `Pointer<Pressed>`/`Pointer<Released>`). Each has fields `entity: Entity`, `pointer: Pointer`, `button: PointerButton`, `hit: HitData` (`PointerPress` additionally has `count: u8`). The touch location is `ev.pointer.position` (`Vec2`); the pointer id is `ev.pointer.id` (`PointerId`). `PointerId::Touch(u64)` is the touch variant.
- Synthesized events carry the touch pick location as `clientX`/`clientY` and empty modifiers (`Modifiers::default()` — touch carries no keyboard modifiers).
- W3C ordering is preserved by the `PendingDomEvents` FIFO: press-frame events (`mousemove`, `mouseover`, `mouseenter`, `mousedown`) enqueue in that order; `mouseup` enqueues on the release frame. `click` + focus are already emitted by the existing `on_pointer_click` observer and must not be re-emitted here.
- `button`/`buttons` are W3C values: press-phase hover events (`mousemove`/`mouseover`/`mouseenter`) carry `button` 0, `buttons` 0 (no button down yet); `mousedown` carries `button` 0, `buttons` 1; `mouseup` carries `button` 0, `buttons` 0.
- `bubbles`: `mouseover`, `mousemove`, `mousedown`, `mouseup` bubble (`PendingDomEvent::new` default `bubbles = true`); `mouseenter` does not (`bubbles = false`).

## Review Focus

- **Touch press on a host-game entity (pick resolves to no `DomNode`).** `resolve_dom_node` returns `None`; the observer must enqueue nothing AND must not consume the `PrimaryTouch` slot, or a later real tap on the UI would be silently dropped. Covered by a Task 1 test.
- **Synthesized events carry empty modifier state.** A JS listener reading `e.shiftKey` on a touch-synthesized `mousedown` must see `false`. Covered by a Task 1 assertion.

---

### Task 1: Primary-touch press synthesis

**Files:**
- Modify: `crates/superui_bridge/src/events.rs` — add the `PrimaryTouch` resource, the `on_touch_press` observer, and the shared enqueue helpers.
- Modify: `crates/superui_bridge/src/lib.rs` — export `PrimaryTouch` and `on_touch_press`.
- Modify: `crates/superui/src/mount.rs` — `init_resource::<PrimaryTouch>()` and `add_observer(on_touch_press)`.
- Test: `crates/superui_bridge/tests/touch_tap.rs` (new).

**Interfaces:**
- Consumes: `resolve_dom_node(entity, &nodes, &parents) -> Option<NodeId>`, `PendingDomEvent::new(NodeId, &str)`, `.with_mouse(MousePayload)`, `.with_modifiers(Modifiers)`, `PendingDomEvents(pub Vec<PendingDomEvent>)` — all existing in `events.rs`.
- Produces (relied on by Task 2):
  - `#[derive(Resource, Default)] pub struct PrimaryTouch(pub Option<PointerId>);`
  - `fn touch_payload(pos: Vec2, button: u8, buttons: u8) -> MousePayload`
  - `fn enqueue_touch_press(node: NodeId, pos: Vec2, pending: &mut PendingDomEvents)`
  - `pub fn on_touch_press(mut ev: On<PointerPress>, nodes: Query<&DomNode>, parents: Query<&ChildOf>, mut pending: ResMut<PendingDomEvents>, mut primary: ResMut<PrimaryTouch>)`

- [ ] **Step 1: Write the failing tests** in `crates/superui_bridge/tests/touch_tap.rs`.

Harness: mirror `input_events.rs`'s real-`PointerClick` pattern. Build an app with `test_app()`, `mount` the DOM, `init_resource::<PendingDomEvents>()`, `app.add_observer(on_touch_press)`, `app.init_resource::<PrimaryTouch>()`, then `app.update()` once so the DOM reconciles and nodes get ECS entities. Trigger presses with `app.world_mut().trigger(PointerPress { entity, pointer: Pointer::new(PointerId::Touch(id), Location { target, position }), button: PointerButton::Primary, hit: HitData::new(camera, 0.0, None, None), count: 1 })`, building `target` the way `input_events.rs` does (spawn `Camera2d`, normalize the `PrimaryWindow`). A helper `entity_of(app, dom, id)` resolves a DOM id to its ECS entity via `UiRuntime::entity_for` (copy from `mouse_events.rs`). Assert directly against `app.world().resource::<PendingDomEvents>().0` (no drain needed).

```rust
#[test]
fn primary_tap_press_enqueues_the_compat_sequence() {
    // DOM "<div id='a'></div>"; trigger PointerPress on #a's entity at position (12, 34).
    // PendingDomEvents is exactly [mousemove, mouseover, mouseenter, mousedown], in order.
    // For each: mouse.client_x == 12.0, mouse.client_y == 34.0.
    // button/buttons: mousemove/mouseover/mouseenter -> (0, 0); mousedown -> (0, 1).
    // bubbles: mouseenter == false; the other three == true.
    // modifiers: Some(Modifiers::default()) on each (shift/ctrl/alt/meta all false).
}

#[test]
fn secondary_concurrent_touch_press_enqueues_nothing() {
    // Trigger PointerPress Touch(1) on #a, then Touch(2) on #a without any release.
    // After the first press PendingDomEvents has 4 events; the second press adds none
    // (len stays 4) — only the primary finger synthesizes.
}

#[test]
fn mouse_press_enqueues_nothing_from_touch_observer() {
    // Trigger PointerPress with PointerId::Mouse on #a.
    // PendingDomEvents is empty — the mouse path stays the sole mouse source.
}

#[test]
fn touch_press_on_a_foreign_entity_enqueues_nothing_and_leaves_primary_free() {
    // Spawn a plain Node entity (no DomNode). Trigger PointerPress Touch(1) on it.
    // PendingDomEvents is empty AND PrimaryTouch.0 is still None, so a following real
    // tap still synthesizes: trigger PointerPress Touch(2) on #a -> 4 events enqueued.
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p superui_bridge --test touch_tap`
Expected: compile error / FAIL — `PrimaryTouch` and `on_touch_press` not found.

- [ ] **Step 3: Implement the resource, helpers, and observer** in `crates/superui_bridge/src/events.rs`.

Add imports: `use bevy::picking::events::PointerPress;` and `use bevy::picking::pointer::PointerId;` (`Vec2` comes from the existing `bevy::prelude::*`).

```rust
#[derive(Resource, Default)]
pub struct PrimaryTouch(pub Option<PointerId>);

fn touch_payload(pos: Vec2, button: u8, buttons: u8) -> MousePayload {
    MousePayload { client_x: pos.x, client_y: pos.y, button, buttons }
}

// mousemove, mouseover (bubbles), mouseenter (no bubble), mousedown (button 0,
// buttons 1) — in that order. mouseenter sets bubbles = false; empty modifiers.
fn enqueue_touch_press(node: NodeId, pos: Vec2, pending: &mut PendingDomEvents) { /* ... */ }

pub fn on_touch_press(
    mut ev: On<PointerPress>,
    nodes: Query<&DomNode>,
    parents: Query<&ChildOf>,
    mut pending: ResMut<PendingDomEvents>,
    mut primary: ResMut<PrimaryTouch>,
) {
    // 1. Touch only; a mouse/custom pointer returns immediately (no propagate change).
    let PointerId::Touch(_) = ev.pointer.id else { return };
    // 2. Resolve BEFORE touching PrimaryTouch — a None resolve (host-game entity)
    //    returns without claiming the event or consuming the primary slot.
    let Some(node) = resolve_dom_node(ev.entity, &nodes, &parents) else { return };
    // 3. Ours: stop propagation so this fires once (mirror on_pointer_click).
    ev.propagate(false);
    // 4. Adopt the first active finger as primary; ignore a second concurrent finger.
    if primary.0.is_some() { return; }
    primary.0 = Some(ev.pointer.id);
    enqueue_touch_press(node, ev.pointer.position, &mut pending);
}
```

- [ ] **Step 4: Wire exports and registration.**

In `crates/superui_bridge/src/lib.rs`, add `on_touch_press` and `PrimaryTouch` to the `pub use events::{...}` list.

In `crates/superui/src/mount.rs`, add `.init_resource::<PrimaryTouch>()` alongside the other `init_resource` calls and `.add_observer(on_touch_press)` alongside the existing observers; add `PrimaryTouch` and `on_touch_press` to the `use superui_bridge::{...}` import.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p superui_bridge --test touch_tap`
Expected: PASS (all four tests).

- [ ] **Step 6: Build the workspace to confirm the wiring compiles**

Run: `cargo build -p superui`
Expected: success.

- [ ] **Step 7: Commit**

```bash
git add crates/superui_bridge/src/events.rs crates/superui_bridge/src/lib.rs crates/superui/src/mount.rs crates/superui_bridge/tests/touch_tap.rs
git commit -m "feat(bridge): synthesize mousedown sequence on primary touch press"
```

---

### Task 2: Primary-touch release synthesis

**Files:**
- Modify: `crates/superui_bridge/src/events.rs` — add the `on_touch_release` observer and the release enqueue helper.
- Modify: `crates/superui_bridge/src/lib.rs` — export `on_touch_release`.
- Modify: `crates/superui/src/mount.rs` — `add_observer(on_touch_release)`.
- Test: `crates/superui_bridge/tests/touch_tap.rs` (extend).

**Interfaces:**
- Consumes (from Task 1): `PrimaryTouch`, `touch_payload`, `resolve_dom_node`, `PendingDomEvent`/`PendingDomEvents`, the `enqueue_touch_press` helper (reused by the full-tap test).
- Produces:
  - `fn enqueue_touch_release(node: NodeId, pos: Vec2, pending: &mut PendingDomEvents)`
  - `pub fn on_touch_release(mut ev: On<PointerRelease>, nodes: Query<&DomNode>, parents: Query<&ChildOf>, mut pending: ResMut<PendingDomEvents>, mut primary: ResMut<PrimaryTouch>)`

- [ ] **Step 1: Write the failing tests** (extend `crates/superui_bridge/tests/touch_tap.rs`; register `on_touch_release` in the test harness too, and import `PointerRelease`).

```rust
#[test]
fn primary_release_enqueues_mouseup() {
    // Register both observers. Trigger PointerPress Touch(1) on #a at (5, 6), then
    // PointerRelease Touch(1) on #a at (5, 6). The last enqueued event is "mouseup"
    // at #a: button 0, buttons 0, bubbles true, client_x 5.0 / client_y 6.0,
    // modifiers Some(default). (The press already enqueued the 4-event sequence,
    // so the full queue is [mousemove, mouseover, mouseenter, mousedown, mouseup].)
}

#[test]
fn following_touch_becomes_new_primary_after_release() {
    // Press+release Touch(1) on #a (clears PrimaryTouch). Then press Touch(2) on #a:
    // it is adopted as the new primary and enqueues a fresh mousedown sequence
    // (the queue grows by 4 after the second press).
}

#[test]
fn non_primary_release_is_ignored() {
    // Press Touch(1) on #a (adopted as primary). Trigger PointerRelease Touch(2) on #a.
    // No mouseup is enqueued (queue length unchanged) and PrimaryTouch.0 stays Some(Touch(1)).
}

#[test]
fn mouse_release_enqueues_nothing_from_touch_observer() {
    // Trigger PointerRelease with PointerId::Mouse on #a. PendingDomEvents is empty.
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p superui_bridge --test touch_tap`
Expected: FAIL — `on_touch_release` not found.

- [ ] **Step 3: Implement the release helper and observer** in `crates/superui_bridge/src/events.rs`.

Add `use bevy::picking::events::PointerRelease;`.

```rust
// mouseup (button 0, buttons 0), bubbles (default), empty modifiers.
fn enqueue_touch_release(node: NodeId, pos: Vec2, pending: &mut PendingDomEvents) { /* ... */ }

pub fn on_touch_release(
    mut ev: On<PointerRelease>,
    nodes: Query<&DomNode>,
    parents: Query<&ChildOf>,
    mut pending: ResMut<PendingDomEvents>,
    mut primary: ResMut<PrimaryTouch>,
) {
    let PointerId::Touch(_) = ev.pointer.id else { return };
    let Some(node) = resolve_dom_node(ev.entity, &nodes, &parents) else { return };
    ev.propagate(false);
    // Only the primary finger's release synthesizes; a non-primary release is ignored.
    if primary.0 != Some(ev.pointer.id) { return; }
    enqueue_touch_release(node, ev.pointer.position, &mut pending);
    primary.0 = None;
}
```

- [ ] **Step 4: Wire exports and registration.**

Add `on_touch_release` to `pub use events::{...}` in `lib.rs`, to the `use superui_bridge::{...}` import in `mount.rs`, and `.add_observer(on_touch_release)` alongside `on_touch_press`.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p superui_bridge --test touch_tap`
Expected: PASS (all tests, Task 1 + Task 2).

- [ ] **Step 6: Build and run the full bridge test suite**

Run: `cargo test -p superui_bridge && cargo build -p superui`
Expected: success — no regression in existing input/mouse tests.

- [ ] **Step 7: Commit**

```bash
git add crates/superui_bridge/src/events.rs crates/superui_bridge/src/lib.rs crates/superui/src/mount.rs crates/superui_bridge/tests/touch_tap.rs
git commit -m "feat(bridge): synthesize mouseup on primary touch release"
```
