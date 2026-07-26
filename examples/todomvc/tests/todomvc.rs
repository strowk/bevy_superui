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
                let rt = app.world().non_send::<UiRuntime>();
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

fn add(app: &mut bevy::prelude::App, label: &str) {
    let input = node_by_selector(app, "#new-todo");
    let add = node_by_selector(app, "#add");
    set_value(app, input, label);
    click(app, add);
}

#[test]
fn toggle_marks_completed_and_updates_count() {
    let mut app = app();
    let _root = mount_todomvc(&mut app);
    add(&mut app, "a");
    add(&mut app, "b");

    let count = node_by_selector(&app, "#count");
    assert_eq!(text_content(&app, count), "2 items left");

    // Click the first todo's checkbox -> completed; count drops to 1.
    let first_toggle = nodes_by_selector(&app, "li .toggle")[0];
    click_checkbox(&mut app, first_toggle); // flip checked + fire change

    assert_eq!(text_content(&app, count), "1 item left");
    // The first li carries the `completed` class.
    let first_li = nodes_by_selector(&app, "li")[0];
    let classes = {
        let rt = app.world().non_send::<UiRuntime>();
        let c = rt.dom.borrow().classes(first_li);
        c
    };
    assert!(classes.iter().any(|c| c == "completed"));
}

#[test]
fn destroy_removes_a_todo() {
    let mut app = app();
    let _root = mount_todomvc(&mut app);
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
    let _root = mount_todomvc(&mut app);
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
fn adding_a_todo_fires_bevy_send_into_ecs() {
    use bevy::prelude::*;
    use serde::Deserialize;
    use superui_bridge::SuperUiApp;

    #[derive(Event, Deserialize, Clone, Debug, PartialEq)]
    struct TodoAdded {
        label: String,
    }
    #[derive(Resource, Default)]
    struct Seen(Vec<String>);

    let mut app = app();
    app.add_superui_command::<TodoAdded>("TodoAdded");
    app.init_resource::<Seen>();
    app.add_observer(|ev: On<TodoAdded>, mut s: ResMut<Seen>| s.0.push(ev.event().label.clone()));

    let _root = mount_todomvc(&mut app);
    add(&mut app, "Ship it");
    tick(&mut app, 2);

    assert_eq!(app.world().resource::<Seen>().0, vec!["Ship it".to_string()]);
}

#[test]
fn stylesheet_loads_and_ui_reconciles_with_it() {
    // If style.css contained a fatal parse error, flair would fail to produce a
    // StyleSheet and mount would stall; reaching a mounted runtime + rendered
    // list proves the CSS loaded and cascaded without aborting.
    let mut app = app();
    let _root = mount_todomvc(&mut app);
    add(&mut app, "styled");
    // The todo rendered under the styled tree.
    assert_eq!(li_labels(&app), vec!["styled".to_string()]);
    // And the app entity carries a TypeName (reconciled body subtree exists).
    let has_h1 = {
        let mut q = app
            .world_mut()
            .query::<&superui_css::prelude::TypeName>();
        q.iter(app.world()).any(|t| t.0 == "h1")
    };
    assert!(has_h1);
}
