//! `superui_bridge` — the single coupling point between the web world (arena
//! DOM + JS — V8 native, the browser's engine on web — + flair CSS) and the
//! ECS world. It owns the per-frame reconciler
//! (DOM -> `bevy_ui` entities), the input -> DOM-event seam, and the `window.bevy`
//! bridge. Only this crate and `superui` (and `superui_css`) depend on Bevy.

mod bevy_bridge;
mod events;
mod mouse;
mod pointer;
mod reconcile;
mod runtime;
mod scroll;
pub mod slider;

pub use bevy_bridge::{
    drain_bevy_outbox_system, emit_bevy_inbox_system, BevyBridgeRegistry, SuperUiApp,
};
pub use events::{
    apply_pointer_click, bevy_pointer_id, click_effect, dim_placeholder_text_system,
    drain_dom_events_system, editable_input_events_system, format_slider_value,
    js_pointer_id, keyboard_events_system, modifiers_now, on_focus_gained, on_focus_lost,
    on_pointer_cancel, on_pointer_click, on_pointer_move, on_pointer_out, on_pointer_over,
    on_pointer_press, on_pointer_release, on_slider_value_change, on_touch_press,
    on_touch_release, pointer_is_primary, pointer_kind, resolve_dom_node, PendingDomEvent,
    PendingDomEvents, PointerCaptures, PrimaryTouch,
};
pub use mouse::{
    mouse_events_system, mouse_listeners_active, sync_mouse_listener_index, MouseListenerIndex,
    PointerState,
};
pub use pointer::sync_hit_rects_system;
pub use reconcile::reconcile_system;
pub use runtime::{DomNode, InputValueText, PickingPolicy, PlaceholderText, UiRuntime};
pub use scroll::{clamp_scroll_position_system, wheel_scroll_system};
pub use slider::position_slider_parts;
pub use superui_js::{element_from_point, EventInit, HitRect, Modifiers, MousePayload};
