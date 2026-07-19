//! Headless Bevy app harness for bridge integration tests: the CSS engine + UI
//! stack, no window/GPU, plus helpers to mount a `UiRuntime` and tick.
#![allow(dead_code)]

use std::cell::RefCell;
use std::rc::Rc;

use bevy::image::{ImagePlugin, TextureAtlasPlugin};
use bevy::input::InputPlugin;
use bevy::input_focus::{InputFocus, InputFocusVisible};
use bevy::picking::{InteractionPlugin, PickingPlugin};
use bevy::prelude::*;
use bevy::text::TextPlugin;
use bevy::ui::UiPlugin;
use superui_bridge::{reconcile_system, UiRuntime};
use superui_css::style::StyleSheet;
use superui_css::SuperUiCssPlugin;
use superui_dom::Dom;

/// A headless app with the CSS engine + UI stack installed.
pub fn test_app() -> App {
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
    app.finish();
    app
}

/// Spawn a root entity, build a `UiRuntime` around `dom`, insert it NonSend, and
/// register the reconcile system in `Update`. Returns the root entity.
pub fn mount(app: &mut App, dom: Rc<RefCell<Dom>>) -> Entity {
    let root = app.world_mut().spawn(Node::default()).id();
    let stylesheet: Handle<StyleSheet> = Handle::default();
    let rt = UiRuntime::new(dom, root, stylesheet);
    app.world_mut().insert_non_send_resource(rt);
    app.add_systems(Update, reconcile_system);
    root
}

/// Number of direct children of `entity`.
pub fn child_count(app: &mut App, entity: Entity) -> usize {
    app.world_mut()
        .get::<Children>(entity)
        .map(|c| c.len())
        .unwrap_or(0)
}
