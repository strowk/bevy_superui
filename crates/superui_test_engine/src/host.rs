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

#[derive(Clone)]
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

/// Load the project's html/css/js handles and spawn a single viewport-filling
/// `SuperUiRoot`. Does NOT pump frames — the caller (or `mount_when_ready`)
/// drives mounting. Returns the spawned root entity.
pub fn spawn_root(world: &mut World) -> Entity {
    let paths = world.resource::<HostAssetPaths>().clone();
    let (html, css, js) = {
        let s = world.resource::<AssetServer>().clone();
        (
            s.load("ui/index.html"),
            s.load::<StyleSheet>("ui/style.css"),
            s.load::<JsSource>(paths.js.clone()),
        )
    };
    // The root MUST fill the viewport: game_menu (and similar UIs) have a
    // `#root`/`.stage` tree with `100%`/`inset:0`/`position:absolute` children
    // that collapse to zero against an auto-sized root, producing BLANK
    // screenshots. Filling the viewport is harmless for the headless DOM tests.
    world
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            SuperUiRoot { html, css, js },
        ))
        .id()
}

pub fn mount(app: &mut App) -> Entity {
    // Idempotency guard: if a UiRuntime is already present the UI has already
    // been mounted.  Return the existing SuperUiRoot entity rather than
    // spawning a second one (which would be a stray, orphaned entity).
    if app.world().contains_non_send::<UiRuntime>() {
        let mut q = app.world_mut().query::<(Entity, &SuperUiRoot)>();
        if let Ok((entity, _)) = q.single(app.world()) {
            return entity;
        }
        // Degenerate: runtime exists but root entity is gone/ambiguous.
        // Fall through to the normal spawn path so the caller always gets a
        // valid entity back.
    }

    let root = spawn_root(app.world_mut());
    for _ in 0..256 {
        app.update();
        if app.world().contains_non_send::<UiRuntime>() {
            break;
        }
    }
    root
}

/// Reset the mounted UI: despawn every `SuperUiRoot` (and its descendants) and
/// remove the `UiRuntime`, so the next `mount_when_ready` rebuilds a fresh DOM.
/// Used by the `--ui` stepper to give each Run isolated state.
pub fn teardown(world: &mut World) {
    let roots: Vec<Entity> = {
        let mut q = world.query::<(Entity, &SuperUiRoot)>();
        q.iter(world).map(|(e, _)| e).collect()
    };
    for root in roots {
        world.entity_mut(root).despawn();
    }
    world.remove_non_send_resource::<UiRuntime>();
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
