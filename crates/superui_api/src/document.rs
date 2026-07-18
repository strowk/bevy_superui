//! The `document` object and its factory/query methods.

use boa_engine::{
    js_string, object::builtins::JsArray, object::FunctionObjectBuilder, property::Attribute,
    Context, JsArgs, JsResult, JsValue, NativeFunction,
};
use superui_js::{dom_of, with_host_state, wrap_node, wrap_opt_node};

fn get_element_by_id(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let id = args.get_or_undefined(0).to_string(context)?.to_std_string_escaped();
    let found = dom_of(context).borrow().get_element_by_id(&id);
    Ok(wrap_opt_node(context, found))
}

fn create_element(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let tag = args.get_or_undefined(0).to_string(context)?.to_std_string_escaped();
    let id = dom_of(context).borrow_mut().create_element(&tag);
    Ok(wrap_node(context, id).into())
}

fn create_text_node(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let data = args.get_or_undefined(0).to_string(context)?.to_std_string_escaped();
    let id = dom_of(context).borrow_mut().create_text(&data);
    Ok(wrap_node(context, id).into())
}

fn query_selector(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let sel = args.get_or_undefined(0).to_string(context)?.to_std_string_escaped();
    let dom = dom_of(context);
    let found = { let d = dom.borrow(); d.query_selector(d.document(), &sel) };
    Ok(wrap_opt_node(context, found))
}

fn query_selector_all(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let sel = args.get_or_undefined(0).to_string(context)?.to_std_string_escaped();
    let dom = dom_of(context);
    let matches = { let d = dom.borrow(); d.query_selector_all(d.document(), &sel) };
    let items: Vec<JsValue> = matches.into_iter().map(|id| wrap_node(context, id).into()).collect();
    Ok(JsArray::from_iter(items, context).into())
}

/// Attach a native method `name` to `proto`.
pub(crate) fn set_method(
    proto: &boa_engine::JsObject,
    name: &str,
    arity: usize,
    f: fn(&JsValue, &[JsValue], &mut Context) -> JsResult<JsValue>,
    context: &mut Context,
) {
    let func = FunctionObjectBuilder::new(context.realm(), NativeFunction::from_fn_ptr(f))
        .name(js_string!(name))
        .length(arity)
        .build();
    proto.set(js_string!(name), func, false, context).unwrap();
}

/// Build the document proto's methods, wrap the document node, and register it
/// as the `document` global.
pub fn install_document(context: &mut Context) {
    let proto = with_host_state(context, |s| s.protos.document.clone()).expect("document proto");
    set_method(&proto, "getElementById", 1, get_element_by_id, context);
    set_method(&proto, "createElement", 1, create_element, context);
    set_method(&proto, "createTextNode", 1, create_text_node, context);
    set_method(&proto, "querySelector", 1, query_selector, context);
    set_method(&proto, "querySelectorAll", 1, query_selector_all, context);

    let doc_id = dom_of(context).borrow().document();
    let document = wrap_node(context, doc_id);
    context
        .register_global_property(js_string!("document"), document, Attribute::all())
        .unwrap();
}
