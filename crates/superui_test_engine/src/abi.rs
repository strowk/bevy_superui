use boa_engine::builtins::promise::{PromiseState, ResolvingFunctions};
use boa_engine::object::builtins::{JsFunction, JsPromise};
use boa_engine::object::FunctionObjectBuilder;
use boa_engine::property::Attribute;
use boa_engine::{
    js_string, Context, JsArgs, JsNativeError, JsObject, JsResult, JsValue, NativeFunction, Source,
};
use boa_gc::{Finalize, Trace};

use crate::command::Queued;

pub struct RegisteredTest {
    pub name: String,
    pub func: JsFunction,
}

pub struct JsPromiseHandle(pub JsValue);

/// Realm-hosted state for the test ABI. Traced because it holds JS handles.
#[derive(Trace, Finalize, boa_engine::JsData, Default)]
struct TestState {
    #[unsafe_ignore_trace]
    next_id: u64,
    tests: Vec<TestEntry>,
    #[unsafe_ignore_trace]
    queue: Vec<Queued>,
    pending: Vec<PendingEntry>,
}

#[derive(Trace, Finalize)]
struct TestEntry {
    #[unsafe_ignore_trace]
    name: String,
    func: JsFunction,
}

#[derive(Trace, Finalize)]
struct PendingEntry {
    #[unsafe_ignore_trace]
    id: u64,
    /// `Option` so we can `.take()` it out past the `Drop` impl that `Trace`
    /// synthesizes (moving a field out of a `Drop` type is otherwise illegal).
    resolvers: Option<ResolvingFunctions>,
}

fn with_state<R>(context: &mut Context, f: impl FnOnce(&mut TestState) -> R) -> R {
    let mut host = context.realm().host_defined_mut();
    let state = host.get_mut::<TestState>().expect("TestState installed");
    f(state)
}

/// Build a native function object from a plain fn pointer.
fn native(context: &mut Context, f: fn(&JsValue, &[JsValue], &mut Context) -> JsResult<JsValue>) -> JsFunction {
    FunctionObjectBuilder::new(context.realm(), NativeFunction::from_fn_ptr(f)).build()
}

pub fn install(context: &mut Context) {
    context.realm().host_defined_mut().insert(TestState::default());

    let obj = JsObject::with_object_proto(context.intrinsics());
    let reg_fn = native(context, js_register);
    let enq_fn = native(context, js_enqueue);
    obj.set(js_string!("register"), reg_fn, false, context).unwrap();
    obj.set(js_string!("enqueue"), enq_fn, false, context).unwrap();
    context
        .register_global_property(js_string!("$sstest"), obj, Attribute::all())
        .unwrap();

    context
        .eval(Source::from_bytes(include_str!("prelude.js")))
        .expect("prelude.js must evaluate");
}

fn js_register(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let name = args.get_or_undefined(0).to_string(context)?.to_std_string_escaped();
    let obj = args
        .get_or_undefined(1)
        .as_object()
        .ok_or_else(|| JsNativeError::typ().with_message("test(fn) requires a function"))?;
    let func = JsFunction::from_object(obj)
        .ok_or_else(|| JsNativeError::typ().with_message("test(fn) requires a function"))?;
    with_state(context, |s| s.tests.push(TestEntry { name, func }));
    Ok(JsValue::undefined())
}

fn js_enqueue(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let raw = args.get_or_undefined(0).to_string(context)?.to_std_string_escaped();
    let command: crate::command::Command = serde_json::from_str(&raw)
        .map_err(|e| JsNativeError::typ().with_message(format!("bad command json: {e}")))?;
    let (promise, resolvers) = JsPromise::new_pending(context);
    with_state(context, |s| {
        let id = s.next_id;
        s.next_id += 1;
        s.queue.push(Queued { id, command, raw });
        s.pending.push(PendingEntry { id, resolvers: Some(resolvers) });
    });
    Ok(promise.into())
}

pub fn take_registered_tests(context: &mut Context) -> Vec<RegisteredTest> {
    with_state(context, |s| {
        // Clone fields out (moving out of a `Trace`/`Drop` type is illegal), then
        // clear the vec.
        let out = s
            .tests
            .iter()
            .map(|t| RegisteredTest { name: t.name.clone(), func: t.func.clone() })
            .collect();
        s.tests.clear();
        out
    })
}

pub fn drain_queue(context: &mut Context) -> Vec<Queued> {
    with_state(context, |s| std::mem::take(&mut s.queue))
}

pub fn resolve(context: &mut Context, id: u64, result_json: &str) {
    let resolvers = with_state(context, |s| {
        if let Some(pos) = s.pending.iter().position(|p| p.id == id) {
            let taken = s.pending[pos].resolvers.take();
            s.pending.remove(pos);
            taken
        } else {
            None
        }
    });
    if let Some(r) = resolvers {
        let val = JsValue::from(js_string!(result_json));
        // Prelude does JSON.parse on the resolved string.
        let _ = r.resolve.call(&JsValue::undefined(), &[val], context);
    }
}

pub fn run_test(context: &mut Context, test: &RegisteredTest) -> JsPromiseHandle {
    // Invoke with a `page`-bearing arg: `({ page })`. The prelude puts `page` on
    // globalThis, so passing the global object as the destructured arg works.
    let global = context.global_object();
    let arg = JsObject::with_object_proto(context.intrinsics());
    let page = global.get(js_string!("page"), context).unwrap();
    arg.set(js_string!("page"), page, false, context).unwrap();
    let ret = test
        .func
        .call(&JsValue::undefined(), &[arg.into()], context)
        .unwrap_or(JsValue::undefined());
    JsPromiseHandle(ret)
}

pub fn promise_settled(context: &mut Context, handle: &JsPromiseHandle) -> Option<Result<(), String>> {
    let obj = handle.0.as_object()?;
    let promise = JsPromise::from_object(obj.clone()).ok()?;
    match promise.state() {
        PromiseState::Pending => None,
        PromiseState::Fulfilled(_) => Some(Ok(())),
        PromiseState::Rejected(v) => {
            let msg = v
                .to_string(context)
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_default();
            Some(Err(msg))
        }
    }
}
