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
use superui::HtmlSource;
use superui_bridge::UiRuntime;

#[derive(Clone)]
pub struct HostProject {
    /// Full manifest HTML (`index.html` content) — must declare `<link>` and
    /// `<script>` that reference the CSS and JS registered in memory.
    pub html: String,
    /// Stylesheet content (registered at `ui/style.css` and `ui/theme.css`).
    pub css: String,
    /// JS or TSX source content (registered at `ui/app.tsx` or `ui/app.js`).
    pub js_or_tsx: String,
    pub tsx: bool,
}

/// Register the in-memory asset source for a project. Registers the manifest
/// at `ui/index.html`, the CSS at `ui/style.css` + `ui/theme.css`, and the
/// script at `ui/app.tsx` or `ui/app.js`. For TSX projects, the content is
/// pre-transpiled and registered at the generated-JS path (`ui/.superui/build/app.js`)
/// so the non-HMR mount seam (`app.tsx` → `.superui/build/app.js`) can find it.
/// Shared by headless and render hosts.
pub(crate) fn register_project_assets(app: &mut App, project: &HostProject) {
    let dir = Dir::new("assets".into());
    dir.insert_asset("ui/index.html".as_ref(), project.html.as_bytes().to_vec());
    // Register under both common names so both `style.css` and `theme.css`
    // hrefs in the manifest work without the caller needing to know which name
    // the manifest uses.
    dir.insert_asset("ui/style.css".as_ref(), project.css.as_bytes().to_vec());
    dir.insert_asset("ui/theme.css".as_ref(), project.css.as_bytes().to_vec());
    if project.tsx {
        // Register the raw source at `ui/app.tsx` (live-HMR path).
        dir.insert_asset("ui/app.tsx".as_ref(), project.js_or_tsx.as_bytes().to_vec());
        // In non-HMR builds (including all test runs) the mount seam maps
        // `app.tsx` → `ui/.superui/build/app.js`. Pre-transpile and register
        // the output there so the JsLoader finds it on that path.
        let opts = supersolid::TranspileOptions {
            tsx: true,
            module_id: Some("ui/app.tsx".to_string()),
            ..Default::default()
        };
        let result = supersolid::transpile(&project.js_or_tsx, &opts);
        dir.insert_asset("ui/.superui/build/app.js".as_ref(), result.code.as_bytes().to_vec());
    } else {
        dir.insert_asset("ui/app.js".as_ref(), project.js_or_tsx.as_bytes().to_vec());
    }

    app.register_asset_source(
        AssetSourceId::Default,
        AssetSource::build().with_reader(move || Box::new(MemoryAssetReader { root: dir.clone() })),
    );
}

pub fn build_headless_app(project: &HostProject) -> App {
    let mut app = App::new();
    register_project_assets(&mut app, project);
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
    app
}

/// Load the entry-HTML handle and spawn a single viewport-filling `SuperUiRoot`.
/// The mount system discovers the CSS and script from the manifest's `<head>`.
/// Does NOT pump frames — the caller (or `mount_when_ready`) drives mounting.
/// Returns the spawned root entity.
pub fn spawn_root(world: &mut World) -> Entity {
    let html = world.resource::<AssetServer>().load::<HtmlSource>("ui/index.html");
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
            SuperUiRoot { html },
        ))
        .id()
}

/// Kept for callers that reference it (e.g. `ui_mode.rs`). With the manifest
/// model the js path is discovered from the entry HTML; this resource is a no-op
/// placeholder for backward compat until all callers are updated.
#[derive(Resource, Clone, Default)]
pub(crate) struct HostAssetPaths;

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

/// World-based variant of [`install_abi`] for the in-world stepper.
pub fn install_abi_world(world: &mut World) {
    let mut rt = world
        .remove_non_send_resource::<UiRuntime>()
        .expect("mounted");
    crate::abi::install(rt.engine.context_mut());
    world.insert_non_send_resource(rt);
}
