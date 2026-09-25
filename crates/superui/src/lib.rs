//! `superui` — the umbrella plugin. Bundles the CSS engine + bridge, registers
//! the `.html`/`.js` asset loaders, mounts authored UI, and hot-reloads it.

mod assets;
mod hot_reload;
mod mount;
#[cfg(all(not(target_arch = "wasm32"), feature = "utilities"))]
mod utilities;

pub use assets::{HtmlLoader, HtmlSource, JsLoader, JsSource};
#[cfg(any(not(target_arch = "wasm32"), feature = "transpiler"))]
pub use assets::TsxLoader;
pub use mount::{SuperUiPlugin, SuperUiRoot};

// Re-exported so callers don't need a direct `superui_bridge` dependency (which
// would also need its own engine feature picked to match `superui`'s).
pub use superui_bridge::{
    click_effect, drain_dom_events_system, emit_bevy_inbox_system, reconcile_system, DomNode,
    PendingDomEvent, PendingDomEvents, UiRuntime,
};

/// The HTML-shaped surface authors/games reach for.
pub mod prelude {
    pub use crate::{
        click_effect, drain_dom_events_system, emit_bevy_inbox_system, reconcile_system, DomNode,
        HtmlSource, JsSource, PendingDomEvent, PendingDomEvents, SuperUiPlugin, SuperUiRoot,
        UiRuntime,
    };
    pub use superui_bridge::{PickingPolicy, SuperUiApp};
    pub use superui_css::prelude::*;
}
