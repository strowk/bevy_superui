//! `UiRuntime` captures uncaught JS eval errors and hands them out via take_errors.
use std::cell::RefCell;
use std::rc::Rc;

use superui_bridge::UiRuntime;

mod support;
use support::{mount, test_app};

#[test]
fn run_script_error_is_captured_and_drained() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document("<div id='root'></div>")));
    let mut app = test_app();
    let _root = mount(&mut app, dom);

    // A throwing top-level statement.
    app.world_mut()
        .non_send_mut::<UiRuntime>()
        .run_script("throw new Error('boom');");

    let errs = app.world_mut().non_send_mut::<UiRuntime>().take_errors();
    assert!(errs.iter().any(|e| e.contains("boom")), "error captured: {errs:?}");

    // Draining clears them.
    let again = app.world_mut().non_send_mut::<UiRuntime>().take_errors();
    assert!(again.is_empty(), "second drain is empty: {again:?}");

    // A clean script leaves no errors.
    app.world_mut()
        .non_send_mut::<UiRuntime>()
        .run_script("var x = 1 + 1;");
    assert!(app.world_mut().non_send_mut::<UiRuntime>().take_errors().is_empty());
}
