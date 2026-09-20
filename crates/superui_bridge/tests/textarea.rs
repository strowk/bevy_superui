//! <textarea> is a multiline EditableText: newlines allowed, wrapping layout,
//! initial text seeded from its content, value bridged like <input>.
mod support;
use support::*;

use std::cell::RefCell;
use std::rc::Rc;

use bevy::prelude::*;
use bevy::text::EditableText;
use superui_bridge::{DomNode, UiRuntime};

fn entity_for(app: &mut App, node: superui_dom::NodeId) -> Entity {
    let mut q = app.world_mut().query::<(Entity, &DomNode)>();
    q.iter(app.world()).find(|(_, d)| d.0 == node).map(|(e, _)| e).unwrap()
}

#[test]
fn textarea_is_multiline_editable() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<textarea id='t' rows='4'></textarea>",
    )));
    let mut app = test_app();
    let _root = mount(&mut app, dom.clone());
    app.update();

    let node = dom.borrow().get_element_by_id("t").unwrap();
    let ta = entity_for(&mut app, node);
    let editable = app.world().get::<EditableText>(ta).expect("textarea has EditableText");
    assert!(editable.allow_newlines, "textarea allows newlines");
    assert_eq!(editable.visible_lines, Some(4.0), "rows maps to visible_lines");
    let layout = app.world().get::<TextLayout>(ta).expect("textarea TextLayout");
    assert_ne!(layout.linebreak, bevy::text::LineBreak::NoWrap, "textarea wraps");
}

#[test]
fn textarea_without_rows_defaults_to_three_visible_lines() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<textarea id='t'></textarea>",
    )));
    let mut app = test_app();
    let _root = mount(&mut app, dom.clone());
    app.update();

    let node = dom.borrow().get_element_by_id("t").unwrap();
    let ta = entity_for(&mut app, node);
    let editable = app.world().get::<EditableText>(ta).unwrap();
    assert_eq!(editable.visible_lines, Some(3.0), "no rows => 3-line default");
}

#[test]
fn textarea_seeds_initial_content() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<textarea id='t'>hello</textarea>",
    )));
    let mut app = test_app();
    let _root = mount(&mut app, dom.clone());
    app.update();
    app.update();
    let node = dom.borrow().get_element_by_id("t").unwrap();
    let ta = entity_for(&mut app, node);
    let val = app.world().get::<EditableText>(ta).unwrap().value().to_string();
    assert_eq!(val, "hello", "textarea seeds from its text content");
}

/// Clearing a textarea seeded with content must leave the buffer empty, not
/// refill it from the original text content.
#[test]
fn textarea_with_initial_content_stays_empty_after_clearing() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<textarea id='t'>hello</textarea>",
    )));
    let mut app = test_app();
    let _root = mount(&mut app, dom.clone());
    app.update();
    app.update();

    let node = dom.borrow().get_element_by_id("t").unwrap();
    let ta = entity_for(&mut app, node);
    assert_eq!(
        app.world().get::<EditableText>(ta).unwrap().value().to_string(),
        "hello"
    );

    dom.borrow_mut().set_value(node, "");
    app.world_mut().non_send_mut::<UiRuntime>().dirty = true;
    app.update();

    let val = app.world().get::<EditableText>(ta).unwrap().value().to_string();
    assert_eq!(val, "", "clearing must not refill from text_content");
}
