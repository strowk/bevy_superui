//! Reconciler integration: DOM tree -> ECS entity tree (structure + text).
mod support;
use support::*;

use std::cell::RefCell;
use std::rc::Rc;

use bevy::prelude::*;
use superui_bridge::{DomNode, UiRuntime};
use superui_css::prelude::{AttributeList, ClassList, TypeName};
use superui_dom::NodeKind;

#[test]
fn body_gets_type_name_and_identity_synced() {
    // html5ever normalises the document: id/class on <body> are preserved.
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<body id='page' class='app'><div></div></body>",
    )));
    let mut app = test_app();
    let root = mount(&mut app, dom.clone());
    app.update();

    // Root entity (= body) must have TypeName("body").
    let tn = app.world().get::<TypeName>(root).expect("root has TypeName");
    assert_eq!(tn.0, "body", "root entity must have TypeName 'body'");

    // ClassList on root must contain "app".
    let cl = app
        .world()
        .get::<ClassList>(root)
        .expect("root has ClassList");
    assert!(cl.contains("app"), "root ClassList must contain 'app'");

    // Name on root must equal "page" (id selector uses Name in flair).
    let name = app.world().get::<Name>(root).expect("root has Name");
    assert_eq!(name.as_str(), "page", "root Name must equal 'page'");
}

#[test]
fn reconciles_dom_tree_into_entities() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<ul class='list'><li>one</li><li>two</li></ul>",
    )));
    let mut app = test_app();
    let root = mount(&mut app, dom.clone());

    app.update(); // reconcile once

    // Root (body) has one child: the <ul>.
    assert_eq!(child_count(&mut app, root), 1);
    let ul = app.world_mut().get::<Children>(root).unwrap()[0];

    // The <ul> entity has a TypeName "ul" and a DomNode mapping back to an element.
    let tn = app.world().get::<TypeName>(ul).expect("ul has TypeName");
    assert_eq!(tn.0, "ul");
    let dn = app.world().get::<DomNode>(ul).expect("ul has DomNode");
    assert!(matches!(
        dom.borrow().get(dn.0).map(|n| &n.kind),
        Some(NodeKind::Element(_))
    ));

    // The <ul> has two <li> children.
    assert_eq!(child_count(&mut app, ul), 2);
    let lis = app.world_mut().get::<Children>(ul).unwrap().to_vec();
    for (li, expected) in lis.iter().zip(["one", "two"]) {
        // Each <li> contains one text-node child entity carrying the text.
        let li_children = app.world().get::<Children>(*li).unwrap();
        let text_entity = li_children[0];
        let text = app.world().get::<Text>(text_entity).expect("text node");
        assert_eq!(text.0, expected);
    }
}

#[test]
fn element_nodes_carry_hovered_for_hover_pseudo_class() {
    // Regression guard: element entities must be spawned with `Hovered`, or every
    // `:hover` CSS rule silently does nothing — `bevy_picking::update_is_hovered`
    // only *updates* entities that already have the component, it never inserts it.
    // Text-node entities are not styled targets and must NOT get it.
    use bevy::picking::hover::Hovered;

    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<ul class='list'><li>one</li></ul>",
    )));
    let mut app = test_app();
    let root = mount(&mut app, dom.clone());
    app.update();

    let ul = app.world_mut().get::<Children>(root).unwrap()[0];
    assert!(
        app.world().get::<Hovered>(ul).is_some(),
        "<ul> element entity must carry Hovered so `:hover` can fire"
    );

    let li = app.world_mut().get::<Children>(ul).unwrap()[0];
    assert!(
        app.world().get::<Hovered>(li).is_some(),
        "<li> element entity must carry Hovered so `:hover` can fire"
    );

    // The <li>'s text-node child entity is not a styled element — no Hovered.
    let text_entity = app.world_mut().get::<Children>(li).unwrap()[0];
    assert!(
        app.world().get::<Text>(text_entity).is_some(),
        "expected the text-node child entity",
    );
    assert!(
        app.world().get::<Hovered>(text_entity).is_none(),
        "text-node entities must not carry Hovered",
    );
}

#[test]
fn syncs_identity_and_updates_in_place() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<div id='root'><input type='checkbox' class='a b'></div>",
    )));
    let mut app = test_app();
    let root = mount(&mut app, dom.clone());
    app.update();

    // Find the <input> entity via its DomNode.
    let input_node = dom.borrow().query_selector(dom.borrow().document(), "input").unwrap();
    let input_ent = {
        let mut q = app.world_mut().query::<(Entity, &DomNode)>();
        q.iter(app.world())
            .find(|(_, d)| d.0 == input_node)
            .map(|(e, _)| e)
            .unwrap()
    };

    // Identity synced: ClassList has a+b; AttributeList has type=checkbox.
    assert!(app.world().get::<ClassList>(input_ent).unwrap().contains("a"));
    assert_eq!(
        app.world().get::<AttributeList>(input_ent).unwrap().get_attribute("type"),
        Some("checkbox")
    );
    // Not checked yet -> no Checked marker.
    assert!(app.world().get::<bevy::ui::Checked>(input_ent).is_none());

    // Mutate the DOM (as JS would) and re-reconcile: SAME entity, updated state.
    dom.borrow_mut().set_checked(input_node, true);
    dom.borrow_mut().class_add(input_node, "done");
    app.world_mut().non_send_resource_mut::<superui_bridge::UiRuntime>().dirty = true;
    app.update();

    let input_ent2 = {
        let mut q = app.world_mut().query::<(Entity, &DomNode)>();
        q.iter(app.world())
            .find(|(_, d)| d.0 == input_node)
            .map(|(e, _)| e)
            .unwrap()
    };
    assert_eq!(input_ent, input_ent2, "entity is stable across reconciles");
    assert!(app.world().get::<bevy::ui::Checked>(input_ent).is_some());
    assert!(app.world().get::<ClassList>(input_ent).unwrap().contains("done"));
}

#[test]
fn input_renders_placeholder_then_value_as_text() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<input id='new' type='text' placeholder='What needs doing?'>",
    )));
    let mut app = test_app();
    let _root = mount(&mut app, dom.clone());
    app.update();

    let input_node = dom
        .borrow()
        .query_selector(dom.borrow().document(), "input")
        .unwrap();
    let input_ent = {
        let mut q = app.world_mut().query::<(Entity, &DomNode)>();
        q.iter(app.world())
            .find(|(_, d)| d.0 == input_node)
            .map(|(e, _)| e)
            .unwrap()
    };

    // The input element is a CONTAINER (so it can render a border): its text lives
    // in a managed `InputValueText` child, kept non-pickable so clicks focus the
    // input. Read the child's text.
    let text_of_input = |app: &mut App, input_ent: Entity| -> String {
        let kids = app.world().get::<Children>(input_ent).unwrap().to_vec();
        for k in kids {
            if app.world().get::<superui_bridge::InputValueText>(k).is_some() {
                return app.world().get::<Text>(k).unwrap().0.clone();
            }
        }
        panic!("input has no managed InputValueText child");
    };
    // Not focused (no autofocus) -> shows the placeholder.
    assert_eq!(text_of_input(&mut app, input_ent), "What needs doing?");

    // Type into the DOM value (as the keyboard seam would) and re-reconcile.
    dom.borrow_mut().set_value(input_node, "Buy milk");
    app.world_mut()
        .non_send_resource_mut::<UiRuntime>()
        .dirty = true;
    app.update();
    assert_eq!(text_of_input(&mut app, input_ent), "Buy milk");

    // Fix invariants (why the live app can focus + type into a bordered input):
    // the input element carries `DomNode` and is NOT itself a text node (so
    // bevy_ui draws its border); the managed child is non-pickable so a click
    // falls through to the input for focus.
    assert!(app.world().get::<DomNode>(input_ent).is_some());
    assert!(
        app.world().get::<Text>(input_ent).is_none(),
        "input element must be a container (no Text on it) so its border renders"
    );
    let child = app.world().get::<Children>(input_ent).unwrap()[0];
    assert!(
        app.world().get::<bevy::picking::Pickable>(child).is_some(),
        "managed text child must be Pickable::IGNORE so clicks focus the input"
    );
}

#[test]
fn insert_before_reorders_children_on_reconcile() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<ul><li>one</li><li>two</li></ul>",
    )));
    let mut app = test_app();
    let _root = mount(&mut app, dom.clone());
    app.update(); // initial reconcile

    let (ul, li_one, li_two) = {
        let d = dom.borrow();
        let ul = d.query_selector(d.document(), "ul").unwrap();
        let kids = d.children(ul).to_vec();
        (ul, kids[0], kids[1])
    };

    // Move the second <li> before the first — a keyed-<For> style reorder.
    dom.borrow_mut()
        .insert_before(ul, li_two, Some(li_one))
        .unwrap();
    app.world_mut().non_send_resource_mut::<UiRuntime>().dirty = true;
    app.update();

    // The <ul> entity's children now read ["two", "one"].
    let ul_entity = app
        .world()
        .non_send_resource::<UiRuntime>()
        .entity_for(ul)
        .unwrap();
    let li_entities = app.world().get::<Children>(ul_entity).unwrap().to_vec();
    let labels: Vec<String> = li_entities
        .iter()
        .map(|&li| {
            let text_entity = app.world().get::<Children>(li).unwrap()[0];
            app.world().get::<Text>(text_entity).unwrap().0.clone()
        })
        .collect();
    assert_eq!(labels, vec!["two".to_string(), "one".to_string()]);
}

#[test]
fn append_child_reparents_entity_on_reconcile() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<div id='a'><span>x</span></div><div id='b'></div>",
    )));
    let mut app = test_app();
    let _root = mount(&mut app, dom.clone());
    app.update(); // initial reconcile

    let (a, b, span) = {
        let d = dom.borrow();
        let doc = d.document();
        (
            d.query_selector(doc, "#a").unwrap(),
            d.query_selector(doc, "#b").unwrap(),
            d.query_selector(doc, "span").unwrap(),
        )
    };

    // Move <span> from #a to #b (append reparents an attached node).
    dom.borrow_mut().append_child(b, span).unwrap();
    app.world_mut().non_send_resource_mut::<UiRuntime>().dirty = true;
    app.update();

    let (a_entity, b_entity) = {
        let rt = app.world().non_send_resource::<UiRuntime>();
        (rt.entity_for(a).unwrap(), rt.entity_for(b).unwrap())
    };
    assert_eq!(child_count(&mut app, a_entity), 0);
    assert_eq!(child_count(&mut app, b_entity), 1);
}
