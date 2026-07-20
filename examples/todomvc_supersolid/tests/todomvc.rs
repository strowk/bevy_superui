//! Integration tests over the REAL authored Supersolid TodoMVC (`app.tsx`
//! transpiled by the native TsxLoader), driven headlessly through `superui`.
mod support;
use support::*;
use superui_bridge::UiRuntime;

#[test]
fn mounts_and_shows_title() {
    let mut app = app();
    let _root = mount(&mut app);
    // The app mounted (a UiRuntime exists) and App's <h1> title rendered.
    let h1 = node_by_selector(&app, "h1");
    assert_eq!(text_content(&app, h1), "todos");
}

/// Labels of the currently rendered `<li>` rows.
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

/// Type a label into the new-todo input and click Add.
fn add(app: &mut bevy::prelude::App, label: &str) {
    let input = node_by_selector(app, "#new-todo");
    type_into(app, input, label);
    let add_btn = node_by_selector(app, "#add");
    click(app, add_btn);
}

#[test]
fn add_button_appends_a_todo() {
    let mut app = app();
    let _root = mount(&mut app);

    add(&mut app, "Buy milk");

    assert_eq!(li_labels(&app), vec!["Buy milk".to_string()]);
    // Controlled input cleared after add (draft signal reset -> value binding).
    let input = node_by_selector(&app, "#new-todo");
    assert_eq!(value_of(&app, input), "");
}

#[test]
fn toggle_marks_completed_and_updates_count() {
    let mut app = app();
    let _root = mount(&mut app);
    add(&mut app, "a");
    add(&mut app, "b");

    let count = node_by_selector(&app, "#count");
    assert_eq!(text_content(&app, count), "2 items left");

    // Toggle the first todo's checkbox -> completed; count drops to 1.
    let first_toggle = nodes_by_selector(&app, "li .toggle")[0];
    click_checkbox(&mut app, first_toggle);

    let count = node_by_selector(&app, "#count");
    assert_eq!(text_content(&app, count), "1 item left");

    // The first li carries the `completed` class.
    let first_li = nodes_by_selector(&app, "li")[0];
    let classes = {
        let rt = app.world().non_send_resource::<UiRuntime>();
        let c = rt.dom.borrow().classes(first_li);
        c
    };
    assert!(classes.iter().any(|c| c == "completed"));
}

#[test]
fn destroy_removes_a_todo() {
    let mut app = app();
    let _root = mount(&mut app);
    add(&mut app, "a");
    add(&mut app, "b");
    assert_eq!(li_labels(&app).len(), 2);

    // Click the destroy button of the first todo.
    let first_destroy = nodes_by_selector(&app, "li .destroy")[0];
    click(&mut app, first_destroy);

    assert_eq!(li_labels(&app), vec!["b".to_string()]);
}

#[test]
fn filters_show_active_and_completed_subsets() {
    let mut app = app();
    let _root = mount(&mut app);
    add(&mut app, "a");
    add(&mut app, "b");
    // Complete "a".
    let first_toggle = nodes_by_selector(&app, "li .toggle")[0];
    click_checkbox(&mut app, first_toggle);

    // Active filter -> only "b".
    let btn_active = node_by_selector(&app, "#filter-active");
    click(&mut app, btn_active);
    assert_eq!(li_labels(&app), vec!["b".to_string()]);

    // Completed filter -> only "a".
    let btn_completed = node_by_selector(&app, "#filter-completed");
    click(&mut app, btn_completed);
    assert_eq!(li_labels(&app), vec!["a".to_string()]);

    // Back to All -> both.
    let btn_all = node_by_selector(&app, "#filter-all");
    click(&mut app, btn_all);
    assert_eq!(li_labels(&app).len(), 2);
}

#[test]
fn footer_hidden_until_first_todo() {
    let mut app = app();
    let _root = mount(&mut app);
    // No todos yet -> <Show> renders nothing, so #count is absent.
    assert!(nodes_by_selector(&app, "#count").is_empty(), "footer hidden when empty");

    add(&mut app, "a");
    // Now the footer (and its count) appears.
    let count = node_by_selector(&app, "#count");
    assert_eq!(text_content(&app, count), "1 item left");
}
