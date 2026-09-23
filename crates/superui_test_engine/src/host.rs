use bevy::asset::io::memory::{Dir, MemoryAssetReader};
use bevy::asset::io::{AssetSourceBuilder, AssetSourceId};
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
        AssetSourceBuilder::new(move || Box::new(MemoryAssetReader { root: dir.clone() })),
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

/// Idempotency guard shared by [`mount`] and [`mount_with_camera`]: if a
/// `UiRuntime` is already present the UI has already been mounted, so return
/// its `SuperUiRoot` entity instead of letting the caller spawn (and, for
/// `mount_with_camera`, camera-tag) a second, orphaned one. `None` means not
/// mounted yet, or the degenerate case (runtime present, root gone/ambiguous)
/// — either way the caller should fall through to its normal spawn-and-tick.
fn existing_root(app: &mut App) -> Option<Entity> {
    if !app.world().contains_non_send::<UiRuntime>() {
        return None;
    }
    let mut q = app.world_mut().query::<(Entity, &SuperUiRoot)>();
    q.single(app.world()).ok().map(|(entity, _)| entity)
}

pub fn mount(app: &mut App) -> Entity {
    if let Some(entity) = existing_root(app) {
        return entity;
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

/// Like [`mount`], but tags the root with `UiTargetCamera(camera)` *before*
/// the first tick, so `100%` sizing resolves against the offscreen render
/// target from the very first reconcile+layout pass instead of an unknown
/// (0x0) viewport. `ui_driver::start_run`'s incremental stepper already does
/// this; `render::build_render_app_and_mount` used to tag the camera only
/// after `mount` returned, so that first pass could compute a negative size
/// for a bordered, auto-height container with margin (e.g. a `margin: 40px`
/// card) — negative sizes panic in `bevy_ui::ui_node::BorderRadius::resolve`
/// rather than just rendering blank.
pub fn mount_with_camera(app: &mut App, camera: Entity) -> Entity {
    if let Some(entity) = existing_root(app) {
        return entity;
    }

    // Tags the one root `spawn_root` just returned, not a query over every
    // `SuperUiRoot` — relies on the current single-root-per-app invariant; a
    // future multi-root feature needs to tag each root here, not just this one.
    let root = spawn_root(app.world_mut());
    app.world_mut()
        .entity_mut(root)
        .insert(bevy::ui::UiTargetCamera(camera));
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
    world.remove_non_send::<UiRuntime>();
}

pub fn tick(app: &mut App, n: usize) {
    for _ in 0..n {
        app.update();
    }
}

/// After mount, install the `$sstest` ABI into the live runtime's JS engine.
pub fn install_abi(app: &mut App) {
    let mut rt = app
        .world_mut()
        .remove_non_send::<UiRuntime>()
        .expect("mounted");
    crate::abi::install(rt.engine.as_mut());
    app.world_mut().insert_non_send(rt);
}

/// World-based variant of [`install_abi`] for the in-world stepper.
pub fn install_abi_world(world: &mut World) {
    let mut rt = world
        .remove_non_send::<UiRuntime>()
        .expect("mounted");
    crate::abi::install(rt.engine.as_mut());
    world.insert_non_send(rt);
}
