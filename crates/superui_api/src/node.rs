//! Structural `Node`/`Element` bindings: appendChild/removeChild/insertBefore/
//! replaceChild + parentNode/childNodes/children/... navigation accessors.

use boa_engine::{
    js_string, object::builtins::JsArray, object::FunctionObjectBuilder, property::PropertyDescriptor,
    Context, JsArgs, JsObject, JsResult, JsValue, NativeFunction,
};
use superui_dom::NodeKind;
use superui_js::{dom_of, jsstr, node_id_of, with_host_state, wrap_node, wrap_opt_node};

use crate::document::set_method;

/// Attach a getter-only accessor `name` to `proto`.
pub(crate) fn set_getter(
    proto: &JsObject,
    name: &str,
    getter: fn(&JsValue, &[JsValue], &mut Context) -> JsResult<JsValue>,
    context: &mut Context,
) {
    let g = FunctionObjectBuilder::new(context.realm(), NativeFunction::from_fn_ptr(getter))
        .name(js_string!(name))
        .length(0)
        .build();
    let desc = PropertyDescriptor::builder().get(g).enumerable(true).configurable(true).build();
    proto.define_property_or_throw(js_string!(name), desc, context).unwrap();
}

/// Attach a getter+setter accessor `name` to `proto`.
pub(crate) fn set_accessor(
    proto: &JsObject,
    name: &str,
    getter: fn(&JsValue, &[JsValue], &mut Context) -> JsResult<JsValue>,
    setter: fn(&JsValue, &[JsValue], &mut Context) -> JsResult<JsValue>,
    context: &mut Context,
) {
    let g = FunctionObjectBuilder::new(context.realm(), NativeFunction::from_fn_ptr(getter))
        .name(js_string!(name)).length(0).build();
    let s = FunctionObjectBuilder::new(context.realm(), NativeFunction::from_fn_ptr(setter))
        .name(js_string!(name)).length(1).build();
    let desc = PropertyDescriptor::builder().get(g).set(s).enumerable(true).configurable(true).build();
    proto.define_property_or_throw(js_string!(name), desc, context).unwrap();
}

// ---- structural methods ----

fn append_child(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let (Some(parent), Some(child)) = (node_id_of(this), node_id_of(args.get_or_undefined(0))) else {
        return Ok(JsValue::undefined());
    };
    let _ = dom_of(context).borrow_mut().append_child(parent, child);
    Ok(args.get_or_undefined(0).clone())
}

fn remove_child(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let (Some(parent), Some(child)) = (node_id_of(this), node_id_of(args.get_or_undefined(0))) else {
        return Ok(JsValue::undefined());
    };
    let _ = dom_of(context).borrow_mut().remove_child(parent, child);
    Ok(args.get_or_undefined(0).clone())
}

fn insert_before(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let parent = node_id_of(this);
    let new = node_id_of(args.get_or_undefined(0));
    let reference = node_id_of(args.get_or_undefined(1)); // may be null -> None -> append
    if let (Some(parent), Some(new)) = (parent, new) {
        let _ = dom_of(context).borrow_mut().insert_before(parent, new, reference);
    }
    Ok(args.get_or_undefined(0).clone())
}

fn replace_child(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let parent = node_id_of(this);
    let new = node_id_of(args.get_or_undefined(0));
    let old = node_id_of(args.get_or_undefined(1));
    if let (Some(parent), Some(new), Some(old)) = (parent, new, old) {
        let _ = dom_of(context).borrow_mut().replace_child(parent, new, old);
    }
    Ok(args.get_or_undefined(1).clone())
}

// ---- navigation accessors ----

fn parent_node(this: &JsValue, _a: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let Some(n) = node_id_of(this) else { return Ok(JsValue::null()) };
    let p = dom_of(context).borrow().parent(n);
    Ok(wrap_opt_node(context, p))
}

fn first_child(this: &JsValue, _a: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let Some(n) = node_id_of(this) else { return Ok(JsValue::null()) };
    let c = dom_of(context).borrow().children(n).first().copied();
    Ok(wrap_opt_node(context, c))
}

fn next_sibling(this: &JsValue, _a: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let Some(n) = node_id_of(this) else { return Ok(JsValue::null()) };
    let s = dom_of(context).borrow().next_sibling(n);
    Ok(wrap_opt_node(context, s))
}

fn previous_sibling(this: &JsValue, _a: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let Some(n) = node_id_of(this) else { return Ok(JsValue::null()) };
    let s = dom_of(context).borrow().previous_sibling(n);
    Ok(wrap_opt_node(context, s))
}

fn child_nodes(this: &JsValue, _a: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let Some(n) = node_id_of(this) else { return Ok(JsArray::from_iter(Vec::new(), context).into()) };
    let kids: Vec<_> = dom_of(context).borrow().children(n).to_vec();
    let items: Vec<JsValue> = kids.into_iter().map(|id| wrap_node(context, id).into()).collect();
    Ok(JsArray::from_iter(items, context).into())
}

fn children(this: &JsValue, _a: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let Some(n) = node_id_of(this) else { return Ok(JsArray::from_iter(Vec::new(), context).into()) };
    let dom = dom_of(context);
    let kids: Vec<_> = { let d = dom.borrow(); d.children(n).iter().copied().filter(|&c| d.is_element(c)).collect() };
    let items: Vec<JsValue> = kids.into_iter().map(|id| wrap_node(context, id).into()).collect();
    Ok(JsArray::from_iter(items, context).into())
}

fn node_type(this: &JsValue, _a: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let Some(n) = node_id_of(this) else { return Ok(JsValue::from(0)) };
    let dom = dom_of(context);
    let d = dom.borrow();
    let t = match d.get(n).map(|nd| &nd.kind) {
        Some(NodeKind::Element(_)) => 1,
        Some(NodeKind::Text(_)) => 3,
        Some(NodeKind::Document) => 9,
        None => 0,
    };
    Ok(JsValue::from(t))
}

fn tag_name(this: &JsValue, _a: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let Some(n) = node_id_of(this) else { return Ok(JsValue::undefined()) };
    let tag = dom_of(context).borrow().tag(n).map(|t| t.to_ascii_uppercase());
    Ok(match tag {
        Some(t) => jsstr(&t),
        None => JsValue::undefined(),
    })
}

/// Install structural methods + navigation accessors on the element proto (and
/// the shared subset — parentNode/childNodes/etc. — on document too).
pub fn install_node(context: &mut Context) {
    let element = with_host_state(context, |s| s.protos.element.clone()).expect("element proto");
    let document = with_host_state(context, |s| s.protos.document.clone()).expect("document proto");
    let text = with_host_state(context, |s| s.protos.text.clone()).expect("text proto");

    for proto in [element.clone(), document.clone(), text.clone()] {
        set_method(&proto, "appendChild", 1, append_child, context);
        set_method(&proto, "removeChild", 1, remove_child, context);
        set_method(&proto, "insertBefore", 2, insert_before, context);
        set_method(&proto, "replaceChild", 2, replace_child, context);
        set_getter(&proto, "parentNode", parent_node, context);
        set_getter(&proto, "firstChild", first_child, context);
        set_getter(&proto, "nextSibling", next_sibling, context);
        set_getter(&proto, "previousSibling", previous_sibling, context);
        set_getter(&proto, "childNodes", child_nodes, context);
        set_getter(&proto, "children", children, context);
        set_getter(&proto, "nodeType", node_type, context);
    }
    // tagName is element-only.
    set_getter(&element, "tagName", tag_name, context);
}
