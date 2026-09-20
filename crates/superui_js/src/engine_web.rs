//! The browser-backed [`JsEngine`] implementation: the SAME JS shadow DOM
//! (`js/dom.js`) and reactive runtime, but executed by the page's own JS engine
//! rather than an embedded interpreter. The Rust wasm module holds no engine; it
//! evaluates scripts and calls the bundle's globals across the wasm boundary via
//! `wasm-bindgen`/`js-sys`.
//!
//! Each instance owns a private scope object and runs all its JS against it, so
//! multiple/sequential `SuperUiRoot`s do not clash on the page's shared `window`
//! (native backends get isolation for free from a fresh Context/isolate). Every
//! script is wrapped so `globalThis`/`window`/`self` and the bundle's published
//! names (`document`, `__ss_*`, `$ss`, `render`, `createSignal`, ...) bind to
//! this instance's scope, while JS built-ins (`Array`, `Math`, `setTimeout`,
//! `Promise`, ...) still resolve to the real page globals. Host functions
//! (`__superui_bevy_send`, `__ss_measure`) and `flush`/`dispatch`/`emit` all go
//! through the instance scope.
//!
//! `console` and the timer globals are the browser's own — no shim, and the
//! reactive scheduler drives microtasks off the page event loop, so
//! [`run_timers`](JsEngine::run_timers) is a no-op here (unlike Boa/V8, which
//! must pump timers and microtasks explicitly).
//!
//! wasm-only. Single-threaded (the page's main thread).

use std::cell::RefCell;
use std::rc::Rc;

use js_sys::{Array, Function, Object, Reflect, Uint8Array};
use serde_json::Value;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use superui_dom::Dom;

use crate::{JsEngine, OpBatch};

/// The JS shadow DOM. Defines `document`, `__ss_root`, `__ss_flush`,
/// `__ss_dispatch` on the scope it runs in; guards `__ss_measure`. Byte-for-byte
/// the bundle the browser runs — identical to the Boa/V8 backends.
const DOM_JS: &str = include_str!("../js/dom.js");

/// A [`JsEngine`] that delegates to the browser's own engine, isolated to a
/// private scope. Owns the shared [`Dom`], the scope, the JS→Bevy outbox, and
/// the host closures.
pub struct WebEngine {
    /// The render-mirror DOM, shared with the bridge. Held for parity with the
    /// native backends and a future real `__ss_measure`; the browser answers DOM
    /// reads from its JS state today.
    #[allow(dead_code)]
    dom: Rc<RefCell<Dom>>,
    /// This instance's private global scope. The bundle's `globalThis.* =`
    /// writes land here and its `__ss_flush`/`__ss_dispatch`/`__ss_emit` live
    /// here; dropping the engine drops it, leaving no state on the page `window`.
    scope: Object,
    /// JS→Bevy messages queued by `__superui_bevy_send`, drained per frame.
    /// Shared with the installed closure.
    outbox: Rc<RefCell<Vec<(String, Value)>>>,
    /// Dropping a `Closure` frees the JS shim referencing it, so the host
    /// functions installed on the scope must be held for the engine's lifetime.
    _bevy_send: Closure<dyn FnMut(JsValue, JsValue)>,
    _measure: Closure<dyn FnMut(JsValue) -> JsValue>,
}

impl WebEngine {
    /// Build an engine sharing `dom`. Creates the private scope, installs the
    /// host functions the bundle needs on it, then evaluates the shadow DOM into
    /// it so `document`/`__ss_root`/`__ss_flush`/`__ss_dispatch` exist on the
    /// scope. Does not evaluate the reactive runtime — that is layered on later
    /// via [`eval`](JsEngine::eval).
    pub fn new(dom: Rc<RefCell<Dom>>) -> Self {
        // A plain object (prototype Object.prototype, not `window`): assigning
        // `document` on it cannot hit `window`'s read-only `document` accessor.
        let scope = Object::new();
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
            scope.as_ref(),
            &JsValue::from_str("__superui_bevy_send"),
            bevy_send.as_ref(),
        );

        // __ss_measure(id): layout stub — real taffy rects arrive via the bridge.
        // A zero rect makes getBoundingClientRect/offset* in dom.js read 0,
        // matching the native backends.
        let measure =
            Closure::<dyn FnMut(JsValue) -> JsValue>::new(|_id: JsValue| -> JsValue { zero_rect() });
        let _ = Reflect::set(
            scope.as_ref(),
            &JsValue::from_str("__ss_measure"),
            measure.as_ref(),
        );

        // dom.js runs into the scope first — it defines the shadow `document` and
        // the __ss_* entry points the later runtime/render/app scripts build on.
        // Non-fatal on failure: keep the engine so later eval() calls still
        // report errors rather than panicking.
        if let Err(e) = eval_in_scope(&scope, DOM_JS) {
            web_error("dom.js evaluation failed", &JsValue::from_str(&e));
        }

        WebEngine {
            dom,
            scope,
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
        eval_in_scope(&self.scope, script)
    }

    fn dispatch_event(
        &mut self,
        target: crate::JsNodeId,
        ty: &str,
        key: Option<&str>,
        bubbles: bool,
        cancelable: bool,
    ) -> bool {
        let Some(dispatch) = scope_fn(&self.scope, "__ss_dispatch") else {
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
        let Some(flush) = scope_fn(&self.scope, "__ss_flush") else {
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
        let Some(hook) = scope_fn(&self.scope, "__ss_emit") else {
            return;
        };
        let payload = json_to_js(value);
        let _ = hook.call2(&JsValue::undefined(), &JsValue::from_str(name), &payload);
    }

    fn drain_outbox(&mut self) -> Vec<(String, Value)> {
        std::mem::take(&mut self.outbox.borrow_mut())
    }
}

// ---- scoped eval -----------------------------------------------------------

/// Evaluate `script` against `scope` as its global. The script is wrapped so
/// `globalThis`/`window`/`self` and every name currently published on `scope`
/// (`document`, `render`, `$ss`, `createSignal`, ...) bind to `scope`, while
/// built-ins (`Array`, `Math`, `setTimeout`, ...) fall through to the real page
/// globals via the lexical scope chain. `globalThis.x = ...` inside the script
/// therefore writes to `scope`, and later scripts pick up those names.
///
/// Param injection (not `with`) so strict-mode bundle code resolves correctly.
/// `js_sys::eval` catches JS errors — including a SyntaxError in `script` — so a
/// bad script yields `Err`, never a wasm trap.
fn eval_in_scope(scope: &Object, script: &str) -> Result<(), String> {
    let names: Vec<String> = Object::keys(scope)
        .iter()
        .filter_map(|k| k.as_string())
        .filter(|n| is_bindable_ident(n))
        .collect();
    let params: String = names.iter().map(|n| format!(", {n}")).collect();
    // {n:?} emits a quoted, escaped JS string literal for the member read.
    let args: String = names.iter().map(|n| format!(", __scope__[{n:?}]")).collect();

    // A factory taking the scope, so the scope crosses the boundary as a call
    // argument rather than a temporary page global — re-entrancy safe.
    let factory_src = format!(
        "(function(__scope__){{ return (function(globalThis, window, self{params}){{\n{script}\n}}).apply(__scope__, [__scope__, __scope__, __scope__{args}]); }})"
    );
    let factory: Function = js_sys::eval(&factory_src)
        .map_err(err_to_string)?
        .dyn_into()
        .map_err(|_| "scoped-eval factory is not a function".to_string())?;
    factory
        .call1(&JsValue::undefined(), scope.as_ref())
        .map(|_| ())
        .map_err(err_to_string)
}

/// Whether `name` is safe to bind as a wrapper parameter: a valid JS identifier,
/// not a reserved word, and not one of the fixed params (`globalThis`/`window`/
/// `self`, bound separately).
fn is_bindable_ident(name: &str) -> bool {
    if matches!(name, "globalThis" | "window" | "self") || is_reserved_word(name) {
        return false;
    }
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c == '_' || c == '$' || c.is_ascii_alphabetic() => {}
        _ => return false,
    }
    chars.all(|c| c == '_' || c == '$' || c.is_ascii_alphanumeric())
}

/// Reserved words that cannot be a parameter name. superui's published names
/// avoid these; the guard keeps a future name from producing an unparseable
/// factory.
fn is_reserved_word(name: &str) -> bool {
    matches!(
        name,
        "break" | "case" | "catch" | "class" | "const" | "continue" | "debugger"
            | "default" | "delete" | "do" | "else" | "enum" | "export" | "extends"
            | "false" | "finally" | "for" | "function" | "if" | "import" | "in"
            | "instanceof" | "new" | "null" | "return" | "super" | "switch" | "this"
            | "throw" | "true" | "try" | "typeof" | "var" | "void" | "while" | "with"
            | "yield" | "let" | "static" | "await" | "async" | "implements"
            | "interface" | "package" | "private" | "protected" | "public"
    )
}

// ---- helpers ---------------------------------------------------------------

/// A property of `scope` as a callable, if present and callable.
fn scope_fn(scope: &Object, name: &str) -> Option<Function> {
    Reflect::get(scope.as_ref(), &JsValue::from_str(name))
        .ok()
        .and_then(|v| v.dyn_into::<Function>().ok())
}

/// A zero layout rect as a plain JS object, mirroring the native `__ss_measure`.
fn zero_rect() -> JsValue {
    let rect = Object::new();
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

/// Diagnostics go to the real page `console`, not the instance scope.
fn console_call(method: &str, msg: &str) {
    let global: JsValue = js_sys::global().into();
    if let Ok(console) = Reflect::get(&global, &JsValue::from_str("console")) {
        if let Ok(f) = Reflect::get(&console, &JsValue::from_str(method)) {
            if let Ok(f) = f.dyn_into::<Function>() {
                let _ = f.call1(&console, &JsValue::from_str(msg));
            }
        }
    }
}
