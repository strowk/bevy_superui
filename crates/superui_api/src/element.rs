//! Element attribute methods, content/value/checked accessors, classList, style.

use boa_engine::{Context, JsArgs, JsObject, JsResult, JsValue};
use superui_js::{dom_of, jsstr, node_id_of, with_host_state, NodeHandle};

use crate::document::set_method;
use crate::node::{set_accessor, set_getter};

// ---- attribute methods ----

fn get_attribute(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let Some(n) = node_id_of(this) else { return Ok(JsValue::null()) };
    let name = args.get_or_undefined(0).to_string(context)?.to_std_string_escaped();
    let v = dom_of(context).borrow().get_attribute(n, &name).map(|s| s.to_string());
    Ok(v.map(|s| jsstr(&s)).unwrap_or(JsValue::null()))
}

fn set_attribute(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let name = args.get_or_undefined(0).to_string(context)?.to_std_string_escaped();
    let value = args.get_or_undefined(1).to_string(context)?.to_std_string_escaped();
    if let Some(n) = node_id_of(this) {
        let _ = dom_of(context).borrow_mut().set_attribute(n, &name, &value);
    }
    Ok(JsValue::undefined())
}

fn remove_attribute(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let name = args.get_or_undefined(0).to_string(context)?.to_std_string_escaped();
    if let Some(n) = node_id_of(this) {
        dom_of(context).borrow_mut().remove_attribute(n, &name);
    }
    Ok(JsValue::undefined())
}

fn has_attribute(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let Some(n) = node_id_of(this) else { return Ok(JsValue::from(false)) };
    let name = args.get_or_undefined(0).to_string(context)?.to_std_string_escaped();
    Ok(JsValue::from(dom_of(context).borrow().has_attribute(n, &name)))
}

// ---- string-attribute-backed accessors (id, className) ----

fn attr_getter(this: &JsValue, context: &mut Context, name: &str) -> JsValue {
    let Some(n) = node_id_of(this) else { return jsstr("") };
    let v = dom_of(context).borrow().get_attribute(n, name).unwrap_or("").to_string();
    jsstr(&v)
}
fn attr_setter(this: &JsValue, args: &[JsValue], context: &mut Context, name: &str) -> JsResult<JsValue> {
    let v = args.get_or_undefined(0).to_string(context)?.to_std_string_escaped();
    if let Some(n) = node_id_of(this) {
        let _ = dom_of(context).borrow_mut().set_attribute(n, name, &v);
    }
    Ok(JsValue::undefined())
}

fn id_get(this: &JsValue, _a: &[JsValue], c: &mut Context) -> JsResult<JsValue> { Ok(attr_getter(this, c, "id")) }
fn id_set(this: &JsValue, a: &[JsValue], c: &mut Context) -> JsResult<JsValue> { attr_setter(this, a, c, "id") }
fn class_name_get(this: &JsValue, _a: &[JsValue], c: &mut Context) -> JsResult<JsValue> { Ok(attr_getter(this, c, "class")) }
fn class_name_set(this: &JsValue, a: &[JsValue], c: &mut Context) -> JsResult<JsValue> { attr_setter(this, a, c, "class") }

// ---- textContent / innerText ----

pub(crate) fn text_content_get(this: &JsValue, _a: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let Some(n) = node_id_of(this) else { return Ok(jsstr("")) };
    let t = dom_of(context).borrow().text_content(n);
    Ok(jsstr(&t))
}
pub(crate) fn text_content_set(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let text = args.get_or_undefined(0).to_string(context)?.to_std_string_escaped();
    if let Some(n) = node_id_of(this) {
        dom_of(context).borrow_mut().set_text_content(n, &text);
    }
    Ok(JsValue::undefined())
}

// ---- value / checked ----

fn value_get(this: &JsValue, _a: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let Some(n) = node_id_of(this) else { return Ok(jsstr("")) };
    let v = dom_of(context).borrow().value(n);
    Ok(jsstr(&v))
}
fn value_set(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let v = args.get_or_undefined(0).to_string(context)?.to_std_string_escaped();
    if let Some(n) = node_id_of(this) { dom_of(context).borrow_mut().set_value(n, &v); }
    Ok(JsValue::undefined())
}
fn checked_get(this: &JsValue, _a: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let Some(n) = node_id_of(this) else { return Ok(JsValue::from(false)) };
    Ok(JsValue::from(dom_of(context).borrow().checked(n)))
}
fn checked_set(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let b = args.get_or_undefined(0).to_boolean();
    if let Some(n) = node_id_of(this) { dom_of(context).borrow_mut().set_checked(n, b); }
    Ok(JsValue::undefined())
}

// ---- classList ----

fn make_token_list(context: &mut Context, owner: superui_dom::NodeId) -> JsObject {
    let proto = with_host_state(context, |s| s.protos.token_list.clone()).expect("token_list proto");
    JsObject::from_proto_and_data(proto, NodeHandle { node: owner })
}
fn class_list_get(this: &JsValue, _a: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let Some(n) = node_id_of(this) else { return Ok(JsValue::null()) };
    Ok(make_token_list(context, n).into())
}
fn cl_add(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let c = args.get_or_undefined(0).to_string(context)?.to_std_string_escaped();
    if let Some(n) = node_id_of(this) { dom_of(context).borrow_mut().class_add(n, &c); }
    Ok(JsValue::undefined())
}
fn cl_remove(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let c = args.get_or_undefined(0).to_string(context)?.to_std_string_escaped();
    if let Some(n) = node_id_of(this) { dom_of(context).borrow_mut().class_remove(n, &c); }
    Ok(JsValue::undefined())
}
fn cl_toggle(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let c = args.get_or_undefined(0).to_string(context)?.to_std_string_escaped();
    let Some(n) = node_id_of(this) else { return Ok(JsValue::from(false)) };
    Ok(JsValue::from(dom_of(context).borrow_mut().class_toggle(n, &c)))
}
fn cl_contains(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let c = args.get_or_undefined(0).to_string(context)?.to_std_string_escaped();
    let Some(n) = node_id_of(this) else { return Ok(JsValue::from(false)) };
    Ok(JsValue::from(dom_of(context).borrow().class_contains(n, &c)))
}

// ---- style (setProperty / getPropertyValue, backed by the `style` attribute) ----

fn parse_style(s: &str) -> Vec<(String, String)> {
    s.split(';').filter_map(|decl| {
        let (k, v) = decl.split_once(':')?;
        let (k, v) = (k.trim(), v.trim());
        if k.is_empty() { None } else { Some((k.to_ascii_lowercase(), v.to_string())) }
    }).collect()
}
fn serialize_style(decls: &[(String, String)]) -> String {
    decls.iter().map(|(k, v)| format!("{k}: {v}")).collect::<Vec<_>>().join("; ")
}
fn make_style(context: &mut Context, owner: superui_dom::NodeId) -> JsObject {
    let proto = with_host_state(context, |s| s.protos.style.clone()).expect("style proto");
    JsObject::from_proto_and_data(proto, NodeHandle { node: owner })
}
fn style_get(this: &JsValue, _a: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let Some(n) = node_id_of(this) else { return Ok(JsValue::null()) };
    Ok(make_style(context, n).into())
}
fn style_set_property(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let name = args.get_or_undefined(0).to_string(context)?.to_std_string_escaped().to_ascii_lowercase();
    let value = args.get_or_undefined(1).to_string(context)?.to_std_string_escaped();
    if let Some(n) = node_id_of(this) {
        let dom = dom_of(context);
        let mut d = dom.borrow_mut();
        let mut decls = parse_style(d.get_attribute(n, "style").unwrap_or(""));
        match decls.iter_mut().find(|(k, _)| *k == name) {
            Some(slot) => slot.1 = value,
            None => decls.push((name, value)),
        }
        let _ = d.set_attribute(n, "style", &serialize_style(&decls));
    }
    Ok(JsValue::undefined())
}
fn style_get_property_value(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let name = args.get_or_undefined(0).to_string(context)?.to_std_string_escaped().to_ascii_lowercase();
    let Some(n) = node_id_of(this) else { return Ok(jsstr("")) };
    let dom = dom_of(context);
    let d = dom.borrow();
    let val = parse_style(d.get_attribute(n, "style").unwrap_or(""))
        .into_iter().find(|(k, _)| *k == name).map(|(_, v)| v).unwrap_or_default();
    Ok(jsstr(&val))
}

/// Install element attribute methods, content/value/checked accessors,
/// classList and style.
pub fn install_element(context: &mut Context) {
    let element = with_host_state(context, |s| s.protos.element.clone()).expect("element proto");
    let token_list = with_host_state(context, |s| s.protos.token_list.clone()).expect("token_list proto");
    let style = with_host_state(context, |s| s.protos.style.clone()).expect("style proto");

    set_method(&element, "getAttribute", 1, get_attribute, context);
    set_method(&element, "setAttribute", 2, set_attribute, context);
    set_method(&element, "removeAttribute", 1, remove_attribute, context);
    set_method(&element, "hasAttribute", 1, has_attribute, context);

    set_accessor(&element, "id", id_get, id_set, context);
    set_accessor(&element, "className", class_name_get, class_name_set, context);
    set_accessor(&element, "textContent", text_content_get, text_content_set, context);
    set_accessor(&element, "innerText", text_content_get, text_content_set, context);
    set_accessor(&element, "value", value_get, value_set, context);
    set_accessor(&element, "checked", checked_get, checked_set, context);
    set_getter(&element, "classList", class_list_get, context);
    set_getter(&element, "style", style_get, context);

    set_method(&token_list, "add", 1, cl_add, context);
    set_method(&token_list, "remove", 1, cl_remove, context);
    set_method(&token_list, "toggle", 1, cl_toggle, context);
    set_method(&token_list, "contains", 1, cl_contains, context);

    set_method(&style, "setProperty", 2, style_set_property, context);
    set_method(&style, "getPropertyValue", 1, style_get_property_value, context);
}
