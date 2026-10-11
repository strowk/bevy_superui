//! Hot reload via Bevy's asset system (design §6): `AssetEvent::Modified` ->
//! re-parse HTML / re-exec JS / re-cascade CSS -> reconcile. Native `file_watcher`
//! fires these automatically; on wasm the watcher is inactive (no-op), same seam.
//!
//! Implementation: split into a normal `detect_hot_reload` system that reads
//! `MessageReader<AssetEvent<T>>` for each asset type (the pattern
//! `bevy_flair_style/src/systems.rs` uses), and an exclusive `apply_hot_reload`
//! system that consumes the flags and performs the actual rebuild / re-exec.

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
/// Runs as a standalone exclusive system scheduled AFTER `detect_hot_reload` and
/// BEFORE the chained bridge set (which is gated by `runtime_exists`). An HTML
/// teardown removes the runtime, so the subsequent bridge chain skips that frame;
/// this ordering is critical because `keyboard_events_system` takes
/// `NonSendMut<UiRuntime>` (not `Option`) and would panic if it ran after removal.
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
    let (entity, js_handle, base_css, applied_css) = {
        let mut q = world.query::<(Entity, &SuperUiSubresources)>();
        match q.iter(world).next() {
            Some((e, sub)) => (e, sub.js.clone(), sub.css.clone(), sub.applied.clone()),
            None => return,
        }
    };

    if html_changed {
        // Full remount: despawn the reconciled subtree, drop the runtime + marker.
        if let Some(rt) = world.remove_non_send::<UiRuntime>() {
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
    let Some(mut rt) = world.remove_non_send::<UiRuntime>() else {
        return;
    };
    if js_changed {
        if let Some(js) = world
            .resource::<Assets<JsSource>>()
            .get(&js_handle)
            .map(|j| j.code.clone())
        {
            rt.run_script(&js);
        }
    }

    // Re-apply the module-graph CSS. Because the `<link>` base sheet is kept
    // pristine (never merged into), rebuilding the applied sheet from base +
    // the current `JsSource.css` fixes both halves of the old limitation: a
    // module `.css` edit (reloaded `JsSource` carries new `css`) and a `<link>`
    // edit (flair reloads the base to pristine, so the module CSS must be
    // re-appended). Writing the applied slot via `get_mut` emits
    // `AssetEvent::Modified`, which flair honors to restyle every node using it.
    let module_css = world
        .resource::<Assets<JsSource>>()
        .get(&js_handle)
        .map(|j| j.css.clone())
        .unwrap_or_default();
    match applied_css {
        Some(applied) => {
            let sheet = crate::mount::merged_sheet(world, base_css.as_ref(), &module_css);
            if let Some(mut slot) = world.resource_mut::<Assets<StyleSheet>>().get_mut(&applied) {
                *slot = sheet;
            }
        }
        // No module CSS at mount; if an edit introduced some (a new `.css`
        // import), promote: build the applied sheet now and repoint the runtime
        // + subresources at it. The reconcile below re-inserts `Styled` from the
        // runtime's handle, so the new sheet takes effect.
        None if !module_css.is_empty() => {
            let sheet = crate::mount::merged_sheet(world, base_css.as_ref(), &module_css);
            let applied = world.resource_mut::<Assets<StyleSheet>>().add(sheet);
            rt.stylesheet = applied.clone();
            if let Some(mut sub) = world.get_mut::<SuperUiSubresources>(entity) {
                sub.applied = Some(applied);
            }
        }
        None => {}
    }

    rt.dirty = true; // CSS-only change still needs a reconcile pass.
    world.insert_non_send(rt);
}
