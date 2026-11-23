//! JS engine boundary for bevy_superui.
//!
//! The engine runs the framework-free JS shadow DOM (`js/dom.js`): author and
//! reactive JS mutate a JS-side tree that records primitive ops. Once per frame
//! the host [`JsEngine::flush_ops`] decodes that batch (see [`opwire`]) so the
//! bridge can replay it onto the render mirror. Knows nothing about Bevy.
//! Headless-testable.
//!
//! The backend is chosen at compile time via two mutually-exclusive features:
//! `engine-v8` (default; native, via `deno_core`/V8) and `engine-web` (wasm,
//! runs in the browser's own JS engine). Build one with [`new_engine`] rather
//! than naming a concrete engine type.

#[cfg(all(feature = "engine-v8", feature = "engine-web"))]
compile_error!("superui_js: enable exactly one engine feature (engine-v8 XOR engine-web)");
#[cfg(not(any(feature = "engine-v8", feature = "engine-web")))]
compile_error!("superui_js: select an engine feature: engine-v8 (native) or engine-web (wasm)");
#[cfg(all(feature = "engine-v8", target_arch = "wasm32"))]
compile_error!("superui_js: engine-v8 is native-only");
#[cfg(all(feature = "engine-web", not(target_arch = "wasm32")))]
compile_error!("superui_js: engine-web is wasm-only");

#[cfg(feature = "engine-v8")]
mod engine_v8;
#[cfg(all(feature = "engine-web", target_arch = "wasm32"))]
mod engine_web;
pub mod opwire;

#[cfg(feature = "engine-v8")]
pub use engine_v8::V8Engine;
#[cfg(all(feature = "engine-web", target_arch = "wasm32"))]
pub use engine_web::WebEngine;
pub use opwire::{JsNodeId, OpBatch};

use std::cell::RefCell;
use std::rc::Rc;

use superui_dom::Dom;

/// Build the compile-time-selected [`JsEngine`] backend on `deno_core`/V8.
#[cfg(feature = "engine-v8")]
pub fn new_engine(dom: Rc<RefCell<Dom>>) -> Box<dyn JsEngine> {
    Box::new(V8Engine::new(dom))
}

/// Build the compile-time-selected [`JsEngine`] backend on the browser's own JS
/// engine (wasm-only; guarded above).
#[cfg(feature = "engine-web")]
pub fn new_engine(dom: Rc<RefCell<Dom>>) -> Box<dyn JsEngine> {
    Box::new(WebEngine::new(dom))
}

/// The device class behind a `PointerEvent` (`PointerEvent.pointerType`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PointerKind {
    #[default]
    Mouse,
    Touch,
}

impl PointerKind {
    /// The W3C `PointerEvent.pointerType` string (`"mouse"`/`"touch"`).
    pub fn as_str(self) -> &'static str {
        match self {
            PointerKind::Mouse => "mouse",
            PointerKind::Touch => "touch",
        }
    }
}

/// Mouse coordinates and button state carried through [`JsEngine::dispatch_event`]
/// for pointer event types (`mousedown`/`mouseup`/`mousemove`/...). `button` is
/// the W3C `MouseEvent.button` of the event itself; `buttons` is the bitmask of
/// all buttons currently held.
///
/// `pointer_id`/`pointer_type`/`is_primary` additionally carry the W3C
/// `PointerEvent` fields (`PointerEvent.pointerId`/`pointerType`/`isPrimary`);
/// `dispatch_event`/`dom.js` only attach them to the event object for pointer
/// event types (`pointerdown`/`pointermove`/...) — a plain `MouseEvent` never
/// sees them, so these fields are inert on the existing mouse/compat stream.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct MousePayload {
    pub client_x: f32,
    pub client_y: f32,
    pub button: u8,
    pub buttons: u8,
    pub pointer_id: u32,
    pub pointer_type: PointerKind,
    pub is_primary: bool,
}

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

/// The coarse boundary the Bevy layers consume so they never name a concrete
/// engine. JS-side mutations arrive as an [`OpBatch`]; nodes are addressed by
/// [`JsNodeId`].
pub trait JsEngine {
    /// Evaluate a script against the current context. `Err` carries a message.
    fn eval(&mut self, script: &str) -> Result<(), String>;

    /// Dispatch a DOM event of `ty` at the shadow-DOM node `target` (W3C
    /// capture→target→bubble, run entirely in JS). `init.mouse` carries
    /// `clientX`/`clientY`/`button`/`buttons` onto the event object for pointer
    /// event types, plus `pointerId`/`pointerType`/`isPrimary` — the latter three
    /// (and the JS-computed `width`/`height`/`pressure`) are attached by `dom.js`
    /// only when `ty` is itself a PointerEvent type (`pointer*`/capture events),
    /// so they never leak onto a plain `MouseEvent`. `init.modifiers` carries
    /// `shiftKey`/`ctrlKey`/`altKey`/`metaKey`. Either or both `None` leave the
    /// corresponding properties `undefined`. Returns whether `preventDefault()`
    /// was called.
    fn dispatch_event(
        &mut self,
        target: JsNodeId,
        ty: &str,
        key: Option<&str>,
        init: EventInit,
        bubbles: bool,
        cancelable: bool,
    ) -> bool;

    /// Advance the timer clock to `now_ms`, fire all due timers (intervals
    /// reschedule), then drain the microtask queue.
    fn run_timers(&mut self, now_ms: f64);

    /// Flush the JS shadow DOM's queued mutations into a decoded [`OpBatch`].
    fn flush_ops(&mut self) -> OpBatch;

    /// Bevy→JS: invoke the optional `globalThis.__ss_emit(name, value)` hook.
    /// No-op if the JS side has not installed one.
    fn emit(&mut self, name: &str, value: &serde_json::Value);

    /// JS→Bevy: take the messages `__superui_bevy_send` queued this frame.
    fn drain_outbox(&mut self) -> Vec<(String, serde_json::Value)>;

    /// Replace the engine's geometry snapshot for `document.elementFromPoint`
    /// (the `__ss_element_from_point` host binding reads it). Called once per
    /// frame by `sync_hit_rects_system`; an empty `Vec` makes every probe miss.
    fn set_hit_rects(&mut self, rects: Vec<HitRect>);
}

/// One node's axis-aligned screen-space bounding box, snapshotted per frame by
/// `sync_hit_rects_system` from `bevy_ui` layout for `element_from_point`'s
/// hit-test. Coordinates are in the same logical-window-pixel space as
/// `MouseEvent.clientX`/`clientY`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HitRect {
    pub js_id: JsNodeId,
    pub min: (f32, f32),
    pub max: (f32, f32),
    /// DOM-tree depth (document root = 0); the containing rect with the
    /// greatest depth wins a hit, matching painter's-order "topmost" semantics.
    pub depth: u32,
}

/// Pure hit-test backing `document.elementFromPoint(x, y)`: the `js_id` of the
/// containing rect with the greatest `depth` (ties broken by last-wins, i.e.
/// snapshot order), or `None` if no rect contains `(x, y)`.
pub fn element_from_point(rects: &[HitRect], x: f32, y: f32) -> Option<JsNodeId> {
    rects
        .iter()
        .filter(|r| r.min.0 <= x && x <= r.max.0 && r.min.1 <= y && y <= r.max.1)
        .max_by_key(|r| r.depth)
        .map(|r| r.js_id)
}

#[cfg(test)]
mod element_from_point_tests {
    use super::{element_from_point, HitRect};

    #[test]
    fn point_inside_single_node_returns_it() {
        let rects = [HitRect { js_id: 2, min: (0.0, 0.0), max: (10.0, 10.0), depth: 0 }];
        assert_eq!(element_from_point(&rects, 5.0, 5.0), Some(2));
    }

    #[test]
    fn overlapping_nodes_topmost_by_depth_wins() {
        let parent = HitRect { js_id: 1, min: (0.0, 0.0), max: (100.0, 100.0), depth: 0 };
        let child = HitRect { js_id: 2, min: (10.0, 10.0), max: (20.0, 20.0), depth: 1 };
        let rects = [parent, child];
        assert_eq!(element_from_point(&rects, 15.0, 15.0), Some(2));
        assert_eq!(element_from_point(&rects, 50.0, 50.0), Some(1));
    }

    #[test]
    fn point_outside_all_nodes_is_none() {
        let rects = [HitRect { js_id: 1, min: (0.0, 0.0), max: (10.0, 10.0), depth: 0 }];
        assert_eq!(element_from_point(&rects, 500.0, 500.0), None);
    }

    #[test]
    fn ties_in_depth_resolve_to_the_last_rect() {
        let a = HitRect { js_id: 1, min: (0.0, 0.0), max: (10.0, 10.0), depth: 0 };
        let b = HitRect { js_id: 2, min: (0.0, 0.0), max: (10.0, 10.0), depth: 0 };
        assert_eq!(element_from_point(&[a, b], 5.0, 5.0), Some(2));
    }
}
