//! Builds a headless app with `AssetPlugin` + the `Bestiary` loader, loads the
//! real asset file from disk, pumps until loaded, and checks the decoded
//! contents. `cargo test` runs this binary with CWD = the crate directory, so
//! the default `AssetPlugin` `file_path` ("assets") resolves to
//! `examples/bestiary/assets` — the test loads "data/bestiary.json" directly.

use bevy::asset::{AssetPlugin, AssetServer, Assets, LoadState};
use bevy::prelude::*;

use bestiary::{Bestiary, BestiaryHandle, BestiaryLoader, BridgeState, UiReady, on_ui_ready, push_bestiary};

fn test_app() -> App {
    let mut app = App::new();
    app.add_plugins((
        bevy::app::TaskPoolPlugin::default(),
        AssetPlugin { file_path: "assets".into(), ..default() },
    ));
    app.init_asset::<Bestiary>().register_asset_loader(BestiaryLoader);
    app.finish();
    app
}

fn pump_until_loaded(app: &mut App, handle: &Handle<Bestiary>) {
    for _ in 0..64 {
        app.update();
        let server = app.world().resource::<AssetServer>();
        if matches!(server.load_state(handle.id()), LoadState::Loaded) {
            return;
        }
    }
    panic!("bestiary asset never reached LoadState::Loaded");
}

#[test]
fn loads_bestiary_from_asset_server() {
    let mut app = test_app();
    let handle: Handle<Bestiary> =
        app.world().resource::<AssetServer>().load("data/bestiary.json");
    pump_until_loaded(&mut app, &handle);
    let assets = app.world().resource::<Assets<Bestiary>>();
    let b = assets.get(&handle).expect("asset loaded");
    assert_eq!(b.creatures.len(), 4);
    assert_eq!(b.creatures[0].name, "Ember Drake");
    assert_eq!(b.creatures[0].element, "fire");
    assert_eq!(b.creatures[0].hp, 120);
    assert_eq!(b.creatures[0].attack, 34);
}

/// A test observer counts how many `Bestiary` events were triggered.
#[derive(Resource, Default)]
struct Seen(u32);

/// Builds the Task-1 [`test_app`], then wires the handshake pieces directly
/// (not via `register_bridge`) so these tests stay independent of
/// `SuperUiPlugin` and its bridge registry: a loaded `BestiaryHandle`,
/// `BridgeState`, `push_bestiary` on `Update`, the `on_ui_ready` observer, and
/// a `Seen` counter observer on `Bestiary`.
fn bridge_app() -> (App, Handle<Bestiary>) {
    let mut app = test_app();
    let handle: Handle<Bestiary> =
        app.world().resource::<AssetServer>().load("data/bestiary.json");
    app.insert_resource(BestiaryHandle(handle.clone()));
    app.init_resource::<BridgeState>();
    app.init_resource::<Seen>();
    app.add_systems(Update, push_bestiary);
    app.add_observer(on_ui_ready);
    app.add_observer(|_: On<Bestiary>, mut s: ResMut<Seen>| s.0 += 1);
    (app, handle)
}

// ui_ready BEFORE the asset is available: push latches and fires once the
// asset loads, exactly once across repeated updates.
#[test]
fn pushes_once_when_ready_then_loaded() {
    let (mut app, handle) = bridge_app();
    app.world_mut().resource_mut::<BridgeState>().ui_ready = true;
    pump_until_loaded(&mut app, &handle);
    for _ in 0..3 {
        app.update();
    }
    assert_eq!(app.world().resource::<Seen>().0, 1);
}

// Asset present but UI not ready: nothing is pushed and nothing panics.
#[test]
fn does_not_push_before_ui_ready() {
    let (mut app, handle) = bridge_app();
    pump_until_loaded(&mut app, &handle);
    for _ in 0..3 {
        app.update();
    }
    assert_eq!(app.world().resource::<Seen>().0, 0);
    assert!(!app.world().resource::<BridgeState>().sent);
}

// on_ui_ready flips the flag when UiReady is triggered (the command path).
#[test]
fn ui_ready_observer_sets_flag() {
    let (mut app, _handle) = bridge_app();
    app.world_mut().trigger(UiReady);
    app.update();
    assert!(app.world().resource::<BridgeState>().ui_ready);
}
