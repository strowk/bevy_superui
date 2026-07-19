//! Headless harness for `superui` integration tests: in-memory `.html`/`.css`/
//! `.js` assets + the full app with `SuperUiPlugin`.
#![allow(dead_code)]

use bevy::asset::io::memory::{Dir, MemoryAssetReader};
use bevy::asset::io::{AssetSource, AssetSourceId};
use bevy::input::InputPlugin;
use bevy::input_focus::{InputFocus, InputFocusVisible};
use bevy::picking::{InteractionPlugin, PickingPlugin};
use bevy::image::TextureAtlasPlugin;
use bevy::prelude::*;
use bevy::text::TextPlugin;
use bevy::ui::UiPlugin;
use std::sync::LazyLock;
use superui::{HtmlSource, JsSource, SuperUiPlugin, SuperUiRoot};
use superui_css::style::StyleSheet;

pub static ASSETS: LazyLock<Dir> = LazyLock::new(|| Dir::new("assets".into()));

pub fn put(name: &str, bytes: &[u8]) {
    ASSETS.insert_asset(name.as_ref(), bytes.to_vec());
}

/// A headless app with the full `SuperUiPlugin` and an in-memory asset source.
pub fn app() -> App {
    let mut app = App::new();
    app.register_asset_source(
        AssetSourceId::Default,
        AssetSource::build()
            .with_reader(move || Box::new(MemoryAssetReader { root: ASSETS.clone() })),
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
        SuperUiPlugin,
    ));
    app.init_resource::<InputFocus>()
        .init_resource::<InputFocusVisible>();
    app.finish();
    app
}

/// Spawn a `SuperUiRoot` referencing the given in-memory asset paths.
pub fn spawn_root(app: &mut App, html: &str, css: &str, js: &str) -> Entity {
    let server = app.world().resource::<AssetServer>().clone();
    let root = SuperUiRoot {
        html: server.load::<HtmlSource>(html.to_string()),
        css: server.load::<StyleSheet>(css.to_string()),
        js: server.load::<JsSource>(js.to_string()),
    };
    app.world_mut().spawn((Node::default(), root)).id()
}

/// Tick `n` frames (enough for asset load + mount + reconcile).
pub fn tick(app: &mut App, n: usize) {
    for _ in 0..n {
        app.update();
    }
}
