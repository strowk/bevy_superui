//! Headless harness: mount the REAL authored TodoMVC assets through the real
//! `superui` runtime, then drive synthetic DOM events and read the DOM back.
#![allow(dead_code)]

use bevy::asset::io::memory::{Dir, MemoryAssetReader};
use bevy::asset::io::{AssetSource, AssetSourceId};
use bevy::asset::AssetPlugin;
use bevy::image::TextureAtlasPlugin;
use bevy::input::InputPlugin;
use bevy::input_focus::{InputFocus, InputFocusVisible};
use bevy::picking::{InteractionPlugin, PickingPlugin};
use bevy::prelude::*;
use bevy::text::TextPlugin;
use bevy::ui::UiPlugin;
use superui::prelude::{SuperUiPlugin, SuperUiRoot};
use superui_bridge::{PendingDomEvent, PendingDomEvents, UiRuntime};
use superui_css::style::StyleSheet;
use superui_dom::NodeId;

/// The real authored files — compiled into the test so the test exercises
/// exactly what ships.
pub const HTML: &str = include_str!("../../assets/ui/todomvc/index.html");
pub const CSS: &str = include_str!("../../assets/ui/todomvc/style.css");
pub const JS: &str = include_str!("../../assets/ui/todomvc/app.js");

/// A headless app with the full SuperUi stack and an in-memory asset source
/// holding the authored files.
pub fn app() -> App {
    let dir = Dir::new("assets".into());
    dir.insert_asset("ui/todomvc/index.html".as_ref(), HTML.as_bytes());
    dir.insert_asset("ui/todomvc/style.css".as_ref(), CSS.as_bytes());
    dir.insert_asset("ui/todomvc/app.js".as_ref(), JS.as_bytes());

    let mut app = App::new();
    app.register_asset_source(
        AssetSourceId::Default,
        AssetSource::build().with_reader(move || Box::new(MemoryAssetReader { root: dir.clone() })),
    );
    app.add_plugins((
        bevy::time::TimePlugin,
        bevy::app::TaskPoolPlugin::default(),
        AssetPlugin::default(),
        WindowPlugin::default(),
        bevy::image::ImagePlugin::default(),
        TextureAtlasPlugin,
        TextPlugin,
        (InputPlugin, PickingPlugin, InteractionPlugin, UiPlugin),
    ));
    app.init_resource::<InputFocus>()
        .init_resource::<InputFocusVisible>();
    app.add_plugins(SuperUiPlugin);
    app.finish();
    app
}

/// Spawn the `SuperUiRoot` and tick until the runtime is mounted.
pub fn mount_todomvc(app: &mut App) -> Entity {
    let (html, css, js) = {
        let server = app.world().resource::<AssetServer>().clone();
        (
            server.load("ui/todomvc/index.html"),
            server.load::<StyleSheet>("ui/todomvc/style.css"),
            server.load("ui/todomvc/app.js"),
        )
    };
    let root = app
        .world_mut()
        .spawn((Node::default(), SuperUiRoot { html, css, js }))
        .id();
    for _ in 0..128 {
        app.update();
        if app.world().contains_non_send::<UiRuntime>() {
            break;
        }
    }
    root
}

pub fn tick(app: &mut App, n: usize) {
    for _ in 0..n {
        app.update();
    }
}

/// Resolve a selector against the live DOM.
pub fn node_by_selector(app: &App, sel: &str) -> NodeId {
    let rt = app.world().non_send_resource::<UiRuntime>();
    let d = rt.dom.borrow();
    d.query_selector(d.document(), sel)
        .unwrap_or_else(|| panic!("selector matched nothing: {sel}"))
}

pub fn nodes_by_selector(app: &App, sel: &str) -> Vec<NodeId> {
    let rt = app.world().non_send_resource::<UiRuntime>();
    let d = rt.dom.borrow();
    d.query_selector_all(d.document(), sel)
}

pub fn text_content(app: &App, node: NodeId) -> String {
    let rt = app.world().non_send_resource::<UiRuntime>();
    let t = rt.dom.borrow().text_content(node);
    t
}

pub fn value_of(app: &App, node: NodeId) -> String {
    let rt = app.world().non_send_resource::<UiRuntime>();
    let v = rt.dom.borrow().value(node);
    v
}

pub fn checked_of(app: &App, node: NodeId) -> bool {
    let rt = app.world().non_send_resource::<UiRuntime>();
    let c = rt.dom.borrow().checked(node);
    c
}

/// Enqueue a synthetic `click` DOM event on `node` (drained + dispatched next tick).
pub fn click(app: &mut App, node: NodeId) {
    app.world_mut()
        .resource_mut::<PendingDomEvents>()
        .0
        .push(PendingDomEvent::new(node, "click"));
    tick(app, 2);
}

/// Set an input's DOM value (stands in for typing — the keyboard seam is
/// unit-tested in `superui_bridge`). Marks the runtime dirty so it reconciles.
pub fn set_value(app: &mut App, node: NodeId, v: &str) {
    let mut rt = app.world_mut().non_send_resource_mut::<UiRuntime>();
    rt.dom.borrow_mut().set_value(node, v);
    rt.dirty = true;
}

/// Simulate a real pointer click on a checkbox: mirror the native toggle the
/// picking observer performs (flip DOM `checked`), then dispatch `change`.
pub fn click_checkbox(app: &mut App, node: NodeId) {
    {
        let rt = app.world_mut().non_send_resource_mut::<UiRuntime>();
        let now = !rt.dom.borrow().checked(node);
        rt.dom.borrow_mut().set_checked(node, now);
    }
    app.world_mut()
        .resource_mut::<PendingDomEvents>()
        .0
        .push(PendingDomEvent::new(node, "change"));
    tick(app, 2);
}
