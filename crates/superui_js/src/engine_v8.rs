//! The V8-backed [`JsEngine`] implementation: a `deno_core` [`JsRuntime`]
//! running the same JS shadow DOM (`js/dom.js`) as the Boa backend, with an
//! equivalent set of host imports.
//!
//! The host functions the JS bundle needs (`console.*`, `__ss_measure`, the
//! timer globals, `__superui_bevy_send`) are provided as `deno_core` ops and
//! wired onto `globalThis` by a small JS bootstrap ([`BOOTSTRAP_JS`]) that
//! forwards to `Deno.core.ops.*`. Per-frame state (timers + the Bevy-bound
//! outbox) lives in the runtime's [`OpState`], mirroring Boa's `HostState`.
//! Single-threaded, native-only.

use std::cell::RefCell;
use std::rc::Rc;

use deno_core::serde::de::DeserializeOwned;
use deno_core::{extension, op2, serde_v8, v8, JsRuntime, OpState, RuntimeOptions};
use serde_json::{json, Value};

use superui_dom::Dom;

use crate::{JsEngine, OpBatch};

/// The JS shadow DOM. Defines `document`, `__ss_root`, `__ss_flush`,
/// `__ss_dispatch` on `globalThis`; guards `__ss_measure`.
const DOM_JS: &str = include_str!("../js/dom.js");

/// Wires the host ops onto the globals `dom.js` and author JS reference. Runs
/// before `dom.js`, so `setTimeout`, `__ss_measure`, etc. resolve to the ops.
const BOOTSTRAP_JS: &str = r#"
"use strict";
(function () {
  var ops = Deno.core.ops;
  function fmt(args) {
    var out = [];
    for (var i = 0; i < args.length; i++) {
      var a = args[i];
      out.push(typeof a === "string" ? a : String(a));
    }
    return out.join(" ");
  }
  var log = function () { ops.op_ss_log(fmt(arguments)); };
  globalThis.console = { log: log, warn: log, error: log, info: log, debug: log };
  globalThis.__ss_measure = function (id) { return ops.op_ss_measure(id >>> 0); };
  globalThis.setTimeout = function (cb, delay) { return ops.op_ss_set_timeout(cb, +delay || 0); };
  globalThis.setInterval = function (cb, delay) { return ops.op_ss_set_interval(cb, +delay || 0); };
  globalThis.clearTimeout = function (id) { ops.op_ss_clear_timer(+id || 0); };
  globalThis.clearInterval = function (id) { ops.op_ss_clear_timer(+id || 0); };
  globalThis.__superui_bevy_send = function (name, value) {
    ops.op_ss_bevy_send(String(name), value === undefined ? null : value);
  };
})();
"#;

/// A scheduled timer callback fired by [`V8Engine::run_timers`].
struct V8Timer {
    id: u64,
    callback: v8::Global<v8::Function>,
    due_ms: f64,
    /// `Some(period)` for `setInterval`, `None` for `setTimeout`.
    interval_ms: Option<f64>,
}

/// Host state stored in the runtime's [`OpState`]. The ops reach the timer
/// queue and the Bevy outbox through it.
struct V8HostState {
    /// The render-mirror DOM, shared with the bridge. Held for parity with the
    /// Boa backend and a future real `__ss_measure`; unused today.
    #[allow(dead_code)]
    dom: Rc<RefCell<Dom>>,
    timers: Vec<V8Timer>,
    /// Monotonic clock (milliseconds) advanced by `run_timers`.
    now_ms: f64,
    next_timer_id: u64,
    /// JS→Bevy messages queued by `__superui_bevy_send`, drained per frame.
    outbox: Vec<(String, Value)>,
}

impl V8HostState {
    fn new(dom: Rc<RefCell<Dom>>) -> Self {
        V8HostState {
            dom,
            timers: Vec::new(),
            now_ms: 0.0,
            next_timer_id: 1,
            outbox: Vec::new(),
        }
    }
}

extension!(
    superui_host,
    ops = [
        op_ss_log,
        op_ss_measure,
        op_ss_set_timeout,
        op_ss_set_interval,
        op_ss_clear_timer,
        op_ss_bevy_send,
    ],
    options = { dom: Rc<RefCell<Dom>> },
    state = |state, options| {
        state.put(V8HostState::new(options.dom));
    },
);

/// A `deno_core` runtime wired to a shared [`Dom`], running the JS shadow DOM.
/// Single-threaded.
pub struct V8Engine {
    runtime: JsRuntime,
    /// A current-thread tokio runtime, entered around every call that drives
    /// `runtime`. deno_core's V8 platform posts delayed tasks (e.g. idle GC)
    /// through `tokio::runtime::Handle::current`, which panics without an
    /// entered context — so consumers need not enter one themselves.
    tokio: tokio::runtime::Runtime,
}

impl V8Engine {
    /// Build an engine sharing `dom`. Installs the host ops + state, wires the
    /// globals ([`BOOTSTRAP_JS`]), and evaluates the shadow DOM. Does not
    /// evaluate the reactive runtime — that is layered on later via [`eval`].
    ///
    /// [`eval`]: JsEngine::eval
    pub fn new(dom: Rc<RefCell<Dom>>) -> Self {
        let tokio = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("failed to build tokio runtime for engine-v8");
        let runtime = {
            let _guard = tokio.enter();
            JsRuntime::new(RuntimeOptions {
                extensions: vec![superui_host::init(dom)],
                ..Default::default()
            })
        };
        let mut engine = V8Engine { runtime, tokio };
        engine
            .eval(BOOTSTRAP_JS)
            .expect("host-global bootstrap evaluates cleanly");
        engine.eval(DOM_JS).expect("dom.js evaluates cleanly");
        engine
    }

    /// Call the global function `name` with `args` (each marshalled from JSON
    /// via `serde_v8`), deserializing its return into `R`. `Ok(None)` if `name`
    /// is undefined or not callable — used for the optional `__ss_emit` hook.
    fn call_global<R: DeserializeOwned>(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Result<Option<R>, String> {
        deno_core::scope!(scope, self.runtime);
        let global = scope.get_current_context().global(scope);
        let Some(key) = v8::String::new(scope, name) else {
            return Err("failed to allocate v8 string".into());
        };
        let Some(val) = global.get(scope, key.into()) else {
            return Ok(None);
        };
        let Ok(func) = v8::Local::<v8::Function>::try_from(val) else {
            return Ok(None);
        };
        let mut argv: Vec<v8::Local<v8::Value>> = Vec::with_capacity(args.len());
        for a in args {
            match serde_v8::to_v8(scope, a) {
                Ok(v) => argv.push(v),
                Err(e) => return Err(format!("marshalling arg for {name} failed: {e}")),
            }
        }
        let recv = v8::undefined(scope).into();
        let Some(ret) = func.call(scope, recv, &argv) else {
            return Err(format!("call to {name} threw or was terminated"));
        };
        serde_v8::from_v8::<R>(scope, ret)
            .map(Some)
            .map_err(|e| format!("marshalling return of {name} failed: {e}"))
    }

    /// Call a stored timer callback with no arguments, ignoring its result.
    fn call_timer(&mut self, callback: &v8::Global<v8::Function>) {
        deno_core::scope!(scope, self.runtime);
        let recv = v8::undefined(scope).into();
        let func = v8::Local::new(scope, callback);
        let _ = func.call(scope, recv, &[]);
    }
}

impl JsEngine for V8Engine {
    fn eval(&mut self, script: &str) -> Result<(), String> {
        let handle = self.tokio.handle().clone();
        let _rt = handle.enter();
        self.runtime
            .execute_script("<eval>", script.to_string())
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    fn dispatch_event(
        &mut self,
        target: crate::JsNodeId,
        ty: &str,
        key: Option<&str>,
        bubbles: bool,
        cancelable: bool,
    ) -> bool {
        let handle = self.tokio.handle().clone();
        let _rt = handle.enter();
        let args = [
            json!(target),
            json!(ty),
            key.map_or(Value::Null, |k| json!(k)),
            json!(bubbles),
            json!(cancelable),
        ];
        match self.call_global::<bool>("__ss_dispatch", &args) {
            Ok(Some(prevented)) => prevented,
            Ok(None) => false,
            Err(e) => {
                eprintln!("__ss_dispatch() failed: {e}");
                false
            }
        }
    }

    fn run_timers(&mut self, now_ms: f64) {
        let handle = self.tokio.handle().clone();
        let _rt = handle.enter();
        let state = self.runtime.op_state();
        state.borrow_mut().borrow_mut::<V8HostState>().now_ms = now_ms;
        loop {
            // Pop the earliest due timer under a short borrow, then fire it
            // outside the borrow so its callback can re-enter the ops.
            let due = {
                let mut op_state = state.borrow_mut();
                let host = op_state.borrow_mut::<V8HostState>();
                let idx = host
                    .timers
                    .iter()
                    .enumerate()
                    .filter(|(_, t)| t.due_ms <= now_ms)
                    .min_by(|(_, a), (_, b)| a.due_ms.total_cmp(&b.due_ms))
                    .map(|(i, _)| i);
                idx.map(|i| {
                    let cb = host.timers[i].callback.clone();
                    match host.timers[i].interval_ms {
                        Some(period) => host.timers[i].due_ms += period.max(1.0),
                        None => {
                            host.timers.remove(i);
                        }
                    }
                    cb
                })
            };
            match due {
                Some(cb) => self.call_timer(&cb),
                None => break,
            }
        }
        // Drain any microtasks (promise jobs) the callbacks queued.
        self.runtime.v8_isolate().perform_microtask_checkpoint();
    }

    fn flush_ops(&mut self) -> OpBatch {
        let handle = self.tokio.handle().clone();
        let _rt = handle.enter();
        let bytes = match self.call_global::<Vec<u8>>("__ss_flush", &[]) {
            Ok(Some(bytes)) => bytes,
            Ok(None) => {
                eprintln!("__ss_flush is not defined");
                return OpBatch::default();
            }
            Err(e) => {
                eprintln!("__ss_flush() failed: {e}");
                return OpBatch::default();
            }
        };
        match OpBatch::decode(&bytes) {
            Ok(batch) => batch,
            Err(e) => {
                eprintln!("op-wire decode failed: {e:?}");
                OpBatch::default()
            }
        }
    }

    fn emit(&mut self, name: &str, value: &Value) {
        let handle = self.tokio.handle().clone();
        let _rt = handle.enter();
        let args = [json!(name), value.clone()];
        if let Err(e) = self.call_global::<Value>("__ss_emit", &args) {
            eprintln!("__ss_emit() failed: {e}");
        }
    }

    fn drain_outbox(&mut self) -> Vec<(String, Value)> {
        let state = self.runtime.op_state();
        let mut op_state = state.borrow_mut();
        std::mem::take(&mut op_state.borrow_mut::<V8HostState>().outbox)
    }
}

// ---- host ops --------------------------------------------------------------

/// `console.{log,warn,error,...}`: the bootstrap joins args to a string; write
/// it to stderr. Pristine — nothing prints unless the JS actually logs.
#[op2(fast)]
fn op_ss_log(#[string] msg: &str) {
    eprintln!("{msg}");
}

/// Layout stub: real taffy rects arrive via the bridge. Returns a zero rect so
/// `getBoundingClientRect`/`offset*` in `dom.js` read as `0`.
#[op2]
#[serde]
fn op_ss_measure(_id: u32) -> serde_json::Value {
    json!({"x": 0, "y": 0, "width": 0, "height": 0, "top": 0, "left": 0, "right": 0, "bottom": 0})
}

/// Register a timer, returning its id. `now_ms` is the clock last set by
/// `run_timers`; `interval_ms` is `Some` for `setInterval`.
fn schedule(
    state: &Rc<RefCell<OpState>>,
    callback: v8::Global<v8::Function>,
    delay: f64,
    repeating: bool,
) -> u32 {
    let mut op_state = state.borrow_mut();
    let host = op_state.borrow_mut::<V8HostState>();
    let id = host.next_timer_id;
    host.next_timer_id += 1;
    let delay = delay.max(0.0);
    let due_ms = host.now_ms + delay;
    host.timers.push(V8Timer {
        id,
        callback,
        due_ms,
        interval_ms: if repeating { Some(delay) } else { None },
    });
    id as u32
}

#[op2(fast)]
fn op_ss_set_timeout(
    scope: &mut v8::PinScope,
    state: Rc<RefCell<OpState>>,
    callback: v8::Local<v8::Function>,
    delay: f64,
) -> u32 {
    let callback = v8::Global::new(scope, callback);
    schedule(&state, callback, delay, false)
}

#[op2(fast)]
fn op_ss_set_interval(
    scope: &mut v8::PinScope,
    state: Rc<RefCell<OpState>>,
    callback: v8::Local<v8::Function>,
    delay: f64,
) -> u32 {
    let callback = v8::Global::new(scope, callback);
    schedule(&state, callback, delay, true)
}

#[op2(fast)]
fn op_ss_clear_timer(state: &mut OpState, id: f64) {
    let id = id as u64;
    state.borrow_mut::<V8HostState>().timers.retain(|t| t.id != id);
}

#[op2]
fn op_ss_bevy_send(state: &mut OpState, #[string] name: String, #[serde] value: serde_json::Value) {
    state.borrow_mut::<V8HostState>().outbox.push((name, value));
}
