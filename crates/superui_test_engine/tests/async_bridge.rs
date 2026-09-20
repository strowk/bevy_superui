//! The harness ABI drives entirely over the `JsEngine` boundary: register a
//! test, run it, drain the command it awaits, resolve it, and observe the test
//! promise settle, with no Boa handle held in Rust.

use std::cell::RefCell;
use std::rc::Rc;

use superui_dom::Dom;
use superui_test_engine::abi;

#[test]
fn registers_tests_and_resolves_awaited_noop() {
    let mut engine = superui_js::new_engine(Rc::new(RefCell::new(Dom::new())));
    let e = engine.as_mut();
    abi::install(e);

    // A spec that awaits one enqueued no-op then finishes.
    abi::eval_spec(
        e,
        r#"test("t", async () => { await __sstest.enqueue(JSON.stringify({type:"noop"})); });"#,
    )
    .unwrap();

    let tests = abi::take_registered_tests(e);
    assert_eq!(tests.len(), 1);
    assert_eq!(tests[0].name, "t");

    // The body enqueues one noop synchronously, then awaits it.
    abi::run_test(e, &tests[0]);
    let q = abi::drain_queue(e);
    assert_eq!(q.len(), 1);

    // Resolve it, then pump jobs so the await continuation and test completion run.
    abi::resolve(e, q[0].id, r#"{"ok":true,"value":null}"#);
    e.run_timers(1.0);
    assert!(matches!(abi::promise_settled(e), Some(Ok(()))));
}
