//! `fetch` — a deliberate warn-and-reject stub (design §2: network is out of scope forever).

use boa_engine::{
    js_string, object::builtins::JsPromise, Context, JsArgs, JsNativeError, JsResult, JsValue,
    NativeFunction,
};

fn fetch_impl(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let url = args
        .get_or_undefined(0)
        .to_string(context)
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();
    // Reject immediately — network is out of scope (design §2). fetch never hits the network.
    let msg = format!("fetch is not supported (network is out of scope): {url}");
    let promise = JsPromise::reject(JsNativeError::typ().with_message(msg), context);
    Ok(promise.into())
}

/// Install the `fetch` global.
pub fn install_fetch(context: &mut Context) {
    context
        .register_global_callable(js_string!("fetch"), 1, NativeFunction::from_fn_ptr(fetch_impl))
        .unwrap();
}
