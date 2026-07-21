//! Headless harness: mount the REAL authored Supersolid assets through the real
//! `superui` runtime, inject a UiSnapshot + GameState, tick, and read the DOM.
#![allow(dead_code)]

use bevy::asset::io::memory::{Dir, MemoryAssetReader};
use bevy::asset::io::{AssetSource, AssetSourceId};
use bevy::asset::AssetPlugin;
use bevy::image::TextureAtlasPlugin;
use bevy::input::InputPlugin;
use bevy::input_focus::{InputFocus, InputFocusVisible};
use bevy::picking::{InteractionPlugin, PickingPlugin};
use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use bevy::text::TextPlugin;
use bevy::ui::UiPlugin;
use superui::prelude::{SuperUiPlugin, SuperUiRoot};
use superui::JsSource;
use superui_bridge::{PendingDomEvent, PendingDomEvents, UiRuntime};
use superui_css::style::StyleSheet;
use superui_dom::NodeId;

use horde::game_state::GameState;
use horde::sim::{IntentQueue, SimConfig, UiSnapshot};
use horde::ui::supersolid::bridge::register_bridge;

pub const HTML: &str = include_str!("../../assets/ui/horde/index.html");
pub const CSS: &str = include_str!("../../assets/ui/horde/theme.css");
pub const TSX: &str = include_str!("../../assets/ui/horde/app.tsx");

pub fn app() -> App {
    let dir = Dir::new("assets".into());
    dir.insert_asset("ui/horde/index.html".as_ref(), HTML.as_bytes());
    dir.insert_asset("ui/horde/theme.css".as_ref(), CSS.as_bytes());
    dir.insert_asset("ui/horde/app.tsx".as_ref(), TSX.as_bytes());

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
        StatesPlugin,
    ));
    app.init_resource::<InputFocus>().init_resource::<InputFocusVisible>();
    app.init_state::<GameState>();
    app.init_resource::<UiSnapshot>();
    app.init_resource::<IntentQueue>();
    app.insert_resource(SimConfig::play());
    app.add_plugins(SuperUiPlugin);
    register_bridge(&mut app);
    // Emit the frame each tick from the injected snapshot (mirrors push_ui_frame,
    // but the test controls the snapshot).
    app.add_systems(Update, emit_frame);
    app.finish();
    app
}

fn emit_frame(
    snap: Res<UiSnapshot>,
    state: Res<State<GameState>>,
    cfg: Res<SimConfig>,
    mut commands: Commands,
) {
    let f = horde::ui::supersolid::bridge::build_frame(&snap, state.get(), cfg.arena_half);
    commands.trigger(f);
}

pub fn mount(app: &mut App) -> Entity {
    let (html, css, js) = {
        let s = app.world().resource::<AssetServer>().clone();
        (s.load("ui/horde/index.html"),
         s.load::<StyleSheet>("ui/horde/theme.css"),
         s.load::<JsSource>("ui/horde/app.tsx"))
    };
    let root = app.world_mut().spawn((Node::default(), SuperUiRoot { html, css, js })).id();
    for _ in 0..256 {
        app.update();
        if app.world().contains_non_send::<UiRuntime>() { break; }
    }
    root
}

pub fn tick(app: &mut App, n: usize) { for _ in 0..n { app.update(); } }

pub fn set_state(app: &mut App, s: GameState) {
    app.world_mut().resource_mut::<NextState<GameState>>().set(s);
    tick(app, 2);
}

pub fn edit_snapshot(app: &mut App, f: impl FnOnce(&mut UiSnapshot)) {
    f(&mut app.world_mut().resource_mut::<UiSnapshot>());
    tick(app, 2);
}

pub fn node_by_selector(app: &App, sel: &str) -> NodeId {
    let rt = app.world().non_send_resource::<UiRuntime>();
    let d = rt.dom.borrow();
    d.query_selector(d.document(), sel).unwrap_or_else(|| panic!("selector matched nothing: {sel}"))
}
pub fn maybe_node(app: &App, sel: &str) -> Option<NodeId> {
    let rt = app.world().non_send_resource::<UiRuntime>();
    let d = rt.dom.borrow();
    d.query_selector(d.document(), sel)
}
pub fn nodes_by_selector(app: &App, sel: &str) -> Vec<NodeId> {
    let rt = app.world().non_send_resource::<UiRuntime>();
    let d = rt.dom.borrow();
    d.query_selector_all(d.document(), sel)
}
pub fn text_content(app: &App, node: NodeId) -> String {
    app.world().non_send_resource::<UiRuntime>().dom.borrow().text_content(node)
}
pub fn attr(app: &App, node: NodeId, name: &str) -> String {
    app.world().non_send_resource::<UiRuntime>().dom.borrow()
        .get_attribute(node, name)
        .map(|s| s.to_string())
        .unwrap_or_default()
}
pub fn classes(app: &App, node: NodeId) -> Vec<String> {
    app.world().non_send_resource::<UiRuntime>().dom.borrow().classes(node)
}
pub fn click(app: &mut App, node: NodeId) {
    app.world_mut().resource_mut::<PendingDomEvents>().0.push(PendingDomEvent::new(node, "click"));
    tick(app, 2);
}
