//! Headless harness for `superui` integration tests: in-memory `.html`/`.css`/
//! `.js` assets + the full app with `SuperUiPlugin`.
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

pub static ASSETS: LazyLock<Dir> = LazyLock::new(|| Dir::new("assets".into()));

pub fn put(name: &str, bytes: &[u8]) {
    ASSETS.insert_asset(name.as_ref(), bytes.to_vec());
}

/// A headless app with the full `SuperUiPlugin` and an in-memory asset source.
pub fn app() -> App {
    let mut app = App::new();
    app.register_asset_source(
        AssetSourceId::Default,
        AssetSourceBuilder::new(move || Box::new(MemoryAssetReader { root: ASSETS.clone() })),
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

/// Build a Model-2 entry document: a `<head>` that links `css` and scripts `js`
/// (both in-memory asset paths at the root), with `body` as the document body.
pub fn entry_doc(body: &str, css: &str, js: &str) -> String {
    format!(
        "<html><head><link rel=\"stylesheet\" href=\"{css}\">\
         <script src=\"{js}\"></script></head><body>{body}</body></html>"
    )
}

/// Spawn a Model-2 `SuperUiRoot` from a freshly synthesized, uniquely-named entry.
pub fn spawn_root(app: &mut App, body: &str, css: &str, js: &str) -> Entity {
    static SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    spawn_root_entry(app, &format!("index_{n}.html"), body, css, js)
}

/// Like [`spawn_root`] but with a caller-chosen entry asset path, so a test can
/// re-`put(entry, entry_doc(...))` later to exercise remount-on-html-change.
pub fn spawn_root_entry(app: &mut App, entry: &str, body: &str, css: &str, js: &str) -> Entity {
    put(entry, entry_doc(body, css, js).as_bytes());
    let server = app.world().resource::<AssetServer>().clone();
    let root = SuperUiRoot { html: server.load::<HtmlSource>(entry.to_string()) };
    app.world_mut().spawn((Node::default(), root)).id()
}

/// Tick `n` frames (enough for asset load + mount + reconcile).
pub fn tick(app: &mut App, n: usize) {
    for _ in 0..n {
        app.update();
    }
}
