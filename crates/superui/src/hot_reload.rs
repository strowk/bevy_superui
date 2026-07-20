//! Hot reload via Bevy's asset system (design §6): `AssetEvent::Modified` ->
//! re-parse HTML / re-exec JS / re-cascade CSS -> reconcile. Native `file_watcher`
//! fires these automatically; on wasm the watcher is inactive (no-op), same seam.
//!
//! Implementation: split into a normal `detect_hot_reload` system that reads
//! `MessageReader<AssetEvent<T>>` for each asset type (the only working form in
//! Bevy 0.17.3 — confirmed by `bevy_flair_style/src/systems.rs` which uses the
//! exact same pattern), and an exclusive `apply_hot_reload` system that consumes
//! the flags and performs the actual rebuild / re-exec.

use std::cell::RefCell;
use std::rc::Rc;

use bevy::ecs::message::MessageReader;
use bevy::prelude::*;
use superui_bridge::UiRuntime;
use superui_css::style::StyleSheet;

use crate::assets::{HtmlSource, JsSource};
use crate::mount::SuperUiRoot;

/// Small resource recording which asset types changed this frame.
#[derive(Resource, Default)]
pub struct HotReloadFlags {
    pub html: bool,
    pub js: bool,
    pub css: bool,
}

/// Normal system: compare `AssetEvent::Modified` ids against the mounted root's
/// handles and write flags. Runs every frame regardless of runtime state (cheap).
pub fn detect_hot_reload(
    mut html_events: MessageReader<AssetEvent<HtmlSource>>,
    mut js_events: MessageReader<AssetEvent<JsSource>>,
    mut css_events: MessageReader<AssetEvent<StyleSheet>>,
    root: Query<&SuperUiRoot>,
    mut flags: ResMut<HotReloadFlags>,
) {
    let Ok(root) = root.single() else {
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
            if *id == root.js.id() {
                flags.js = true;
            }
        }
    }
    for e in css_events.read() {
        if let AssetEvent::Modified { id } = e {
            if *id == root.css.id() {
                flags.css = true;
            }
        }
    }
}

/// Exclusive system: consume `HotReloadFlags` and perform rebuild/re-exec when
/// the runtime is live. Runs inside the chained bridge set (after `detect_hot_reload`,
/// before `reconcile_system`), gated by `runtime_exists`.
///
/// - HTML changed → rebuild DOM from fresh HTML, then re-run JS against new DOM.
/// - JS-only changed → full JS re-execution against current DOM.
/// - Any change → `dirty = true` so reconcile runs.
pub fn apply_hot_reload(world: &mut World) {
    // Read and clear flags atomically.
    let (html_changed, js_changed, css_changed) = {
        let mut flags = world.resource_mut::<HotReloadFlags>();
        let h = flags.html;
        let j = flags.js;
        let c = flags.css;
        flags.html = false;
        flags.js = false;
        flags.css = false;
        (h, j, c)
    };

    if !(html_changed || js_changed || css_changed) {
        return;
    }

    // Pull the root handles before we touch the runtime.
    let root = {
        let mut q = world.query::<&SuperUiRoot>();
        match q.iter(world).next() {
            Some(r) => SuperUiRoot {
                html: r.html.clone(),
                css: r.css.clone(),
                js: r.js.clone(),
            },
            None => return,
        }
    };

    let Some(mut rt) = world.remove_non_send_resource::<UiRuntime>() else {
        return;
    };

    if html_changed {
        if let Some(src) = world
            .resource::<Assets<HtmlSource>>()
            .get(&root.html)
            .map(|h| h.0.clone())
        {
            // FIX 3: Despawn the old child subtree before rebuilding so
            // stale entities don't leak (the new runtime's stale-sweep never
            // sees them because its node_to_entity map is empty).
            let bound_non_root: Vec<Entity> = rt.bound_non_root_entities();
            for entity in bound_non_root {
                if let Ok(ec) = world.get_entity_mut(entity) {
                    ec.despawn();
                }
            }

            // Rebuild the whole runtime around the fresh DOM.
            let dom = Rc::new(RefCell::new(superui_html::parse_document(&src)));
            let entity = rt.root;
            let stylesheet = rt.stylesheet.clone();
            rt = UiRuntime::new(dom, entity, stylesheet, false);
        }
        // After an HTML rebuild we must also re-run JS (fresh DOM).
        if let Some(js) = world
            .resource::<Assets<JsSource>>()
            .get(&root.js)
            .map(|j| j.0.clone())
        {
            rt.run_script(&js);
        }
    } else if js_changed {
        // Full JS re-execution against the current DOM (design §6).
        if let Some(js) = world
            .resource::<Assets<JsSource>>()
            .get(&root.js)
            .map(|j| j.0.clone())
        {
            rt.run_script(&js);
        }
    }

    // CSS-only change still needs a reconcile pass so flair re-applies styles.
    rt.dirty = true;
    world.insert_non_send_resource(rt);
}
