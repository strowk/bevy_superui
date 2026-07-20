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

/// Marks an entity as an authored-UI mount point (holds its asset handles). The
/// entity is also the ECS parent the DOM `<body>` reconciles into.
#[derive(Component, Default)]
pub struct SuperUiRoot {
    pub html: Handle<HtmlSource>,
    pub css: Handle<StyleSheet>,
    pub js: Handle<JsSource>,
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
            .add_systems(
                Update,
                // Ordering rule: JS-dispatching systems (drain_dom_events,
                // keyboard_events, emit_bevy_inbox) must run BEFORE
                // drain_bevy_outbox so that a bevy.send issued from a DOM-event
                // or timer callback is triggered the same frame. (A game-event
                // → bevy.on callback that itself calls bevy.send still lags one
                // frame — acceptable Phase 1 trade-off.)
                (
                    apply_hot_reload,
                    drain_dom_events_system,
                    keyboard_events_system,
                    blink_caret_system,
                    emit_bevy_inbox_system,
                    drain_bevy_outbox_system,
                    tick_timers_system,
                    reconcile_system,
                )
                    .chain()
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

/// When a `SuperUiRoot`'s three assets are all loaded and no runtime exists yet,
/// build the runtime: parse HTML -> Dom, mount at the root entity with the css
/// handle, run the author JS. Inserts the `UiRuntime` NonSend resource.
///
/// Exclusive system (`&mut World`) so we can `insert_non_send_resource` on the
/// runtime without requiring `Send` (Boa's `Rc<_>` types are `!Send`).
pub fn mount_when_ready(world: &mut World) {
    // Guard: one mounted UI at a time.
    if world.contains_non_send::<UiRuntime>() {
        return;
    }

    // Get the single SuperUiRoot entity (skip if none / ambiguous).
    let (entity, html_handle, css_handle, js_handle) = {
        let mut q = world.query::<(Entity, &SuperUiRoot)>();
        let Ok((entity, root)) = q.single(world) else {
            return;
        };
        (entity, root.html.clone(), root.css.clone(), root.js.clone())
    };

    // All three assets must be loaded.
    let all_loaded = {
        let server = world.resource::<AssetServer>();
        let html_loaded = matches!(server.load_state(html_handle.id()), LoadState::Loaded);
        let css_loaded = matches!(server.load_state(css_handle.id()), LoadState::Loaded);
        let js_loaded = matches!(server.load_state(js_handle.id()), LoadState::Loaded);
        html_loaded && css_loaded && js_loaded
    };
    if !all_loaded {
        return;
    }

    // Retrieve HTML and JS source bytes.
    let html_src = {
        let assets = world.resource::<Assets<HtmlSource>>();
        match assets.get(&html_handle) {
            Some(h) => h.0.clone(),
            None => return,
        }
    };
    let js_src = {
        let assets = world.resource::<Assets<JsSource>>();
        match assets.get(&js_handle) {
            Some(j) => j.0.clone(),
            None => return,
        }
    };

    // Plan 5 gate: feature + asset watcher. Warn once (here, the single mount
    // point) if the feature is enabled but nothing is watching, then stay off.
    let watching = world.resource::<AssetServer>().watching_for_changes();
    let hmr = hmr_active(watching);
    #[cfg(feature = "hmr")]
    if !watching {
        bevy::log::warn!(
            "superui: `hmr` feature is enabled but the AssetServer is not watching for \
             changes; state-preserving hot reload is OFF. Enable `bevy/file_watcher` (or set \
             AssetPlugin.watch_for_changes_override = Some(true)) to activate it."
        );
    }

    // Build the runtime: parse HTML -> Dom, wire engine, run author JS.
    let dom = Rc::new(RefCell::new(superui_html::parse_document(&html_src)));
    let mut rt = UiRuntime::new(dom, entity, css_handle, hmr);
    rt.run_script(&js_src);

    // Insert as a NonSend resource — valid because we're in an exclusive system
    // running on the main thread.
    world.insert_non_send_resource(rt);
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
