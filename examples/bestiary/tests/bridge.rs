//! Builds a headless app with `AssetPlugin` + the `Bestiary` loader, loads the
//! real asset file from disk, pumps until loaded, and checks the decoded
//! contents. `cargo test` runs this binary with CWD = the crate directory, so
//! the default `AssetPlugin` `file_path` ("assets") resolves to
//! `examples/bestiary/assets` — the test loads "data/bestiary.json" directly.

use bevy::asset::{AssetPlugin, AssetServer, Assets, LoadState};
use bevy::prelude::*;

use bestiary::{Bestiary, BestiaryLoader};

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
