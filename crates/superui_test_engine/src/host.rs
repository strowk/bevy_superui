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
use superui_bridge::UiRuntime;
use superui_css::style::StyleSheet;

pub struct HostProject {
    pub html: String,
    pub css: String,
    pub js_or_tsx: String,
    pub tsx: bool,
}

/// Register the in-memory asset source for a project and return the js/tsx path
/// that `mount()` should load. Shared by the headless and render hosts.
pub(crate) fn register_project_assets(app: &mut App, project: &HostProject) -> String {
    let dir = Dir::new("assets".into());
    dir.insert_asset("ui/index.html".as_ref(), project.html.as_bytes().to_vec());
    dir.insert_asset("ui/style.css".as_ref(), project.css.as_bytes().to_vec());
    let ui_js_path = if project.tsx { "ui/app.tsx" } else { "ui/app.js" };
    dir.insert_asset(ui_js_path.as_ref(), project.js_or_tsx.as_bytes().to_vec());

    app.register_asset_source(
        AssetSourceId::Default,
        AssetSource::build().with_reader(move || Box::new(MemoryAssetReader { root: dir.clone() })),
    );
    ui_js_path.to_string()
}

pub fn build_headless_app(project: &HostProject) -> App {
    let mut app = App::new();
    let ui_js_path = register_project_assets(&mut app, project);
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
    app.add_plugins(SuperUiPlugin);
    app.finish();

    // Store the js path so mount() can load the right handle type.
    app.insert_resource(HostAssetPaths { js: ui_js_path });
    app
}

#[derive(Resource, Clone)]
pub(crate) struct HostAssetPaths {
    pub(crate) js: String,
}

pub fn mount(app: &mut App) -> Entity {
    let paths = app.world().resource::<HostAssetPaths>().clone();
    let (html, css, js) = {
        let s = app.world().resource::<AssetServer>().clone();
        (
            s.load("ui/index.html"),
            s.load::<StyleSheet>("ui/style.css"),
            s.load::<JsSource>(paths.js.clone()),
        )
    };
    let root = app
        .world_mut()
        .spawn((Node::default(), SuperUiRoot { html, css, js }))
        .id();
    for _ in 0..256 {
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

/// After mount, install the `$sstest` ABI into the live runtime's Boa context.
pub fn install_abi(app: &mut App) {
    let mut rt = app
        .world_mut()
        .remove_non_send_resource::<UiRuntime>()
        .expect("mounted");
    crate::abi::install(rt.engine.context_mut());
    app.world_mut().insert_non_send_resource(rt);
}
