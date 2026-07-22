use boa_engine::Context;
use superui_test_engine::abi::{self, JsPromiseHandle};

#[test]
fn registers_tests_and_resolves_awaited_noop() {
    let mut ctx = Context::default();
    abi::install(&mut ctx);

    // A spec that awaits one enqueued no-op then finishes.
    ctx.eval(boa_engine::Source::from_bytes(
        br#"test("t", async () => { await $sstest.enqueue(JSON.stringify({type:"noop"})); });"#,
    )).unwrap();

    let tests = abi::take_registered_tests(&mut ctx);
    assert_eq!(tests.len(), 1);
    assert_eq!(tests[0].name, "t");

    let handle: JsPromiseHandle = abi::run_test(&mut ctx, &tests[0]);
    // Pump: the test body enqueues one noop, awaiting it.
    let _ = ctx.run_jobs();
    let q = abi::drain_queue(&mut ctx);
    assert_eq!(q.len(), 1);
    // Resolve it, then pump jobs so the await continuation + test completion run.
    abi::resolve(&mut ctx, q[0].id, r#"{"ok":true,"value":null}"#);
    let _ = ctx.run_jobs();
    assert!(matches!(abi::promise_settled(&mut ctx, &handle), Some(Ok(()))));
}
