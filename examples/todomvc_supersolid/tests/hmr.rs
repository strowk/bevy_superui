//! State-preserving HMR over the REAL app.tsx: re-exec the transpiled module on
//! the same HMR-enabled runtime and assert todos + filter + draft survive the
//! DOM rebuild -> reconcile. Mirrors superui_bridge/tests/supersolid_render.rs.

use std::cell::RefCell;
use std::rc::Rc;

use bevy::asset::AssetPlugin;
use bevy::image::{ImagePlugin, TextureAtlasPlugin};
use bevy::input::InputPlugin;
use bevy::input_focus::{InputFocus, InputFocusVisible};
use bevy::picking::{InteractionPlugin, PickingPlugin};
use bevy::prelude::*;
use bevy::text::TextPlugin;
use bevy::ui::UiPlugin;
use superui_bridge::{reconcile_system, PendingDomEvent, PendingDomEvents, UiRuntime};
use superui_css::style::StyleSheet;
use superui_css::SuperUiCssPlugin;
use superui_dom::{Dom, NodeId};

const TSX: &str = include_str!("../assets/ui/todomvc_supersolid/app.tsx");

/// Transpile the real app.tsx exactly as the loader would (with a module_id so
/// component HMR ids are path-qualified).
fn transpile_app() -> String {
    let opts = supersolid::TranspileOptions {
        module_id: Some("ui/todomvc_supersolid/app.tsx".into()),
        ..Default::default()
    };
    supersolid::transpile(TSX, &opts).code
}

fn test_app() -> App {
    let mut app = App::new();
    app.add_plugins((
        bevy::time::TimePlugin,
        bevy::app::TaskPoolPlugin::default(),
        AssetPlugin::default(),
        WindowPlugin::default(),
        ImagePlugin::default(),
        TextureAtlasPlugin,
        TextPlugin,
        (InputPlugin, PickingPlugin, InteractionPlugin, UiPlugin),
        SuperUiCssPlugin,
    ));
    app.init_resource::<InputFocus>()
        .init_resource::<InputFocusVisible>();
    app.init_resource::<PendingDomEvents>();
    app.finish();
    app
}

/// Build an HMR-enabled UiRuntime around a fresh shell DOM, insert it, and add
/// the reconcile + event-drain systems.
fn mount_hmr(app: &mut App) -> Rc<RefCell<Dom>> {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<div id='root'></div>",
    )));
    let root = app.world_mut().spawn(Node::default()).id();
    let stylesheet: Handle<StyleSheet> = Handle::default();
    let rt = UiRuntime::new(dom.clone(), root, stylesheet, /* hmr */ true);
    app.world_mut().insert_non_send_resource(rt);
    app.add_systems(
        Update,
        (superui_bridge::drain_dom_events_system, reconcile_system).chain(),
    );
    dom
}

fn run(app: &mut App, js: &str) {
    app.world_mut()
        .non_send_resource_mut::<UiRuntime>()
        .run_script(js);
}

fn node(dom: &Rc<RefCell<Dom>>, sel: &str) -> NodeId {
    let d = dom.borrow();
    d.query_selector(d.document(), sel).unwrap()
}

fn label_texts(dom: &Rc<RefCell<Dom>>) -> Vec<String> {
    let d = dom.borrow();
    d.query_selector_all(d.document(), ".label")
        .into_iter()
        .map(|n| d.text_content(n))
        .collect()
}

#[test]
fn hot_reload_preserves_todos_filter_and_draft() {
    let js = transpile_app();
    let mut app = test_app();
    let dom = mount_hmr(&mut app);

    // Initial mount.
    run(&mut app, &js);
    app.update();

    // Add two todos by driving the real controlled input + Add button.
    let input = node(&dom, "#new-todo");
    dom.borrow_mut().set_value(input, "alpha");
    app.world_mut().non_send_resource_mut::<UiRuntime>().dirty = true;
    push_event(&mut app, input, "input");
    app.update();
    click(&mut app, &dom, "#add");

    let input = node(&dom, "#new-todo");
    dom.borrow_mut().set_value(input, "beta");
    app.world_mut().non_send_resource_mut::<UiRuntime>().dirty = true;
    push_event(&mut app, input, "input");
    app.update();
    click(&mut app, &dom, "#add");

    assert_eq!(label_texts(&dom), vec!["alpha".to_string(), "beta".to_string()]);

    // Switch the filter to "active" and type an un-submitted draft.
    click(&mut app, &dom, "#filter-active");
    let filter_active = node(&dom, "#filter-active");
    let sel_before = dom.borrow().classes(filter_active);
    assert!(sel_before.iter().any(|c| c == "selected"), "active filter selected");

    let input = node(&dom, "#new-todo");
    dom.borrow_mut().set_value(input, "half-typed");
    app.world_mut().non_send_resource_mut::<UiRuntime>().dirty = true;
    push_event(&mut app, input, "input");
    app.update();

    // Hot reload: re-exec the SAME module on the SAME runtime (as apply_hot_reload
    // does for a JsSource Modified event).
    run(&mut app, &js);
    app.update();

    // Todos preserved.
    assert_eq!(
        label_texts(&dom),
        vec!["alpha".to_string(), "beta".to_string()],
        "todos preserved across hot reload"
    );
    // Filter preserved (active still selected).
    let filter_active = node(&dom, "#filter-active");
    let sel_after = dom.borrow().classes(filter_active);
    assert!(sel_after.iter().any(|c| c == "selected"), "filter preserved across reload");
    // Draft preserved (controlled input value survived).
    let input = node(&dom, "#new-todo");
    assert_eq!(dom.borrow().value(input), "half-typed", "draft preserved across reload");
}

// --- small local event helpers (avoid a second support module) ---

fn push_event(app: &mut App, n: NodeId, ty: &str) {
    app.world_mut()
        .resource_mut::<PendingDomEvents>()
        .0
        .push(PendingDomEvent::new(n, ty));
}

fn click(app: &mut App, dom: &Rc<RefCell<Dom>>, sel: &str) {
    let n = node(dom, sel);
    push_event(app, n, "click");
    app.update();
    app.update();
}
