//! `SuperUiPlugin` + the mount system that turns loaded assets into a live
//! `UiRuntime` and schedules the per-frame reconcile/input/bridge systems.

use std::cell::RefCell;
use std::rc::Rc;

use bevy::asset::LoadState;
use bevy::prelude::*;
use superui_bridge::{
    blink_caret_system, drain_bevy_outbox_system, drain_dom_events_system, emit_bevy_inbox_system,
    keyboard_events_system, on_pointer_click, reconcile_system, PendingDomEvents, UiRuntime,
};
use superui_css::style::StyleSheet;
use superui_css::SuperUiCssPlugin;

use crate::assets::{HtmlLoader, HtmlSource, JsLoader, JsSource};
use crate::hot_reload::{HotReloadFlags, apply_hot_reload, detect_hot_reload};

/// The Plan 5 HMR gate: active only when the `hmr` feature is compiled in AND the
/// asset server is watching for changes (there is no point collecting HMR state
/// when no edit can ever be observed). `cfg!` folds to `false` with the feature off.
pub(crate) fn hmr_active(watching: bool) -> bool {
    cfg!(feature = "hmr") && watching
}

/// Marks an authored-UI mount point. The single authored asset is the entry HTML;
/// stylesheets and the script are discovered from its `<head>` at mount. The root
/// entity is the bevy_ui parent the DOM `<body>` reconciles under, so it needs a
/// `Node` — `#[require(Node)]` inserts a default one when a spawn omits it.
#[derive(Component, Default)]
#[require(Node)]
pub struct SuperUiRoot {
    pub html: Handle<HtmlSource>,
}

impl SuperUiRoot {
    /// Spawn an authored UI from `<dir>/index.html` (an asset-root-relative dir).
    /// Bundles a full-viewport `Node`.
    pub fn from_asset_dir(dir: &str, assets: &AssetServer) -> impl Bundle {
        Self::from_asset_dir_with(dir, fill_node(), assets)
    }

    /// Like [`SuperUiRoot::from_asset_dir`] but with a caller-supplied root `Node`.
    pub fn from_asset_dir_with(dir: &str, node: Node, assets: &AssetServer) -> impl Bundle {
        let path = format!("{dir}/{}", superui_paths::ENTRY_HTML);
        (node, SuperUiRoot { html: assets.load::<HtmlSource>(path) })
    }
}

fn fill_node() -> Node {
    Node { width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }
}

/// The subresources discovered from the entry HTML's `<head>` (inserted in mount
/// Phase 1; consumed in Phase 2 and by hot reload).
#[derive(Component)]
pub(crate) struct SuperUiSubresources {
    /// First `<link rel=stylesheet>`, or `None` when the document declares none.
    pub css: Option<Handle<StyleSheet>>,
    /// The `<script src>` resolved through the tsx/js seam.
    pub js: Handle<JsSource>,
}

/// Live authoring: native + the `hmr` feature. Every other build loads generated JS.
pub(crate) fn live_source() -> bool {
    cfg!(all(not(target_arch = "wasm32"), feature = "hmr"))
}

/// Map a `<script src>` to the asset path to load, applying the tsx/js seam.
fn resolve_script(dir: &str, src: &str, live: bool) -> String {
    let path = superui_paths::join_asset(dir, src);
    let is_tsx = path.ends_with(".tsx") || path.ends_with(".ts");
    if is_tsx && !live {
        superui_paths::generated_js(&path)
    } else {
        path
    }
}

/// Depth-first (document order) walk collecting the first stylesheet `href` and the
/// first `<script src>` (already resolved through the seam). Warns on extras.
fn collect_refs(dom: &superui_dom::Dom, dir: &str, live: bool) -> (Option<String>, Option<String>) {
    fn walk(
        dom: &superui_dom::Dom,
        node: superui_dom::NodeId,
        dir: &str,
        live: bool,
        css: &mut Option<String>,
        js: &mut Option<String>,
    ) {
        match dom.tag(node) {
            Some("link") => {
                let is_sheet = dom
                    .get_attribute(node, "rel")
                    .map(|r| r.eq_ignore_ascii_case("stylesheet"))
                    .unwrap_or(false);
                if is_sheet {
                    if let Some(href) = dom.get_attribute(node, "href") {
                        if css.is_none() {
                            *css = Some(superui_paths::join_asset(dir, href));
                        } else {
                            warn!(
                                "superui: only one stylesheet is supported for now; using \
                                 the first <link rel=stylesheet> and ignoring \"{href}\""
                            );
                        }
                    }
                }
            }
            Some("script") => {
                if let Some(src) = dom.get_attribute(node, "src") {
                    if js.is_none() {
                        *js = Some(resolve_script(dir, src, live));
                    } else {
                        warn!("superui: multiple <script src>; ignoring {src}");
                    }
                }
            }
            _ => {}
        }
        for &child in dom.children(node) {
            walk(dom, child, dir, live, css, js);
        }
    }

    let mut css = None;
    let mut js = None;
    walk(dom, dom.document(), dir, live, &mut css, &mut js);
    (css, js)
}

/// Turn discovered ref paths into loaded handles (kicks off the async loads).
fn discover_subresources(
    dom: &superui_dom::Dom,
    dir: &str,
    live: bool,
    assets: &AssetServer,
) -> (Option<Handle<StyleSheet>>, Handle<JsSource>) {
    let (css_path, js_path) = collect_refs(dom, dir, live);
    let css = css_path.map(|p| assets.load::<StyleSheet>(p));
    let js = match js_path {
        Some(p) => assets.load::<JsSource>(p),
        None => {
            warn!("superui: entry HTML declares no <script src>; the UI will be inert");
            Handle::default()
        }
    };
    (css, js)
}

/// The umbrella plugin.
pub struct SuperUiPlugin;

impl Plugin for SuperUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(SuperUiCssPlugin)
            .init_asset::<HtmlSource>()
            .init_asset::<JsSource>()
            .register_asset_loader(HtmlLoader)
            .register_asset_loader(JsLoader);
        #[cfg(not(target_arch = "wasm32"))]
        app.register_asset_loader(crate::assets::TsxLoader);
        app
            .init_resource::<PendingDomEvents>()
            .init_resource::<HotReloadFlags>()
            .add_observer(on_pointer_click)
            .add_systems(Update, mount_when_ready)
            .add_systems(Update, detect_hot_reload.after(mount_when_ready))
            // apply_hot_reload runs OUTSIDE the runtime_exists chain: for an HTML
            // change it tears down the runtime and returns, so the bridge chain
            // (which checks runtime_exists after this) correctly skips that frame.
            .add_systems(
                Update,
                apply_hot_reload
                    .after(detect_hot_reload)
                    .after(mount_when_ready),
            )
            .add_systems(
                Update,
                // Ordering rule: JS-dispatching systems (drain_dom_events,
                // keyboard_events, emit_bevy_inbox) must run BEFORE
                // drain_bevy_outbox so that a bevy.send issued from a DOM-event
                // or timer callback is triggered the same frame. (A game-event
                // → bevy.on callback that itself calls bevy.send still lags one
                // frame — acceptable Phase 1 trade-off.)
                (
                    drain_dom_events_system,
                    keyboard_events_system,
                    blink_caret_system,
                    emit_bevy_inbox_system,
                    drain_bevy_outbox_system,
                    tick_timers_system,
                    reconcile_system,
                )
                    .chain()
                    .after(apply_hot_reload)
                    .after(detect_hot_reload)
                    .after(mount_when_ready)
                    .run_if(runtime_exists),
            );
    }
}

/// Run condition: only run the bridge systems when a `UiRuntime` is present.
fn runtime_exists(world: &World) -> bool {
    world.contains_non_send::<UiRuntime>()
}

/// Drive Boa timers each frame from Bevy's clock.
fn tick_timers_system(time: Res<Time>, rt: Option<NonSendMut<UiRuntime>>) {
    use superui_js::JsEngine;
    if let Some(mut rt) = rt {
        let now_ms = time.elapsed_secs_f64() * 1000.0;
        rt.engine.run_timers(now_ms);
    }
}

/// Two-phase mount: Phase 1 waits for the entry HTML to load, then discovers and
/// kicks off loads for the declared subresources. Phase 2 waits for subresources,
/// then builds the runtime, runs the author JS, and inserts the `UiRuntime`.
///
/// Exclusive system (`&mut World`) so we can `insert_non_send_resource` on the
/// runtime without requiring `Send` (Boa's `Rc<_>` types are `!Send`).
pub fn mount_when_ready(world: &mut World) {
    // Guard: one mounted UI at a time.
    if world.contains_non_send::<UiRuntime>() {
        return;
    }

    // The single SuperUiRoot entity + its entry-HTML handle.
    let (entity, html_handle) = {
        let mut q = world.query::<(Entity, &SuperUiRoot)>();
        let Ok((entity, root)) = q.single(world) else {
            return;
        };
        (entity, root.html.clone())
    };

    // The entry HTML must be loaded before we can read the manifest.
    if !matches!(
        world.resource::<AssetServer>().load_state(html_handle.id()),
        LoadState::Loaded
    ) {
        return;
    }

    // Phase 1: discover the declared subresources and kick off their loads.
    if world.get::<SuperUiSubresources>(entity).is_none() {
        let html_src = match world.resource::<Assets<HtmlSource>>().get(&html_handle) {
            Some(h) => h.0.clone(),
            None => return,
        };
        let dir = html_handle
            .path()
            .map(|p| superui_paths::parent_dir(&p.to_string()).to_string())
            .unwrap_or_default();
        let dom = superui_html::parse_document(&html_src);
        let assets = world.resource::<AssetServer>().clone();
        let (css, js) = discover_subresources(&dom, &dir, live_source(), &assets);
        world.entity_mut(entity).insert(SuperUiSubresources { css, js });
        return;
    }

    // Phase 2: wait for the subresources, then build the runtime.
    let (css_handle, js_handle) = {
        let sub = world.get::<SuperUiSubresources>(entity).unwrap();
        (sub.css.clone(), sub.js.clone())
    };
    {
        let server = world.resource::<AssetServer>();
        let css_ok = match &css_handle {
            Some(h) => matches!(server.load_state(h.id()), LoadState::Loaded),
            None => true,
        };
        let js_ok = matches!(server.load_state(js_handle.id()), LoadState::Loaded);
        if !(css_ok && js_ok) {
            return;
        }
    }
    let html_src = match world.resource::<Assets<HtmlSource>>().get(&html_handle) {
        Some(h) => h.0.clone(),
        None => return,
    };
    let js_src = match world.resource::<Assets<JsSource>>().get(&js_handle) {
        Some(j) => j.0.clone(),
        None => return,
    };

    // HMR gate: feature + asset watcher. Warn once if the feature is on but nothing
    // is watching, then stay off.
    let watching = world.resource::<AssetServer>().watching_for_changes();
    let hmr = hmr_active(watching);
    #[cfg(feature = "hmr")]
    if !watching {
        bevy::log::warn!(
            "superui: `hmr` feature is enabled but the AssetServer is not watching for \
             changes; state-preserving hot reload is OFF. Enable `bevy/file_watcher` to \
             activate it."
        );
    }

    let dom = Rc::new(RefCell::new(superui_html::parse_document(&html_src)));
    let mut rt = UiRuntime::new(dom, entity, css_handle.unwrap_or_default(), hmr);
    rt.run_script(&js_src);
    world.insert_non_send(rt);
}

#[cfg(test)]
mod hmr_gate_tests {
    use super::hmr_active;

    #[test]
    fn hmr_active_requires_watching_and_feature() {
        // Without the `hmr` feature, the gate is always false.
        // With it, the gate follows `watching`.
        if cfg!(feature = "hmr") {
            assert!(hmr_active(true), "feature on + watching => active");
            assert!(!hmr_active(false), "feature on + not watching => inactive");
        } else {
            assert!(!hmr_active(true), "feature off => inactive even when watching");
            assert!(!hmr_active(false));
        }
    }
}

#[cfg(test)]
mod model2_tests {
    use super::{collect_refs, resolve_script};

    #[test]
    fn resolve_script_applies_the_tsx_js_seam() {
        // Non-live: .tsx/.ts map to the generated build artifact.
        assert_eq!(resolve_script("ui/counter", "app.tsx", false), "ui/counter/.superui/build/app.js");
        assert_eq!(resolve_script("ui/counter", "app.ts", false), "ui/counter/.superui/build/app.js");
        // Live: load the author source as-is.
        assert_eq!(resolve_script("ui/counter", "app.tsx", true), "ui/counter/app.tsx");
        // Plain .js passes through regardless.
        assert_eq!(resolve_script("ui/counter", "app.js", false), "ui/counter/app.js");
    }

    #[test]
    fn collect_refs_finds_first_link_and_script_resolved() {
        let dom = superui_html::parse_document(
            r#"<html><head>
                 <link rel="stylesheet" href="style.css">
                 <script type="module" src="app.tsx"></script>
               </head><body><div id="root"></div></body></html>"#,
        );
        let (css, js) = collect_refs(&dom, "ui/counter", false);
        assert_eq!(css.as_deref(), Some("ui/counter/style.css"));
        assert_eq!(js.as_deref(), Some("ui/counter/.superui/build/app.js"));
    }

    #[test]
    fn collect_refs_live_keeps_tsx_source() {
        let dom = superui_html::parse_document(
            r#"<html><head><script src="app.tsx"></script></head><body></body></html>"#,
        );
        let (_css, js) = collect_refs(&dom, "ui/x", true);
        assert_eq!(js.as_deref(), Some("ui/x/app.tsx"));
    }
}
