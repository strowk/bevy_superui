# EditableText-backed Text Input Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the bespoke Phase-1 text-input subsystem with Bevy 0.19's `EditableText`, so `<input type="text">` and `<textarea>` gain real cursor/selection/clipboard/IME/multiline editing, and unify keyboard focus on `bevy_input_focus::InputFocus`.

**Architecture:** Approach A — `EditableText` is inserted directly onto the `<input>`/`<textarea>` element entity (the reconciler's element node), which flair keeps styling. Text edits flow out via `Changed<EditableText>` → DOM `value` + `input` event; JS/JSX value changes flow in via `editor_mut().set_text`. `InputFocus` becomes the single focus source of truth, driving DOM `focus`/`blur`/`change` events.

**Tech Stack:** Rust, Bevy 0.19 (`default-features = false`), the project's shadow-DOM/render-mirror bridge (`superui_bridge`), V8 JS engine.

**Spec:** `docs/superpowers/specs/2026-09-20-editabletext-input-design.md` — read it alongside this plan.

## Global Constraints

- Bevy 0.19, `default-features = false`. New Bevy features go on the `bevy` dependency of the crates that need them: `crates/superui_bridge/Cargo.toml` and `crates/superui/Cargo.toml`.
- DO NOT use git worktrees (huge `target/`). Work in-place on branch `feat/editabletext-input`.
- Do not touch `website/src/assets/*` (parallel WIP) or `crates/superui/src/scroll.rs` (unrelated).
- Commit style: summary says what was done; body says *why* (not a diff restatement).
- **Import paths to verify against the compiler** (Bevy 0.19 re-exports; treat compile errors as the source of truth, adjust the `use` and move on — this is not a design change):
  - `bevy::text::{EditableText, TextEdit}` — the component + edit enum.
  - `bevy::ui_widgets::EditableTextInputPlugin` — keyboard/IME/pointer-selection systems (needs the `bevy_ui_widgets` Cargo feature).
  - `bevy::input_focus::{InputFocus, InputFocusVisible, FocusCause, InputDispatchPlugin}` and `bevy::input_focus::{FocusGained, FocusLost}` (may live at `bevy::input_focus::gained_and_lost::{FocusGained, FocusLost}`).
  - `bevy::input_focus::AutoFocus` (component).
  - `bevy::ui::widget::TextCursorStyle` or `bevy::text::TextCursorStyle` — cursor styling (optional; insert if it resolves).
- `EditableText` API (verified at `v0.19.0`): `EditableText::default()`, `.value() -> SplitString` (`.to_string()`), `.editor_mut().set_text(&str)`, `.clear()`, public fields `allow_newlines: bool`, `visible_width: Option<f32>`, `visible_lines: Option<f32>`, `max_characters: Option<usize>`. Editing mutates the component, so `Changed<EditableText>` fires on real edits.
- `InputFocus` API: `.set(entity, FocusCause)`, `.get() -> Option<Entity>`, `.clear()`. `FocusCause::{Navigated, Pressed}`.
- `apply_text_edits` (bevy_text `TextPlugin`, PostUpdate, set `EditableTextSystems`), `update_editable_text_layout`/`scroll_editable_text` (bevy_ui `UiPlugin`), and `ClipboardPlugin`/`FontCx`/`LayoutCx` are already registered by the plugins the app/test harness add. The only missing piece is `EditableTextInputPlugin` + `InputDispatchPlugin`.

## Review Focus

- **Echo loop on controlled inputs** (`value={signal}` re-set every render): a naive Bevy→JS `input` on every `Changed<EditableText>` re-fires `input`, updates the signal, re-renders, re-sets the buffer — an infinite loop or cursor thrash. Pinned by the value-compare guard test in Task 2 (`js_value_set_does_not_re_emit_input`).
- **Spurious `input` on mount / initial value** (`<input value="x">`): inserting `EditableText` and seeding the buffer must not fire `input`. Pinned in Task 2 (`initial_value_does_not_fire_input`).
- **`change` fired when value did not change** (focus an input, click away without typing): `change` must fire only on real change since focus-gain. Pinned in Task 3 (`blur_without_edit_fires_no_change`).
- **Enter in a single-line input still reaches JS** (todomvc Enter-to-add): with `allow_newlines: false`, Enter must not be swallowed and must dispatch a `keydown` with `e.key === "Enter"`. Pinned in Task 3 (`enter_dispatches_keydown_in_single_line_input`).
- **Focus leaves no ghost** (blur clears the mirror; a later keystroke with nothing focused is a no-op, not a panic): pinned in Task 3 (`blur_clears_focus_mirror`).

---

### Task 1: Register EditableText plumbing and prove headless editing

Wire the two missing Bevy plugins and the `bevy_ui_widgets` feature, then prove — in the headless bridge test harness — that a focused `EditableText` entity applies keyboard edits. This de-risks the whole stack (clipboard/font/layout/focus-dispatch) before any reconcile changes.

**Files:**
- Modify: `crates/superui_bridge/Cargo.toml` (bevy features)
- Modify: `crates/superui/Cargo.toml` (bevy features)
- Modify: `crates/superui/src/mount.rs` (add plugins in `SuperUiPlugin::build`)
- Modify: `crates/superui_bridge/tests/support/mod.rs` (add plugins to `test_app`)
- Test: `crates/superui_bridge/tests/editable_text_plumbing.rs` (new)

**Interfaces:**
- Produces: an app/test-harness where `EditableText` entities edit under `InputFocus`. Later tasks rely on `EditableTextInputPlugin` + `InputDispatchPlugin` being present.

- [ ] **Step 1: Add the Cargo features**

In `crates/superui_bridge/Cargo.toml`, extend the `bevy` feature list (currently `std, bevy_log, bevy_ui, bevy_text, bevy_picking, ui_picking, bevy_input_focus, default_font`) with:

```toml
    "bevy_ui_widgets",
    "system_clipboard",
```

Apply the same two additions to the `bevy` feature list in `crates/superui/Cargo.toml`.

- [ ] **Step 2: Write the failing test**

Create `crates/superui_bridge/tests/editable_text_plumbing.rs`:

```rust
//! Plumbing proof: a focused `EditableText` entity applies keyboard edits inside
//! the headless bridge test harness (no window/GPU). If this fails, the reconcile
//! work in later tasks has no foundation.
mod support;
use support::*;

use bevy::input::keyboard::{Key, KeyCode, KeyboardInput};
use bevy::input::ButtonState;
use bevy::input_focus::{FocusCause, InputFocus};
use bevy::prelude::*;
use bevy::text::EditableText;

#[test]
fn focused_editable_text_applies_keyboard_edits() {
    let mut app = test_app();
    let e = app
        .world_mut()
        .spawn((Node::default(), EditableText::default()))
        .id();
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(e, FocusCause::Pressed);
    app.update(); // let focus dispatch settle

    for ch in ["h", "i"] {
        app.world_mut().write_message(KeyboardInput {
            key_code: KeyCode::KeyH, // ignored; logical_key drives text
            logical_key: Key::Character(ch.into()),
            state: ButtonState::Pressed,
            repeat: false,
            window: Entity::PLACEHOLDER,
            text: None,
        });
        app.update();
    }
    app.update(); // let PostUpdate apply_text_edits run once more

    let val = app
        .world()
        .get::<EditableText>(e)
        .unwrap()
        .value()
        .to_string();
    assert_eq!(val, "hi", "focused EditableText must apply typed characters");
}
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test -p superui_bridge --test editable_text_plumbing`
Expected: FAIL — either a compile error (plugins not added) or `assert_eq` failure (edits not applied because `EditableTextInputPlugin`/`InputDispatchPlugin` are missing).

- [ ] **Step 4: Add the plugins to the test harness**

In `crates/superui_bridge/tests/support/mod.rs`, inside `test_app()`, add the two plugins to the `add_plugins((...))` tuple (after `UiPlugin`, before/after `SuperUiCssPlugin`). Guard against double-registration since `UiPlugin` may already pull `InputDispatchPlugin`:

```rust
    if !app.is_plugin_added::<bevy::input_focus::InputDispatchPlugin>() {
        app.add_plugins(bevy::input_focus::InputDispatchPlugin);
    }
    app.add_plugins(bevy::ui_widgets::EditableTextInputPlugin);
```

(The existing `app.init_resource::<InputFocus>()`/`InputFocusVisible` lines are harmless if `InputDispatchPlugin` also inits them — `init_resource` is idempotent. Leave them.)

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test -p superui_bridge --test editable_text_plumbing`
Expected: PASS. If it fails to compile on a plugin path, correct the `use`/path per the compiler and Global Constraints, then re-run.

- [ ] **Step 6: Add the same plugins to the real app**

In `crates/superui/src/mount.rs`, in `SuperUiPlugin::build`, add (near the other `add_plugins`/`add_observer` calls, before the `Update` system chain):

```rust
        if !app.is_plugin_added::<bevy::input_focus::InputDispatchPlugin>() {
            app.add_plugins(bevy::input_focus::InputDispatchPlugin);
        }
        app.add_plugins(bevy::ui_widgets::EditableTextInputPlugin);
```

- [ ] **Step 7: Verify the workspace still builds**

Run: `cargo build -p superui`
Expected: builds clean.

- [ ] **Step 8: Commit**

```bash
git add crates/superui_bridge/Cargo.toml crates/superui/Cargo.toml \
        crates/superui/src/mount.rs crates/superui_bridge/tests/support/mod.rs \
        crates/superui_bridge/tests/editable_text_plumbing.rs
git commit -m "feat(superui): register Bevy EditableText input plugins

Adds the bevy_ui_widgets + system_clipboard features and the
EditableTextInputPlugin/InputDispatchPlugin so focused EditableText
entities edit under InputFocus, headless included. Foundation for
replacing the hand-crafted text-input path."
```

---

### Task 2: Reconcile `<input type="text">` to EditableText (core swap)

Replace the managed-child rendering path with `EditableText` on the input element, wire the value bridge both directions, focus the input via `InputFocus` on click, and delete the manual character editing. This is the atomic subsystem swap: after it, typing into a text input is real Bevy editing.

**Files:**
- Modify: `crates/superui_bridge/src/reconcile.rs` (replace the `is_text_input` branch; add `sync_editable_input`; remove `sync_input_text` + `fit_tail` if now unused)
- Modify: `crates/superui_bridge/src/events.rs` (`focus_and_click` also sets `InputFocus`; remove the text-edit branch from `keyboard_events_system`; add `editable_input_events_system`)
- Modify: `crates/superui_bridge/src/lib.rs` (export `editable_input_events_system`)
- Modify: `crates/superui/src/mount.rs` (add `editable_input_events_system` to the Update chain)
- Modify: `crates/superui_bridge/tests/input_events.rs` (rewrite the typing test)
- Modify: `crates/superui_bridge/tests/input_behaviors.rs` (rewrite placeholder + single-line tests; delete the caret-blink test)

**Interfaces:**
- Consumes: Task 1's plugins.
- Produces:
  - `Reconciler::sync_editable_input(&mut self, world, dom, node, entity)` — ensures `EditableText` + `TextLayout::no_wrap()` on a text `<input>`, seeds `max_characters` from `maxlength`, pushes DOM `value` into the buffer when it differs, and manages a placeholder overlay child.
  - `editable_input_events_system(q: Query<(&DomNode, &EditableText), Changed<EditableText>>, rt: Option<NonSendMut<UiRuntime>>, pending: ResMut<PendingDomEvents>)` — emits `input` and mirrors the buffer to DOM `value` on real edits.
  - `focus_and_click` now also calls `input_focus.set(entity, FocusCause::Pressed)`.

- [ ] **Step 1: Write the failing tests**

Rewrite the typing test in `crates/superui_bridge/tests/input_events.rs`. Replace the whole `typing_into_focused_input_updates_value_and_fires_input` test body with the version below (drives real Bevy editing through `InputFocus`, asserts DOM `value` and the `input` listener). Keep the other tests in the file unchanged.

```rust
#[test]
fn typing_into_focused_input_updates_value_and_fires_input() {
    use bevy::input::keyboard::{Key, KeyCode, KeyboardInput};
    use bevy::input::ButtonState;
    use bevy::input_focus::{FocusCause, InputFocus};

    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<input id='t' type='text'>",
    )));
    let mut app = test_app();
    let _root = mount(&mut app, dom.clone());
    app.init_resource::<PendingDomEvents>();
    // Full input pipeline: emit input events from EditableText edits, then drain.
    app.add_systems(
        Update,
        (
            superui_bridge::editable_input_events_system,
            superui_bridge::keyboard_events_system,
            drain_dom_events_system,
        )
            .chain()
            .before(superui_bridge::reconcile_system),
    );

    app.world_mut().non_send_mut::<UiRuntime>().run_script(
        "globalThis.inputs = 0; \
         document.getElementById('t').addEventListener('input', function(){ globalThis.inputs++; });",
    );
    app.update();

    // Focus the input entity via InputFocus (the source of truth).
    let node = dom.borrow().get_element_by_id("t").unwrap();
    let input_ent = {
        let mut q = app.world_mut().query::<(Entity, &superui_bridge::DomNode)>();
        q.iter(app.world()).find(|(_, d)| d.0 == node).map(|(e, _)| e).unwrap()
    };
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(input_ent, FocusCause::Pressed);
    app.update();

    for ch in ["h", "i"] {
        app.world_mut().write_message(KeyboardInput {
            key_code: KeyCode::KeyH,
            logical_key: Key::Character(ch.into()),
            state: ButtonState::Pressed,
            repeat: false,
            window: Entity::PLACEHOLDER,
            text: None,
        });
        app.update();
    }
    app.update(); // settle: apply_text_edits -> Changed -> input event -> drain

    assert_eq!(dom.borrow().value(node), "hi", "typed value reaches the DOM");

    app.world_mut().non_send_mut::<UiRuntime>().run_script(
        "document.getElementById('t').setAttribute('data-inputs', String(globalThis.inputs));",
    );
    let n: i32 = dom
        .borrow()
        .get_attribute(node, "data-inputs")
        .unwrap_or("0")
        .parse()
        .unwrap_or(0);
    assert!(n >= 1, "the input listener fired at least once, got {n}");
}
```

Add two new tests to the same file (echo guard + no spurious mount input):

```rust
/// A JSX-style controlled input sets `.value` from JS every render. Pushing that
/// into the EditableText buffer must NOT re-emit `input` (which would loop).
#[test]
fn js_value_set_does_not_re_emit_input() {
    use bevy::input_focus::InputFocus;
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<input id='t' type='text'>",
    )));
    let mut app = test_app();
    let _root = mount(&mut app, dom.clone());
    app.init_resource::<PendingDomEvents>();
    app.add_systems(
        Update,
        (superui_bridge::editable_input_events_system, drain_dom_events_system)
            .chain()
            .before(superui_bridge::reconcile_system),
    );
    app.world_mut().non_send_mut::<UiRuntime>().run_script(
        "globalThis.inputs = 0; \
         document.getElementById('t').addEventListener('input', function(){ globalThis.inputs++; });",
    );
    app.update();

    // JS sets the value (controlled input).
    app.world_mut()
        .non_send_mut::<UiRuntime>()
        .run_script("document.getElementById('t').value = 'abc';");
    app.update(); // reconcile pushes into buffer
    app.update(); // apply_text_edits -> Changed -> compare -> (no input)
    app.update();

    let node = dom.borrow().get_element_by_id("t").unwrap();
    app.world_mut().non_send_mut::<UiRuntime>().run_script(
        "document.getElementById('t').setAttribute('data-inputs', String(globalThis.inputs));",
    );
    assert_eq!(
        dom.borrow().get_attribute(node, "data-inputs").unwrap_or("0"),
        "0",
        "setting .value from JS must not fire input"
    );
}

/// An input with an initial value seeds the buffer without firing `input` on mount.
#[test]
fn initial_value_does_not_fire_input() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<input id='t' type='text' value='seed'>",
    )));
    let mut app = test_app();
    let _root = mount(&mut app, dom.clone());
    app.init_resource::<PendingDomEvents>();
    app.add_systems(
        Update,
        (superui_bridge::editable_input_events_system, drain_dom_events_system)
            .chain()
            .before(superui_bridge::reconcile_system),
    );
    app.world_mut().non_send_mut::<UiRuntime>().run_script(
        "globalThis.inputs = 0; \
         document.getElementById('t').addEventListener('input', function(){ globalThis.inputs++; });",
    );
    for _ in 0..4 { app.update(); }
    let node = dom.borrow().get_element_by_id("t").unwrap();
    app.world_mut().non_send_mut::<UiRuntime>().run_script(
        "document.getElementById('t').setAttribute('data-inputs', String(globalThis.inputs));",
    );
    assert_eq!(
        dom.borrow().get_attribute(node, "data-inputs").unwrap_or("0"),
        "0",
        "seeding an initial value must not fire input"
    );
}
```

In `crates/superui_bridge/tests/input_behaviors.rs`: (a) **delete** the test `focused_input_shows_and_blinks_a_caret` (the caret is now owned by `EditableText`/`TextCursorStyle`, not a managed-child glyph); (b) replace `input_text_child_is_single_line_no_wrap` with a test asserting the input *element* is single-line and carries `EditableText`:

```rust
/// A text input is single-line: EditableText on the element with allow_newlines
/// false and a no-wrap TextLayout (the fix for the field growing like a textarea).
#[test]
fn text_input_is_single_line_editable() {
    use bevy::text::EditableText;
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<input id='t' type='text'>",
    )));
    let mut app = test_app();
    let _root = mount(&mut app, dom.clone());
    app.update();

    let node = dom.borrow().get_element_by_id("t").unwrap();
    let input = entity_for(&mut app, node);

    let editable = app.world().get::<EditableText>(input).expect("input has EditableText");
    assert!(!editable.allow_newlines, "a text input does not allow newlines");
    let layout = app.world().get::<TextLayout>(input).expect("input has TextLayout");
    assert_eq!(layout.linebreak, bevy::text::LineBreak::NoWrap, "text input does not wrap");
}
```

And rewrite `placeholder_and_value_use_distinct_colors` to assert the placeholder *overlay* (shown when empty, gone when a value exists):

```rust
/// An empty input shows a dim-grey placeholder overlay child; a value removes it.
#[test]
fn placeholder_overlay_shows_when_empty_and_hides_with_value() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<input id='t' type='text' placeholder='hint'>",
    )));
    let mut app = test_app();
    let _root = mount(&mut app, dom.clone());
    app.update();

    let node = dom.borrow().get_element_by_id("t").unwrap();
    let input = entity_for(&mut app, node);

    // Empty -> placeholder overlay present, dim grey.
    let child = managed_child(&mut app, input).expect("placeholder overlay when empty");
    assert_eq!(app.world().get::<Text>(child).unwrap().0, "hint");
    assert_eq!(
        app.world().get::<TextColor>(child).unwrap().0,
        Color::srgb(0.6, 0.6, 0.6),
        "placeholder renders dim grey"
    );

    // Non-empty -> overlay removed.
    dom.borrow_mut().set_value(node, "typed");
    set_dirty(&mut app);
    app.update();
    assert!(
        managed_child(&mut app, input).is_none(),
        "a value removes the placeholder overlay"
    );
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p superui_bridge --test input_events --test input_behaviors`
Expected: compile error (`editable_input_events_system` / `sync_editable_input` not defined) or assertion failures.

- [ ] **Step 3: Add `sync_editable_input` and switch the reconcile branch**

In `crates/superui_bridge/src/reconcile.rs`, change the dispatch at the end of `sync_children` (the `if Self::is_text_input(...) { self.sync_input_text(...) }` block, ~lines 218-222) to:

```rust
        if Self::is_text_input(dom, parent_node) {
            self.sync_editable_input(world, dom, parent_node, parent_entity);
        } else if Self::is_checkbox(dom, parent_node) {
            self.sync_checkbox_mark(world, dom, parent_node, parent_entity);
        }
```

Delete the `sync_input_text` method (~lines 294-404) and its doc comment. If `fit_tail` (referenced only there) becomes unused, delete it too and its imports; if the compiler still reports it used elsewhere, leave it.

Add the new method (place it where `sync_input_text` was). Import at the top of the file: `use bevy::text::EditableText;` and `use bevy::text::TextLayout;` if not already imported (TextLayout likely already is).

```rust
    /// A text-entry `<input>` carries `EditableText` on the element itself (flair
    /// styles the same node). The DOM `value` is pushed into the buffer only when
    /// it differs (so controlled inputs don't reset the cursor mid-edit); Bevy's
    /// editor owns the text otherwise. An empty field shows a dim placeholder
    /// overlay child (EditableText has no placeholder of its own).
    fn sync_editable_input(
        &mut self,
        world: &mut World,
        dom: &superui_dom::Dom,
        input_node: NodeId,
        input_entity: Entity,
    ) {
        let value = dom.value(input_node);
        let max_chars = dom
            .get_attribute(input_node, "maxlength")
            .and_then(|s| s.parse::<usize>().ok());

        // A text input must not be a plain `Text` node (bevy_ui won't border one),
        // and any stray managed value-text from the old path is gone.
        if world.get::<Text>(input_entity).is_some() {
            world.entity_mut(input_entity).remove::<Text>();
        }

        // Ensure EditableText (single-line) + no-wrap layout on the element.
        if world.get::<EditableText>(input_entity).is_none() {
            let mut editable = EditableText::default();
            editable.allow_newlines = false;
            editable.editor_mut().set_text(&value);
            editable.max_characters = max_chars;
            world
                .entity_mut(input_entity)
                .insert((editable, TextLayout::no_wrap()));
        } else {
            // Keep buffer in sync with the DOM value when JS/JSX changed it.
            let mut ed = world.get_mut::<EditableText>(input_entity).unwrap();
            if ed.max_characters != max_chars {
                ed.max_characters = max_chars;
            }
            if ed.value().to_string() != value {
                ed.editor_mut().set_text(&value);
            }
        }

        self.sync_placeholder_overlay(world, dom, input_node, input_entity, value.is_empty());
    }

    /// Show/hide the dim placeholder overlay: a non-pickable `Text` child present
    /// only while the field is empty. Tracked in `input_texts` like the old child.
    fn sync_placeholder_overlay(
        &mut self,
        world: &mut World,
        dom: &superui_dom::Dom,
        input_node: NodeId,
        input_entity: Entity,
        is_empty: bool,
    ) {
        let existing = self
            .input_texts
            .get(&input_node)
            .copied()
            .filter(|e| world.get_entity(*e).is_ok());
        let placeholder = dom
            .get_attribute(input_node, "placeholder")
            .unwrap_or("")
            .to_string();

        if is_empty && !placeholder.is_empty() {
            let child = match existing {
                Some(c) => {
                    if let Some(mut t) = world.get_mut::<Text>(c) {
                        if t.0 != placeholder {
                            t.0 = placeholder.clone();
                        }
                    }
                    c
                }
                None => {
                    let c = world
                        .spawn((
                            Text::new(placeholder.clone()),
                            TextColor(Color::srgb(0.6, 0.6, 0.6)),
                            TextLayout::no_wrap(),
                            InputValueText,
                            Pickable::IGNORE,
                        ))
                        .id();
                    self.input_texts.insert(input_node, c);
                    c
                }
            };
            world.entity_mut(input_entity).add_child(child);
        } else if let Some(c) = existing {
            if let Ok(ec) = world.get_entity_mut(c) {
                ec.despawn();
            }
            self.input_texts.remove(&input_node);
        }
    }
```

Note: `InputValueText` and `Pickable` are already imported in `reconcile.rs` (used by `sync_checkbox_mark`).

- [ ] **Step 4: Add `editable_input_events_system` and set InputFocus on click**

In `crates/superui_bridge/src/events.rs`:

Add the import near the top: `use bevy::text::EditableText;` and `use bevy::input_focus::{FocusCause, InputFocus};`.

Change `focus_and_click` to also set `InputFocus`. It currently takes `(node, rt, pending)`; give it access to the focus resource by threading it through the observer. Update the observer `on_pointer_click` to add a `mut input_focus: ResMut<InputFocus>` param and pass it down, and update `focus_and_click`/`apply_pointer_click` signatures:

```rust
fn focus_and_click(
    node: NodeId,
    rt: &mut UiRuntime,
    pending: &mut PendingDomEvents,
    input_focus: &mut InputFocus,
) {
    rt.set_focus(Some(node));
    if let Some(entity) = rt.entity_for(node) {
        input_focus.set(entity, FocusCause::Pressed);
    }
    click_effect(rt, node, pending);
}
```

Update `on_pointer_click` to take `mut input_focus: ResMut<InputFocus>` and call `focus_and_click(node, &mut dom, &mut pending, &mut input_focus)`. Update `apply_pointer_click` similarly (add an `input_focus: &mut InputFocus` param and forward it). The old `rt.caret_visible`/`rt.caret_accum` writes in `focus_and_click` are removed (caret state is retired in Task 3; if they still exist as fields, leaving them unset is fine).

In `keyboard_events_system`, **delete** the text-editing branch (the block from the `// Text input editing:` comment through the `if changed { ... dispatch "input" ... }`, ~lines 229-249). Keep everything else (keydown/keyup dispatch, Tab, button/checkbox activation). Remove `key_to_text` if it becomes unused (it is still used by `key_name`, so keep both for now).

Add the new system at the end of the file:

```rust
/// Emit DOM `input` (and mirror the buffer to DOM `value`) when a user edit
/// changes an `EditableText`. The value-compare guard suppresses the echo from a
/// JS-driven `set_text` (buffer already equals the DOM value in that case) and any
/// no-op change, so controlled inputs don't loop.
pub fn editable_input_events_system(
    q: Query<(&DomNode, &EditableText), Changed<EditableText>>,
    rt: Option<NonSendMut<UiRuntime>>,
    mut pending: ResMut<PendingDomEvents>,
) {
    let Some(mut rt) = rt else {
        return;
    };
    let edits: Vec<(NodeId, String)> = q
        .iter()
        .map(|(d, e)| (d.0, e.value().to_string()))
        .collect();
    for (node, val) in edits {
        let cur = rt.dom.borrow().value(node);
        if cur != val {
            rt.dom.borrow_mut().set_value(node, &val);
            let mut ev = PendingDomEvent::new(node, "input");
            ev.cancelable = false;
            pending.0.push(ev);
            rt.dirty = true;
        }
    }
}
```

- [ ] **Step 5: Export and wire the new system**

In `crates/superui_bridge/src/lib.rs`, add `editable_input_events_system` to the `pub use events::{ ... }` list.

In `crates/superui/src/mount.rs`, add `editable_input_events_system` to the front of the chained Update tuple (it must run before `drain_dom_events_system` so its enqueued `input` events dispatch the same frame):

```rust
                (
                    editable_input_events_system,
                    drain_dom_events_system,
                    keyboard_events_system,
                    blink_caret_system,
                    emit_bevy_inbox_system,
                    drain_bevy_outbox_system,
                    tick_timers_system,
                    reconcile_system,
                )
```

Import `editable_input_events_system` in `mount.rs` (same `use superui_bridge::{...}` line the other systems come from).

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p superui_bridge --test input_events --test input_behaviors --test editable_text_plumbing`
Expected: PASS. Fix any signature mismatches the compiler flags (e.g. call sites of `focus_and_click`/`apply_pointer_click` in tests — `input_events.rs` and `input_behaviors.rs` call `on_pointer_click`/`click_effect`, not `focus_and_click`, so they should be unaffected; if a test constructs `apply_pointer_click` it needs the new arg).

- [ ] **Step 7: Run the full bridge test suite**

Run: `cargo test -p superui_bridge`
Expected: PASS (the `tsx_loader` flake noted in project memory may need a re-run; genuine failures must be fixed).

- [ ] **Step 8: Commit**

```bash
git add crates/superui_bridge/src/reconcile.rs crates/superui_bridge/src/events.rs \
        crates/superui_bridge/src/lib.rs crates/superui/src/mount.rs \
        crates/superui_bridge/tests/input_events.rs crates/superui_bridge/tests/input_behaviors.rs
git commit -m "feat(superui_bridge): back text <input> with Bevy EditableText

Replaces the managed-child value renderer and manual keyboard editing with
EditableText on the input element: real cursor/selection/clipboard editing,
value bridged both ways with an echo guard, focus set via InputFocus on
click. Placeholder is reimplemented as a dim overlay since EditableText has
none."
```

---

### Task 3: Unify focus on InputFocus (focus/blur/change events, Tab, caret cleanup)

Make `InputFocus` the single focus source of truth: derive `UiRuntime.focused` from focus events, emit DOM `focus`/`blur`/`change`, retarget Tab, and delete the retired caret machinery.

**Files:**
- Modify: `crates/superui_bridge/src/runtime.rs` (replace caret state with a focus snapshot; drop `advance_caret`)
- Modify: `crates/superui_bridge/src/events.rs` (focus observers; retarget Tab; drop `blink_caret_system`, `key_to_text` if now unused)
- Modify: `crates/superui_bridge/src/reconcile.rs` (autofocus → `AutoFocus` + `InputFocus`)
- Modify: `crates/superui_bridge/src/lib.rs` (export the focus observers; drop `blink_caret_system`)
- Modify: `crates/superui/src/mount.rs` (register focus observers; drop `blink_caret_system` from the chain)
- Modify: `crates/superui_bridge/tests/input_behaviors.rs` (adapt `click_stops_propagation...`)
- Test: `crates/superui_bridge/tests/focus_events.rs` (new)

**Interfaces:**
- Consumes: Task 2's `InputFocus`-on-click and `EditableText` inputs.
- Produces:
  - `on_focus_gained(...)` / `on_focus_lost(...)` observers (registered via `add_observer`) that dispatch DOM `focus`/`blur`, fire `change` on real edits, and write the derived `UiRuntime.focused` mirror.
  - `UiRuntime.focus_snapshot: Option<(NodeId, String)>` and its use; `set_focus`/`focused()` unchanged.

- [ ] **Step 1: Write the failing tests**

Create `crates/superui_bridge/tests/focus_events.rs`:

```rust
//! Focus is unified on InputFocus: focus/blur DOM events fire, change fires only
//! on a real edit at blur, and the runtime focus mirror tracks it.
mod support;
use support::*;

use std::cell::RefCell;
use std::rc::Rc;

use bevy::input_focus::{FocusCause, InputFocus};
use bevy::prelude::*;
use superui_bridge::{DomNode, PendingDomEvents, UiRuntime};

fn entity_for(app: &mut App, node: superui_dom::NodeId) -> Entity {
    let mut q = app.world_mut().query::<(Entity, &DomNode)>();
    q.iter(app.world()).find(|(_, d)| d.0 == node).map(|(e, _)| e).unwrap()
}

fn mount_with_focus(app: &mut App, dom: Rc<RefCell<superui_dom::Dom>>) -> Entity {
    let root = mount(app, dom);
    app.init_resource::<PendingDomEvents>();
    app.add_observer(superui_bridge::on_focus_gained);
    app.add_observer(superui_bridge::on_focus_lost);
    app.add_systems(
        Update,
        superui_bridge::drain_dom_events_system.before(superui_bridge::reconcile_system),
    );
    root
}

#[test]
fn focus_and_blur_dispatch_dom_events() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<input id='a' type='text'><input id='b' type='text'>",
    )));
    let mut app = test_app();
    let _root = mount_with_focus(&mut app, dom.clone());
    app.world_mut().non_send_mut::<UiRuntime>().run_script(
        "globalThis.log = ''; \
         document.getElementById('a').addEventListener('focus', function(){ globalThis.log += 'fa'; }); \
         document.getElementById('a').addEventListener('blur', function(){ globalThis.log += 'ba'; }); \
         document.getElementById('b').addEventListener('focus', function(){ globalThis.log += 'fb'; });",
    );
    app.update();

    let (a, b) = {
        let d = dom.borrow();
        (d.get_element_by_id("a").unwrap(), d.get_element_by_id("b").unwrap())
    };
    let (ea, eb) = (entity_for(&mut app, a), entity_for(&mut app, b));

    app.world_mut().resource_mut::<InputFocus>().set(ea, FocusCause::Pressed);
    app.update();
    app.update();
    app.world_mut().resource_mut::<InputFocus>().set(eb, FocusCause::Pressed);
    app.update();
    app.update();

    app.world_mut().non_send_mut::<UiRuntime>().run_script(
        "document.getElementById('a').setAttribute('data-log', globalThis.log);",
    );
    let log = dom.borrow().get_attribute(a, "data-log").unwrap_or("").to_string();
    assert!(log.contains("fa"), "focus fired on a: {log}");
    assert!(log.contains("ba"), "blur fired on a when focus moved: {log}");
    assert!(log.contains("fb"), "focus fired on b: {log}");
}

#[test]
fn blur_without_edit_fires_no_change() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<input id='a' type='text'><input id='b' type='text'>",
    )));
    let mut app = test_app();
    let _root = mount_with_focus(&mut app, dom.clone());
    app.world_mut().non_send_mut::<UiRuntime>().run_script(
        "globalThis.changes = 0; \
         document.getElementById('a').addEventListener('change', function(){ globalThis.changes++; });",
    );
    app.update();
    let (a, b) = {
        let d = dom.borrow();
        (d.get_element_by_id("a").unwrap(), d.get_element_by_id("b").unwrap())
    };
    let (ea, eb) = (entity_for(&mut app, a), entity_for(&mut app, b));

    app.world_mut().resource_mut::<InputFocus>().set(ea, FocusCause::Pressed);
    app.update(); app.update();
    // Move focus away without editing.
    app.world_mut().resource_mut::<InputFocus>().set(eb, FocusCause::Pressed);
    app.update(); app.update();

    app.world_mut().non_send_mut::<UiRuntime>().run_script(
        "document.getElementById('a').setAttribute('data-changes', String(globalThis.changes));",
    );
    assert_eq!(
        dom.borrow().get_attribute(a, "data-changes").unwrap_or("0"),
        "0",
        "no edit means no change event"
    );
}

#[test]
fn blur_clears_focus_mirror() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<input id='a' type='text'>",
    )));
    let mut app = test_app();
    let _root = mount_with_focus(&mut app, dom.clone());
    app.update();
    let a = dom.borrow().get_element_by_id("a").unwrap();
    let ea = entity_for(&mut app, a);
    app.world_mut().resource_mut::<InputFocus>().set(ea, FocusCause::Pressed);
    app.update(); app.update();
    assert_eq!(app.world().non_send::<UiRuntime>().focused(), Some(a));
    app.world_mut().resource_mut::<InputFocus>().clear();
    app.update(); app.update();
    assert_eq!(app.world().non_send::<UiRuntime>().focused(), None, "blur clears the mirror");
}
```

Add a `change`-on-edit test to `crates/superui_bridge/tests/input_events.rs` and an Enter test:

```rust
/// Editing then blurring fires exactly one `change` (web semantics).
#[test]
fn edit_then_blur_fires_change() {
    use bevy::input::keyboard::{Key, KeyCode, KeyboardInput};
    use bevy::input::ButtonState;
    use bevy::input_focus::{FocusCause, InputFocus};

    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<input id='a' type='text'><input id='b' type='text'>",
    )));
    let mut app = test_app();
    let _root = mount(&mut app, dom.clone());
    app.init_resource::<PendingDomEvents>();
    app.add_observer(superui_bridge::on_focus_gained);
    app.add_observer(superui_bridge::on_focus_lost);
    app.add_systems(
        Update,
        (superui_bridge::editable_input_events_system, drain_dom_events_system)
            .chain()
            .before(superui_bridge::reconcile_system),
    );
    app.world_mut().non_send_mut::<UiRuntime>().run_script(
        "globalThis.changes = 0; \
         document.getElementById('a').addEventListener('change', function(){ globalThis.changes++; });",
    );
    app.update();
    let (a, b) = {
        let d = dom.borrow();
        (d.get_element_by_id("a").unwrap(), d.get_element_by_id("b").unwrap())
    };
    let (ea, eb) = ({
        let mut q = app.world_mut().query::<(Entity, &superui_bridge::DomNode)>();
        q.iter(app.world()).find(|(_, d)| d.0 == a).map(|(e, _)| e).unwrap()
    }, {
        let mut q = app.world_mut().query::<(Entity, &superui_bridge::DomNode)>();
        q.iter(app.world()).find(|(_, d)| d.0 == b).map(|(e, _)| e).unwrap()
    });

    app.world_mut().resource_mut::<InputFocus>().set(ea, FocusCause::Pressed);
    app.update(); app.update();
    app.world_mut().write_message(KeyboardInput {
        key_code: KeyCode::KeyX,
        logical_key: Key::Character("x".into()),
        state: ButtonState::Pressed,
        repeat: false,
        window: Entity::PLACEHOLDER,
        text: None,
    });
    app.update(); app.update();
    app.world_mut().resource_mut::<InputFocus>().set(eb, FocusCause::Pressed);
    app.update(); app.update();

    app.world_mut().non_send_mut::<UiRuntime>().run_script(
        "document.getElementById('a').setAttribute('data-changes', String(globalThis.changes));",
    );
    assert_eq!(
        dom.borrow().get_attribute(a, "data-changes").unwrap_or("0"),
        "1",
        "editing then blurring fires exactly one change"
    );
}

/// Enter in a single-line input is not swallowed by the editor; it dispatches a
/// keydown with key "Enter" (todomvc's add-on-Enter).
#[test]
fn enter_dispatches_keydown_in_single_line_input() {
    use bevy::input::keyboard::{Key, KeyCode, KeyboardInput};
    use bevy::input::ButtonState;
    use bevy::input_focus::{FocusCause, InputFocus};

    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<input id='t' type='text'>",
    )));
    let mut app = test_app();
    let _root = mount(&mut app, dom.clone());
    app.init_resource::<PendingDomEvents>();
    app.add_systems(Update, superui_bridge::keyboard_events_system);
    app.world_mut().non_send_mut::<UiRuntime>().run_script(
        "globalThis.enters = 0; \
         document.getElementById('t').addEventListener('keydown', function(e){ if (e.key === 'Enter') globalThis.enters++; });",
    );
    app.update();
    let node = dom.borrow().get_element_by_id("t").unwrap();
    let ent = {
        let mut q = app.world_mut().query::<(Entity, &superui_bridge::DomNode)>();
        q.iter(app.world()).find(|(_, d)| d.0 == node).map(|(e, _)| e).unwrap()
    };
    app.world_mut().resource_mut::<InputFocus>().set(ent, FocusCause::Pressed);
    // Ensure the runtime mirror knows focus (keyboard_events_system dispatches to it).
    app.world_mut().non_send_mut::<UiRuntime>().set_focus(Some(node));
    // key_name maps KeyCode::Enter -> "Enter" regardless of logical_key, so any
    // constructible logical_key works here.
    app.world_mut().write_message(KeyboardInput {
        key_code: KeyCode::Enter,
        logical_key: Key::Character("x".into()),
        state: ButtonState::Pressed,
        repeat: false,
        window: Entity::PLACEHOLDER,
        text: None,
    });
    app.update();
    app.world_mut().non_send_mut::<UiRuntime>().run_script(
        "document.getElementById('t').setAttribute('data-enters', String(globalThis.enters));",
    );
    assert_eq!(
        dom.borrow().get_attribute(node, "data-enters").unwrap_or("0"),
        "1",
        "Enter dispatches a keydown with key Enter"
    );
}
```

Adapt `click_stops_propagation_and_focuses_the_deepest_dom_node` in `input_behaviors.rs`: it relies on a managed `InputValueText` child as the pick target. The input in that test starts empty with no placeholder, so no overlay exists. Change its fixture to include a placeholder so the overlay child is present:

```rust
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<div id='wrap'><input id='field' type='text' placeholder='p'></div>",
    )));
```

The rest of that test (resolve-to-nearest-DomNode, focus lands on the input) stays; it uses `on_pointer_click`, which now also sets `InputFocus` — add `app.init_resource::<InputFocus>()` is already done by `test_app`, so no change needed. Assert focus via `rt.focused()` still holds (the observer path sets the mirror; `on_pointer_click` sets it directly too).

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p superui_bridge --test focus_events --test input_events`
Expected: compile error (`on_focus_gained`/`on_focus_lost` undefined).

- [ ] **Step 3: Replace caret state with a focus snapshot in the runtime**

In `crates/superui_bridge/src/runtime.rs`:
- Remove the fields `caret_visible: bool` and `caret_accum: f32` and the `input_texts` doc that mentions caret if inaccurate (keep `input_texts` — the placeholder overlay uses it).
- Add: `pub(crate) focus_snapshot: Option<(NodeId, String)>,` next to `focused`.
- In `UiRuntime::new`, remove `caret_visible: true, caret_accum: 0.0,` and add `focus_snapshot: None,`.
- Delete the `advance_caret` method entirely.

- [ ] **Step 4: Add focus observers, retarget Tab, drop the caret system**

In `crates/superui_bridge/src/events.rs`:

Delete `blink_caret_system` entirely. Add the two observers:

```rust
/// Observer: an entity gained input focus. Dispatch DOM `focus`, snapshot the
/// current value for change-on-blur, and update the runtime focus mirror.
pub fn on_focus_gained(
    ev: On<bevy::input_focus::FocusGained>,
    nodes: Query<&DomNode>,
    parents: Query<&ChildOf>,
    rt: Option<NonSendMut<UiRuntime>>,
    mut pending: ResMut<PendingDomEvents>,
) {
    let Some(mut rt) = rt else { return; };
    let Some(node) = resolve_dom_node(ev.entity, &nodes, &parents) else { return; };
    let cur = rt.dom.borrow().value(node);
    rt.set_focus(Some(node));
    rt.focus_snapshot = Some((node, cur));
    let mut e = PendingDomEvent::new(node, "focus");
    e.bubbles = false;
    pending.0.push(e);
}

/// Observer: an entity lost input focus. Fire `change` if its value changed since
/// focus-gain, dispatch DOM `blur`, and clear the mirror if it pointed here.
pub fn on_focus_lost(
    ev: On<bevy::input_focus::FocusLost>,
    nodes: Query<&DomNode>,
    parents: Query<&ChildOf>,
    rt: Option<NonSendMut<UiRuntime>>,
    mut pending: ResMut<PendingDomEvents>,
) {
    let Some(mut rt) = rt else { return; };
    let Some(node) = resolve_dom_node(ev.entity, &nodes, &parents) else { return; };
    if let Some((snap_node, old)) = rt.focus_snapshot.take() {
        if snap_node == node && rt.dom.borrow().value(node) != old {
            let mut c = PendingDomEvent::new(node, "change");
            c.cancelable = false;
            pending.0.push(c);
        }
    }
    let mut e = PendingDomEvent::new(node, "blur");
    e.bubbles = false;
    pending.0.push(e);
    if rt.focused() == Some(node) {
        rt.set_focus(None);
    }
}
```

(`ChildOf` is already imported in `events.rs`. `ev.entity` reads the event's `entity` field — the focused entity — regardless of propagation.)

In `keyboard_events_system`, retarget Tab to `InputFocus`. Add a `mut input_focus: ResMut<InputFocus>` param to the system. Replace the Tab body (currently sets `rt.focused = Some(next)`) with:

```rust
        if code == KeyCode::Tab {
            let focusables = collect_focusable(&rt.dom.borrow());
            if !focusables.is_empty() {
                let next = match focusables.iter().position(|&n| Some(n) == rt.focused()) {
                    Some(i) => focusables[(i + 1) % focusables.len()],
                    None => focusables[0],
                };
                if let Some(entity) = rt.entity_for(next) {
                    input_focus.set(entity, FocusCause::Navigated);
                }
            }
            continue;
        }
```

(The `on_focus_gained`/`on_focus_lost` observers then update `rt.focused` and fire DOM focus/blur.)

- [ ] **Step 5: Autofocus via AutoFocus + InputFocus in the reconciler**

In `crates/superui_bridge/src/reconcile.rs`, the autofocus block (~lines 179-183) currently sets `self.focused = Some(child)`. Replace it so it adds Bevy's `AutoFocus` component to the element (which sets `InputFocus` on spawn). Import `use bevy::input_focus::AutoFocus;`:

```rust
                if dom.get_attribute(child, "autofocus").is_some()
                    && world.get::<AutoFocus>(entity).is_none()
                {
                    world.entity_mut(entity).insert(AutoFocus);
                }
```

(Remove the `self.focused.is_none()` gate — `AutoFocus`/`InputFocus` resolves the first-wins behavior. `self.focused` is no longer written by the reconciler.)

- [ ] **Step 6: Update exports and the system chain**

In `crates/superui_bridge/src/lib.rs`: remove `blink_caret_system` from the `pub use events::{...}` list; add `on_focus_gained, on_focus_lost`.

In `crates/superui/src/mount.rs`:
- Remove `blink_caret_system` from the chained Update tuple.
- Register the observers next to `on_pointer_click`: `.add_observer(on_focus_gained).add_observer(on_focus_lost)`.
- Update the `use superui_bridge::{...}` imports accordingly (drop `blink_caret_system`, add `on_focus_gained, on_focus_lost`).

- [ ] **Step 7: Run the tests**

Run: `cargo test -p superui_bridge --test focus_events --test input_events --test input_behaviors`
Expected: PASS. Then the whole crate:
Run: `cargo test -p superui_bridge`
Expected: PASS (re-run once if the known `tsx_loader` async test flakes).

- [ ] **Step 8: Commit**

```bash
git add crates/superui_bridge/src/runtime.rs crates/superui_bridge/src/events.rs \
        crates/superui_bridge/src/reconcile.rs crates/superui_bridge/src/lib.rs \
        crates/superui/src/mount.rs crates/superui_bridge/tests/focus_events.rs \
        crates/superui_bridge/tests/input_events.rs crates/superui_bridge/tests/input_behaviors.rs
git commit -m "feat(superui_bridge): unify keyboard focus on InputFocus

Focus becomes single-sourced: InputFocus drives a derived UiRuntime.focused
mirror and DOM focus/blur/change events (previously roadmap), Tab and
autofocus write InputFocus, and flair's :focus now reflects text-input
focus. Retires the hand-rolled caret blink, now owned by EditableText."
```

---

### Task 4: `<textarea>` multiline support

Add `<textarea>` as a multiline `EditableText` (wrapping, newlines allowed), seeded from its text content, with `rows` → visible lines.

**Files:**
- Modify: `crates/superui_bridge/src/reconcile.rs` (`is_textarea`; route textarea to a multiline variant of `sync_editable_input`)
- Test: `crates/superui_bridge/tests/textarea.rs` (new)

**Interfaces:**
- Consumes: Task 2/3's `sync_editable_input`.
- Produces: `Reconciler::is_textarea(dom, node) -> bool`; `sync_editable_input` gains a `multiline: bool` behavior (allow_newlines, wrapping layout, `visible_lines` from `rows`).

- [ ] **Step 1: Write the failing test**

Create `crates/superui_bridge/tests/textarea.rs`:

```rust
//! <textarea> is a multiline EditableText: newlines allowed, wrapping layout,
//! initial text seeded from its content, value bridged like <input>.
mod support;
use support::*;

use std::cell::RefCell;
use std::rc::Rc;

use bevy::prelude::*;
use bevy::text::EditableText;
use superui_bridge::DomNode;

fn entity_for(app: &mut App, node: superui_dom::NodeId) -> Entity {
    let mut q = app.world_mut().query::<(Entity, &DomNode)>();
    q.iter(app.world()).find(|(_, d)| d.0 == node).map(|(e, _)| e).unwrap()
}

#[test]
fn textarea_is_multiline_editable() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<textarea id='t' rows='4'></textarea>",
    )));
    let mut app = test_app();
    let _root = mount(&mut app, dom.clone());
    app.update();

    let node = dom.borrow().get_element_by_id("t").unwrap();
    let ta = entity_for(&mut app, node);
    let editable = app.world().get::<EditableText>(ta).expect("textarea has EditableText");
    assert!(editable.allow_newlines, "textarea allows newlines");
    assert_eq!(editable.visible_lines, Some(4.0), "rows maps to visible_lines");
    let layout = app.world().get::<TextLayout>(ta).expect("textarea TextLayout");
    assert_ne!(layout.linebreak, bevy::text::LineBreak::NoWrap, "textarea wraps");
}

#[test]
fn textarea_seeds_initial_content() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<textarea id='t'>hello</textarea>",
    )));
    let mut app = test_app();
    let _root = mount(&mut app, dom.clone());
    app.update();
    app.update();
    let node = dom.borrow().get_element_by_id("t").unwrap();
    let ta = entity_for(&mut app, node);
    let val = app.world().get::<EditableText>(ta).unwrap().value().to_string();
    assert_eq!(val, "hello", "textarea seeds from its text content");
}
```

(The trailing `let _ = (...)` line only keeps otherwise-unused imports live; delete it if the compiler is happy without.)

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p superui_bridge --test textarea`
Expected: FAIL — textarea currently falls through as a plain element (no `EditableText`).

- [ ] **Step 3: Detect textarea and seed content; route to multiline**

In `crates/superui_bridge/src/reconcile.rs`:

Add the detector:

```rust
    /// Is `node` a `<textarea>`?
    fn is_textarea(dom: &superui_dom::Dom, node: NodeId) -> bool {
        matches!(dom.tag(node), Some("textarea"))
    }
```

At the reconcile dispatch (the block edited in Task 2), route textarea through the same method with a `multiline` flag:

```rust
        if Self::is_text_input(dom, parent_node) {
            self.sync_editable_input(world, dom, parent_node, parent_entity, false);
        } else if Self::is_textarea(dom, parent_node) {
            self.sync_editable_input(world, dom, parent_node, parent_entity, true);
        } else if Self::is_checkbox(dom, parent_node) {
            self.sync_checkbox_mark(world, dom, parent_node, parent_entity);
        }
```

Change `sync_editable_input`'s signature to take `multiline: bool` and branch. For a textarea, the initial value comes from the element's text content when the DOM `value` prop is empty (HTML seeds a textarea from its children). Use `dom.text_content(input_node)` as the seed. Apply `allow_newlines`, a wrapping `TextLayout`, and `visible_lines` from `rows`:

```rust
    fn sync_editable_input(
        &mut self,
        world: &mut World,
        dom: &superui_dom::Dom,
        input_node: NodeId,
        input_entity: Entity,
        multiline: bool,
    ) {
        // A textarea seeds from its text content; an input from its value.
        let dom_value = dom.value(input_node);
        let value = if multiline && dom_value.is_empty() {
            dom.text_content(input_node)
        } else {
            dom_value
        };
        let max_chars = dom
            .get_attribute(input_node, "maxlength")
            .and_then(|s| s.parse::<usize>().ok());
        let visible_lines = if multiline {
            dom.get_attribute(input_node, "rows").and_then(|s| s.parse::<f32>().ok())
        } else {
            None
        };
        let layout = if multiline { TextLayout::default() } else { TextLayout::no_wrap() };

        if world.get::<Text>(input_entity).is_some() {
            world.entity_mut(input_entity).remove::<Text>();
        }

        if world.get::<EditableText>(input_entity).is_none() {
            let mut editable = EditableText::default();
            editable.allow_newlines = multiline;
            editable.max_characters = max_chars;
            editable.visible_lines = visible_lines.or(editable.visible_lines);
            editable.editor_mut().set_text(&value);
            world.entity_mut(input_entity).insert((editable, layout));
        } else {
            let mut ed = world.get_mut::<EditableText>(input_entity).unwrap();
            if ed.max_characters != max_chars { ed.max_characters = max_chars; }
            if multiline && ed.visible_lines != visible_lines.or(ed.visible_lines) {
                ed.visible_lines = visible_lines.or(ed.visible_lines);
            }
            if ed.value().to_string() != value {
                ed.editor_mut().set_text(&value);
            }
        }

        // Textareas render their own multiline content; only inputs get a
        // single-line placeholder overlay.
        if !multiline {
            self.sync_placeholder_overlay(world, dom, input_node, input_entity, value.is_empty());
        }
    }
```

If `dom.text_content(node)` is not the exact method name in `superui_dom`, use the crate's equivalent (the checkbox/text code and tests call `dom.text_content(node)` — see `input_events.rs` — so it exists).

Update the Task-2 unit test call sites if any referenced the 4-arg form directly (they call through reconcile, so none should).

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p superui_bridge --test textarea`
Expected: PASS.

- [ ] **Step 5: Run the crate tests**

Run: `cargo test -p superui_bridge`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/superui_bridge/src/reconcile.rs crates/superui_bridge/tests/textarea.rs
git commit -m "feat(superui_bridge): support <textarea> as multiline EditableText

Textareas reconcile to a wrapping, newline-allowing EditableText seeded from
their text content, with rows mapped to visible_lines — reusing the input
value bridge and focus path."
```

---

### Task 5: Documentation

Update the capability reference to match the new behavior.

**Files:**
- Modify: `website/src/docs/reference/js-dom.md`
- Modify: `website/src/docs/reference/known-issues.md`

**Interfaces:** none (docs).

- [ ] **Step 1: Load the docs skills**

Invoke `reference-docs` and `tech-writing:technical-writing` before editing. Keep entries terse, present-tense, describing what the software does — no mechanism narration.

- [ ] **Step 2: Update `js-dom.md`**

- Promote the `focus`/`blur` **events** from roadmap (line ~67) to supported: "`focus` / `blur` — dispatched to the node as keyboard focus enters/leaves (via `InputFocus`)."
- Leave the `focus()` / `blur()` **methods** entry (line ~47) as-is: still click/`InputFocus`-driven, not JS-callable.
- Under the `input` event entry, add: "coalesced — one `input` per frame in which the text changed, not strictly one per character."
- Note `change` fires on blur when the value changed since focus.

- [ ] **Step 3: Update `known-issues.md`**

- Remove the now-fixed limitations: end-of-field-only caret, no uppercase/shift/symbols, single-line only.
- Add: `<textarea>` is supported (multiline). `input` events are frame-coalesced. `el.focus()`/`el.blur()` are not yet JS-callable (focus is set by clicking or Tab). Undo/redo and password masking (`type="password"`) are not implemented.

- [ ] **Step 4: Verify the docs build (if the site has a build/lint step)**

If a docs build/lint command exists (check `website/package.json`), run it; otherwise skip. Do not touch `website/src/assets/*`.

- [ ] **Step 5: Commit**

```bash
git add website/src/docs/reference/js-dom.md website/src/docs/reference/known-issues.md
git commit -m "docs: reference EditableText-backed input capabilities

focus/blur events now fire; textarea and richer editing supported; records
the input-coalescing behavior and the remaining gaps (focus() method,
undo/redo, password masking)."
```

---

### Task 6: Workspace verification, dead-code sweep, examples

Confirm the whole workspace builds and tests, remove any now-dead code, and build the todomvc examples for the human's manual check.

**Files:**
- Modify: any file with dead code the compiler flags (e.g. `key_to_text` if unused, stale doc comments on `InputValueText`).

**Interfaces:** none.

- [ ] **Step 1: Full workspace test**

Run: `cargo test --workspace`
Expected: PASS. Re-run once if the known `tsx_loader` async test flakes (per project memory `flaky-tsx-loader-tests`). Investigate any other failure.

- [ ] **Step 2: Clippy / warnings sweep**

Run: `cargo clippy --workspace --all-targets`
Expected: no new warnings. Remove genuinely dead code surfaced by the swap: if `key_to_text` is unused after Task 3 (it is still called by `key_name`, so likely retained), leave it; delete `advance_caret` remnants, unused `fit_tail`, and correct the `InputValueText` doc comment (now a placeholder overlay marker, not a value renderer).

- [ ] **Step 3: Build the examples**

Run: `cargo build -p todomvc_supersolid` (or the example's package/bin name — check `examples/todomvc_supersolid/Cargo.toml`) and the vanilla `todomvc` example.
Expected: both compile. (The human will run them to verify typing, add-on-Enter, and checkboxes interactively.)

- [ ] **Step 4: Commit any cleanup**

```bash
git add -A
git commit -m "chore(superui_bridge): remove dead text-input code after EditableText swap

Drops the retired caret helpers and corrects stale comments left by the
hand-crafted input path."
```

- [ ] **Step 5: Report for manual verification**

Summarize for the human: what changed, that `cargo test --workspace` is green, and that todomvc (both `todomvc_supersolid` and vanilla `todomvc`) build and are ready for their interactive check (typing, Enter-to-add, checkbox toggle, focus/blur styling).
