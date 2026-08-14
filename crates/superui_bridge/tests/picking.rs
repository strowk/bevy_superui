//! Picking seam: which reconciled nodes are allowed to hide the layers below.
//!
//! Bevy's UI picking backend treats a node with no `Pickable` as blocking, and it
//! runs at camera order +0.5 — ahead of the sprite and mesh backends. So what the
//! reconciler stamps here decides whether a mounted UI leaves the host app's world
//! pickable, which is the whole question for anything drawn over live gameplay.
mod support;
use support::*;

use std::cell::RefCell;
use std::rc::Rc;

use bevy::picking::Pickable;
use bevy::prelude::*;
use superui_bridge::{PickingPolicy, UiRuntime};
use superui_dom::NodeId;

/// The `Pickable` on the entity a DOM id resolves to.
fn pickable_of(app: &mut App, id: &str) -> Option<Pickable> {
    let node = {
        let rt = app.world().non_send_resource::<UiRuntime>();
        let dom = rt.dom.borrow();
        dom.get_element_by_id(id).expect("element exists")
    };
    entity_pickable(app, node)
}

fn entity_pickable(app: &mut App, node: NodeId) -> Option<Pickable> {
    let entity = app
        .world()
        .non_send_resource::<UiRuntime>()
        .entity_for(node)
        .expect("node is bound to an entity");
    app.world().get::<Pickable>(entity).cloned()
}

const BLOCKS: Pickable = Pickable {
    should_block_lower: true,
    is_hoverable: true,
};
const TRANSPARENT: Pickable = Pickable {
    should_block_lower: false,
    is_hoverable: true,
};

#[test]
fn layout_scaffolding_does_not_block_the_world() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<div id='root'><div id='card'><span id='label'>hp 40</span></div></div>",
    )));
    let mut app = test_app();
    mount(&mut app, dom.clone());
    app.update();

    // Nothing here listens for anything: it is a HUD, and the world behind it has
    // to stay reachable.
    for id in ["root", "card", "label"] {
        assert_eq!(
            pickable_of(&mut app, id),
            Some(TRANSPARENT),
            "#{id} has no listener anywhere above it, so it must not block"
        );
    }

    // Hover is untouched by the pass-through: `Hovered` is what flair reads
    // `:hover` from, so blocking and hovering have to stay separable.
    assert!(
        pickable_of(&mut app, "card").unwrap().is_hoverable,
        "pass-through must not cost the UI its :hover styling"
    );
}

#[test]
fn a_listener_makes_its_whole_subtree_block() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<div id='root'><button id='btn'><span id='cap'>fire</span></button>\
         <div id='chrome'></div></div>",
    )));
    let mut app = test_app();
    mount(&mut app, dom.clone());
    app.update();

    // Before any listener exists the button is just another box.
    assert_eq!(pickable_of(&mut app, "btn"), Some(TRANSPARENT));

    app.world_mut()
        .non_send_resource_mut::<UiRuntime>()
        .run_script("document.getElementById('btn').addEventListener('click', function(){});");
    app.update();

    assert_eq!(
        pickable_of(&mut app, "btn"),
        Some(BLOCKS),
        "an interactive node must not let clicks through to the world behind it"
    );
    // The caption blocks too. A non-blocking child would leave its ancestors in
    // the hit list as well, so the same physical click would be reported twice
    // and dispatched twice into the DOM.
    assert_eq!(
        pickable_of(&mut app, "cap"),
        Some(BLOCKS),
        "descendants of an interactive node must block so one click stays one click"
    );
    // A sibling that is not under the listener is unaffected.
    assert_eq!(pickable_of(&mut app, "chrome"), Some(TRANSPARENT));
}

#[test]
fn removing_the_last_listener_makes_the_node_transparent_again() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<div id='root'><button id='btn'>fire</button></div>",
    )));
    let mut app = test_app();
    mount(&mut app, dom.clone());
    app.world_mut().non_send_resource_mut::<UiRuntime>().run_script(
        "globalThis.h = function(){}; \
         document.getElementById('btn').addEventListener('click', globalThis.h);",
    );
    app.update();
    assert_eq!(pickable_of(&mut app, "btn"), Some(BLOCKS));

    app.world_mut()
        .non_send_resource_mut::<UiRuntime>()
        .run_script("document.getElementById('btn').removeEventListener('click', globalThis.h);");
    app.update();

    assert_eq!(
        pickable_of(&mut app, "btn"),
        Some(TRANSPARENT),
        "picking has to follow the DOM back down, not just up"
    );
}

#[test]
fn text_nodes_never_take_part_in_picking() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<div id='root'>plain text</div>",
    )));
    let mut app = test_app();
    mount(&mut app, dom.clone());
    app.update();

    let text_node = {
        let rt = app.world().non_send_resource::<UiRuntime>();
        let dom = rt.dom.borrow();
        let root = dom.get_element_by_id("root").unwrap();
        dom.children(root)[0]
    };
    assert_eq!(
        entity_pickable(&mut app, text_node),
        Some(Pickable::IGNORE),
        "a label is never a meaningful pick target, and picks resolve to its \
         nearest DomNode ancestor anyway"
    );
}

#[test]
fn the_root_entity_is_transparent_too() {
    // The root carries the full-viewport node in the usual bundle, so it is the
    // first thing that would hide the world.
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<div id='root'></div>",
    )));
    let mut app = test_app();
    let root = mount(&mut app, dom.clone());
    app.update();

    assert_eq!(app.world().get::<Pickable>(root).cloned(), Some(TRANSPARENT));
}

#[test]
fn the_solid_policy_keeps_every_node_blocking() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<div id='root'><div id='card'></div></div>",
    )));
    let mut app = test_app();
    let root = mount(&mut app, dom.clone());
    app.world_mut().entity_mut(root).insert(PickingPolicy::Solid);
    app.update();

    // Absence of `Pickable` is how bevy_ui spells "blocks", so a full-screen menu
    // opting out gets exactly the pre-policy behaviour.
    assert_eq!(pickable_of(&mut app, "card"), None);
    assert_eq!(app.world().get::<Pickable>(root).cloned(), None);
}

#[test]
fn switching_policy_at_runtime_takes_effect() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<div id='root'><div id='card'></div></div>",
    )));
    let mut app = test_app();
    let root = mount(&mut app, dom.clone());
    app.update();
    assert_eq!(pickable_of(&mut app, "card"), Some(TRANSPARENT));

    app.world_mut().entity_mut(root).insert(PickingPolicy::Solid);
    app.world_mut().non_send_resource_mut::<UiRuntime>().dirty = true;
    app.update();

    assert_eq!(
        pickable_of(&mut app, "card"),
        None,
        "a stale Pickable would leave the UI transparent after it asked to be solid"
    );
}
