//! Reconciler integration: DOM tree -> ECS entity tree (structure + text).
mod support;
use support::*;

use std::cell::RefCell;
use std::rc::Rc;

use bevy::prelude::*;
use superui_bridge::DomNode;
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
