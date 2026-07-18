//! `setTimeout`/`setInterval`/`clearTimeout`/`clearInterval`.

use boa_engine::{
    js_string, object::builtins::JsFunction, Context, JsArgs, JsResult, JsValue, NativeFunction,
};
use superui_js::{with_host_state, with_host_state_mut, Timer};

fn schedule(args: &[JsValue], context: &mut Context, repeating: bool) -> JsResult<JsValue> {
    let Some(cb_obj) = args.get_or_undefined(0).as_object() else { return Ok(JsValue::from(0)) };
    let Some(cb) = JsFunction::from_object(cb_obj.clone()) else { return Ok(JsValue::from(0)) };
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

fn set_timeout(_t: &JsValue, a: &[JsValue], c: &mut Context) -> JsResult<JsValue> { schedule(a, c, false) }
fn set_interval(_t: &JsValue, a: &[JsValue], c: &mut Context) -> JsResult<JsValue> { schedule(a, c, true) }

fn clear(args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let id = args.get_or_undefined(0).to_number(context).unwrap_or(0.0) as u64;
    with_host_state_mut(context, |s| s.timers.retain(|t| t.id != id));
    Ok(JsValue::undefined())
}
fn clear_timeout(_t: &JsValue, a: &[JsValue], c: &mut Context) -> JsResult<JsValue> { clear(a, c) }
fn clear_interval(_t: &JsValue, a: &[JsValue], c: &mut Context) -> JsResult<JsValue> { clear(a, c) }

/// Install the four timer globals.
pub fn install_timers(context: &mut Context) {
    context.register_global_callable(js_string!("setTimeout"), 2, NativeFunction::from_fn_ptr(set_timeout)).unwrap();
    context.register_global_callable(js_string!("setInterval"), 2, NativeFunction::from_fn_ptr(set_interval)).unwrap();
    context.register_global_callable(js_string!("clearTimeout"), 1, NativeFunction::from_fn_ptr(clear_timeout)).unwrap();
    context.register_global_callable(js_string!("clearInterval"), 1, NativeFunction::from_fn_ptr(clear_interval)).unwrap();
}
