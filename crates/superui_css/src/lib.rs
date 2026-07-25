//! `superui_css` — the bevy_superui CSS layer: an in-tree fork of `bevy_flair`
//! 0.7 (Bevy 0.18) re-exported behind an HTML-shaped surface.
//!
//! The fork already matches real HTML element/attribute/class/id and
//! `:hover`/`:focus`/`:checked` selectors; this crate bundles it into one
//! plugin and adds an HTML tag-name interner (see [`html_type_name`]). Only
//! Bevy-facing crates depend on this one (design §4).

pub use superui_flair_core as core;
pub use superui_flair_css_parser as parser;
pub use superui_flair_style as style;

mod tag;
pub use tag::{html_type_name, intern_tag};

bevy_app::plugin_group! {
    /// The one plugin Plan 5's `SuperUiPlugin` adds to get the full CSS engine:
    /// property registry, the style/selector systems, default animations, and
    /// the `.css` asset loader. Mirrors upstream `bevy_flair::FlairPlugin`.
    #[derive(Clone, Debug)]
    pub struct SuperUiCssPlugin {
        superui_flair_core:::PropertyRegistryPlugin,
        superui_flair_core:::ImplComponentPropertiesPlugin,
        superui_flair_style:::FlairStylePlugin,
        superui_flair_style:::FlairDefaultStyleAnimationsPlugin,
        superui_flair_css_parser:::FlairCssParserPlugin,
    }
}

/// The HTML-shaped surface Plan 5's reconciler and authored code reach for.
pub mod prelude {
    pub use crate::SuperUiCssPlugin;
    pub use crate::{html_type_name, intern_tag};
    pub use superui_flair_css_parser::InlineStyle;
    pub use superui_flair_style::components::{
        AttributeList, ClassList, NodeStyleData, NodeStyleSheet, TypeName,
    };
    pub use superui_flair_style::{NodePseudoState, StyleSheet};
}

#[cfg(test)]
mod tests {
    // Compile-smoke: every item the prelude promises must resolve and be
    // nameable. If flair re-exports one of these through a different module,
    // the compiler names the right path — follow it and fix the `use` above.
    #[allow(unused_imports)]
    use crate::prelude::*;

    #[test]
    fn prelude_items_resolve() {
        // Name each type/plugin so the test fails to compile if a path is wrong.
        fn _assert_nameable() {
            let _: Option<StyleSheet> = None;
            let _: Option<NodeStyleSheet> = None;
            let _: Option<ClassList> = None;
            let _: Option<AttributeList> = None;
            let _: Option<TypeName> = None;
            let _: Option<NodePseudoState> = None;
            let _: Option<InlineStyle> = None;
            let _plugin = SuperUiCssPlugin;
        }
        // Nothing to assert at runtime; resolution is the test.
        let _ = _assert_nameable;
    }
}
