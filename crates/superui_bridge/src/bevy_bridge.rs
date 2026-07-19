//! The `window.bevy` bridge. Filled in by Task 6.
use bevy::prelude::*;
use superui_js::{BoaEngine, JsEngine};

/// Registry of JS-exposed commands/events. Filled in by Task 6.
#[derive(Resource, Default)]
pub struct BevyBridgeRegistry;

/// Extension trait on `App` for registering the `window.bevy` surface (Task 6).
pub trait SuperUiApp {}
impl SuperUiApp for App {}

/// Install the `window.bevy` global into `engine`. Stub until Task 6 — for now
/// it just aliases `window` to `globalThis` so `window.bevy` won't throw later.
pub fn install_bevy_bridge(engine: &mut BoaEngine) {
    let _ = engine.eval("globalThis.window = globalThis;");
}
