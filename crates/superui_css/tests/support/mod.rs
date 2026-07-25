//! Headless Bevy app harness for selector integration tests. Serves `.css`
//! from an in-memory asset dir and installs the full CSS engine, no window/GPU.
#![allow(dead_code)]

use bevy::asset::io::memory::{Dir, MemoryAssetReader};
use bevy::asset::io::{AssetSourceBuilder, AssetSourceId};
use bevy::asset::{AssetApp, AssetPlugin, AssetServer, Handle};
use bevy::image::{ImagePlugin, TextureAtlasPlugin};
use bevy::input::InputPlugin;
use bevy::input_focus::{InputFocus, InputFocusVisible};
use bevy::picking::{InteractionPlugin, PickingPlugin};
use bevy::prelude::*;
use bevy::text::TextPlugin;
use bevy::ui::UiPlugin;
use bevy_app::{TaskPoolOptions, TaskPoolPlugin};
use std::sync::LazyLock;

use superui_css::parser::{CssStyleLoaderErrorMode, CssStyleLoaderSetting};
use superui_css::style::StyleSheet;
use superui_css::SuperUiCssPlugin;

/// In-memory asset dir the tests write `.css` bytes into before building the app.
pub static ASSETS_DIR: LazyLock<Dir> = LazyLock::new(|| Dir::new("assets".into()));

/// Write a `.css` file into the in-memory asset dir under `name`.
pub fn put_css(name: &str, contents: &str) {
    // `Dir::insert_asset` stores the bytes, so it needs an owned/`'static`
    // `Value` (`Vec<u8>`), not a borrowed `&[u8]` tied to `contents`'s lifetime.
    ASSETS_DIR.insert_asset(name.as_ref(), contents.as_bytes().to_vec());
}

/// Load a stylesheet in the *continue-loading* error mode. In this mode flair
/// reports malformed rules as warnings but keeps the sheet: the valid rules are
/// still applied. This is exactly the graceful-degradation behaviour the
/// capstone asserts — a single malformed line must NOT abort the whole sheet.
///
/// (The plan's original draft used `ReturnError`, but that mode is documented as
/// "fail to load": under it *any* error — including an unknown property or an
/// unmatched/malformed selector — fails the entire asset, which contradicts the
/// "malformed line does not abort the sheet" requirement in the same task. The
/// capstone needs the sheet to load-with-degradation, so `PrintWarn` — flair's
/// own default — is the mode that matches the stated intent.)
pub trait LoadStyleSheet {
    fn load_style_sheet(&self, path: &str) -> Handle<StyleSheet> {
        self.load_style_sheet_with(path, CssStyleLoaderErrorMode::PrintWarn)
    }
    fn load_style_sheet_with(&self, path: &str, mode: CssStyleLoaderErrorMode)
        -> Handle<StyleSheet>;
}
impl LoadStyleSheet for AssetServer {
    fn load_style_sheet_with(
        &self,
        path: &str,
        mode: CssStyleLoaderErrorMode,
    ) -> Handle<StyleSheet> {
        self.load_with_settings(path.to_string(), move |s: &mut CssStyleLoaderSetting| {
            s.error_mode = mode
        })
    }
}

/// A headless app with the CSS engine installed. Call after `put_css(...)`.
pub fn test_app() -> App {
    let mut app = App::new();

    app.register_asset_source(
        AssetSourceId::Default,
        AssetSourceBuilder::new(move || {
            Box::new(MemoryAssetReader { root: ASSETS_DIR.clone() })
        }),
    );

    app.add_plugins((
        bevy::time::TimePlugin,
        TaskPoolPlugin { task_pool_options: TaskPoolOptions::with_num_threads(1) },
        AssetPlugin::default(),
        WindowPlugin::default(),
        ImagePlugin::default(),
        TextureAtlasPlugin,
        TextPlugin,
        (InputPlugin, PickingPlugin, InteractionPlugin, UiPlugin),
        SuperUiCssPlugin,
    ));

    app.init_resource::<InputFocus>().init_resource::<InputFocusVisible>();
    app.finish(); // installs the CSS asset loader + property registries
    app
}

/// Poll `app.update()` until `path` has finished loading (or panic after N tries).
pub fn load_until_ready(app: &mut App, handle: &Handle<StyleSheet>) {
    use bevy::asset::LoadState;
    for _ in 0..64 {
        app.update();
        let server = app.world().resource::<AssetServer>();
        match server.load_state(handle.id()) {
            LoadState::Loaded => {
                // A couple more frames so the style systems apply computed values.
                app.update();
                app.update();
                return;
            }
            LoadState::Failed(e) => panic!("stylesheet failed to load: {e}"),
            _ => {}
        }
    }
    panic!("stylesheet did not finish loading within 64 frames");
}
