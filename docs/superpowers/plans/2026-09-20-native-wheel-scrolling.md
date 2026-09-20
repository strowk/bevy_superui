# Native Wheel Scrolling Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `overflow: scroll` nodes actually scroll under the mouse wheel (native-only — no JS involvement).

**Architecture:** Bevy 0.19's `bevy_ui` already clips scrollable nodes and offsets their children by a `ScrollPosition(Vec2)` component that layout auto-clamps to the valid range. Two wires are missing: (1) nothing inserts `ScrollPosition` on rendered nodes, (2) nothing reads `MouseWheel` to update it. We add `ScrollPosition::default()` at the reconciler's element-spawn chokepoint (alongside the existing `Hovered`), and a small pure-Bevy system that maps wheel deltas onto the hovered scrollable node's `ScrollPosition`.

**Tech Stack:** Rust, Bevy 0.19 (`bevy_ui`, `bevy_picking`, `bevy_input`), the `superui_bridge` crate.

**Spec:** N/A — bounded feature. The approved in-chat design is embedded in the Design section below.

## Design

- `overflow-y: scroll` already maps to `Node.overflow.y = OverflowAxis::Scroll` (`superui_flair_core/src/impls.rs:284-288`); clipping already works.
- `ScrollPosition(pub Vec2)` in `bevy_ui-0.19` is **inert on any node without an axis set to `Scroll`**, and layout **auto-clamps** it when content/layout changes — so we never clamp manually and injecting it everywhere is safe.
- Target resolution uses `bevy_picking`'s `HoverMap` resource (`HashMap<PointerId, EntityHashMap<HitData>>`), the same source the stock Bevy `ui/scroll.rs` example uses.
- Sign convention: `ScrollPosition` increases as content moves up; wheel-up gives positive `MouseWheel.y`; so we subtract the delta (`scroll.0.y -= dy`), matching browsers and the stock example.
- Shift+wheel scrolls horizontally (browser convention), gated on `ButtonInput<KeyCode>` shift keys.

## Global Constraints

- Bevy version is pinned to **0.19** workspace-wide (`Cargo.toml:15-30`). Use only 0.19 APIs verified in this plan.
- `MouseWheel` is a **Message** in 0.19 (read with `MessageReader<MouseWheel>`, registered by `InputPlugin`), not an `Event`.
- `ScrollPosition` is the tuple struct `ScrollPosition(pub Vec2)` — access the offset as `.0.x` / `.0.y`. It has `ScrollPosition::DEFAULT`.
- The new system must not touch `UiRuntime`, must not set the reconcile dirty flag, and must not require the DOM to be mounted (it is pure Bevy operating on `ScrollPosition` components).
- Commit messages: summary says what was done; body says *why* (project `CLAUDE.md`). Do not restate the diff in the body.

## Review Focus

- **Wheel over a non-scrollable node** (`overflow: visible`) must leave every `ScrollPosition` at zero — covered by Task 2's `wheel_over_non_scroll_node_does_nothing`.
- **Shift+wheel** must scroll a horizontal (`overflow-x: scroll`) node on the x axis, not the y — covered by Task 2's `shift_wheel_scrolls_horizontally`.
- **`Pixel` vs `Line` units**: `Pixel` deltas apply raw, `Line` deltas multiply by `LINE_HEIGHT` — covered by Task 2's `pixel_unit_applies_raw_delta`.
- **No hovered node** (empty `HoverMap`) must not panic and must scroll nothing — covered by Task 2's `wheel_with_no_hover_target_does_nothing`.
- **Over-scroll past content bounds** is delegated to Bevy's layout clamp; a full-app integration tick must keep the offset within `[0, max]` — covered by Task 3's `overscroll_is_clamped_by_layout`.

---

### Task 1: Inject `ScrollPosition` on element nodes at reconcile

**Files:**
- Modify: `crates/superui_bridge/src/reconcile.rs:8` (import) and `:145-152` (element spawn tuple)
- Test: `crates/superui_bridge/tests/reconcile.rs` (append a new test)

**Interfaces:**
- Consumes: nothing new.
- Produces: every element-node entity carries a `bevy::ui::ScrollPosition` component (default `Vec2::ZERO`), which Task 2's system mutates.

- [ ] **Step 1: Write the failing test**

Append to `crates/superui_bridge/tests/reconcile.rs`:

```rust
#[test]
fn element_nodes_get_a_scroll_position() {
    use bevy::ui::ScrollPosition;

    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<div class='scroller'><p>content</p></div>",
    )));
    let mut app = test_app();
    let root = mount(&mut app, dom.clone());
    app.update();

    // The <div> element entity must carry a ScrollPosition so the wheel system
    // has something to move; layout leaves it inert unless an axis is Scroll.
    let div = app.world_mut().get::<Children>(root).unwrap()[0];
    assert!(
        app.world().get::<ScrollPosition>(div).is_some(),
        "element node must have a ScrollPosition component"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p superui_bridge --test reconcile element_nodes_get_a_scroll_position`
Expected: FAIL — assertion fires because the element has no `ScrollPosition`.

- [ ] **Step 3: Add the import**

In `crates/superui_bridge/src/reconcile.rs`, extend the existing `bevy::ui` import at line 12 from:

```rust
use bevy::ui::{Checked, ComputedNode};
```

to:

```rust
use bevy::ui::{Checked, ComputedNode, ScrollPosition};
```

- [ ] **Step 4: Insert the component at the element spawn site**

In `crates/superui_bridge/src/reconcile.rs`, in the `NodeKind::Element(el) =>` spawn arm (currently `:145-152`), add `ScrollPosition::default()` to the spawned tuple, next to `Hovered::default()`:

```rust
        NodeKind::Element(el) => world
            .spawn((
                Node::default(),
                html_type_name(&el.tag),
                DomNode(child),
                Hovered::default(),
                // `ScrollPosition::default()`: the wheel-scroll system moves this
                // offset on hovered scrollable nodes. It is inert on any node whose
                // overflow is not `Scroll`, so attaching it to every element here
                // (the one spawn chokepoint, like `Hovered`) is safe and cheap.
                ScrollPosition::default(),
            ))
            .id(),
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p superui_bridge --test reconcile element_nodes_get_a_scroll_position`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/superui_bridge/src/reconcile.rs crates/superui_bridge/tests/reconcile.rs
git commit -m "feat(superui_bridge): attach ScrollPosition to element nodes

Bevy offsets a node's children by its ScrollPosition, but only ever
inserts the component where one already exists. Without it on our
reconciled nodes there is nothing for a wheel-scroll system to move, so
overflow:scroll clips content with no way to reach it. The component is
inert unless an axis is Scroll, so the element spawn chokepoint is the
right place, mirroring how Hovered is attached for :hover."
```

---

### Task 2: Wheel-scroll system

**Files:**
- Create: `crates/superui_bridge/src/scroll.rs`
- Modify: `crates/superui_bridge/src/lib.rs:7-10` (add `mod scroll;`) and `:19` area (re-export)
- Test: `crates/superui_bridge/tests/scroll.rs`

**Interfaces:**
- Consumes: element entities carrying `ScrollPosition` (Task 1) and `Node`.
- Produces: `pub fn wheel_scroll_system(...)` — a Bevy system, exported from the crate root as `superui_bridge::wheel_scroll_system`, that Task 3 registers.

- [ ] **Step 1: Write the system file with the implementation**

Create `crates/superui_bridge/src/scroll.rs`:

```rust
//! Mouse-wheel scrolling for `overflow: scroll` nodes. Bevy's layout offsets a
//! node's children by its `ScrollPosition` and auto-clamps that offset to the
//! scrollable range; this system is the only thing that *moves* it. Native-only:
//! the wheel never reaches JS, and no DOM event is dispatched.

use bevy::ecs::message::MessageReader;
use bevy::input::keyboard::KeyCode;
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::input::ButtonInput;
use bevy::picking::hover::HoverMap;
use bevy::picking::pointer::PointerId;
use bevy::prelude::*;
use bevy::ui::{OverflowAxis, ScrollPosition};

/// Logical pixels scrolled per `MouseScrollUnit::Line` notch.
const LINE_HEIGHT: f32 = 20.0;

/// Read this frame's wheel deltas and move the `ScrollPosition` of every
/// hovered node whose overflow is `Scroll` on the relevant axis. Shift swaps a
/// vertical wheel to horizontal (browser convention). The offset is subtracted
/// so wheel-up reveals content above, matching browsers; Bevy's layout clamps
/// the result to the valid range next frame.
pub fn wheel_scroll_system(
    mut wheel: MessageReader<MouseWheel>,
    hover_map: Res<HoverMap>,
    keys: Res<ButtonInput<KeyCode>>,
    mut scrollables: Query<(&Node, &mut ScrollPosition)>,
) {
    // Sum the frame's deltas into logical pixels.
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let (mut dx, mut dy) = (0.0f32, 0.0f32);
    for ev in wheel.read() {
        let mult = match ev.unit {
            MouseScrollUnit::Line => LINE_HEIGHT,
            MouseScrollUnit::Pixel => 1.0,
        };
        let (mut ex, mut ey) = (ev.x * mult, ev.y * mult);
        // Shift turns a purely vertical wheel into horizontal movement.
        if shift && ex == 0.0 {
            ex = ey;
            ey = 0.0;
        }
        dx += ex;
        dy += ey;
    }
    if dx == 0.0 && dy == 0.0 {
        return;
    }

    let Some(hovered) = hover_map.get(&PointerId::Mouse) else {
        return;
    };
    for (&entity, _hit) in hovered.iter() {
        if let Ok((node, mut scroll)) = scrollables.get_mut(entity) {
            if node.overflow.y == OverflowAxis::Scroll {
                scroll.0.y -= dy;
            }
            if node.overflow.x == OverflowAxis::Scroll {
                scroll.0.x -= dx;
            }
        }
    }
}
```

- [ ] **Step 2: Register the module and re-export**

In `crates/superui_bridge/src/lib.rs`, add the module declaration next to the others (after `mod runtime;`):

```rust
mod scroll;
```

and add the re-export next to `pub use reconcile::reconcile_system;`:

```rust
pub use scroll::wheel_scroll_system;
```

- [ ] **Step 3: Write the failing tests**

Create `crates/superui_bridge/tests/scroll.rs`:

```rust
//! Wheel-scroll system: maps MouseWheel deltas onto hovered scrollable nodes.
mod support;
use support::*;

use bevy::ecs::system::RunSystemOnce;
use bevy::input::keyboard::KeyCode;
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::input::ButtonInput;
use bevy::picking::backend::HitData;
use bevy::picking::hover::HoverMap;
use bevy::picking::pointer::PointerId;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::ui::{Node, Overflow, ScrollPosition};
use superui_bridge::wheel_scroll_system;

/// Spawn one node with the given overflow + a zero ScrollPosition, mark it the
/// single hovered entity, and queue one MouseWheel message. Returns the entity.
fn setup(app: &mut App, overflow: Overflow, wheel: MouseWheel) -> Entity {
    let e = app
        .world_mut()
        .spawn((
            Node {
                overflow,
                ..default()
            },
            ScrollPosition::default(),
        ))
        .id();

    let mut inner = bevy::ecs::entity::EntityHashMap::default();
    inner.insert(e, HitData::new(Entity::PLACEHOLDER, 0.0, None, None));
    let mut map = HashMap::default();
    map.insert(PointerId::Mouse, inner);
    app.world_mut().insert_resource(HoverMap(map));

    app.world_mut().write_message(wheel);
    e
}

fn scroll_of(app: &App, e: Entity) -> Vec2 {
    app.world().get::<ScrollPosition>(e).unwrap().0
}

#[test]
fn wheel_scrolls_a_vertical_scroll_node() {
    let mut app = test_app();
    let e = setup(
        &mut app,
        Overflow::scroll_y(),
        MouseWheel {
            unit: MouseScrollUnit::Line,
            x: 0.0,
            y: 1.0, // wheel up
            window: Entity::PLACEHOLDER,
            phase: bevy::input::touch::TouchPhase::Moved,
        },
    );
    app.world_mut().run_system_once(wheel_scroll_system).unwrap();
    // Wheel-up subtracts: offset goes negative by one line before layout clamps.
    assert_eq!(scroll_of(&app, e), Vec2::new(0.0, -20.0));
}

#[test]
fn wheel_over_non_scroll_node_does_nothing() {
    let mut app = test_app();
    let e = setup(
        &mut app,
        Overflow::visible(),
        MouseWheel {
            unit: MouseScrollUnit::Line,
            x: 0.0,
            y: 1.0,
            window: Entity::PLACEHOLDER,
            phase: bevy::input::touch::TouchPhase::Moved,
        },
    );
    app.world_mut().run_system_once(wheel_scroll_system).unwrap();
    assert_eq!(scroll_of(&app, e), Vec2::ZERO);
}

#[test]
fn pixel_unit_applies_raw_delta() {
    let mut app = test_app();
    let e = setup(
        &mut app,
        Overflow::scroll_y(),
        MouseWheel {
            unit: MouseScrollUnit::Pixel,
            x: 0.0,
            y: 13.0,
            window: Entity::PLACEHOLDER,
            phase: bevy::input::touch::TouchPhase::Moved,
        },
    );
    app.world_mut().run_system_once(wheel_scroll_system).unwrap();
    assert_eq!(scroll_of(&app, e), Vec2::new(0.0, -13.0));
}

#[test]
fn shift_wheel_scrolls_horizontally() {
    let mut app = test_app();
    let e = setup(
        &mut app,
        Overflow::scroll_x(),
        MouseWheel {
            unit: MouseScrollUnit::Pixel,
            x: 0.0,
            y: 10.0,
            window: Entity::PLACEHOLDER,
            phase: bevy::input::touch::TouchPhase::Moved,
        },
    );
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::ShiftLeft);
    app.world_mut().run_system_once(wheel_scroll_system).unwrap();
    // Shift maps the vertical delta onto x; y stays put.
    assert_eq!(scroll_of(&app, e), Vec2::new(-10.0, 0.0));
}

#[test]
fn wheel_with_no_hover_target_does_nothing() {
    let mut app = test_app();
    let e = app
        .world_mut()
        .spawn((
            Node {
                overflow: Overflow::scroll_y(),
                ..default()
            },
            ScrollPosition::default(),
        ))
        .id();
    // Empty hover map: nothing under the cursor.
    app.world_mut().insert_resource(HoverMap(HashMap::default()));
    app.world_mut().write_message(MouseWheel {
        unit: MouseScrollUnit::Line,
        x: 0.0,
        y: 1.0,
        window: Entity::PLACEHOLDER,
        phase: bevy::input::touch::TouchPhase::Moved,
    });
    app.world_mut().run_system_once(wheel_scroll_system).unwrap();
    assert_eq!(scroll_of(&app, e), Vec2::ZERO);
}
```

- [ ] **Step 4: Run tests to verify they fail, then pass**

Run: `cargo test -p superui_bridge --test scroll`
Expected: after Steps 1-2 the tests compile and PASS. If Step 1/2 were skipped they would fail to compile (`wheel_scroll_system` unresolved). Confirm all five PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/superui_bridge/src/scroll.rs crates/superui_bridge/src/lib.rs crates/superui_bridge/tests/scroll.rs
git commit -m "feat(superui_bridge): scroll hovered overflow:scroll nodes on wheel

bevy_ui clips scrollable nodes and offsets their children by
ScrollPosition but never drives that offset from input. This reads the
frame's MouseWheel deltas and moves the ScrollPosition of whichever
hovered node scrolls on that axis, so overflowing content is reachable.
Native-only by design: the wheel does not surface as a DOM event, and
bounds-clamping is left to Bevy's layout pass."
```

---

### Task 3: Wire the system into the plugin, verify integration, drop the known-issue

**Files:**
- Modify: `crates/superui/src/mount.rs:11` (import) and `:166-167` (registration)
- Modify: `website/src/docs/reference/known-issues.md:5-13` (remove the section)
- Test: `crates/superui_bridge/tests/scroll.rs` (append an integration test)

**Interfaces:**
- Consumes: `superui_bridge::wheel_scroll_system` (Task 2).
- Produces: the system runs every frame in `Update` under the real plugin.

- [ ] **Step 1: Write the failing integration test (layout clamp)**

Append to `crates/superui_bridge/tests/scroll.rs`:

```rust
#[test]
fn overscroll_is_clamped_by_layout() {
    use std::cell::RefCell;
    use std::rc::Rc;

    // A short scroll box with content taller than it: max_scroll_y > 0.
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<div style='height:40px;overflow-y:scroll'>\
           <div style='height:400px'></div>\
         </div>",
    )));
    let mut app = test_app();
    let root = mount(&mut app, dom.clone());
    app.add_systems(Update, wheel_scroll_system);
    app.update(); // reconcile + first layout

    let scroller = app.world_mut().get::<Children>(root).unwrap()[0];

    // Hover the scroller and scroll far past the bottom.
    let mut inner = bevy::ecs::entity::EntityHashMap::default();
    inner.insert(scroller, HitData::new(Entity::PLACEHOLDER, 0.0, None, None));
    let mut map = HashMap::default();
    map.insert(PointerId::Mouse, inner);
    app.world_mut().insert_resource(HoverMap(map));
    app.world_mut().write_message(MouseWheel {
        unit: MouseScrollUnit::Pixel,
        x: 0.0,
        y: -100000.0, // wheel down, absurdly far
        window: Entity::PLACEHOLDER,
        phase: bevy::input::touch::TouchPhase::Moved,
    });
    app.update(); // wheel system runs, then layout clamps

    let y = app.world().get::<ScrollPosition>(scroller).unwrap().0.y;
    assert!(y > 0.0, "should have scrolled down some");
    assert!(
        y <= 400.0 - 40.0 + 1.0,
        "layout must clamp within content bounds, got {y}"
    );
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p superui_bridge --test scroll overscroll_is_clamped_by_layout`
Expected: FAIL to compile — `wheel_scroll_system` is not yet added in this file's imports? It is (Task 2 imported it). It should compile and FAIL only if the system is not registered — but this test registers it locally, so it should PASS already, proving the clamp works. If it PASSES, that is the expected green; the failing-first here is degenerate (the assertion depends on Bevy layout, not on plugin wiring). Record the actual result.

> Note: this test validates the clamp behavior directly by adding the system to a local app; it does not depend on Task 3 Step 3's plugin wiring. Steps 3-4 wire the real plugin, which has no isolated unit test (registration is verified by build + manual run).

- [ ] **Step 3: Register the system in the real plugin**

In `crates/superui/src/mount.rs`, extend the `superui_bridge` import at line 11 to include `wheel_scroll_system`:

```rust
    keyboard_events_system, reconcile_system, wheel_scroll_system, PendingDomEvents, UiRuntime,
```

(Keep the other names already on that `use` line; just add `wheel_scroll_system`.)

Then register it in `build()`, right after the `on_pointer_click` observer registration (`:166`):

```rust
            .add_observer(on_pointer_click)
            // Wheel scrolling is pure Bevy (no UiRuntime), so it runs plainly in
            // Update rather than inside the runtime_exists DOM chain below.
            .add_systems(Update, wheel_scroll_system)
```

- [ ] **Step 4: Verify the workspace builds and all bridge tests pass**

Run: `cargo build -p superui`
Expected: builds clean.

Run: `cargo test -p superui_bridge`
Expected: all tests PASS (reconcile + scroll suites).

- [ ] **Step 5: Manual check in a real example**

Run an example whose UI has (or can be given) a short `overflow-y: scroll` container with content taller than it, and confirm the mouse wheel scrolls it while hovering. Suggested: add a temporary scroll box to `examples/game_menu` or use an existing scrollable view.

Run: `cargo run -p game_menu` (or another UI example)
Expected: wheeling over the overflow container moves its content; wheeling elsewhere does not. (Revert any temporary example edits after confirming.)

- [ ] **Step 6: Remove the "No scrolling" known issue**

In `website/src/docs/reference/known-issues.md`, delete the entire `## No scrolling` section (lines 5-13, from the `## No scrolling` heading through the blank line before `## No official teardown`). Leave the intro paragraph and the remaining sections intact.

- [ ] **Step 7: Commit**

```bash
git add crates/superui/src/mount.rs crates/superui_bridge/tests/scroll.rs website/src/docs/reference/known-issues.md
git commit -m "feat(superui): enable wheel scrolling and retire the no-scroll caveat

Register the wheel-scroll system in the plugin schedule so overflow:scroll
containers respond to the mouse in real apps, and add an integration test
confirming Bevy's layout clamps the offset to content bounds. The
known-issues entry that told users to paginate instead no longer holds."
```

---

## Notes on remaining limitations (do not fix here)

Documented as out of scope for this plan, matching the approved design:

- Nested scroll containers: every hovered scrollable node on the axis scrolls (near-always one); proper innermost-first-then-bubble is deferred.
- No smooth/momentum scrolling, no keyboard scrolling (PgUp/Down/arrows), no draggable scrollbar thumb.
- No JS `wheel`/`scroll` DOM events, no `preventDefault`, no programmatic `element.scrollTop` — the "JS-later" path.
