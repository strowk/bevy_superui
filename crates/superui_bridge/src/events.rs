//! Input -> DOM event seam. `bevy_picking`/keyboard produce DOM events, which we
//! dispatch into JS (W3C capture/bubble, synchronous) and then reconcile.

use bevy::ecs::message::MessageReader;
use bevy::input::keyboard::{Key, KeyCode, KeyboardInput};
use bevy::input::ButtonState;
use bevy::input_focus::{FocusCause, InputFocus};
use bevy::picking::events::{
    PointerCancel, PointerClick, PointerMove, PointerOut, PointerOver, PointerPress,
    PointerRelease,
};
use bevy::picking::pointer::PointerId;
use bevy::prelude::*;
use bevy::text::EditableText;
use superui_dom::NodeId;
use superui_js::{EventInit, Modifiers, MousePayload, PointerKind};

use crate::runtime::{DomNode, PlaceholderText, UiRuntime};

/// One pending DOM event to dispatch into JS on the next drain.
#[derive(Clone, Debug)]
pub struct PendingDomEvent {
    pub target: NodeId,
    pub type_: String,
    /// `KeyboardEvent.key`, when this event carries one (e.g. a synthetic
    /// `keydown` from the test engine's `press(key)`). `None` for non-key
    /// events (click, input, focus, …).
    pub key: Option<String>,
    /// `clientX`/`clientY`/`button`/`buttons`, when this event carries a mouse
    /// payload (e.g. `mousedown`/`mousemove`). `None` for non-pointer events.
    pub mouse: Option<MousePayload>,
    /// Modifier-key state for a mouse or keyboard event; `None` carries no
    /// `*Key` properties (focus/change/input).
    pub modifiers: Option<Modifiers>,
    pub bubbles: bool,
    pub cancelable: bool,
}

impl PendingDomEvent {
    pub fn new(target: NodeId, type_: &str) -> Self {
        PendingDomEvent {
            target,
            type_: type_.to_string(),
            key: None,
            mouse: None,
            modifiers: None,
            bubbles: true,
            cancelable: true,
        }
    }

    /// Attach a `KeyboardEvent.key` value (browser-standard, e.g. `"Enter"`,
    /// `"Escape"`, `` "`" ``) to a `keydown`/`keyup` event.
    pub fn with_key(mut self, key: &str) -> Self {
        self.key = Some(key.to_string());
        self
    }

    /// Attach a [`MousePayload`] (`clientX`/`clientY`/`button`/`buttons`) to a
    /// pointer event (`mousedown`/`mouseup`/`mousemove`/…).
    pub fn with_mouse(mut self, m: MousePayload) -> Self {
        self.mouse = Some(m);
        self
    }

    /// Attach [`Modifiers`] (`shiftKey`/`ctrlKey`/`altKey`/`metaKey`) to a mouse
    /// or keyboard event.
    pub fn with_modifiers(mut self, m: Modifiers) -> Self {
        self.modifiers = Some(m);
        self
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
/// call it without needing to construct a `PointerClick` event.
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

/// Observer: a pointer click on a UI entity becomes a DOM `click` on its node.
/// For a checkbox input, also mirror the native toggle: flip DOM `checked` and
/// enqueue a `change` event (dispatched after the click). Also sets keyboard focus
/// to the clicked node (Task 5).
pub fn on_pointer_click(
    mut ev: On<PointerClick>,
    nodes: Query<&DomNode>,
    parents: Query<&ChildOf>,
    dom: Option<NonSendMut<UiRuntime>>,
    mut pending: ResMut<PendingDomEvents>,
    mut input_focus: ResMut<InputFocus>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    // No UI is mounted when `UiRuntime` is absent — e.g. the `superui_test --ui`
    // shell (which runs `SuperUiPlugin` in the same world as its egui runner)
    // before the first Run mounts a spec, or between runs while the DOM is torn
    // down. A click then has no DOM to dispatch to, so skip rather than fail
    // param validation on the missing non-send resource (which panics the app).
    let Some(mut dom) = dom else {
        return;
    };
    // Resolve BEFORE claiming the event. This is a global observer on the
    // app-wide `PointerClick`, so it also runs for entities that have nothing
    // to do with superui — a game's own buttons, world objects, another UI. Those
    // must be left alone: stopping propagation for them cancels bubbling to the
    // handler on their ancestor, which is where Bevy UIs put it (the pick lands
    // on a `Text` child and only reaches the button by propagation).
    let Some(node) = resolve_dom_node(ev.entity, &nodes, &parents) else {
        return;
    };
    // Ours. `PointerClick` bubbles up the entity hierarchy, firing this observer
    // once per ancestor. We only want the actual (deepest) target — otherwise focus
    // would be overwritten by each ancestor up to `<body>`. Stop propagation so we
    // handle the click exactly once. (DOM-level bubbling is done separately by our
    // own W3C dispatch in `click_effect`/`dispatch_event`.)
    ev.propagate(false);
    let mods = modifiers_now(&keys);
    focus_and_click(node, &mut dom, &mut pending, &mut input_focus, mods);
}

/// The active primary touch finger, if any. A touch tap synthesizes the browser
/// compatibility mouse sequence (`mousemove`/`mouseover`/`mouseenter`/`mousedown`
/// on press, `mouseup` on release, plus `click` via the existing `on_pointer_click`
/// observer); this slot records which finger is doing so, so only the first
/// concurrent finger does, mirroring a browser's single synthesized mouse pointer.
#[derive(Resource, Default)]
pub struct PrimaryTouch(pub Option<PointerId>);

/// A touch-synthesized `MousePayload`: the touch position as `clientX`/`clientY`,
/// with the caller-supplied `button`/`buttons` state.
fn touch_payload(pos: Vec2, button: u8, buttons: u8) -> MousePayload {
    MousePayload { client_x: pos.x, client_y: pos.y, button, buttons, ..Default::default() }
}

/// Enqueue the browser compatibility mouse sequence for a primary-touch press:
/// `mousemove`, `mouseover`, `mouseenter` (non-bubbling), then `mousedown`
/// (button 0 down: `button = 0`, `buttons = 1`). All carry empty `Modifiers`
/// (touch has no keyboard-modifier concept of its own) and the touch position.
fn enqueue_touch_press(node: NodeId, pos: Vec2, pending: &mut PendingDomEvents) {
    let idle = touch_payload(pos, 0, 0);
    let down = touch_payload(pos, 0, 1);
    let mods = Modifiers::default();
    pending.0.push(PendingDomEvent::new(node, "mousemove").with_mouse(idle).with_modifiers(mods));
    pending.0.push(PendingDomEvent::new(node, "mouseover").with_mouse(idle).with_modifiers(mods));
    let mut enter = PendingDomEvent::new(node, "mouseenter").with_mouse(idle).with_modifiers(mods);
    enter.bubbles = false;
    pending.0.push(enter);
    pending.0.push(PendingDomEvent::new(node, "mousedown").with_mouse(down).with_modifiers(mods));
}

/// Observer: the primary-touch finger's initial press synthesizes the browser
/// compatibility mouse sequence into `PendingDomEvents` — the touch-tap half of
/// making touch devices behave like a mouse to code that only listens for mouse
/// events. Only the first concurrent finger does this (`PrimaryTouch`); a second
/// finger landing while the first is still down is ignored, like a browser's
/// single synthesized mouse pointer.
pub fn on_touch_press(
    mut ev: On<PointerPress>,
    nodes: Query<&DomNode>,
    parents: Query<&ChildOf>,
    mut pending: ResMut<PendingDomEvents>,
    mut primary: ResMut<PrimaryTouch>,
) {
    // Touch only; a mouse (or other custom) pointer returns immediately without
    // touching propagation, same reasoning as `on_pointer_click`'s foreign-entity
    // guard below.
    let PointerId::Touch(_) = ev.pointer.id else { return };
    // Resolve BEFORE touching `PrimaryTouch` — a `None` resolve (host-game entity)
    // returns without claiming the event or consuming the primary slot, so a
    // foreign touch doesn't block a later real tap from becoming primary.
    let Some(node) = resolve_dom_node(ev.entity, &nodes, &parents) else { return };
    // Ours: stop propagation so this fires once (mirror `on_pointer_click`).
    ev.propagate(false);
    // Adopt the first active finger as primary; ignore a second concurrent finger.
    if primary.0.is_some() {
        return;
    }
    primary.0 = Some(ev.pointer.id);
    enqueue_touch_press(node, ev.pointer.position, &mut pending);
}

/// Enqueue the browser compatibility mouse sequence for a primary-touch release:
/// a single `mouseup` (button 0 up: `button = 0`, `buttons = 0`), bubbling, with
/// empty `Modifiers` and the touch position (mirrors `enqueue_touch_press`).
fn enqueue_touch_release(node: NodeId, pos: Vec2, pending: &mut PendingDomEvents) {
    let up = touch_payload(pos, 0, 0);
    let mods = Modifiers::default();
    pending.0.push(PendingDomEvent::new(node, "mouseup").with_mouse(up).with_modifiers(mods));
}

/// Observer: the primary-touch finger's release synthesizes the compatibility
/// `mouseup` into `PendingDomEvents` and frees the primary slot — the release
/// half of `on_touch_press`'s tap-to-mouse sequence. A non-primary finger's
/// release (a second finger lifting while the primary is still down) is
/// ignored, same as `on_touch_press` ignoring a second finger's press.
pub fn on_touch_release(
    mut ev: On<PointerRelease>,
    nodes: Query<&DomNode>,
    parents: Query<&ChildOf>,
    mut pending: ResMut<PendingDomEvents>,
    mut primary: ResMut<PrimaryTouch>,
) {
    let PointerId::Touch(_) = ev.pointer.id else { return };
    // Non-primary release ignored; leaves the primary slot untouched.
    if primary.0 != Some(ev.pointer.id) {
        return;
    }
    // The primary finger's gesture has ended regardless of where it resolves —
    // always free the slot here, BEFORE resolving to a `DomNode`. Bevy picking
    // only fires `PointerRelease` on a previously-hovered entity, which can be a
    // host-game entity with no `DomNode` (`resolve_dom_node` returns `None`); if
    // clearing were gated on a successful resolve, that case would leave
    // `PrimaryTouch` stuck `Some` forever, permanently blocking every future
    // `on_touch_press` (it only adopts when the slot is `None`).
    primary.0 = None;
    let Some(node) = resolve_dom_node(ev.entity, &nodes, &parents) else { return };
    // Ours: stop propagation so this fires once (mirror `on_pointer_click`), but
    // only once resolved — an unresolved release has nothing to claim.
    ev.propagate(false);
    enqueue_touch_release(node, ev.pointer.position, &mut pending);
}

/// Walk up from `entity` to the nearest ancestor that carries a [`DomNode`],
/// returning `None` when the entity belongs to no mounted UI.
///
/// The hit entity may be a reconciler-internal child (e.g. an input's managed
/// text child, or an element's `Text` child) with no `DomNode` — so clicking
/// anywhere inside the input resolves to the input, like a browser.
pub fn resolve_dom_node(
    entity: Entity,
    nodes: &Query<&DomNode>,
    parents: &Query<&ChildOf>,
) -> Option<NodeId> {
    let mut cur = entity;
    loop {
        if let Ok(dom_node) = nodes.get(cur) {
            return Some(dom_node.0);
        }
        match parents.get(cur) {
            Ok(parent) => cur = parent.parent(),
            Err(_) => return None,
        }
    }
}

/// JS `PointerEvent.pointerId` for a Bevy `PointerId` — the numbering this
/// bridge invents: `Mouse` is always `1` (the convention most browsers use for
/// their single mouse pointer); `Touch(n)` is `(n as u32).wrapping_add(2)`, so
/// it never collides with `1`. `Custom` (mocked/software pointers) has no
/// canonical browser id and isn't expected to flow through this bridge; it
/// maps to `0`.
pub fn js_pointer_id(id: PointerId) -> u32 {
    match id {
        PointerId::Mouse => 1,
        PointerId::Touch(n) => (n as u32).wrapping_add(2),
        PointerId::Custom(_) => 0,
    }
}

/// Inverse of [`js_pointer_id`] for the `Mouse`/`Touch` numbering it defines
/// (there is no way back to `Custom`, which that numbering never produces).
pub fn bevy_pointer_id(js: u32) -> PointerId {
    if js == 1 {
        PointerId::Mouse
    } else {
        PointerId::Touch(js.wrapping_sub(2) as u64)
    }
}

/// `PointerEvent.pointerType` for a picking `PointerId`. `Custom` (mocked or
/// software-controlled pointers) reports as `Mouse`, the closer W3C analogue
/// absent a third `PointerKind` variant.
pub fn pointer_kind(id: PointerId) -> PointerKind {
    match id {
        PointerId::Touch(_) => PointerKind::Touch,
        _ => PointerKind::Mouse,
    }
}

/// `PointerEvent.isPrimary` for `id`. A mouse pointer is always primary (there
/// is only ever one). A touch pointer is primary iff no other touch currently
/// holds `PrimaryTouch`'s slot, or it holds it itself — order-independent,
/// unlike the touch-compat mouse stream (`PrimaryTouch`'s own consumer), which
/// only ever adopts the first concurrent finger and ignores every other.
pub fn pointer_is_primary(id: PointerId, primary: &PrimaryTouch) -> bool {
    match id {
        PointerId::Touch(_) => primary.0.is_none() || primary.0 == Some(id),
        _ => true,
    }
}

/// A pointer-event `MousePayload`: position + button state + the W3C pointer
/// identity fields (`pointerId`/`pointerType`/`isPrimary`).
fn pointer_payload(
    pos: Vec2,
    button: u8,
    buttons: u8,
    pointer_id: u32,
    pointer_type: PointerKind,
    is_primary: bool,
) -> MousePayload {
    MousePayload {
        client_x: pos.x,
        client_y: pos.y,
        button,
        buttons,
        pointer_id,
        pointer_type,
        is_primary,
    }
}

/// Captured pointers: JS `pointerId` -> the node that holds its capture (via
/// `setPointerCapture`, a later task). Each uncaptured-pointer observer below
/// skips a pointer already in this map — the raw-location path (Stage B) owns
/// it instead. Empty until that stage populates it.
#[derive(Resource, Default)]
pub struct PointerCaptures(pub std::collections::HashMap<u32, NodeId>);

/// Shared preamble for every observer below: resolve the hit entity to a
/// `DomNode`, then skip a pointer that's already captured. `None` covers both
/// a foreign entity (nothing of ours to claim) and a captured pointer (the
/// raw-location path owns its events instead) — in either case the caller
/// must return without touching propagation, leaving the event alone for
/// whoever else is handling it.
fn uncaptured_node(
    entity: Entity,
    id: PointerId,
    nodes: &Query<&DomNode>,
    parents: &Query<&ChildOf>,
    captures: &PointerCaptures,
) -> Option<NodeId> {
    let node = resolve_dom_node(entity, nodes, parents)?;
    if captures.0.contains_key(&js_pointer_id(id)) {
        return None;
    }
    Some(node)
}

/// The common `MousePayload` for a pointer event: position + button state +
/// the pointer identity fields derived from `id` (`pointerId`/`pointerType`/
/// `isPrimary`).
fn payload_for(pos: Vec2, button: u8, buttons: u8, id: PointerId, primary: &PrimaryTouch) -> MousePayload {
    pointer_payload(pos, button, buttons, js_pointer_id(id), pointer_kind(id), pointer_is_primary(id, primary))
}

/// Observer: a pointer entering a target's bounds becomes DOM `pointerover`
/// (bubbles) then `pointerenter` (not bubbling) — derived the same way
/// `mouse_events_system` derives `mouseenter` from `mouseover`. Fires for
/// every pointer type; a captured pointer is left to the raw-location path.
pub fn on_pointer_over(
    mut ev: On<PointerOver>,
    nodes: Query<&DomNode>,
    parents: Query<&ChildOf>,
    mut pending: ResMut<PendingDomEvents>,
    primary: Res<PrimaryTouch>,
    captures: Res<PointerCaptures>,
) {
    let id = ev.pointer.id;
    let Some(node) = uncaptured_node(ev.entity, id, &nodes, &parents, &captures) else { return };
    ev.propagate(false);
    let m = payload_for(ev.pointer.position, 0, 0, id, &primary);
    let mods = Modifiers::default();
    pending.0.push(PendingDomEvent::new(node, "pointerover").with_mouse(m).with_modifiers(mods));
    let mut enter = PendingDomEvent::new(node, "pointerenter").with_mouse(m).with_modifiers(mods);
    enter.bubbles = false;
    pending.0.push(enter);
}

/// Observer: a pointer leaving a target's bounds becomes DOM `pointerout`
/// (bubbles) then `pointerleave` (not bubbling) — the `on_pointer_over` mirror.
pub fn on_pointer_out(
    mut ev: On<PointerOut>,
    nodes: Query<&DomNode>,
    parents: Query<&ChildOf>,
    mut pending: ResMut<PendingDomEvents>,
    primary: Res<PrimaryTouch>,
    captures: Res<PointerCaptures>,
) {
    let id = ev.pointer.id;
    let Some(node) = uncaptured_node(ev.entity, id, &nodes, &parents, &captures) else { return };
    ev.propagate(false);
    let m = payload_for(ev.pointer.position, 0, 0, id, &primary);
    let mods = Modifiers::default();
    pending.0.push(PendingDomEvent::new(node, "pointerout").with_mouse(m).with_modifiers(mods));
    let mut leave = PendingDomEvent::new(node, "pointerleave").with_mouse(m).with_modifiers(mods);
    leave.bubbles = false;
    pending.0.push(leave);
}

/// Observer: a pointer button pressed on a target becomes DOM `pointerdown`
/// (button 0 down: `button = 0`, `buttons = 1`), for every pointer type —
/// unlike `on_touch_press`, which only synthesizes the browser mouse-compat
/// sequence, and only for touch.
pub fn on_pointer_press(
    mut ev: On<PointerPress>,
    nodes: Query<&DomNode>,
    parents: Query<&ChildOf>,
    mut pending: ResMut<PendingDomEvents>,
    primary: Res<PrimaryTouch>,
    captures: Res<PointerCaptures>,
) {
    let id = ev.pointer.id;
    let Some(node) = uncaptured_node(ev.entity, id, &nodes, &parents, &captures) else { return };
    ev.propagate(false);
    let m = payload_for(ev.pointer.position, 0, 1, id, &primary);
    pending.0.push(PendingDomEvent::new(node, "pointerdown").with_mouse(m).with_modifiers(Modifiers::default()));
}

/// Observer: a pointer button released on a target becomes DOM `pointerup`
/// (button 0 up: `button = 0`, `buttons = 0`).
pub fn on_pointer_release(
    mut ev: On<PointerRelease>,
    nodes: Query<&DomNode>,
    parents: Query<&ChildOf>,
    mut pending: ResMut<PendingDomEvents>,
    primary: Res<PrimaryTouch>,
    captures: Res<PointerCaptures>,
) {
    let id = ev.pointer.id;
    let Some(node) = uncaptured_node(ev.entity, id, &nodes, &parents, &captures) else { return };
    ev.propagate(false);
    let m = payload_for(ev.pointer.position, 0, 0, id, &primary);
    pending.0.push(PendingDomEvent::new(node, "pointerup").with_mouse(m).with_modifiers(Modifiers::default()));
}

/// Observer: a pointer moving over a target becomes DOM `pointermove`.
pub fn on_pointer_move(
    mut ev: On<PointerMove>,
    nodes: Query<&DomNode>,
    parents: Query<&ChildOf>,
    mut pending: ResMut<PendingDomEvents>,
    primary: Res<PrimaryTouch>,
    captures: Res<PointerCaptures>,
) {
    let id = ev.pointer.id;
    let Some(node) = uncaptured_node(ev.entity, id, &nodes, &parents, &captures) else { return };
    ev.propagate(false);
    let m = payload_for(ev.pointer.position, 0, 0, id, &primary);
    pending.0.push(PendingDomEvent::new(node, "pointermove").with_mouse(m).with_modifiers(Modifiers::default()));
}

/// Observer: a pointer's gesture is canceled (palm rejection, the browser
/// taking a touch over for its own scroll/zoom, …) becomes DOM `pointercancel`.
pub fn on_pointer_cancel(
    mut ev: On<PointerCancel>,
    nodes: Query<&DomNode>,
    parents: Query<&ChildOf>,
    mut pending: ResMut<PendingDomEvents>,
    primary: Res<PrimaryTouch>,
    captures: Res<PointerCaptures>,
) {
    let id = ev.pointer.id;
    let Some(node) = uncaptured_node(ev.entity, id, &nodes, &parents, &captures) else { return };
    ev.propagate(false);
    let m = payload_for(ev.pointer.position, 0, 0, id, &primary);
    pending.0.push(PendingDomEvent::new(node, "pointercancel").with_mouse(m).with_modifiers(Modifiers::default()));
}

/// Focus a resolved node and enqueue its `click` (+ checkbox `change`) DOM event.
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

/// The core of a pointer click on a UI `entity`: resolve it to a DOM node, focus
/// it, and enqueue the `click` (+ checkbox `change`) DOM event. Shared by the
/// picking observer and by test/automation drivers that can't synthesize a real
/// `PointerClick` (e.g. the `mcp_debug` click injector). Silently does nothing
/// when the entity belongs to no mounted UI.
pub fn apply_pointer_click(
    entity: Entity,
    nodes: &Query<&DomNode>,
    parents: &Query<&ChildOf>,
    rt: &mut UiRuntime,
    pending: &mut PendingDomEvents,
    input_focus: &mut InputFocus,
    modifiers: Modifiers,
) {
    if let Some(node) = resolve_dom_node(entity, nodes, parents) {
        focus_and_click(node, rt, pending, input_focus, modifiers);
    }
}

/// Observer: an entity gained input focus. Dispatch DOM `focus`, snapshot the
/// current value for change-on-blur, and update the runtime focus mirror.
pub fn on_focus_gained(
    mut ev: On<bevy::input_focus::FocusGained>,
    nodes: Query<&DomNode>,
    parents: Query<&ChildOf>,
    rt: Option<NonSendMut<UiRuntime>>,
    mut pending: ResMut<PendingDomEvents>,
) {
    // FocusGained auto-propagates up the hierarchy; a global observer would
    // otherwise fire once per ancestor. Handle the focused entity exactly once.
    ev.propagate(false);
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
    mut ev: On<bevy::input_focus::FocusLost>,
    nodes: Query<&DomNode>,
    parents: Query<&ChildOf>,
    rt: Option<NonSendMut<UiRuntime>>,
    mut pending: ResMut<PendingDomEvents>,
) {
    // FocusLost auto-propagates; handle the blurred entity exactly once.
    ev.propagate(false);
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

/// Format `value` to the decimal precision implied by `step`, so `.value` reads
/// back a clean number (e.g. "0.3", not "0.30000004").
pub fn format_slider_value(value: f32, step: f32) -> String {
    let decimals = step_decimals(step);
    let s = format!("{value:.decimals$}");
    if decimals == 0 { s } else { s.trim_end_matches('0').trim_end_matches('.').to_string() }
}

fn step_decimals(step: f32) -> usize {
    if step <= 0.0 || step.fract() == 0.0 {
        return 0;
    }
    let s = format!("{step}");
    s.split_once('.').map(|(_, frac)| frac.len()).unwrap_or(0)
}

/// The single seam turning `bevy_ui_widgets` slider input into DOM events. On each
/// `ValueChange`: self-update `SliderValue` (`SliderPlugin` doesn't do this itself —
/// it's the app's job), mirror the value into DOM `value` (recording it in
/// `range_synced`, the echo-guard's last-agreed value), and emit `input` (always)
/// plus `change` (on commit).
pub fn on_slider_value_change(
    ev: On<bevy::ui_widgets::ValueChange<f32>>,
    steps: Query<&bevy::ui_widgets::SliderStep>,
    nodes: Query<&DomNode>,
    rt: Option<NonSendMut<UiRuntime>>,
    mut commands: Commands,
    mut pending: ResMut<PendingDomEvents>,
) {
    let source = ev.source;
    let Some(mut rt) = rt else { return };
    let Some(node) = nodes.get(source).ok().map(|d| d.0) else { return };
    let value = ev.value;

    commands.entity(source).insert(bevy::ui_widgets::SliderValue(value));

    let step = steps.get(source).map(|s| s.0).unwrap_or(1.0);
    let text = format_slider_value(value, step);
    // `range_synced` guards the next reconcile's `sync_range_input`, which reparses
    // the DOM's *formatted* string — so this must record that reparsed value, not
    // the raw `value`, or float drift beyond step precision (e.g. 0.30000004 vs the
    // "0.3" reconcile reads back) makes the echo guard misfire.
    let mirrored = text.parse::<f32>().unwrap_or(value);
    rt.dom.borrow_mut().set_value(node, &text);
    rt.range_synced.insert(node, mirrored);

    let mut input = PendingDomEvent::new(node, "input");
    input.cancelable = false;
    pending.0.push(input);
    if ev.is_final {
        let mut c = PendingDomEvent::new(node, "change");
        c.cancelable = false;
        pending.0.push(c);
    }
    rt.dirty = true;
}

fn tag_of(dom: &superui_dom::Dom, node: NodeId) -> Option<String> {
    match dom.get(node).map(|n| &n.kind) {
        Some(superui_dom::NodeKind::Element(e)) => Some(e.tag.clone()),
        _ => None,
    }
}

/// Normal system: route keyboard input to the focused DOM node as `keydown`/`keyup`,
/// plus Tab focus-cycling and Enter/Space activation for buttons and checkboxes.
/// Text-input editing is `EditableText`'s job now (`editable_input_events_system`).
///
/// `NonSendMut<UiRuntime>` forces main-thread execution.
pub fn keyboard_events_system(
    mut reader: MessageReader<KeyboardInput>,
    mut rt: NonSendMut<UiRuntime>,
    mut input_focus: ResMut<InputFocus>,
    keys: Res<ButtonInput<KeyCode>>,
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
    let mods = modifiers_now(&keys);
    let mut any = false;
    for (key, code, pressed) in presses {
        let Some(focused) = rt.focused else {
            continue;
        };
        let type_ = if pressed { "keydown" } else { "keyup" };
        let kn = key_name(&key, code);
        rt.dispatch_dom_event(
            focused,
            type_,
            Some(&kn),
            EventInit { modifiers: Some(mods), ..Default::default() },
            true,
            true,
        );
        any = true;
        if !pressed {
            continue;
        }

        // Tab moves keyboard focus to the next focusable element (browser std).
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

        let tag = rt.dom.borrow().tag(focused).map(|s| s.to_string());
        let is_checkbox = tag.as_deref() == Some("input")
            && rt.dom.borrow().get_attribute(focused, "type") == Some("checkbox");
        let is_button = tag.as_deref() == Some("button");

        // Enter/Space activates a focused button; Space toggles a focused
        // checkbox — the browser's default keyboard activation.
        if is_button && matches!(code, KeyCode::Enter | KeyCode::Space) {
            rt.dispatch_dom_event(
                focused,
                "click",
                None,
                EventInit { modifiers: Some(mods), ..Default::default() },
                true,
                true,
            );
            continue;
        }
        if is_checkbox && code == KeyCode::Space {
            let now = !rt.dom.borrow().checked(focused);
            rt.dom.borrow_mut().set_checked(focused, now);
            rt.dispatch_dom_event(focused, "change", None, EventInit::default(), true, false);
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
        rt.dispatch_dom_event(
            e.target,
            &e.type_,
            e.key.as_deref(),
            EventInit { mouse: e.mouse, modifiers: e.modifiers },
            e.bubbles,
            e.cancelable,
        );
    }
    rt.dirty = true;
    world.insert_non_send(rt);
}

/// Emit DOM `input` (and mirror the buffer to DOM `value`) when a user edit
/// changes an `EditableText`. Compares against `UiRuntime::editable_synced` (the
/// value the reconciler last pushed into the buffer), not a live re-read of the
/// DOM: `Changed<EditableText>` is observed one frame after the mutation, and by
/// then the live DOM value may have moved again (e.g. a controlled input's next
/// JS write), which would misread that unrelated move as the edit.
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
        let synced = rt.editable_synced.get(&node).cloned().unwrap_or_default();
        if synced != val {
            rt.dom.borrow_mut().set_value(node, &val);
            rt.editable_synced.insert(node, val);
            let mut ev = PendingDomEvent::new(node, "input");
            ev.cancelable = false;
            pending.0.push(ev);
            rt.dirty = true;
        }
    }
}

/// Fade each placeholder overlay to a translucent version of its input's resolved
/// text color. Runs after flair's cascade, which would otherwise give the overlay
/// the same inherited `color` as the typed value; superui has no `::placeholder`
/// selector, so this is the placeholder's only styling.
pub fn dim_placeholder_text_system(
    inputs: Query<&TextColor, Without<PlaceholderText>>,
    mut overlays: Query<(&ChildOf, &mut TextColor), With<PlaceholderText>>,
) {
    for (child_of, mut color) in &mut overlays {
        let Ok(input_color) = inputs.get(child_of.parent()) else {
            continue;
        };
        let base = input_color.0;
        let faded = base.with_alpha(base.alpha() * 0.5);
        if color.0 != faded {
            color.0 = faded;
        }
    }
}

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
