//! `superui_bridge` — the single coupling point between the web world (arena
//! DOM + JS — V8 native, the browser's engine on web — + flair CSS) and the
//! ECS world. It owns the per-frame reconciler
//! (DOM -> `bevy_ui` entities), the input -> DOM-event seam, and the `window.bevy`
//! bridge. Only this crate and `superui` (and `superui_css`) depend on Bevy.

mod bevy_bridge;
mod events;
mod reconcile;
mod runtime;
mod scroll;

pub use bevy_bridge::{
    drain_bevy_outbox_system, emit_bevy_inbox_system, BevyBridgeRegistry, SuperUiApp,
};
pub use events::{
    apply_pointer_click, click_effect, dim_placeholder_text_system, drain_dom_events_system,
    editable_input_events_system, keyboard_events_system, on_focus_gained, on_focus_lost,
    on_pointer_click, resolve_dom_node, PendingDomEvent, PendingDomEvents,
};
pub use reconcile::reconcile_system;
pub use runtime::{DomNode, InputValueText, PickingPolicy, PlaceholderText, UiRuntime};
pub use scroll::{clamp_scroll_position_system, wheel_scroll_system};
