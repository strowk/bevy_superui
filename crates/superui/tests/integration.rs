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

#[test]
fn html_hot_reload_despawns_old_entities_no_leak() {
    // Mount with 3 lis, then hot-reload HTML+JS to produce 1 li;
    // assert old 3 are gone (no leak), exactly 1 remains.
    put("leak.html", b"<ul id='h'></ul>");
    put("leak.css", b"li { }");
    put(
        "leak.js",
        b"var h=document.getElementById('h'); \
          for(var i=0;i<3;i++){var li=document.createElement('li');h.appendChild(li);}",
    );

    let mut app = app();
    let _root = spawn_root(&mut app, "leak.html", "leak.css", "leak.js");
    tick(&mut app, 32);

    let count_li = |app: &mut App| {
        let mut q = app.world_mut().query::<&superui_css::prelude::TypeName>();
        q.iter(app.world()).filter(|t| t.0 == "li").count()
    };
    assert_eq!(count_li(&mut app), 3, "initial JS should create 3 lis");

    // Mutate HTML asset in place (same handle, new JS produces 1 li).
    let html_handle = {
        let server = app.world().resource::<bevy::asset::AssetServer>().clone();
        server.load::<superui::HtmlSource>("leak.html")
    };
    {
        let mut assets = app
            .world_mut()
            .resource_mut::<Assets<superui::HtmlSource>>();
        if let Some(h) = assets.get_mut(&html_handle) {
            h.0 = "<ul id='h'></ul>".to_string();
        }
    }
    // Also mutate JS so re-run produces only 1 li.
    let js_handle = {
        let server = app.world().resource::<bevy::asset::AssetServer>().clone();
        server.load::<superui::JsSource>("leak.js")
    };
    {
        let mut assets = app.world_mut().resource_mut::<Assets<superui::JsSource>>();
        if let Some(j) = assets.get_mut(&js_handle) {
            j.0 = "var h=document.getElementById('h'); \
                   var li=document.createElement('li');h.appendChild(li);"
                .to_string();
        }
    }
    // Explicitly fire Modified for the HTML handle to trigger hot reload.
    app.world_mut().write_message(bevy::asset::AssetEvent::Modified {
        id: html_handle.id(),
    });
    tick(&mut app, 8);

    assert_eq!(
        count_li(&mut app),
        1,
        "after HTML hot-reload exactly 1 li must exist (old 3 must be despawned)"
    );
}

use serde::{Deserialize, Serialize};
use superui_bridge::{PendingDomEvent, PendingDomEvents, SuperUiApp};

#[derive(Event, Serialize, Deserialize, Clone, Debug, PartialEq)]
struct Added {
    label: String,
}

#[test]
fn capstone_click_drives_js_and_bevy_send() {
    put("cap.html", b"<button id='add'>Add</button><ul id='list'></ul>");
    put("cap.css", b".done { }");
    put(
        "cap.js",
        b"document.getElementById('add').addEventListener('click', function(){ \
             var li=document.createElement('li'); li.textContent='item'; \
             document.getElementById('list').appendChild(li); \
             bevy.send('Added', { label: 'item' }); \
          });",
    );

    let mut app = app();
    app.add_superui_command::<Added>("Added");
    #[derive(Resource, Default)]
    struct Log(Vec<Added>);
    app.init_resource::<Log>();
    app.add_observer(|ev: On<Added>, mut l: ResMut<Log>| l.0.push(ev.event().clone()));

    let _root = spawn_root(&mut app, "cap.html", "cap.css", "cap.js");
    tick(&mut app, 32);

    // No <li> yet.
    let count_li = |app: &mut App| {
        let mut q = app.world_mut().query::<&superui_css::prelude::TypeName>();
        q.iter(app.world()).filter(|t| t.0 == "li").count()
    };
    assert_eq!(count_li(&mut app), 0);

    // Find the button's DOM node id via its entity's DomNode, enqueue a click.
    let btn_node = {
        let mut q = app
            .world_mut()
            .query::<(&superui_bridge::DomNode, &superui_css::prelude::TypeName)>();
        q.iter(app.world())
            .find(|(_, t)| t.0 == "button")
            .map(|(d, _)| d.0)
            .expect("button entity exists")
    };
    app.world_mut()
        .resource_mut::<PendingDomEvents>()
        .0
        .push(PendingDomEvent::new(btn_node, "click"));
    tick(&mut app, 4);

    // The click ran the JS listener: a <li> was created AND a bevy.send fired.
    assert_eq!(count_li(&mut app), 1, "click should create one <li>");
    assert_eq!(
        app.world().resource::<Log>().0,
        vec![Added { label: "item".into() }]
    );
}
