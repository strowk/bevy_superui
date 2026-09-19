//! The Boa-backed [`JsEngine`] implementation: a Boa context running the JS
//! shadow DOM (`js/dom.js`) with a minimal set of host imports.

use std::cell::RefCell;
use std::rc::Rc;

use boa_engine::object::builtins::{JsArray, JsFunction};
use boa_engine::{
    js_string, Context, JsArgs, JsObject, JsResult, JsString, JsValue, NativeFunction, Source,
};

use superui_dom::Dom;

use crate::state::{with_host_state, with_host_state_mut, HostState, Timer};
use crate::{JsEngine, OpBatch};

/// The JS shadow DOM. Defines `document`, `__ss_root`, `__ss_flush`,
/// `__ss_dispatch` on `globalThis`; guards `__ss_measure`.
const DOM_JS: &str = include_str!("../js/dom.js");

/// Build the Boa [`Context`]. Native keeps Boa's default `StdClock` — unchanged.
#[cfg(not(target_arch = "wasm32"))]
fn build_context() -> Context {
    Context::default()
}

/// On `wasm32-unknown-unknown` Boa's default `StdClock` calls
/// `std::time::SystemTime::now()`, which panics ("time not implemented on this
/// platform"). Inject a `web-time`-backed clock instead — Boa's `Date` builtin
/// reads it, so without this the engine dies the first time author JS touches the
/// current time. This path exists only on wasm; native is untouched above.
#[cfg(target_arch = "wasm32")]
fn build_context() -> Context {
    use boa_engine::context::time::{Clock, JsInstant};

    #[derive(Debug, Clone, Copy, Default)]
    struct WebClock;

    impl Clock for WebClock {
        fn now(&self) -> JsInstant {
            let since_epoch = web_time::SystemTime::now()
                .duration_since(web_time::UNIX_EPOCH)
                .unwrap_or_default();
            JsInstant::new(since_epoch.as_secs(), since_epoch.subsec_nanos())
        }
    }

    Context::builder()
        .clock(Rc::new(WebClock))
        .build()
        .expect("failed to build Boa context")
}

/// A Boa JS context wired to a shared [`Dom`], running the JS shadow DOM.
/// Single-threaded.
pub struct BoaEngine {
    pub(crate) context: Context,
    pub(crate) dom: Rc<RefCell<Dom>>,
}

impl BoaEngine {
    /// Build an engine sharing `dom`. Installs [`HostState`] into the realm's
    /// `HostDefined` slot, registers the host imports, and evaluates the shadow
    /// DOM. Does not evaluate the reactive runtime — that is layered on later.
    pub fn new(dom: Rc<RefCell<Dom>>) -> Self {
        let context = build_context();
        context
            .realm()
            .host_defined_mut()
            .insert(HostState::new(dom.clone()));
        let mut engine = BoaEngine { context, dom };
        engine.install_host_imports();
        engine
            .context
            .eval(Source::from_bytes(DOM_JS))
            .expect("dom.js evaluates cleanly");
        engine
    }

    /// Mutable access to the underlying Boa context.
    #[deprecated(note = "removed once superui_api/supersolid_runtime move off Boa internals")]
    pub fn context_mut(&mut self) -> &mut Context {
        &mut self.context
    }

    /// A clone of the shared DOM handle.
    pub fn dom(&self) -> Rc<RefCell<Dom>> {
        self.dom.clone()
    }

    /// Register the minimum host functions `dom.js` and author JS need:
    /// `console.*`, `__ss_measure`, the timer globals, and `__superui_bevy_send`.
    fn install_host_imports(&mut self) {
        let ctx = &mut self.context;

        // console.{log,warn,error}: format args, write to stderr. Pristine —
        // nothing is printed unless the JS actually logs.
        let console = JsObject::with_object_proto(ctx.intrinsics());
        for name in ["log", "warn", "error"] {
            let fun = NativeFunction::from_fn_ptr(console_log).to_js_function(ctx.realm());
            console
                .set(JsString::from(name), JsValue::from(fun), false, ctx)
                .expect("set console method");
        }
        ctx.register_global_property(js_string!("console"), console, boa_engine::property::Attribute::all())
            .expect("register console");

        ctx.register_global_callable(js_string!("__ss_measure"), 1, NativeFunction::from_fn_ptr(ss_measure))
            .expect("register __ss_measure");

        ctx.register_global_callable(js_string!("setTimeout"), 2, NativeFunction::from_fn_ptr(set_timeout))
            .expect("register setTimeout");
        ctx.register_global_callable(js_string!("setInterval"), 2, NativeFunction::from_fn_ptr(set_interval))
            .expect("register setInterval");
        ctx.register_global_callable(js_string!("clearTimeout"), 1, NativeFunction::from_fn_ptr(clear_timer))
            .expect("register clearTimeout");
        ctx.register_global_callable(js_string!("clearInterval"), 1, NativeFunction::from_fn_ptr(clear_timer))
            .expect("register clearInterval");

        ctx.register_global_callable(
            js_string!("__superui_bevy_send"),
            2,
            NativeFunction::from_fn_ptr(bevy_send),
        )
        .expect("register __superui_bevy_send");
    }

    /// Fetch a global by `name` as a callable, if present and callable.
    fn global_fn(&mut self, name: &str) -> Option<JsFunction> {
        let value = self
            .context
            .global_object()
            .get(JsString::from(name), &mut self.context)
            .ok()?;
        value.as_object().and_then(|o| JsFunction::from_object(o.clone()))
    }
}

impl JsEngine for BoaEngine {
    fn eval(&mut self, script: &str) -> Result<(), String> {
        self.context
            .eval(Source::from_bytes(script))
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
        let Some(dispatch) = self.global_fn("__ss_dispatch") else {
            return false;
        };
        let args = [
            JsValue::from(target),
            JsValue::from(JsString::from(ty)),
            key.map_or(JsValue::null(), |k| JsValue::from(JsString::from(k))),
            JsValue::from(bubbles),
            JsValue::from(cancelable),
        ];
        dispatch
            .call(&JsValue::undefined(), &args, &mut self.context)
            .ok()
            .and_then(|v| v.as_boolean())
            .unwrap_or(false)
    }

    fn run_timers(&mut self, now_ms: f64) {
        with_host_state_mut(&mut self.context, |s| s.now_ms = now_ms);
        loop {
            // Pop the earliest due timer (short mutable borrow), fire outside it.
            let due = with_host_state_mut(&mut self.context, |s| {
                let idx = s
                    .timers
                    .iter()
                    .enumerate()
                    .filter(|(_, t)| t.due_ms <= now_ms)
                    .min_by(|(_, a), (_, b)| a.due_ms.total_cmp(&b.due_ms))
                    .map(|(i, _)| i);
                idx.map(|i| {
                    let cb = s.timers[i].callback.clone();
                    match s.timers[i].interval_ms {
                        Some(period) => s.timers[i].due_ms += period.max(1.0),
                        None => {
                            s.timers.remove(i);
                        }
                    }
                    cb
                })
            });
            match due {
                Some(cb) => {
                    let _ = cb.call(&JsValue::undefined(), &[], &mut self.context);
                }
                None => break,
            }
        }
        let _ = self.context.run_jobs(); // drain any microtasks the callbacks queued
    }

    fn flush_ops(&mut self) -> OpBatch {
        let value = match self.context.eval(Source::from_bytes("__ss_flush()")) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("__ss_flush() failed: {e}");
                return OpBatch::default();
            }
        };
        let Some(array) = value.as_object().and_then(|o| JsArray::from_object(o.clone()).ok()) else {
            eprintln!("__ss_flush() did not return an array");
            return OpBatch::default();
        };
        let len = array.length(&mut self.context).unwrap_or(0);
        let mut bytes = Vec::with_capacity(len as usize);
        for i in 0..len {
            let byte = array
                .at(i as i64, &mut self.context)
                .ok()
                .and_then(|v| v.as_number())
                .unwrap_or(0.0);
            bytes.push(byte as u8);
        }
        match OpBatch::decode(&bytes) {
            Ok(batch) => batch,
            Err(e) => {
                eprintln!("op-wire decode failed: {e:?}");
                OpBatch::default()
            }
        }
    }

    fn emit(&mut self, name: &str, value: &serde_json::Value) {
        let Some(hook) = self.global_fn("__ss_emit") else {
            return;
        };
        let payload = JsValue::from_json(value, &mut self.context).unwrap_or(JsValue::undefined());
        let args = [JsValue::from(JsString::from(name)), payload];
        let _ = hook.call(&JsValue::undefined(), &args, &mut self.context);
    }

    fn drain_outbox(&mut self) -> Vec<(String, serde_json::Value)> {
        with_host_state_mut(&mut self.context, |s| std::mem::take(&mut s.outbox))
    }
}

// ---- host import implementations -------------------------------------------

fn console_log(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let mut parts = Vec::with_capacity(args.len());
    for a in args {
        let s = a
            .to_string(context)
            .map(|s| s.to_std_string_escaped())
            .unwrap_or_else(|_| "<unprintable>".to_string());
        parts.push(s);
    }
    eprintln!("{}", parts.join(" "));
    Ok(JsValue::undefined())
}

/// Layout stub: real taffy rects arrive via the bridge. Returns a zero rect so
/// `getBoundingClientRect`/`offset*` in `dom.js` read as `0`.
fn ss_measure(_this: &JsValue, _args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let rect = JsObject::with_object_proto(context.intrinsics());
    for field in ["x", "y", "width", "height", "top", "left", "right", "bottom"] {
        rect.set(JsString::from(field), JsValue::from(0), false, context)?;
    }
    Ok(rect.into())
}

fn schedule(args: &[JsValue], context: &mut Context, repeating: bool) -> JsResult<JsValue> {
    let Some(cb_obj) = args.get_or_undefined(0).as_object() else {
        return Ok(JsValue::from(0));
    };
    let Some(cb) = JsFunction::from_object(cb_obj.clone()) else {
        return Ok(JsValue::from(0));
    };
    let delay = args.get_or_undefined(1).to_number(context)?.max(0.0);
    let now = with_host_state(context, |s| s.now_ms);
    let id = with_host_state_mut(context, |s| {
        let id = s.next_timer_id;
        s.next_timer_id += 1;
        s.timers.push(Timer {
            id,
            callback: cb,
            due_ms: now + delay,
            interval_ms: if repeating { Some(delay) } else { None },
        });
        id
    });
    Ok(JsValue::from(id as u32))
}

fn set_timeout(_t: &JsValue, a: &[JsValue], c: &mut Context) -> JsResult<JsValue> {
    schedule(a, c, false)
}
fn set_interval(_t: &JsValue, a: &[JsValue], c: &mut Context) -> JsResult<JsValue> {
    schedule(a, c, true)
}

fn clear_timer(_t: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let id = args.get_or_undefined(0).to_number(context).unwrap_or(0.0) as u64;
    with_host_state_mut(context, |s| s.timers.retain(|t| t.id != id));
    Ok(JsValue::undefined())
}

fn bevy_send(_t: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let name = args
        .first()
        .and_then(|v| v.as_string())
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();
    let payload = match args.get(1) {
        Some(v) => v.to_json(context)?.unwrap_or(serde_json::Value::Null),
        None => serde_json::Value::Null,
    };
    with_host_state_mut(context, |s| s.outbox.push((name, payload)));
    Ok(JsValue::undefined())
}
