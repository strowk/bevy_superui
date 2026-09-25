//! Headless harness for `superui_playground_web` integration tests: mirrors
//! `crates/superui/tests/support/mod.rs`, plus `PlaygroundBridgePlugin` and a
//! watch override so `hmr_active()` is true (state-preserving rehydration, exactly
//! as a real playground build configures itself).
#![allow(dead_code)]

use bevy::asset::io::memory::{Dir, MemoryAssetReader};
use bevy::asset::io::{AssetSourceBuilder, AssetSourceId};
use bevy::input::InputPlugin;
use bevy::input_focus::{InputFocus, InputFocusVisible};
use bevy::picking::{InteractionPlugin, PickingPlugin};
use bevy::image::TextureAtlasPlugin;
use bevy::prelude::*;
use bevy::text::TextPlugin;
use bevy::ui::UiPlugin;
use std::sync::LazyLock;
use superui::{HtmlSource, SuperUiPlugin, SuperUiRoot};
use superui_playground_web::PlaygroundBridgePlugin;

pub static ASSETS: LazyLock<Dir> = LazyLock::new(|| Dir::new("assets".into()));

pub fn put(name: &str, bytes: &[u8]) {
    ASSETS.insert_asset(name.as_ref(), bytes.to_vec());
}

pub fn app() -> App {
    let mut app = App::new();
    app.register_asset_source(
        AssetSourceId::Default,
        AssetSourceBuilder::new(move || Box::new(MemoryAssetReader { root: ASSETS.clone() })),
    );
    // watch override ON so hmr_active() is true -> state-preserving rehydration,
    // exactly as a real playground build configures itself.
    let asset_plugin = AssetPlugin { watch_for_changes_override: Some(true), ..default() };
    app.add_plugins((
        bevy::time::TimePlugin,
        bevy::app::TaskPoolPlugin::default(),
        asset_plugin,
        WindowPlugin::default(),
        bevy::image::ImagePlugin::default(),
        TextureAtlasPlugin,
        TextPlugin,
        (InputPlugin, PickingPlugin, InteractionPlugin, UiPlugin),
        SuperUiPlugin,
        PlaygroundBridgePlugin,
    ));
    app.init_resource::<InputFocus>().init_resource::<InputFocusVisible>();
    app.finish();
    app
}

pub fn entry_doc(body: &str, css: &str, js: &str) -> String {
    format!(
        "<html><head><link rel=\"stylesheet\" href=\"{css}\">\
         <script src=\"{js}\"></script></head><body>{body}</body></html>"
    )
}

pub fn spawn_root(app: &mut App, entry: &str, body: &str, css: &str, js: &str) -> Entity {
    put(entry, entry_doc(body, css, js).as_bytes());
    let server = app.world().resource::<AssetServer>().clone();
    let root = SuperUiRoot { html: server.load::<HtmlSource>(entry.to_string()) };
    app.world_mut().spawn((Node::default(), root)).id()
}

pub fn tick(app: &mut App, n: usize) {
    for _ in 0..n { app.update(); }
}

/// Reconciled `Text` of the first `<span>`'s text-child entity (counter label).
pub fn label_text(app: &mut App) -> String {
    use superui_css::prelude::TypeName;
    let mut spans = app.world_mut().query::<(&TypeName, &Children)>();
    let child = spans
        .iter(app.world())
        .find(|(t, _)| t.0 == "span")
        .and_then(|(_, c)| c.iter().next());
    match child {
        Some(e) => app.world().get::<Text>(e).map(|t| t.0.clone()).unwrap_or_default(),
        None => String::new(),
    }
}
