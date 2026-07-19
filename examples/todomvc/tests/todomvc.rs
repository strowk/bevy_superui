//! Integration tests over the REAL authored TodoMVC files (compiled in via
//! `include_str!` in the harness), driven headlessly through the `superui` stack.
mod support;
use support::*;

#[test]
fn mounts_and_shows_title() {
    let mut app = app();
    let _root = mount_todomvc(&mut app);
    // The app mounted (a UiRuntime exists) and the <h1> title is present.
    let h1 = node_by_selector(&app, "h1");
    assert_eq!(text_content(&app, h1), "todos");
}
