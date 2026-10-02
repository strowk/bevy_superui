//! page.emit's JS surface enqueues a Command::Emit that drains and resolves over
//! the JsEngine boundary, with no UI mounted.

use std::cell::RefCell;
use std::rc::Rc;

use superui_dom::Dom;
use superui_test_engine::abi;
use superui_test_engine::command::Command;

#[test]
fn emit_enqueues_command_and_resolves() {
    let mut engine = superui_js::new_engine(Rc::new(RefCell::new(Dom::new())));
    let e = engine.as_mut();
    abi::install(e);

    abi::eval_spec(
        e,
        r#"test("e", async ({ page }) => { await page.emit("score", { a: 1 }); });"#,
    )
    .unwrap();

    let tests = abi::take_registered_tests(e);
    abi::run_test(e, &tests[0]);

    let q = abi::drain_queue(e);
    assert_eq!(q.len(), 1);
    match &q[0].command {
        Command::Emit { name, value } => {
            assert_eq!(name, "score");
            assert_eq!(value, &serde_json::json!({ "a": 1 }));
        }
        other => panic!("expected Command::Emit, got {other:?}"),
    }

    abi::resolve(e, q[0].id, r#"{"ok":true,"value":null}"#);
    e.run_timers(1.0);
    assert!(matches!(abi::promise_settled(e), Some(Ok(()))));
}
