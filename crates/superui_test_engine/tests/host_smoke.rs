use superui_test_engine::host::{build_headless_app, mount, HostProject};

fn fixture() -> HostProject {
    HostProject {
        html: include_str!("fixtures/basic/index.html").to_string(),
        css: include_str!("fixtures/basic/style.css").to_string(),
        js_or_tsx: include_str!("fixtures/basic/app.tsx").to_string(),
        tsx: true,
    }
}

#[test]
fn mounts_and_renders_fixture_dom() {
    let mut app = build_headless_app(&fixture());
    let _root = mount(&mut app);
    let rt = app.world().non_send::<superui_bridge::UiRuntime>();
    let dom = rt.dom.borrow();
    let node = dom.query_selector(dom.document(), "#hello").expect("#hello exists");
    assert_eq!(dom.text_content(node), "Hello");
}
