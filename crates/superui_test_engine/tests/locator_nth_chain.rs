//! Regression: `.nth()` before `.locator()` must NOT drop the index.
//!
//! The prelude threads `nth` as a real value through `makeLocator`. Before the
//! fix, `nth(i)` stashed `_nth` on a sliced *array* and `locator()` used
//! `Array.concat`, which does not carry custom array properties forward — so
//! chaining after `.nth()` serialized with `nth: null`.

use boa_engine::{Context, JsValue, Source};

/// Install the $sstest ABI + prelude into a fresh Boa context.
fn ctx_with_prelude() -> Context {
    let mut context = Context::default();
    superui_test_engine::abi::install(&mut context);
    context
}

/// Evaluate `expr` and return it as a JSON string via `JSON.stringify`.
fn eval_json(context: &mut Context, expr: &str) -> String {
    let src = format!("JSON.stringify({expr})");
    let v = context
        .eval(Source::from_bytes(src.as_bytes()))
        .expect("eval");
    v.as_string()
        .expect("string")
        .to_std_string_escaped()
}

fn eval_val(context: &mut Context, expr: &str) -> JsValue {
    context
        .eval(Source::from_bytes(expr.as_bytes()))
        .expect("eval")
}

#[test]
fn nth_is_carried_through_chaining() {
    let mut context = ctx_with_prelude();
    // `.nth(0)` then a further `.locator("b")` must keep nth = 0.
    let json = eval_json(
        &mut context,
        r#"page.locator("a").nth(0).locator("b")._nth"#,
    );
    assert_eq!(json, "0", "nth must survive chaining after .nth()");

    // And the appended step must be present (steps carried forward too).
    let steps_len = eval_json(
        &mut context,
        r#"page.locator("a").nth(0).locator("b").steps.length"#,
    );
    assert_eq!(steps_len, "2");
}

#[test]
fn nth_and_first_terminal_still_resolve() {
    let mut context = ctx_with_prelude();
    // Terminal `.nth(2)` (as used by game_menu specs).
    let nth = eval_json(&mut context, r#"page.locator("a").nth(2)._nth"#);
    assert_eq!(nth, "2");

    // Terminal `.first()` is nth(0).
    let first = eval_json(&mut context, r#"page.locator("a").first()._nth"#);
    assert_eq!(first, "0");

    // A plain locator with no nth serializes to null.
    let plain = eval_val(&mut context, r#"page.locator("a")._nth === null"#);
    assert_eq!(plain.as_boolean(), Some(true));
}
