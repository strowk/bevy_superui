//! `window.bevy` round trips: JS `bevy.send` -> registered Bevy Event, and a
//! game-triggered Event -> JS `bevy.on` callback.
mod support;
use support::*;

use std::cell::RefCell;
use std::rc::Rc;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use superui_bridge::{
    drain_bevy_outbox_system, emit_bevy_inbox_system, SuperUiApp, UiRuntime,
};

#[derive(Event, Serialize, Deserialize, Clone, Debug, PartialEq)]
struct SpawnEnemy {
    x: i64,
    y: i64,
}

#[derive(Event, Serialize, Deserialize, Clone, Debug)]
struct ScoreChanged {
    value: i64,
}

#[derive(Resource, Default)]
struct Received(Vec<SpawnEnemy>);

#[test]
fn js_send_triggers_registered_event() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document("<div></div>")));
    let mut app = test_app();
    mount(&mut app, dom.clone());
    app.init_resource::<Received>();
    app.add_superui_command::<SpawnEnemy>("SpawnEnemy");
    app.add_observer(|ev: On<SpawnEnemy>, mut r: ResMut<Received>| {
        r.0.push(ev.event().clone());
    });
    app.add_systems(Update, drain_bevy_outbox_system);

    app.world_mut()
        .non_send_resource_mut::<UiRuntime>()
        .run_script("bevy.send('SpawnEnemy', { x: 10, y: 4 });");
    app.update();

    let received = app.world().resource::<Received>();
    assert_eq!(received.0, vec![SpawnEnemy { x: 10, y: 4 }]);
}

#[test]
fn game_event_reaches_js_on_callback() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<div id='score'></div>",
    )));
    let mut app = test_app();
    mount(&mut app, dom.clone());
    app.add_superui_event::<ScoreChanged>("ScoreChanged");
    app.add_systems(Update, emit_bevy_inbox_system);

    app.world_mut().non_send_resource_mut::<UiRuntime>().run_script(
        "bevy.on('ScoreChanged', function(e){ \
           document.getElementById('score').textContent = String(e.value); });",
    );
    app.update();

    // The game triggers ScoreChanged; the observer forwards it to JS.
    app.world_mut().trigger(ScoreChanged { value: 42 });
    app.update(); // emit -> JS callback mutates DOM
    app.update(); // reconcile if the harness wired it (not needed for DOM read)

    let score_node = dom.borrow().get_element_by_id("score").unwrap();
    assert_eq!(dom.borrow().text_content(score_node), "42");
}
