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
    ev: On<Pointer<Click>>,
    nodes: Query<&DomNode>,
    mut dom: NonSendMut<UiRuntime>,
    mut pending: ResMut<PendingDomEvents>,
) {
    let Ok(dom_node) = nodes.get(ev.event().entity) else {
        return;
    };
    let node = dom_node.0;
    dom.focused = Some(node);
    click_effect(&dom, node, &mut pending);
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
    let Some(focused) = rt.focused else {
        return;
    };

    use superui_js::JsEngine;
    let mut any = false;
    for (key, code, pressed) in presses {
        let type_ = if pressed { "keydown" } else { "keyup" };
        let kn = key_name(&key, code);
        rt.engine.dispatch_event(focused, type_, Some(&kn), true, true);
        any = true;
        if !pressed {
            continue;
        }

        let is_text_input = {
            let d = rt.dom.borrow();
            matches!(tag_of(&d, focused).as_deref(), Some("input"))
                && d.get_attribute(focused, "type").unwrap_or("text") != "checkbox"
        };
        if !is_text_input {
            continue;
        }

        // Editing keys for a text input: Backspace deletes, printable chars
        // append. `format!` is fine (Phase-1 caret is always end-of-field).
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
    let Some(mut rt) = world.remove_non_send_resource::<UiRuntime>() else {
        return;
    };
    for e in queued {
        use superui_js::JsEngine;
        rt.engine
            .dispatch_event(e.target, &e.type_, None, e.bubbles, e.cancelable);
    }
    rt.dirty = true;
    world.insert_non_send_resource(rt);
}
