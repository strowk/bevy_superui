//! The browser-backed [`JsEngine`] implementation: the SAME JS shadow DOM
//! (`js/dom.js`) and reactive runtime, but executed by the page's own JS engine
//! rather than an embedded interpreter. The Rust wasm module holds no engine; it
//! evaluates scripts and calls the bundle's globals across the wasm boundary via
//! `wasm-bindgen`/`js-sys`.
//!
//! Host functions the bundle needs (`__superui_bevy_send`, `__ss_measure`) are
//! Rust closures installed on `globalThis`. `console` and the timer globals
//! (`setTimeout`/`queueMicrotask`/...) are the browser's own — no shim, and the
//! reactive scheduler drives microtasks off the page event loop, so
//! [`run_timers`](JsEngine::run_timers) is a no-op here (unlike Boa/V8, which
//! must pump timers and microtasks explicitly).
//!
//! wasm-only. Single-threaded (the page's main thread).

use std::cell::RefCell;
use std::rc::Rc;

use js_sys::{Array, Function, Reflect, Uint8Array};
use serde_json::Value;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use superui_dom::Dom;

use crate::{JsEngine, OpBatch};

/// The JS shadow DOM. Defines `document`, `__ss_root`, `__ss_flush`,
/// `__ss_dispatch` on `globalThis`; guards `__ss_measure`. Byte-for-byte the
/// bundle the browser runs — identical to the Boa/V8 backends.
const DOM_JS: &str = include_str!("../js/dom.js");

/// A [`JsEngine`] that delegates to the browser's own engine. Owns the shared
/// [`Dom`], the JS→Bevy outbox, and the host closures.
pub struct WebEngine {
    /// The render-mirror DOM, shared with the bridge. Held for parity with the
    /// native backends and a future real `__ss_measure`; the browser answers DOM
    /// reads from its JS state today.
    #[allow(dead_code)]
    dom: Rc<RefCell<Dom>>,
    /// JS→Bevy messages queued by `__superui_bevy_send`, drained per frame.
    /// Shared with the installed closure.
    outbox: Rc<RefCell<Vec<(String, Value)>>>,
    /// Dropping a `Closure` frees the JS shim referencing it, so the installed
    /// globals must be held for the engine's lifetime.
    _bevy_send: Closure<dyn FnMut(JsValue, JsValue)>,
    _measure: Closure<dyn FnMut(JsValue) -> JsValue>,
}

impl WebEngine {
    /// Build an engine sharing `dom`. Installs the host globals the bundle needs,
    /// then evaluates the shadow DOM in the page so `document`/`__ss_root`/
    /// `__ss_flush`/`__ss_dispatch` exist. Does not evaluate the reactive runtime
    /// — that is layered on later via [`eval`](JsEngine::eval).
    pub fn new(dom: Rc<RefCell<Dom>>) -> Self {
        let global = global();
        let outbox: Rc<RefCell<Vec<(String, Value)>>> = Rc::new(RefCell::new(Vec::new()));

        // __superui_bevy_send(name, value): JS→Bevy. Marshal the value through
        // JSON (avoids a serde-wasm-bindgen dependency) and queue it.
        let bevy_send = {
            let outbox = outbox.clone();
            Closure::<dyn FnMut(JsValue, JsValue)>::new(move |name: JsValue, value: JsValue| {
                let name = name.as_string().unwrap_or_default();
                outbox.borrow_mut().push((name, js_to_json(&value)));
            })
        };
        let _ = Reflect::set(
            &global,
            &JsValue::from_str("__superui_bevy_send"),
            bevy_send.as_ref(),
        );

        // __ss_measure(id): layout stub — real taffy rects arrive via the bridge.
        // A zero rect makes getBoundingClientRect/offset* in dom.js read 0,
        // matching the native backends.
        let measure =
            Closure::<dyn FnMut(JsValue) -> JsValue>::new(|_id: JsValue| -> JsValue { zero_rect() });
        let _ = Reflect::set(&global, &JsValue::from_str("__ss_measure"), measure.as_ref());

        // dom.js is a self-installing IIFE that publishes its globals, so
        // indirect `eval` (global scope) is correct. A failure here is
        // non-fatal: keep the engine so later eval() calls still report errors.
        if let Err(e) = js_sys::eval(DOM_JS) {
            web_error("dom.js evaluation failed", &e);
        }

        WebEngine {
            dom,
            outbox,
            _bevy_send: bevy_send,
            _measure: measure,
        }
    }

    /// A clone of the shared DOM handle.
    pub fn dom(&self) -> Rc<RefCell<Dom>> {
        self.dom.clone()
    }
}

impl JsEngine for WebEngine {
    fn eval(&mut self, script: &str) -> Result<(), String> {
        js_sys::eval(script).map(|_| ()).map_err(err_to_string)
    }

    fn dispatch_event(
        &mut self,
        target: crate::JsNodeId,
        ty: &str,
        key: Option<&str>,
        bubbles: bool,
        cancelable: bool,
    ) -> bool {
        let Some(dispatch) = global_fn("__ss_dispatch") else {
            return false;
        };
        // __ss_dispatch takes 5 args — past call3 — so apply an argument array.
        let args = Array::new();
        args.push(&JsValue::from(target));
        args.push(&JsValue::from_str(ty));
        args.push(&key.map_or(JsValue::null(), JsValue::from_str));
        args.push(&JsValue::from_bool(bubbles));
        args.push(&JsValue::from_bool(cancelable));
        match Reflect::apply(&dispatch, &JsValue::undefined(), &args) {
            Ok(ret) => ret.as_bool().unwrap_or(false),
            Err(e) => {
                web_error("__ss_dispatch() failed", &e);
                false
            }
        }
    }

    fn run_timers(&mut self, _now_ms: f64) {
        // No-op on web: the browser owns setTimeout/queueMicrotask and its event
        // loop drives the reactive scheduler's microtasks between frames. Boa/V8
        // must pump their timer queue and microtasks explicitly; the page does
        // not.
    }

    fn flush_ops(&mut self) -> OpBatch {
        let Some(flush) = global_fn("__ss_flush") else {
            web_warn("__ss_flush is not defined");
            return OpBatch::default();
        };
        let value = match flush.call0(&JsValue::undefined()) {
            Ok(v) => v,
            Err(e) => {
                web_error("__ss_flush() failed", &e);
                return OpBatch::default();
            }
        };
        // dom.js returns an Array<number> of bytes. `Uint8Array::new` copies the
        // whole array-like across the boundary in one call and `to_vec` bulk
        // copies it into Rust — the web advantage over Boa's per-element read.
        let bytes = Uint8Array::new(&value).to_vec();
        match OpBatch::decode(&bytes) {
            Ok(batch) => batch,
            Err(e) => {
                web_warn(&format!("op-wire decode failed: {e:?}"));
                OpBatch::default()
            }
        }
    }

    fn emit(&mut self, name: &str, value: &Value) {
        let Some(hook) = global_fn("__ss_emit") else {
            return;
        };
        let payload = json_to_js(value);
        let _ = hook.call2(&JsValue::undefined(), &JsValue::from_str(name), &payload);
    }

    fn drain_outbox(&mut self) -> Vec<(String, Value)> {
        std::mem::take(&mut self.outbox.borrow_mut())
    }
}

// ---- helpers ---------------------------------------------------------------

/// `globalThis` as a [`JsValue`] for `Reflect` lookups and installs.
fn global() -> JsValue {
    js_sys::global().into()
}

/// A `globalThis` property as a callable, if present and callable.
fn global_fn(name: &str) -> Option<Function> {
    Reflect::get(&global(), &JsValue::from_str(name))
        .ok()
        .and_then(|v| v.dyn_into::<Function>().ok())
}

/// A zero layout rect as a plain JS object, mirroring the native `__ss_measure`.
fn zero_rect() -> JsValue {
    let rect = js_sys::Object::new();
    for field in ["x", "y", "width", "height", "top", "left", "right", "bottom"] {
        let _ = Reflect::set(&rect, &JsValue::from_str(field), &JsValue::from_f64(0.0));
    }
    rect.into()
}

/// Marshal a JS value into [`Value`] via `JSON.stringify` + parse. `undefined`
/// and unstringifiable values (e.g. cyclic) become `Null`.
fn js_to_json(value: &JsValue) -> Value {
    match js_sys::JSON::stringify(value) {
        Ok(s) => s
            .as_string()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or(Value::Null),
        Err(_) => Value::Null,
    }
}

/// Marshal a [`Value`] into a JS value via `JSON.parse`, or `undefined` on failure.
fn json_to_js(value: &Value) -> JsValue {
    match serde_json::to_string(value) {
        Ok(s) => js_sys::JSON::parse(&s).unwrap_or(JsValue::UNDEFINED),
        Err(_) => JsValue::UNDEFINED,
    }
}

/// A thrown JS value as a String for `Result::Err`.
fn err_to_string(e: JsValue) -> String {
    if let Some(s) = e.as_string() {
        return s;
    }
    if let Some(err) = e.dyn_ref::<js_sys::Error>() {
        return String::from(err.to_string());
    }
    format!("{e:?}")
}

/// Log to `console.error` with `context`; best effort.
fn web_error(context: &str, e: &JsValue) {
    console_call("error", &format!("{context}: {}", err_to_string(e.clone())));
}

/// Log to `console.warn`; best effort.
fn web_warn(msg: &str) {
    console_call("warn", msg);
}

fn console_call(method: &str, msg: &str) {
    if let Ok(console) = Reflect::get(&global(), &JsValue::from_str("console")) {
        if let Ok(f) = Reflect::get(&console, &JsValue::from_str(method)) {
            if let Ok(f) = f.dyn_into::<Function>() {
                let _ = f.call1(&console, &JsValue::from_str(msg));
            }
        }
    }
}
