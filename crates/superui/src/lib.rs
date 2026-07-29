//! `superui` — the umbrella plugin. Bundles the CSS engine + bridge, registers
//! the `.html`/`.js` asset loaders, mounts authored UI, and hot-reloads it.

mod assets;
mod hot_reload;
mod mount;
#[cfg(all(not(target_arch = "wasm32"), feature = "utilities"))]
mod utilities;

pub use assets::{HtmlLoader, HtmlSource, JsLoader, JsSource};
#[cfg(not(target_arch = "wasm32"))]
pub use assets::TsxLoader;
pub use mount::{SuperUiPlugin, SuperUiRoot};

/// The HTML-shaped surface authors/games reach for.
pub mod prelude {
    pub use crate::{HtmlSource, JsSource, SuperUiPlugin, SuperUiRoot};
    pub use superui_bridge::SuperUiApp;
    pub use superui_css::prelude::*;
}
