//! Regression: `.nth()` before `.locator()` must NOT drop the index.
//!
//! The prelude threads `nth` as a real value through `makeLocator`. Before the
//! fix, `nth(i)` stashed `_nth` on a sliced *array* and `locator()` used
//! `Array.concat`, which does not carry custom array properties forward, so
//! chaining after `.nth()` serialized with `nth: null`.

use std::cell::RefCell;
use std::rc::Rc;

use superui_dom::Dom;
use superui_js::JsEngine;
use superui_test_engine::abi;

/// A fresh engine with the `$sstest` ABI + prelude installed.
fn engine_with_prelude() -> Box<dyn JsEngine> {
    let mut engine = superui_js::new_engine(Rc::new(RefCell::new(Dom::new())));
    abi::install(engine.as_mut());
    engine
}

/// Evaluate `expr` and read its value back over the outbox.
fn read(engine: &mut dyn JsEngine, expr: &str) -> serde_json::Value {
    engine
        .eval(&format!("__superui_bevy_send('r', ({expr}));"))
        .expect("eval");
    engine
        .drain_outbox()
        .into_iter()
        .find(|(n, _)| n == "r")
        .map(|(_, v)| v)
        .unwrap_or(serde_json::Value::Null)
}

#[test]
fn nth_is_carried_through_chaining() {
    let mut engine = engine_with_prelude();
    let e = engine.as_mut();
    // `.nth(0)` then a further `.locator("b")` must keep nth = 0.
    assert_eq!(
        read(e, r#"page.locator("a").nth(0).locator("b")._nth"#),
        serde_json::json!(0),
        "nth must survive chaining after .nth()"
    );
    // And the appended step must be present (steps carried forward too).
    assert_eq!(
        read(e, r#"page.locator("a").nth(0).locator("b").steps.length"#),
        serde_json::json!(2)
    );
}

#[test]
fn nth_and_first_terminal_still_resolve() {
    let mut engine = engine_with_prelude();
    let e = engine.as_mut();
    // Terminal `.nth(2)` (as used by game_menu specs).
    assert_eq!(read(e, r#"page.locator("a").nth(2)._nth"#), serde_json::json!(2));
    // Terminal `.first()` is nth(0).
    assert_eq!(read(e, r#"page.locator("a").first()._nth"#), serde_json::json!(0));
    // A plain locator with no nth serializes to null.
    assert_eq!(
        read(e, r#"page.locator("a")._nth === null"#),
        serde_json::json!(true)
    );
}
