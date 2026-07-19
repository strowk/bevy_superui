//! Reconciler integration: DOM tree -> ECS entity tree (structure + text).
mod support;
use support::*;

use std::cell::RefCell;
use std::rc::Rc;

use bevy::prelude::*;
use superui_bridge::DomNode;
use superui_css::prelude::TypeName;
use superui_dom::NodeKind;

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
