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

/// The coarse boundary the Bevy layers consume so they never name a concrete
/// engine. JS-side mutations arrive as an [`OpBatch`]; nodes are addressed by
/// [`JsNodeId`].
pub trait JsEngine {
    /// Evaluate a script against the current context. `Err` carries a message.
    fn eval(&mut self, script: &str) -> Result<(), String>;

    /// Dispatch a DOM event of `ty` at the shadow-DOM node `target` (W3C
    /// capture→target→bubble, run entirely in JS). Returns whether
    /// `preventDefault()` was called.
    fn dispatch_event(
        &mut self,
        target: JsNodeId,
        ty: &str,
        key: Option<&str>,
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
}
