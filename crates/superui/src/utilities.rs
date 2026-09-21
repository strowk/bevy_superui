//! Opt-in **class utilities** regeneration (design §3, §7), gated behind the
//! native-only `utilities` feature.
//!
//! While the AssetServer is watching for changes (dev / HMR), this system
//! regenerates `<ui>/.superui/build/utilities.generated.css` from the app's
//! `.tsx`/`.ts` sources whenever a source changes, and logs each dropped
//! utility class via `warn!`. It writes the file under `assets/` so Bevy's
//! `file_watcher` + flair's `@import` dependency reload propagate the new
//! cascade on their own — we never force-reload the stylesheet here.
//!
//! The regeneration trigger reuses the SAME signal the HMR path keys off of
//! (`AssetEvent::Modified<JsSource>` for the mounted root's script handle — in
//! live-source builds a `.tsx` edit reloads as a `JsSource`), plus one initial
//! pass so the `@import` never dangles under HMR (where `build.rs` skips the
//! offline generation).

use std::path::PathBuf;

use bevy::ecs::message::MessageReader;
use bevy::prelude::*;

use crate::assets::JsSource;
use crate::mount::{SuperUiRoot, SuperUiSubresources};

/// Watch-gated regeneration system. No-op unless the AssetServer is watching for
/// changes (matches the design's "regenerate only while watching" gate and the
/// existing HMR activation gate).
pub(crate) fn regenerate_utilities(
    mut js_events: MessageReader<AssetEvent<JsSource>>,
    root: Query<(&SuperUiRoot, Option<&SuperUiSubresources>)>,
    assets: Res<AssetServer>,
    mut did_initial: Local<bool>,
) {
    // Dev/HMR only: no point scanning source that can never change under us.
    if !assets.watching_for_changes() {
        return;
    }

    let Ok((root, sub)) = root.single() else {
        return;
    };

    // Trigger on the same event the HMR `flags.js` path uses: the mounted root's
    // script handle was modified (a `.tsx` edit reloads through the tsx→JsSource
    // seam). Always run once first so the generated sheet exists.
    let source_changed = js_events.read().any(|e| match e {
        AssetEvent::Modified { id } => sub.map(|s| *id == s.js.id()).unwrap_or(false),
        _ => false,
    });
    if *did_initial && !source_changed {
        return;
    }
    *did_initial = true;

    let Some(ui_dir) = ui_source_dir(root) else {
        warn!(
            "superui/utilities: could not resolve the UI source directory from the entry-HTML \
             asset path; skipping class-utilities regeneration"
        );
        return;
    };

    let ui_dir = ui_dir.to_string_lossy().into_owned();
    for d in superui_css_utilities::write_generated(&ui_dir) {
        warn!("superui/utilities: dropped `{}` — {}", d.class, d.reason);
    }
}

/// Resolve the on-disk UI source directory for the mounted root: the entry
/// HTML's asset path (e.g. `ui/todomvc_supersolid/index.html`) gives the
/// asset-relative UI dir; joining it onto the filesystem asset base yields the
/// directory `write_generated` scans.
fn ui_source_dir(root: &SuperUiRoot) -> Option<PathBuf> {
    let asset_path = root.html.path()?;
    let rel_dir = asset_path.path().parent()?;
    Some(asset_base().join(rel_dir))
}

/// The filesystem directory holding the app's assets, resolved the way
/// `bevy_asset`'s default `FileAssetReader` does for dev builds: honor
/// `BEVY_ASSET_ROOT`, else `CARGO_MANIFEST_DIR` (both set during `cargo run`,
/// which is where the file watcher / HMR live), else the current directory —
/// then join Bevy's default `assets/` file path.
///
/// NOTE: this assumes the default asset source and the default `assets/` file
/// path. Apps that relocate their asset root would need this to consult the
/// configured `AssetSource` instead; that indirection is not publicly exposed by
/// the AssetServer, so it is deliberately out of scope for this dev-only path.
fn asset_base() -> PathBuf {
    let root = std::env::var_os("BEVY_ASSET_ROOT")
        .or_else(|| std::env::var_os("CARGO_MANIFEST_DIR"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    root.join("assets")
}
