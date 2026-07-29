//! Regression test for the `css-import-relative-resolution` fork patch
//! (docs/fork-patches.md#css-import-relative-resolution).
//!
//! A stylesheet at `dir/main.css` that says `@import "sub/child.css";` must load
//! `dir/sub/child.css` — the import resolved relative to the importing sheet's
//! *directory*, per the CSS spec — NOT the asset-root-relative `sub/child.css`
//! (which doesn't exist here). If the child sheet's rule applies to a spawned
//! entity, the subdirectory-relative import resolved correctly.

mod support;
use support::*;

use bevy::color::palettes::css;
use bevy::prelude::*;

use superui_css::html_type_name;
use superui_css::prelude::*;

/// Look up an entity by its `Name` and assert its computed BackgroundColor.
macro_rules! assert_bg {
    ($app:expr, $name:literal, $expected:expr) => {{
        let world = $app.world_mut();
        let mut q = world.query::<(&Name, &BackgroundColor)>();
        let found = q
            .iter(world)
            .find(|(n, _)| n.as_str() == $name)
            .map(|(_, bg)| bg.0);
        let color = found
            .unwrap_or_else(|| panic!("no entity named '{}' with BackgroundColor", $name));
        assert_eq!(
            color.to_srgba().to_u8_array(),
            $expected.to_u8_array(),
            "'{}' background mismatch",
            $name
        );
    }};
}

// The importing sheet lives in `dir/`; the import target is a sibling
// subdirectory `dir/sub/child.css`. The import string is directory-relative.
const MAIN_CSS: &str = r#"
@import "sub/child.css";
li { background-color: white; }
"#;

// The imported sheet's rule — this only lands if `dir/sub/child.css` resolved.
const CHILD_CSS: &str = r#"
.from-child { background-color: green; }
"#;

#[test]
fn import_resolves_relative_to_importing_sheet() {
    put_css("dir/main.css", MAIN_CSS);
    put_css("dir/sub/child.css", CHILD_CSS);

    let mut app = test_app();
    let handle = {
        let server = app.world().resource::<AssetServer>().clone();
        // `ReturnError`: if the subdir-relative import failed to resolve, the
        // asset load itself would fail (no root-relative `sub/child.css`
        // exists), so `load_until_ready` would panic on `LoadState::Failed`.
        server.load_style_sheet_with(
            "dir/main.css",
            superui_css::parser::CssStyleLoaderErrorMode::ReturnError,
        )
    };

    let root = app
        .world_mut()
        .spawn((
            Node::default(),
            html_type_name("ul"),
            Styled::new(handle.clone()),
        ))
        .id();

    // Entity carrying the class that ONLY the imported child sheet styles.
    let child_styled = app
        .world_mut()
        .spawn((
            Node::default(),
            html_type_name("li"),
            ClassList::new("from-child"),
            Name::new("child-styled"),
        ))
        .id();

    app.world_mut().entity_mut(root).add_children(&[child_styled]);

    load_until_ready(&mut app, &handle);

    // `.from-child` (0,1,0) from the imported sheet beats the plain `li` (0,0,1)
    // in the importing sheet → green. Proves the subdirectory-relative
    // `@import "sub/child.css"` resolved to `dir/sub/child.css`.
    assert_bg!(app, "child-styled", css::GREEN);
}
