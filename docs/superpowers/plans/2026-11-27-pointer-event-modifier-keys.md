# Pointer/Touch Event Modifier Keys Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make every synthesized pointer and touch-compat DOM event report the live keyboard modifier state, and give the test engine the modifier + pointer-event input needed to prove shift-click quick-move works in `examples/inventory`.

**Architecture:** Two independent sites in `superui_bridge/src/events.rs` hardcode `Modifiers::default()`; replace both with `modifiers_now(&keys)` sampled from `Res<ButtonInput<KeyCode>>` at the observer, mirroring the existing mouse/click path. The test engine gains a per-action `modifiers` option and three new pointer actions (`pointerDown`/`pointerMove`/`pointerUp`), threaded through the JS prelude, the `Command` enum, and both near-identical drivers (`driver.rs`, `ui_driver.rs`).

**Tech Stack:** Rust (Bevy 0.20 ECS observers, `bevy::input::ButtonInput<KeyCode>`), serde-tagged command JSON, a pure-JS Playwright-shaped prelude.

**Spec:** `docs/superpowers/specs/2026-11-25-pointer-event-modifier-keys-design.md`

## Global Constraints

- No change to `modifiers_now`, the `Modifiers` type, `dom.js`, or the `EventInit` plumbing. Reuse `modifiers_now(&keys)` inline — no new `CurrentModifiers` resource, no folding the keyboard read into the payload/event builders.
- No change to `examples/inventory/assets/ui/inventory/app.tsx`. The demo is already correct.
- No `page.keyboard.down/up` persistent held-modifier state; the per-action `modifiers` option is the whole surface.
- Modifier names use Playwright's spelling: `'Shift'`, `'Control'`, `'Alt'`, `'Meta'`. Unknown names are ignored (not an error).
- Pointer-action defaults: `pointerId: 1`, `pointerType: 'mouse'`, `x: 0`, `y: 0`.
- New `modifiers` option and pointer actions are additive: existing calls pass no modifiers and behave exactly as before. Both drivers change identically.
- Commit messages: summary says what was done; body says why (not a restatement of the diff).

## Review Focus

- **Multiple modifiers in one array** (`['Shift','Control']`) → both `shiftKey` and `ctrlKey` arrive `true`. Pinned in Task 2's test.
- **Unknown modifier name** (`['Hyper']`) → ignored, no panic, recognized siblings still apply. Pinned in Task 2's test.
- **`dragTo({modifiers})`** → the modifier reaches the `mouseup` (drop) event, not only the `mousedown`. Pinned in Task 2's test.
- **`pointerType: 'touch'`** → the listener sees `e.pointerType === 'touch'`. Pinned in Task 3's test.
- **Omitted pointer fields** → `pointerDown()` with no options still fires a valid `pointerdown` (`pointerId` 1, `pointerType` 'mouse'). Pinned in Task 3's test.

---

## File Structure

- `crates/superui_bridge/src/events.rs` — the two modifier defect sites (6 pointer observers, 2 touch observers, 2 touch helpers, 2 doc comments). Modified in Task 1.
- `crates/superui_bridge/tests/pointer_events.rs`, `.../touch_tap.rs` — bridge regression tests. Extended in Task 1.
- `crates/superui_test_engine/src/prelude.js` — locator methods serialize the new `modifiers` option and the three pointer commands. Modified in Tasks 2, 3.
- `crates/superui_test_engine/src/command.rs` — `Command` variants gain `modifiers`; three new pointer variants. Modified in Tasks 2, 3.
- `crates/superui_test_engine/src/driver.rs`, `.../ui_driver.rs` — `parse_modifiers`, `Modifiers` carried on in-flight actions, effect fns apply it; new pointer effect fn. Modified identically in Tasks 2, 3.
- `crates/superui_test_engine/tests/modifiers.rs` — new, Task 2.
- `crates/superui_test_engine/tests/pointer_actions.rs` — new, Task 3.
- `examples/inventory/tests/shift_click.rs` — new, Task 4.

---

### Task 1: Bridge sources live keyboard modifiers at every synthesized-event site

**Files:**
- Modify: `crates/superui_bridge/src/events.rs`
- Test: `crates/superui_bridge/tests/pointer_events.rs` (the `pointer_observers` mod), `crates/superui_bridge/tests/touch_tap.rs`

**Interfaces:**
- Consumes: `modifiers_now(&keys: &ButtonInput<KeyCode>) -> Modifiers` (already in `events.rs`); `PendingDomEvent::with_modifiers(Modifiers)`.
- Produces: observer fns `on_pointer_over/out/press/release/move/cancel` and `on_touch_press/on_touch_release` each gain a `keys: Res<ButtonInput<KeyCode>>` param (public signatures used only via `add_observer`, so no caller updates). Helpers `enqueue_touch_press(node, pos, mods: Modifiers, pending)` and `enqueue_touch_release(node, pos, mods: Modifiers, pending)` gain a `mods` parameter before `pending`.

- [ ] **Step 1: Write the failing bridge tests**

In `crates/superui_bridge/tests/pointer_events.rs`, inside `mod pointer_observers`, add a test that drives a mouse `PointerPress` with `KeyCode::ShiftLeft` held and asserts the enqueued `pointerdown` carries `shift: true`, and a second assertion with no key held asserting `false`:

```rust
#[test]
fn pointer_press_carries_live_shift_modifier() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document("<div id='a'></div>")));
    let mut app = test_app();
    mount_pointer(&mut app, dom.clone());
    app.update();
    let a = entity_of(&app, &dom, "a");

    // Shift held when the press fires -> pointerdown reports shiftKey true.
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::ShiftLeft);
    trigger_press(&mut app, a, PointerId::Mouse, Vec2::new(1.0, 1.0));
    let events = &app.world().resource::<PendingDomEvents>().0;
    assert_eq!(events.last().unwrap().modifiers.unwrap().shift, true);

    // Release and re-press: no modifier held -> shiftKey false.
    app.world_mut().resource_mut::<PendingDomEvents>().0.clear();
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(KeyCode::ShiftLeft);
    trigger_press(&mut app, a, PointerId::Mouse, Vec2::new(1.0, 1.0));
    let events = &app.world().resource::<PendingDomEvents>().0;
    assert_eq!(events.last().unwrap().modifiers.unwrap().shift, false);
}
```

(Use the existing `Modifiers` import; add `superui_bridge::Modifiers` to the mod's `use` if missing. `trigger_press` and `mount_pointer` already exist in this mod.)

In `crates/superui_bridge/tests/touch_tap.rs`, add a test: hold `KeyCode::ShiftLeft`, fire a primary-touch `press`, and assert the synthesized `mousedown` (and the idle preamble events) carry `shift: true`:

```rust
#[test]
fn primary_tap_carries_live_shift_modifier() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document("<div id='a'></div>")));
    let mut app = test_app();
    mount_touch(&mut app, dom.clone());
    app.update();
    let a = entity_of(&app, &dom, "a");

    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::ShiftLeft);
    press(&mut app, a, PointerId::Touch(1), Vec2::new(1.0, 1.0));

    let events = &app.world().resource::<PendingDomEvents>().0;
    for e in events.iter() {
        assert_eq!(e.modifiers.unwrap().shift, true, "event {} must carry live shift", e.type_);
    }
}
```

(`ButtonInput` and `KeyCode` come from `bevy::prelude::*`, already imported in both files.)

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p superui_bridge --test pointer_events --test touch_tap`
Expected: FAIL — the new tests see `shift: false` because the observers hardcode `Modifiers::default()`.

- [ ] **Step 3: Thread live modifiers through all eight observers and both helpers**

In `crates/superui_bridge/src/events.rs`:
- Add `keys: Res<ButtonInput<KeyCode>>` to the param list of `on_pointer_over`, `on_pointer_out`, `on_pointer_press`, `on_pointer_release`, `on_pointer_move`, `on_pointer_cancel`, `on_touch_press`, `on_touch_release`.
- In each of the six pointer observers, replace `Modifiers::default()` with `modifiers_now(&keys)`. (`on_pointer_over`/`on_pointer_out` already bind `let mods = ...`; set that to `modifiers_now(&keys)`. The other four inline `Modifiers::default()` in the `push` call; replace with `modifiers_now(&keys)`.)
- In `on_touch_press`/`on_touch_release`, compute `let mods = modifiers_now(&keys);` and pass `mods` into `enqueue_touch_press`/`enqueue_touch_release`.
- Change `enqueue_touch_press` and `enqueue_touch_release` to take `mods: Modifiers` (parameter before `pending`) instead of constructing `let mods = Modifiers::default();`. Apply `mods` to every emitted event (unchanged bodies otherwise — the idle `mousemove`/`mouseover`/`mouseenter` preamble and `mousedown`/`mouseup` already use the local `mods`).
- Rewrite the `enqueue_touch_press`/`enqueue_touch_release` doc comments (the "touch has no keyboard-modifier concept of its own" and "All carry empty `Modifiers`" lines) to state the sequence carries the live keyboard modifiers.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p superui_bridge --test pointer_events --test touch_tap`
Expected: PASS, including the pre-existing `touch_tap` tests that assert `Some(Modifiers::default())` (they hold no key, so `modifiers_now` returns the default).

- [ ] **Step 5: Run the full bridge suite to confirm nothing regressed**

Run: `cargo test -p superui_bridge`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/superui_bridge/src/events.rs crates/superui_bridge/tests/pointer_events.rs crates/superui_bridge/tests/touch_tap.rs
git commit -m "fix(bridge): sample live keyboard modifiers for pointer and touch-compat events"
```

---

### Task 2: Test engine per-action `modifiers` option

**Files:**
- Modify: `crates/superui_test_engine/src/prelude.js`, `crates/superui_test_engine/src/command.rs`, `crates/superui_test_engine/src/driver.rs`, `crates/superui_test_engine/src/ui_driver.rs`
- Test: `crates/superui_test_engine/tests/modifiers.rs` (new)

**Interfaces:**
- Consumes: `superui_bridge::Modifiers`, `superui_bridge::click_effect(rt, node, Modifiers, pending)` (already takes `Modifiers`).
- Produces: `pub(crate) fn parse_modifiers(names: &[String]) -> Modifiers` in `driver.rs`, called from `ui_driver.rs` as `crate::driver::parse_modifiers`. `ActionInFlight` (both drivers) gains `mods: Modifiers`. `perform_action(..., mods: Modifiers, ...)`, `dispatch(app/world, spec, event, mods)`, `push_mouse_event(app/world, node, event, bubbles, buttons, mods)`, `hover(app/world, spec, mods, last_hovered)`, `mouse_dispatch(app/world, spec, event, buttons, mods)` all take and apply `mods`.

- [ ] **Step 1: Write the failing test**

Create `crates/superui_test_engine/tests/modifiers.rs`. Fixture: an `onClick` handler that writes `e.shiftKey`/`e.ctrlKey` into visible text (mirror `tests/press_key.rs`'s signal-to-text pattern). Also a `dragTo` drop target recording `e.shiftKey` on `mouseup`.

```rust
use superui_test_engine::driver::run_spec;
use superui_test_engine::host::{build_headless_app, HostProject};
use superui_test_engine::transpile::transpile_spec;

fn project() -> HostProject {
    HostProject {
        html: "<html><head><link rel=\"stylesheet\" href=\"style.css\"><script type=\"module\" src=\"app.tsx\"></script></head><body><div id=\"root\"></div></body></html>".into(),
        css: String::new(),
        js_or_tsx: r#"
            import { createSignal, render } from "supersolid";
            function App() {
                const [click, setClick] = createSignal("none");
                const [drop, setDrop] = createSignal("none");
                return <div>
                    <div id="btn" onClick={(e) => setClick(`${e.shiftKey}/${e.ctrlKey}`)}>btn</div>
                    <div id="src">src</div>
                    <div id="dst" onMouseUp={(e) => setDrop(String(e.shiftKey))}>dst</div>
                    <div id="click">{click()}</div>
                    <div id="drop">{drop()}</div>
                </div>;
            }
            render(App, document.getElementById("root"));
        "#.into(),
        tsx: true,
        mount_root: "ui".into(),
        extra_assets: vec![],
        ..Default::default()
    }
}

#[test]
fn click_and_drag_carry_modifiers() {
    let mut app = build_headless_app(&project());
    let spec = r##"
        import { test, expect } from "superui/test";
        test("plain click has no modifiers", async ({ page }) => {
            await page.locator("#btn").click();
            await expect(page.locator("#click")).toHaveText("false/false");
        });
        test("click carries Shift and Control, ignores unknown", async ({ page }) => {
            await page.locator("#btn").click({ modifiers: ["Shift", "Control", "Hyper"] });
            await expect(page.locator("#click")).toHaveText("true/true");
        });
        test("dragTo carries the modifier through to the drop", async ({ page }) => {
            await page.locator("#src").dragTo(page.locator("#dst"), { modifiers: ["Shift"] });
            await expect(page.locator("#drop")).toHaveText("true");
        });
    "##;
    let js = transpile_spec(spec, "t.spec.ts", std::path::Path::new(".")).unwrap();
    let results = run_spec(&mut app, &js);
    assert_eq!(results.len(), 3);
    for r in &results { assert!(r.passed, "{}: {:?}", r.name, r.error); }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p superui_test_engine --test modifiers`
Expected: FAIL — the prelude drops the options object and the drivers dispatch with `Modifiers::default()`.

- [ ] **Step 3: Serialize the `modifiers` option in the prelude**

In `prelude.js`, make `click`, `mouseDown`, `mouseUp`, `hover`, `moveTo`, and `dragTo` accept a trailing `opts` object and include `modifiers: (opts && opts.modifiers) || []` in the enqueued command. For `dragTo(target, opts)` the modifiers come from `opts`. Example for `click`:

```js
async click(opts) { return enqueue({ type: "click", locator: serialize(this), modifiers: (opts && opts.modifiers) || [] }); },
```

- [ ] **Step 4: Add `modifiers` to the `Command` variants**

In `command.rs`, add `#[serde(default)] modifiers: Vec<String>` to `Click`, `MouseDown`, `MouseUp`, `Hover`, `MoveTo`, and `DragTo`.

- [ ] **Step 5: Implement `parse_modifiers` and apply modifiers in `driver.rs`**

In `driver.rs`:
- Add `pub(crate) fn parse_modifiers(names: &[String]) -> Modifiers` mapping `"Shift"→shift`, `"Control"→ctrl`, `"Alt"→alt`, `"Meta"→meta`, folding over `names` and ignoring anything else.
- Add `mods: Modifiers` to `ActionInFlight`. In each `Command::{Click,Hover,MouseDown,MouseUp,MoveTo}` arm, compute `parse_modifiers(modifiers)` and pass it to `start_action` (add a `mods` param to `start_action`, stored on `ActionInFlight` and forwarded to the immediate `perform_action` call). For `DragTo`, parse `modifiers` the same way.
- Thread `mods` into `perform_action(app, spec, kind, mods, last_hovered)`, then into `dispatch`, `hover`, `mouse_dispatch`, and `push_mouse_event` (each gains a `mods: Modifiers` param). `push_mouse_event` adds `.with_modifiers(mods)` to the built `PendingDomEvent`. The `click` branch of `dispatch` passes `mods` into `click_effect` instead of `Modifiers::default()`.

- [ ] **Step 6: Mirror the same changes in `ui_driver.rs`**

Apply the identical changes to `ui_driver.rs` (`ActionInFlight.mods`, `start_action`, `perform_action`, `dispatch`, `hover`, `mouse_dispatch`, `push_mouse_event`). Call `crate::driver::parse_modifiers` rather than redefining it.

- [ ] **Step 7: Run the test to verify it passes**

Run: `cargo test -p superui_test_engine --test modifiers`
Expected: PASS.

- [ ] **Step 8: Run the full test-engine suite**

Run: `cargo test -p superui_test_engine`
Expected: PASS (existing `actions.rs`, `press_key.rs`, `inworld_stepper.rs` etc. unchanged — they pass no modifiers).

- [ ] **Step 9: Commit**

```bash
git add crates/superui_test_engine/src/prelude.js crates/superui_test_engine/src/command.rs crates/superui_test_engine/src/driver.rs crates/superui_test_engine/src/ui_driver.rs crates/superui_test_engine/tests/modifiers.rs
git commit -m "feat(test-engine): per-action keyboard modifier option"
```

---

### Task 3: Test engine `pointerDown`/`pointerMove`/`pointerUp` actions

**Files:**
- Modify: `crates/superui_test_engine/src/prelude.js`, `crates/superui_test_engine/src/command.rs`, `crates/superui_test_engine/src/driver.rs`, `crates/superui_test_engine/src/ui_driver.rs`
- Test: `crates/superui_test_engine/tests/pointer_actions.rs` (new)

**Interfaces:**
- Consumes: `parse_modifiers` (Task 2), `superui_bridge::{MousePayload, PendingDomEvent, PendingDomEvents}`, `superui_js::PointerKind`.
- Produces: three new `Command` variants `PointerDown`/`PointerMove`/`PointerUp`, each with `#[serde(default)] modifiers: Vec<String>`, `#[serde(default = ...)] pointer_id: u32`, `#[serde(default = ...)] pointer_type: String`, `#[serde(default)] x: f32`, `#[serde(default)] y: f32`. Three new `PendingActionKind` variants carrying `pointer_id: u32`, `pointer_type: String`, `x: f32`, `y: f32`. A new effect fn `push_pointer_event(app/world, node, event, buttons, pointer_id, pointer_type, x, y, mods)` in both drivers.

- [ ] **Step 1: Write the failing test**

Create `crates/superui_test_engine/tests/pointer_actions.rs`. Fixture: one element recording the ordered sequence of `pointerdown`/`pointermove`/`pointerup` into visible text, plus `e.shiftKey` and `e.pointerType` on `pointerdown`.

```rust
use superui_test_engine::driver::run_spec;
use superui_test_engine::host::{build_headless_app, HostProject};
use superui_test_engine::transpile::transpile_spec;

fn project() -> HostProject {
    HostProject {
        html: "<html><head><link rel=\"stylesheet\" href=\"style.css\"><script type=\"module\" src=\"app.tsx\"></script></head><body><div id=\"root\"></div></body></html>".into(),
        css: String::new(),
        js_or_tsx: r#"
            import { createSignal, render } from "supersolid";
            function App() {
                const [log, setLog] = createSignal("");
                const [info, setInfo] = createSignal("none");
                const push = (s) => setLog(log() + s + ";");
                return <div>
                    <div id="pad"
                        onPointerDown={(e) => { push("down"); setInfo(`${e.shiftKey}/${e.pointerType}/${e.pointerId}`); }}
                        onPointerMove={() => push("move")}
                        onPointerUp={() => push("up")}>pad</div>
                    <div id="log">{log()}</div>
                    <div id="info">{info()}</div>
                </div>;
            }
            render(App, document.getElementById("root"));
        "#.into(),
        tsx: true,
        mount_root: "ui".into(),
        extra_assets: vec![],
        ..Default::default()
    }
}

#[test]
fn pointer_actions_fire_in_order_and_carry_defaults_and_modifiers() {
    let mut app = build_headless_app(&project());
    let spec = r##"
        import { test, expect } from "superui/test";
        test("pointer sequence and defaults", async ({ page }) => {
            await page.locator("#pad").pointerDown();
            await page.locator("#pad").pointerMove();
            await page.locator("#pad").pointerUp();
            await expect(page.locator("#log")).toHaveText("down;move;up;");
            await expect(page.locator("#info")).toHaveText("false/mouse/1");
        });
        test("pointerDown carries Shift and touch type", async ({ page }) => {
            await page.locator("#pad").pointerDown({ modifiers: ["Shift"], pointerType: "touch", pointerId: 7 });
            await expect(page.locator("#info")).toHaveText("true/touch/7");
        });
    "##;
    let js = transpile_spec(spec, "t.spec.ts", std::path::Path::new(".")).unwrap();
    let results = run_spec(&mut app, &js);
    assert_eq!(results.len(), 2);
    for r in &results { assert!(r.passed, "{}: {:?}", r.name, r.error); }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p superui_test_engine --test pointer_actions`
Expected: FAIL — `pointerDown`/`pointerMove`/`pointerUp` do not exist on the locator.

- [ ] **Step 3: Add the three locator methods in the prelude**

In `prelude.js`, add to `makeLocator`:

```js
async pointerDown(opts) { return enqueue(pointerCmd("pointerDown", this, opts)); },
async pointerMove(opts) { return enqueue(pointerCmd("pointerMove", this, opts)); },
async pointerUp(opts) { return enqueue(pointerCmd("pointerUp", this, opts)); },
```

with a small helper (defined near `serialize`/`enqueue`) that fills defaults:

```js
const pointerCmd = function (type, loc, opts) {
    opts = opts || {};
    return {
        type: type, locator: serialize(loc),
        modifiers: opts.modifiers || [],
        pointerId: opts.pointerId === undefined ? 1 : opts.pointerId,
        pointerType: opts.pointerType || "mouse",
        x: opts.x || 0, y: opts.y || 0,
    };
};
```

- [ ] **Step 4: Add the three `Command` variants**

In `command.rs`, add `PointerDown`/`PointerMove`/`PointerUp`, each:

```rust
PointerDown {
    locator: crate::locator::LocatorSpec,
    #[serde(default)] modifiers: Vec<String>,
    #[serde(default = "default_pointer_id")] pointer_id: u32,
    #[serde(default = "default_pointer_type")] pointer_type: String,
    #[serde(default)] x: f32,
    #[serde(default)] y: f32,
},
```

with free fns `fn default_pointer_id() -> u32 { 1 }` and `fn default_pointer_type() -> String { "mouse".into() }`. (The prelude always sends these, so the defaults mainly document intent and keep hand-written JSON valid.)

- [ ] **Step 5: Implement the effect in `driver.rs`**

In `driver.rs`:
- Add `PendingActionKind::PointerDown { pointer_id: u32, pointer_type: String, x: f32, y: f32 }` and `PointerMove`/`PointerUp` with the same fields.
- Add arms in the command loop for the three variants: build the action label, `parse_modifiers(modifiers)`, and call `start_action` with the new kind and parsed `mods`.
- In `perform_action`, add arms calling a new `push_pointer_event` that resolves the locator's first node and pushes a `PendingDomEvent::new(node, "pointerdown"|"pointermove"|"pointerup").with_mouse(MousePayload { client_x: x, client_y: y, button: 0, buttons, pointer_id, pointer_type: kind, is_primary: true }).with_modifiers(mods)`, bubbling. `buttons` is `1` for down and move, `0` for up. `pointer_type` maps `"touch" → PointerKind::Touch`, anything else → `PointerKind::Mouse`.

- [ ] **Step 6: Mirror the effect in `ui_driver.rs`**

Apply the identical variants, command arms, and `push_pointer_event` to `ui_driver.rs`.

- [ ] **Step 7: Run the test to verify it passes**

Run: `cargo test -p superui_test_engine --test pointer_actions`
Expected: PASS.

- [ ] **Step 8: Run the full test-engine suite**

Run: `cargo test -p superui_test_engine`
Expected: PASS.

- [ ] **Step 9: Commit**

```bash
git add crates/superui_test_engine/src/prelude.js crates/superui_test_engine/src/command.rs crates/superui_test_engine/src/driver.rs crates/superui_test_engine/src/ui_driver.rs crates/superui_test_engine/tests/pointer_actions.rs
git commit -m "feat(test-engine): pointerDown/pointerMove/pointerUp locator actions"
```

---

### Task 4: Inventory shift-click end-to-end proof

**Files:**
- Create: `examples/inventory/tests/shift_click.rs`

**Interfaces:**
- Consumes: `superui_test_engine::driver::run_spec`, `superui_test_engine::host::{build_headless_app, HostProject}`, `superui_test_engine::transpile::transpile_spec`; `pointerDown({modifiers})` from Task 3; the real inventory assets via `include_str!`.

- [ ] **Step 1: Write the failing test**

Create `examples/inventory/tests/shift_click.rs`. Build a `HostProject` from the real assets (mirror `inventory.rs`'s `include_str!` of `index.html`/`style.css`/`app.tsx`, `mount_root: "ui/inventory"`), then run a spec that shift-`pointerDown`s `#tile-sword` and asserts it is now under `#inv`:

```rust
use superui_test_engine::driver::run_spec;
use superui_test_engine::host::{build_headless_app, HostProject};
use superui_test_engine::transpile::transpile_spec;

const HTML: &str = include_str!("../assets/ui/inventory/index.html");
const CSS: &str = include_str!("../assets/ui/inventory/style.css");
const TSX: &str = include_str!("../assets/ui/inventory/app.tsx");

fn project() -> HostProject {
    HostProject {
        html: HTML.into(),
        css: CSS.into(),
        js_or_tsx: TSX.into(),
        tsx: true,
        mount_root: "ui/inventory".into(),
        extra_assets: vec![],
        ..Default::default()
    }
}

#[test]
fn shift_pointerdown_quick_moves_sword_to_inventory() {
    let mut app = build_headless_app(&project());
    let spec = r##"
        import { test, expect } from "superui/test";
        test("shift-click quick-moves the sword", async ({ page }) => {
            await expect(page.locator("#chest #tile-sword")).toHaveCount(1);
            await page.locator("#tile-sword").pointerDown({ modifiers: ["Shift"] });
            await expect(page.locator("#inv #tile-sword")).toHaveCount(1);
        });
    "##;
    let js = transpile_spec(spec, "t.spec.ts", std::path::Path::new(".")).unwrap();
    let results = run_spec(&mut app, &js);
    assert_eq!(results.len(), 1);
    assert!(results[0].passed, "error: {:?}", results[0].error);
}
```

- [ ] **Step 2: Run the test to verify it passes**

Run: `cargo test -p chest_inventory --test shift_click`
Expected: PASS. (With Tasks 1–3 landed, the shift modifier reaches `onPointerDown`, `startDrag` sees `e.shiftKey`, and `quickMove` moves the sword before any capture/drag begins.)

Note: if this fails *before* Tasks 1–3 were fully correct, that is the regression this plan fixes — debug with `superpowers:systematic-debugging`, do not weaken the assertion.

- [ ] **Step 3: Commit**

```bash
git add examples/inventory/tests/shift_click.rs
git commit -m "test(inventory): shift-click quick-move end-to-end through the test engine"
```

---

## Final verification

- [ ] Run `cargo test -p superui_bridge -p superui_test_engine -p chest_inventory` and confirm all pass.
- [ ] Confirm the two previously-failing behaviors are now green: bridge `pointer_press_carries_live_shift_modifier` and inventory `shift_pointerdown_quick_moves_sword_to_inventory`.
