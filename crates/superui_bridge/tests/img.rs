//! `<img>` reconciliation: src -> ImageNode with a base_dir-resolved handle.
mod support;
use support::*;

use std::cell::RefCell;
use std::rc::Rc;

use bevy::prelude::*;
use superui_bridge::UiRuntime;

/// The first child entity of `parent`.
fn first_child(app: &App, parent: Entity) -> Entity {
    app.world().get::<Children>(parent).expect("has children")[0]
}

/// The asset path of `e`'s `ImageNode` handle, if any.
fn image_path(app: &App, e: Entity) -> Option<String> {
    let node = app.world().get::<ImageNode>(e)?;
    node.image.path().map(|p| p.path().to_string_lossy().into_owned())
}

#[test]
fn img_gets_image_node_resolved_against_base_dir() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<img id='pic' src='pic.png'>",
    )));
    let mut app = test_app();
    let root = mount_with_dir(&mut app, dom.clone(), "ui/x");
    app.update();

    let img = first_child(&app, root);
    assert_eq!(image_path(&app, img).as_deref(), Some("ui/x/pic.png"));
}

#[test]
fn img_src_resolution_handles_absolute_and_dot_prefix() {
    for (src, expect) in [("/a.png", "a.png"), ("./b.png", "ui/x/b.png")] {
        let dom = Rc::new(RefCell::new(superui_html::parse_document(&format!(
            "<img src='{src}'>"
        ))));
        let mut app = test_app();
        let root = mount_with_dir(&mut app, dom.clone(), "ui/x");
        app.update();
        let img = first_child(&app, root);
        assert_eq!(image_path(&app, img).as_deref(), Some(expect), "src={src}");
    }
}

#[test]
fn empty_base_dir_resolves_to_src_unchanged() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<img src='logo.png'>",
    )));
    let mut app = test_app();
    let root = mount(&mut app, dom.clone());
    app.update();
    let img = first_child(&app, root);
    assert_eq!(image_path(&app, img).as_deref(), Some("logo.png"));
}

#[test]
fn img_without_src_has_no_image_node() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document("<img>")));
    let mut app = test_app();
    let root = mount(&mut app, dom.clone());
    app.update();
    let img = first_child(&app, root);
    assert!(app.world().get::<ImageNode>(img).is_none());
    // and reconciling an srcless <img> must not panic (already proven by reaching here)
}

#[test]
fn changing_src_swaps_the_handle() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<img id='pic' src='a.png'>",
    )));
    let mut app = test_app();
    let root = mount_with_dir(&mut app, dom.clone(), "d");
    app.update();
    let img = first_child(&app, root);
    assert_eq!(image_path(&app, img).as_deref(), Some("d/a.png"));

    let node = dom.borrow().get_element_by_id("pic").unwrap();
    dom.borrow_mut().set_attribute(node, "src", "b.png").unwrap();
    app.world_mut()
        .get_non_send_mut::<UiRuntime>()
        .unwrap()
        .dirty = true;
    app.update();
    assert_eq!(image_path(&app, img).as_deref(), Some("d/b.png"));
}

#[test]
fn clearing_src_removes_the_image_node() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<img id='pic' src='a.png'>",
    )));
    let mut app = test_app();
    let root = mount(&mut app, dom.clone());
    app.update();
    let img = first_child(&app, root);
    assert!(app.world().get::<ImageNode>(img).is_some());

    let node = dom.borrow().get_element_by_id("pic").unwrap();
    dom.borrow_mut().set_attribute(node, "src", "").unwrap();
    app.world_mut()
        .get_non_send_mut::<UiRuntime>()
        .unwrap()
        .dirty = true;
    app.update();
    assert!(app.world().get::<ImageNode>(img).is_none());
}
