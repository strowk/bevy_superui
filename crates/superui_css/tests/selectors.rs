//! End-to-end proof that the vendored fork matches HTML-shaped selectors:
//! element, attribute, class, id, descendant, `:checked`, `:hover`.

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

const CSS: &str = r#"
li                   { background-color: white; }
.todo-list li        { background-color: purple; }
.todo-list .completed { background-color: green; }
#special             { background-color: blue; }
input[type="checkbox"] { background-color: orange; }
input:checked    { background-color: red; }
button:hover     { background-color: teal; }
.#this-is-not-valid { background-color: white; }
"#;

#[test]
fn matches_html_selectors_end_to_end() {
    put_css("selectors.css", CSS);

    let mut app = test_app();
    let handle = {
        let server = app.world().resource::<AssetServer>().clone();
        server.load_style_sheet("selectors.css")
    };

    // Spawn a small DOM-shaped tree. `html_type_name(tag)` is the element-selector
    // identity the Plan-5 bridge will insert; `Name` is the id; `ClassList` the
    // classes; `AttributeList` the attributes; `Checked`/`Interaction` drive
    // pseudo-state exactly as flair syncs it.
    let root = app
        .world_mut()
        .spawn((
            Node::default(),
            html_type_name("ul"),
            ClassList::new("todo-list"),
            NodeStyleSheet::new(handle.clone()),
        ))
        .id();

    let plain_li = app.world_mut().spawn((Node::default(), html_type_name("li"), Name::new("plain-li"))).id();
    let done_li = app.world_mut().spawn((Node::default(), html_type_name("li"), ClassList::new("completed"), Name::new("done-li"))).id();
    let special_li = app.world_mut().spawn((Node::default(), html_type_name("li"), Name::new("special"))).id();
    let checkbox = app
        .world_mut()
        .spawn((
            Node::default(),
            html_type_name("input"),
            AttributeList::from_iter([("type", "checkbox")]),
            bevy::ui::Checked,
            Name::new("checkbox"),
        ))
        .id();
    let btn = app
        .world_mut()
        .spawn((Node::default(), html_type_name("button"), Interaction::Hovered, Name::new("btn")))
        .id();

    app.world_mut().entity_mut(root).add_children(&[plain_li, done_li, special_li, checkbox, btn]);

    load_until_ready(&mut app, &handle);

    // Element selector `li` matches, but the descendant `.todo-list li`
    // (specificity 0,1,1 > 0,0,1) wins → purple:
    assert_bg!(app, "plain-li", css::PURPLE);
    // Class match: `.todo-list .completed` (0,2,0) beats `.todo-list li` (0,1,1)
    // → green. Proves class selectors and specificity ordering:
    assert_bg!(app, "done-li", css::GREEN);
    // Id `#special` (specificity 1,0,0) — highest specificity → blue:
    assert_bg!(app, "special", css::BLUE);
    // Attribute `input[type="checkbox"]` + `:checked` — checked wins (red):
    assert_bg!(app, "checkbox", css::RED);
    // `:hover`:
    assert_bg!(app, "btn", css::TEAL);
}
