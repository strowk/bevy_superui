//! Input -> DOM event seam. `bevy_picking`/keyboard produce DOM events, which we
//! dispatch into JS (W3C capture/bubble, synchronous) and then reconcile.

use bevy::ecs::message::MessageReader;
use bevy::input::keyboard::{Key, KeyCode, KeyboardInput};
use bevy::input::ButtonState;
use bevy::picking::events::{Click, Pointer};
use bevy::prelude::*;
use superui_dom::NodeId;

use crate::runtime::{DomNode, UiRuntime};

/// One pending DOM event to dispatch into JS on the next drain.
#[derive(Clone, Debug)]
pub struct PendingDomEvent {
    pub target: NodeId,
    pub type_: String,
    pub bubbles: bool,
    pub cancelable: bool,
}

impl PendingDomEvent {
    pub fn new(target: NodeId, type_: &str) -> Self {
        PendingDomEvent {
            target,
            type_: type_.to_string(),
            bubbles: true,
            cancelable: true,
        }
    }
}

/// Queue of input-originated DOM events awaiting dispatch. Send resource so
/// picking observers can push to it.
#[derive(Resource, Default)]
pub struct PendingDomEvents(pub Vec<PendingDomEvent>);

/// Core logic for a click on a DOM node: enqueue a `"click"` event and,
/// for a checkbox `<input>`, also mirror the native toggle (flip `checked`)
/// and enqueue a subsequent `"change"` event.
///
/// Extracted as a free function so both the observer and the test harness can
/// call it without needing to construct a `Pointer<Click>` event.
pub fn click_effect(rt: &UiRuntime, node: NodeId, pending: &mut PendingDomEvents) {
    let is_checkbox = {
        let d = rt.dom.borrow();
        matches!(tag_of(&d, node).as_deref(), Some("input"))
            && d.get_attribute(node, "type") == Some("checkbox")
    };

    pending.0.push(PendingDomEvent::new(node, "click"));
    if is_checkbox {
        let now = !rt.dom.borrow().checked(node);
        rt.dom.borrow_mut().set_checked(node, now);
        pending.0.push(PendingDomEvent::new(node, "change"));
    }
}

/// Observer: a pointer click on a UI entity becomes a DOM `click` on its node.
/// For a checkbox input, also mirror the native toggle: flip DOM `checked` and
/// enqueue a `change` event (dispatched after the click). Also sets keyboard focus
/// to the clicked node (Task 5).
pub fn on_pointer_click(
    mut ev: On<Pointer<Click>>,
    nodes: Query<&DomNode>,
    parents: Query<&ChildOf>,
    dom: Option<NonSendMut<UiRuntime>>,
    mut pending: ResMut<PendingDomEvents>,
) {
    // No UI is mounted when `UiRuntime` is absent — e.g. the `superui_test --ui`
    // shell (which runs `SuperUiPlugin` in the same world as its egui runner)
    // before the first Run mounts a spec, or between runs while the DOM is torn
    // down. A click then has no DOM to dispatch to, so skip rather than fail
    // param validation on the missing non-send resource (which panics the app).
    let Some(mut dom) = dom else {
        return;
    };
    // `Pointer<Click>` bubbles up the entity hierarchy, firing this observer once
    // per ancestor. We only want to act on the actual (deepest) target — otherwise
    // focus would be overwritten by each ancestor up to `<body>`. Stop propagation
    // so we handle the click exactly once. (DOM-level bubbling is done separately
    // by our own W3C dispatch in `click_effect`/`dispatch_event`.)
    let target = ev.event().entity;
    ev.propagate(false);
    apply_pointer_click(target, &nodes, &parents, &mut dom, &mut pending);
}

/// The core of a pointer click on a UI `entity`: resolve it to a DOM node, focus
/// it, and enqueue the `click` (+ checkbox `change`) DOM event. Shared by the
/// picking observer and by test/automation drivers that can't synthesize a real
/// `Pointer<Click>` (e.g. the `mcp_debug` click injector).
///
/// The hit entity may be a reconciler-internal child (e.g. an input's managed
/// text child, or an element's `Text` child) with no `DomNode`. So resolve to
/// the nearest ancestor that *is* a DOM node — clicking anywhere inside the
/// input focuses the input, like a browser.
pub fn apply_pointer_click(
    entity: Entity,
    nodes: &Query<&DomNode>,
    parents: &Query<&ChildOf>,
    rt: &mut UiRuntime,
    pending: &mut PendingDomEvents,
) {
    let mut cur = entity;
    let node = loop {
        if let Ok(dom_node) = nodes.get(cur) {
            break Some(dom_node.0);
        }
        match parents.get(cur) {
            Ok(parent) => cur = parent.parent(),
            Err(_) => break None,
        }
    };
    let Some(node) = node else {
        return;
    };
    rt.focused = Some(node);
    rt.caret_visible = true;
    rt.caret_accum = 0.0;
    click_effect(rt, node, pending);
}

/// Blink the text caret (~2 Hz) while a text field is focused; re-renders on flip.
pub fn blink_caret_system(time: Res<Time>, rt: Option<NonSendMut<UiRuntime>>) {
    if let Some(mut rt) = rt {
        let dt = time.delta_secs();
        if rt.advance_caret(dt) {
            rt.dirty = true;
        }
    }
}

fn tag_of(dom: &superui_dom::Dom, node: NodeId) -> Option<String> {
    match dom.get(node).map(|n| &n.kind) {
        Some(superui_dom::NodeKind::Element(e)) => Some(e.tag.clone()),
        _ => None,
    }
}

/// Normal system: route keyboard input to the focused DOM node as `keydown`/`keyup`,
/// and for printable characters typed into a text input, mutate the DOM `value` and
/// fire `input` (Phase-1 text entry — TodoMVC needs Enter-to-add and character typing).
///
/// `NonSendMut<UiRuntime>` forces main-thread execution.
pub fn keyboard_events_system(
    mut reader: MessageReader<KeyboardInput>,
    mut rt: NonSendMut<UiRuntime>,
) {
    // Collect key messages first (the reader borrow must not overlap with rt).
    let presses: Vec<(Key, KeyCode, bool)> = reader
        .read()
        .map(|k| {
            (
                k.logical_key.clone(),
                k.key_code,
                matches!(k.state, ButtonState::Pressed),
            )
        })
        .collect();

    if presses.is_empty() {
        return;
    }
    use superui_js::JsEngine;
    let mut any = false;
    for (key, code, pressed) in presses {
        let Some(focused) = rt.focused else {
            continue;
        };
        let type_ = if pressed { "keydown" } else { "keyup" };
        let kn = key_name(&key, code);
        rt.engine.dispatch_event(focused, type_, Some(&kn), true, true);
        any = true;
        if !pressed {
            continue;
        }

        // Tab moves keyboard focus to the next focusable element (browser std).
        if code == KeyCode::Tab {
            let focusables = collect_focusable(&rt.dom.borrow());
            if !focusables.is_empty() {
                let next = match focusables.iter().position(|&n| Some(n) == rt.focused) {
                    Some(i) => focusables[(i + 1) % focusables.len()],
                    None => focusables[0],
                };
                rt.focused = Some(next);
            }
            continue;
        }

        let tag = rt.dom.borrow().tag(focused).map(|s| s.to_string());
        let is_checkbox = tag.as_deref() == Some("input")
            && rt.dom.borrow().get_attribute(focused, "type") == Some("checkbox");
        let is_button = tag.as_deref() == Some("button");

        // Enter/Space activates a focused button; Space toggles a focused
        // checkbox — the browser's default keyboard activation.
        if is_button && matches!(code, KeyCode::Enter | KeyCode::Space) {
            rt.engine.dispatch_event(focused, "click", None, true, true);
            continue;
        }
        if is_checkbox && code == KeyCode::Space {
            let now = !rt.dom.borrow().checked(focused);
            rt.dom.borrow_mut().set_checked(focused, now);
            rt.engine.dispatch_event(focused, "change", None, true, false);
            continue;
        }

        // Text input editing: Backspace deletes, printable chars append.
        // `format!` is fine (Phase-1 caret is always end-of-field).
        let is_text_input = tag.as_deref() == Some("input") && !is_checkbox;
        if !is_text_input {
            continue;
        }
        let mut changed = false;
        if code == KeyCode::Backspace {
            let mut cur = rt.dom.borrow().value(focused);
            if cur.pop().is_some() {
                rt.dom.borrow_mut().set_value(focused, &cur);
                changed = true;
            }
        } else if let Some(text) = key_to_text(&key, code) {
            let cur = rt.dom.borrow().value(focused);
            rt.dom.borrow_mut().set_value(focused, &format!("{cur}{text}"));
            changed = true;
        }
        if changed {
            rt.engine.dispatch_event(focused, "input", None, true, false);
        }
    }
    if any {
        rt.dirty = true;
    }
}

/// Focusable elements (buttons + inputs) in document order — the Tab ring.
fn collect_focusable(dom: &superui_dom::Dom) -> Vec<NodeId> {
    fn walk(dom: &superui_dom::Dom, node: NodeId, out: &mut Vec<NodeId>) {
        for &child in dom.children(node) {
            if matches!(dom.tag(child), Some("button") | Some("input")) {
                out.push(child);
            }
            walk(dom, child, out);
        }
    }
    let mut out = Vec::new();
    walk(dom, dom.document(), &mut out);
    out
}

/// The `KeyboardEvent.key` value for a press: the printable character, or a
/// named key for non-printables (only the ones the UI needs). Lets JS do
/// `if (e.key === "Enter") …` (browser-standard).
fn key_name(logical: &Key, code: KeyCode) -> String {
    match code {
        KeyCode::Enter | KeyCode::NumpadEnter => return "Enter".to_string(),
        KeyCode::Backspace => return "Backspace".to_string(),
        KeyCode::Escape => return "Escape".to_string(),
        KeyCode::Tab => return "Tab".to_string(),
        _ => {}
    }
    key_to_text(logical, code).unwrap_or_else(|| "Unidentified".to_string())
}

/// Resolve a printable character for a key press. Real keyboards populate the
/// `logical_key` with a `Character`; synthetic injectors (e.g. `bevy_brp_extras`
/// `send_keys`) leave it `Unidentified`, so fall back to the physical `KeyCode`.
/// Phase-1 scope: unshifted letters, digits, space (enough for authoring todos).
fn key_to_text(logical: &Key, code: KeyCode) -> Option<String> {
    if let Key::Character(s) = logical {
        return Some(s.to_string());
    }
    let ch = match code {
        KeyCode::KeyA => "a", KeyCode::KeyB => "b", KeyCode::KeyC => "c",
        KeyCode::KeyD => "d", KeyCode::KeyE => "e", KeyCode::KeyF => "f",
        KeyCode::KeyG => "g", KeyCode::KeyH => "h", KeyCode::KeyI => "i",
        KeyCode::KeyJ => "j", KeyCode::KeyK => "k", KeyCode::KeyL => "l",
        KeyCode::KeyM => "m", KeyCode::KeyN => "n", KeyCode::KeyO => "o",
        KeyCode::KeyP => "p", KeyCode::KeyQ => "q", KeyCode::KeyR => "r",
        KeyCode::KeyS => "s", KeyCode::KeyT => "t", KeyCode::KeyU => "u",
        KeyCode::KeyV => "v", KeyCode::KeyW => "w", KeyCode::KeyX => "x",
        KeyCode::KeyY => "y", KeyCode::KeyZ => "z",
        KeyCode::Digit0 => "0", KeyCode::Digit1 => "1", KeyCode::Digit2 => "2",
        KeyCode::Digit3 => "3", KeyCode::Digit4 => "4", KeyCode::Digit5 => "5",
        KeyCode::Digit6 => "6", KeyCode::Digit7 => "7", KeyCode::Digit8 => "8",
        KeyCode::Digit9 => "9",
        KeyCode::Space => " ",
        _ => return None,
    };
    Some(ch.to_string())
}

/// Exclusive system: dispatch queued DOM events into the engine, then mark dirty.
pub fn drain_dom_events_system(world: &mut World) {
    let queued = std::mem::take(&mut world.resource_mut::<PendingDomEvents>().0);
    if queued.is_empty() {
        return;
    }
    let Some(mut rt) = world.remove_non_send::<UiRuntime>() else {
        return;
    };
    for e in queued {
        use superui_js::JsEngine;
        rt.engine
            .dispatch_event(e.target, &e.type_, None, e.bubbles, e.cancelable);
    }
    rt.dirty = true;
    world.insert_non_send(rt);
}
