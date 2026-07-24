//! Hot reload via Bevy's asset system (design §6): `AssetEvent::Modified` ->
//! re-parse HTML / re-exec JS / re-cascade CSS -> reconcile. Native `file_watcher`
//! fires these automatically; on wasm the watcher is inactive (no-op), same seam.
//!
//! Implementation: split into a normal `detect_hot_reload` system that reads
//! `MessageReader<AssetEvent<T>>` for each asset type (the only working form in
//! Bevy 0.17.3 — confirmed by `bevy_flair_style/src/systems.rs` which uses the
//! exact same pattern), and an exclusive `apply_hot_reload` system that consumes
//! the flags and performs the actual rebuild / re-exec.

use bevy::ecs::message::MessageReader;
use bevy::prelude::*;
use superui_bridge::UiRuntime;
use superui_css::style::StyleSheet;

use crate::assets::{HtmlSource, JsSource};
use crate::mount::{SuperUiRoot, SuperUiSubresources};

/// Small resource recording which asset types changed this frame.
#[derive(Resource, Default)]
pub struct HotReloadFlags {
    pub html: bool,
    pub js: bool,
    pub css: bool,
}

/// Compare `AssetEvent::Modified` ids against the mounted root's handles and set
/// flags. `html` comes from `SuperUiRoot`; `js`/`css` from the discovered
/// `SuperUiSubresources` (absent until Phase 1 runs).
pub fn detect_hot_reload(
    mut html_events: MessageReader<AssetEvent<HtmlSource>>,
    mut js_events: MessageReader<AssetEvent<JsSource>>,
    mut css_events: MessageReader<AssetEvent<StyleSheet>>,
    root: Query<(&SuperUiRoot, Option<&SuperUiSubresources>)>,
    mut flags: ResMut<HotReloadFlags>,
) {
    let Ok((root, sub)) = root.single() else {
        return;
    };
    for e in html_events.read() {
        if let AssetEvent::Modified { id } = e {
            if *id == root.html.id() {
                flags.html = true;
            }
        }
    }
    for e in js_events.read() {
        if let AssetEvent::Modified { id } = e {
            if sub.map(|s| *id == s.js.id()).unwrap_or(false) {
                flags.js = true;
            }
        }
    }
    for e in css_events.read() {
        if let AssetEvent::Modified { id } = e {
            if sub.and_then(|s| s.css.as_ref()).map(|h| *id == h.id()).unwrap_or(false) {
                flags.css = true;
            }
        }
    }
}

/// Consume `HotReloadFlags`. HTML change → full remount (state lost): tear down the
/// runtime + `SuperUiSubresources` so `mount_when_ready` re-reads the manifest and
/// re-discovers subresources. JS/CSS change → mutate the live runtime in place.
///
/// Runs inside the chained bridge set gated by `runtime_exists`; removing the
/// runtime here makes the remaining chained systems skip (their inherited
/// `runtime_exists` condition re-evaluates to false), so the teardown is safe.
pub fn apply_hot_reload(world: &mut World) {
    let (html_changed, js_changed, _css_changed) = {
        let mut flags = world.resource_mut::<HotReloadFlags>();
        let v = (flags.html, flags.js, flags.css);
        flags.html = false;
        flags.js = false;
        flags.css = false;
        v
    };
    if !(html_changed || js_changed || _css_changed) {
        return;
    }

    // Root entity + discovered subresources (nothing to do before Phase 1 ran).
    let (entity, js_handle) = {
        let mut q = world.query::<(Entity, &SuperUiSubresources)>();
        match q.iter(world).next() {
            Some((e, sub)) => (e, sub.js.clone()),
            None => return,
        }
    };

    if html_changed {
        // Full remount: despawn the reconciled subtree, drop the runtime + marker.
        if let Some(rt) = world.remove_non_send_resource::<UiRuntime>() {
            for e in rt.bound_non_root_entities() {
                if let Ok(ec) = world.get_entity_mut(e) {
                    ec.despawn();
                }
            }
        }
        world.entity_mut(entity).remove::<SuperUiSubresources>();
        return; // mount_when_ready rebuilds next frame
    }

    // JS/CSS change: keep state, re-exec / restyle against the current DOM.
    let Some(mut rt) = world.remove_non_send_resource::<UiRuntime>() else {
        return;
    };
    if js_changed {
        if let Some(js) = world
            .resource::<Assets<JsSource>>()
            .get(&js_handle)
            .map(|j| j.0.clone())
        {
            rt.run_script(&js);
        }
    }
    rt.dirty = true; // CSS-only change still needs a reconcile pass.
    world.insert_non_send_resource(rt);
}
