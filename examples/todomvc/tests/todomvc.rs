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

fn li_labels(app: &bevy::prelude::App) -> Vec<String> {
    nodes_by_selector(app, "li")
        .into_iter()
        .map(|li| {
            let label = {
                let rt = app.world().non_send_resource::<UiRuntime>();
                let d = rt.dom.borrow();
                d.query_selector(li, ".label").unwrap()
            };
            text_content(app, label)
        })
        .collect()
}
use superui_bridge::UiRuntime;

#[test]
fn add_button_appends_a_todo() {
    let mut app = app();
    let _root = mount_todomvc(&mut app);

    let input = node_by_selector(&app, "#new-todo");
    let add = node_by_selector(&app, "#add");

    set_value(&mut app, input, "Buy milk");
    click(&mut app, add);

    assert_eq!(li_labels(&app), vec!["Buy milk".to_string()]);
    // Input cleared after add; placeholder shows again in the rendered text.
    assert_eq!(value_of(&app, input), "");
}
