//! `console.{log,warn,error,info}` bindings.

use std::cell::RefCell;

use boa_engine::{
    js_string, object::FunctionObjectBuilder, property::Attribute, Context, JsObject, JsResult,
    JsValue, NativeFunction,
};

thread_local! {
    /// Test-visible sink of console output lines (level-prefixed).
    static SINK: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// Drain and return the console lines captured so far (for tests).
pub fn console_take() -> Vec<String> {
    SINK.with(|s| std::mem::take(&mut *s.borrow_mut()))
}

fn stringify_args(args: &[JsValue], context: &mut Context) -> String {
    args.iter()
        .map(|a| {
            a.to_string(context)
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_else(|_| "<unprintable>".to_string())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn log_at(level: &str, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let line = format!("{level}: {}", stringify_args(args, context));
    SINK.with(|s| s.borrow_mut().push(line));
    Ok(JsValue::undefined())
}

fn method(context: &mut Context, name: &str, f: fn(&JsValue, &[JsValue], &mut Context) -> JsResult<JsValue>) -> JsValue {
    FunctionObjectBuilder::new(context.realm(), NativeFunction::from_fn_ptr(f))
        .name(js_string!(name))
        .length(1)
        .build()
        .into()
}

/// Install the `console` global.
pub fn install_console(context: &mut Context) {
    let console = JsObject::with_object_proto(context.intrinsics());
    let log = method(context, "log", |_, a, c| log_at("log", a, c));
    let warn = method(context, "warn", |_, a, c| log_at("warn", a, c));
    let error = method(context, "error", |_, a, c| log_at("error", a, c));
    let info = method(context, "info", |_, a, c| log_at("info", a, c));
    console.set(js_string!("log"), log, false, context).unwrap();
    console.set(js_string!("warn"), warn, false, context).unwrap();
    console.set(js_string!("error"), error, false, context).unwrap();
    console.set(js_string!("info"), info, false, context).unwrap();
    context
        .register_global_property(js_string!("console"), console, Attribute::all())
        .unwrap();
}
