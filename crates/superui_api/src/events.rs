//! `addEventListener`/`removeEventListener` + the JS `Event` object surface.

use boa_engine::{
    object::builtins::JsFunction, object::JsObject, Context, JsArgs, JsResult, JsValue,
};
use superui_js::{dom_of, jsstr, node_id_of, with_host_state, with_host_state_mut, wrap_opt_node, EventData};

use crate::document::set_method;
use crate::node::set_getter;

// ---- Event object methods/accessors (operate on EventData via `this`) ----

fn with_event<R>(this: &JsValue, f: impl FnOnce(&mut superui_dom::Event) -> R) -> Option<R> {
    let obj = this.as_object()?;
    let data = obj.downcast_ref::<EventData>()?;
    let mut ev = data.inner.borrow_mut();
    Some(f(&mut ev))
}

fn prevent_default(this: &JsValue, _a: &[JsValue], _c: &mut Context) -> JsResult<JsValue> {
    with_event(this, |e| e.prevent_default());
    Ok(JsValue::undefined())
}
fn stop_propagation(this: &JsValue, _a: &[JsValue], _c: &mut Context) -> JsResult<JsValue> {
    with_event(this, |e| e.stop_propagation());
    Ok(JsValue::undefined())
}
fn stop_immediate(this: &JsValue, _a: &[JsValue], _c: &mut Context) -> JsResult<JsValue> {
    with_event(this, |e| e.stop_immediate_propagation());
    Ok(JsValue::undefined())
}
fn ev_type(this: &JsValue, _a: &[JsValue], _c: &mut Context) -> JsResult<JsValue> {
    let obj = this.as_object();
    let t = obj
        .as_ref()
        .and_then(|o| o.downcast_ref::<EventData>())
        .map(|d| d.inner.borrow().type_.clone());
    Ok(t.map(|s| jsstr(&s)).unwrap_or(JsValue::undefined()))
}
fn ev_default_prevented(this: &JsValue, _a: &[JsValue], _c: &mut Context) -> JsResult<JsValue> {
    let v = with_event(this, |e| e.default_prevented()).unwrap_or(false);
    Ok(JsValue::from(v))
}
fn ev_target(this: &JsValue, _a: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let obj = this.as_object();
    let n = obj
        .as_ref()
        .and_then(|o| o.downcast_ref::<EventData>())
        .map(|d| d.inner.borrow().target);
    Ok(wrap_opt_node(context, n))
}
fn ev_current_target(this: &JsValue, _a: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let obj = this.as_object();
    let n = obj
        .as_ref()
        .and_then(|o| o.downcast_ref::<EventData>())
        .and_then(|d| d.inner.borrow().current_target);
    Ok(wrap_opt_node(context, n))
}

// ---- addEventListener / removeEventListener (element proto) ----

fn add_event_listener(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let Some(n) = node_id_of(this) else {
        return Ok(JsValue::undefined());
    };
    let ty = args.get_or_undefined(0).to_string(context)?.to_std_string_escaped();
    let Some(cb_obj) = args.get_or_undefined(1).as_object() else {
        return Ok(JsValue::undefined());
    };
    let Some(cb) = JsFunction::from_object(cb_obj.clone()) else {
        return Ok(JsValue::undefined());
    };
    let capture = args.get_or_undefined(2).to_boolean();

    let listener_id = dom_of(context).borrow_mut().add_event_listener(n, &ty, capture);
    if let Some(lid) = listener_id {
        with_host_state_mut(context, |s| {
            s.listeners.insert(lid.0, cb);
        });
    }
    Ok(JsValue::undefined())
}

fn remove_event_listener(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let Some(n) = node_id_of(this) else {
        return Ok(JsValue::undefined());
    };
    let ty = args.get_or_undefined(0).to_string(context)?.to_std_string_escaped();
    let Some(cb_obj) = args.get_or_undefined(1).as_object() else {
        return Ok(JsValue::undefined());
    };
    let capture = args.get_or_undefined(2).to_boolean();

    // Find the listener id whose stored callback === the given function AND whose
    // (type, capture) match, then remove from the DOM and the registry.
    let target_fn = JsFunction::from_object(cb_obj.clone());
    let Some(target_fn) = target_fn else {
        return Ok(JsValue::undefined());
    };

    let candidates: Vec<u64> = {
        let dom = dom_of(context);
        let d = dom.borrow();
        d.listeners(n)
            .iter()
            .filter(|l| l.event_type == ty && l.capture == capture)
            .map(|l| l.id.0)
            .collect()
    };
    let mut to_remove = None;
    for lid in candidates {
        let same = with_host_state(context, |s| {
            s.listeners
                .get(&lid)
                .map(|f| JsObject::equals(f, &target_fn))
                .unwrap_or(false)
        });
        if same {
            to_remove = Some(lid);
            break;
        }
    }
    if let Some(lid) = to_remove {
        dom_of(context)
            .borrow_mut()
            .remove_event_listener(n, superui_dom::ListenerId(lid));
        with_host_state_mut(context, |s| {
            s.listeners.remove(&lid);
        });
    }
    Ok(JsValue::undefined())
}

/// Install the event proto surface + element-proto listener methods.
pub fn install_events(context: &mut Context) {
    let event = with_host_state(context, |s| s.protos.event.clone()).expect("event proto");
    set_method(&event, "preventDefault", 0, prevent_default, context);
    set_method(&event, "stopPropagation", 0, stop_propagation, context);
    set_method(&event, "stopImmediatePropagation", 0, stop_immediate, context);
    set_getter(&event, "type", ev_type, context);
    set_getter(&event, "target", ev_target, context);
    set_getter(&event, "currentTarget", ev_current_target, context);
    set_getter(&event, "defaultPrevented", ev_default_prevented, context);

    let element = with_host_state(context, |s| s.protos.element.clone()).expect("element proto");
    set_method(&element, "addEventListener", 3, add_event_listener, context);
    set_method(&element, "removeEventListener", 3, remove_event_listener, context);
}
