//! Integration tests over the REAL authored Supersolid TodoMVC (`app.tsx`
//! transpiled by the native TsxLoader), driven headlessly through `superui`.
mod support;
use support::*;

#[test]
fn mounts_and_shows_title() {
    let mut app = app();
    let _root = mount(&mut app);
    // The app mounted (a UiRuntime exists) and App's <h1> title rendered.
    let h1 = node_by_selector(&app, "h1");
    assert_eq!(text_content(&app, h1), "todos");
}
