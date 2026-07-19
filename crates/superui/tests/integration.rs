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
