//! `superui` integration: authored assets mount, JS runs, DOM reconciles to ECS.
mod support;
use support::*;

use bevy::prelude::*;
use superui_css::prelude::TypeName;

#[test]
fn mounts_authored_ui_and_runs_js() {
    put("t8.html", b"<ul id='list'></ul>");
    put("t8.css", b"li { }");
    put(
        "t8.js",
        b"var li = document.createElement('li'); \
          li.textContent = 'hello'; \
          document.getElementById('list').appendChild(li);",
    );

    let mut app = app();
    let root = spawn_root(&mut app, "t8.html", "t8.css", "t8.js");
    tick(&mut app, 32);

    // The JS-created <li> exists as an entity with TypeName "li".
    let mut q = app.world_mut().query::<&TypeName>();
    let has_li = q.iter(app.world()).any(|t| t.0 == "li");
    assert!(has_li, "JS-created <li> should reconcile into an entity");

    // And it is under the root's subtree.
    assert!(app.world().get::<Children>(root).is_some());
}

#[test]
fn hot_reload_js_re_executes_and_reconciles() {
    put("t9.html", b"<div id='host'></div>");
    put("t9.css", b"span { }");
    put(
        "t9.js",
        b"var s=document.createElement('span'); s.textContent='v1'; \
          document.getElementById('host').appendChild(s);",
    );

    let mut app = app();
    let _root = spawn_root(&mut app, "t9.html", "t9.css", "t9.js");
    tick(&mut app, 32);

    // v1 span exists.
    let count_spans = |app: &mut App| {
        let mut q = app.world_mut().query::<&superui_css::prelude::TypeName>();
        q.iter(app.world()).filter(|t| t.0 == "span").count()
    };
    assert_eq!(count_spans(&mut app), 1);

    // Modify the JS asset in place.
    let js_handle = {
        let server = app.world().resource::<bevy::asset::AssetServer>().clone();
        server.load::<superui::JsSource>("t9.js")
    };
    {
        let mut assets = app.world_mut().resource_mut::<Assets<superui::JsSource>>();
        if let Some(js) = assets.get_mut(&js_handle) {
            js.0 = "var s=document.createElement('span'); s.textContent='v2'; \
                    document.getElementById('host').appendChild(s); \
                    var s2=document.createElement('span'); \
                    document.getElementById('host').appendChild(s2);"
                .to_string();
        }
    }
    // Explicitly fire Modified in case get_mut doesn't emit it in this Bevy version.
    app.world_mut().write_message(bevy::asset::AssetEvent::Modified {
        id: js_handle.id(),
    });
    // Tick to process the hot reload.
    tick(&mut app, 8);

    // Re-execution ran against the current DOM: host now has more spans.
    assert!(count_spans(&mut app) >= 2, "hot reload should re-run the JS");
}
