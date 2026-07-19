//! `superui_bridge` — the single coupling point between the web world (arena DOM
//! + Boa JS + flair CSS) and the ECS world. It owns the per-frame reconciler
//! (DOM -> `bevy_ui` entities), the input -> DOM-event seam, and the `window.bevy`
//! bridge. Only this crate and `superui` (and `superui_css`) depend on Bevy.

mod bevy_bridge;
mod events;
mod reconcile;
mod runtime;

pub use bevy_bridge::{
    drain_bevy_outbox_system, emit_bevy_inbox_system, install_bevy_bridge, BevyBridgeRegistry,
    SuperUiApp,
};
pub use events::{
    click_effect, drain_dom_events_system, keyboard_events_system, on_pointer_click,
    PendingDomEvent, PendingDomEvents,
};
pub use reconcile::reconcile_system;
pub use runtime::{DomNode, UiRuntime};
