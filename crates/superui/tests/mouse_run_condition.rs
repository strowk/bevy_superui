//! `mouse_events_system` must be gated by `mouse_listeners_active` in the real
//! plugin wiring (not just in `superui_bridge`'s own unit tests): a mounted UI
//! with no mouse listener anywhere must never seed `PointerState`, and adding
//! one listener at runtime must flip that on the very next frame.
mod support;
use support::*;

use superui_bridge::{PointerState, UiRuntime};

#[test]
fn run_condition_toggles_with_listeners() {
    put("mrc.css", b"div { }");
    put("mrc.js", b""); // no listeners registered at mount

    let mut app = app();
    let _root = spawn_root(&mut app, "<div id='a'></div>", "mrc.css", "mrc.js");
    tick(&mut app, 32);

    // No mouse listener anywhere: the run condition must keep mouse_events_system
    // from ever running, so PointerState stays at its unseeded default no matter
    // how many frames pass.
    tick(&mut app, 8);
    assert!(
        !app.world().resource::<PointerState>().seeded,
        "with no mouse listener registered, mouse_events_system must never run"
    );

    // Register a mouse listener at runtime.
    app.world_mut().non_send_mut::<UiRuntime>().run_script(
        "document.getElementById('a').addEventListener('mousemove', function(){});",
    );
    app.update();

    assert!(
        app.world().resource::<PointerState>().seeded,
        "once a mouse listener exists, the run condition must let \
         mouse_events_system run (and seed) on the very next frame"
    );
}
