//! `superui` integration: authored assets mount, JS runs, DOM reconciles to ECS.
mod support;
use support::*;

use bevy::prelude::*;
use superui::UiRuntime;
use superui_css::prelude::TypeName;

#[test]
fn mounts_authored_ui_and_runs_js() {
    put("t8.css", b"li { }");
    put(
        "t8.js",
        b"var li = document.createElement('li'); \
          li.textContent = 'hello'; \
          document.getElementById('list').appendChild(li);",
    );

    let mut app = app();
    let root = spawn_root(&mut app, "<ul id='list'></ul>", "t8.css", "t8.js");
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
    put("t9.css", b"span { }");
    put(
        "t9.js",
        b"var s=document.createElement('span'); s.textContent='v1'; \
          document.getElementById('host').appendChild(s);",
    );

    let mut app = app();
    let _root = spawn_root(&mut app, "<div id='host'></div>", "t9.css", "t9.js");
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
        if let Some(mut js) = assets.get_mut(&js_handle) {
            js.code = "var s=document.createElement('span'); s.textContent='v2'; \
                    document.getElementById('host').appendChild(s); \
                    var s2=document.createElement('span'); \
                    document.getElementById('host').appendChild(s2);"
                .to_string();
        };
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

/// Graph-wide HMR (design §4): editing a module that is only an *import*, not
/// the mounted entry itself, must still change what's on screen. Unlike
/// [`hot_reload_js_re_executes_and_reconciles`] above — which proves the
/// generic `AssetEvent::Modified` -> re-exec path by mutating the already-loaded
/// `JsSource` in place — this proves the *loader* re-runs: the entry's `.tsx`
/// text never changes; only its dependency's does, and `TsxLoader`'s async
/// graph walk re-reads that dependency via `read_asset_bytes` (registering it
/// as a load dependency the first time through) and re-bundles.
///
/// Harness limitation: `MemoryAssetReader` has no file watcher, so there is no
/// "edit a file on disk and the watcher notices" to exercise. Instead it
/// re-`put`s the dependency's bytes into the shared `ASSETS` dir, then
/// explicitly calls `AssetServer::reload` on the entry path to force
/// `TsxLoader` to run again (exactly the trigger a real file watcher
/// would deliver). It does NOT edit the loaded `JsSource` asset directly, so
/// it genuinely exercises the loader, not just the re-exec seam.
///
/// Requires the `hmr` feature: without it, `mount.rs`'s tsx/js seam
/// (`live_source`) maps a `<script src="*.tsx">` to a pre-built
/// `.superui/build/*.js` path instead of loading the `.tsx` through
/// `TsxLoader` directly, which would defeat the point of this test (see
/// `resolve_script` / `live_source` in `src/mount.rs`).
#[cfg(feature = "hmr")]
#[test]
fn graph_wide_hmr_reloads_when_an_imported_module_changes() {
    put("gw.css", b"span { }");
    put("gw_lib.tsx", b"export const LABEL = \"v1\";");
    put(
        "gw_app.tsx",
        br#"import { LABEL } from "./gw_lib.tsx";
            var el = document.createElement('span'); el.id = 'mark'; el.textContent = LABEL;
            document.getElementById('host').appendChild(el);"#,
    );

    let mut app = app();
    let _root = spawn_root(&mut app, "<div id='host'></div>", "gw.css", "gw_app.tsx");
    tick(&mut app, 32);

    // All text-node contents under a `<span id="mark">` (re-exec, like the
    // plain-JS hot reload test above, appends rather than replacing — a
    // second `#mark` span after reload is expected, not a bug).
    fn mark_texts(app: &mut App) -> Vec<String> {
        let mut by_name = app.world_mut().query::<(Entity, &Name)>();
        let marks: Vec<Entity> = by_name
            .iter(app.world())
            .filter(|(_, n)| n.as_str() == "mark")
            .map(|(e, _)| e)
            .collect();
        marks
            .into_iter()
            .filter_map(|e| {
                let text_entity = *app.world().get::<Children>(e)?.first()?;
                app.world().get::<Text>(text_entity).map(|t| t.0.clone())
            })
            .collect()
    }

    assert_eq!(
        mark_texts(&mut app),
        vec!["v1".to_string()],
        "initial mount should reflect the dependency's export"
    );

    // Edit the DEPENDENCY (not the entry) and force the loader to re-run.
    put("gw_lib.tsx", b"export const LABEL = \"v2\";");
    {
        let server = app.world().resource::<bevy::asset::AssetServer>().clone();
        server.reload("gw_app.tsx");
    }
    tick(&mut app, 32);

    let texts = mark_texts(&mut app);
    assert!(
        texts.contains(&"v2".to_string()),
        "reloading the entry must re-walk the graph, re-read the edited \
         dependency, and re-exec so the DOM reflects the edit: {texts:?}"
    );
}

/// Module-graph CSS (spec §5): a `.css` imported by a module that is only a
/// *dependency* of the mounted entry (not the entry's own `<link>` sheet) must
/// still apply, merged in AFTER the `<link>` sheet so it wins on equal
/// specificity — the cascade order `mount.rs::merged_sheet` produces (spec §5).
///
/// The `<link>` sheet and the module both declare a rule for `.c`, with
/// different `width`s; the module's value wins, which is only possible if the
/// merge actually concatenated the two sheets (a sheet containing only one of
/// the two rules, or the wrong cascade order, would fail this assertion).
///
/// Same gate as `graph_wide_hmr_reloads_when_an_imported_module_changes`:
/// without `hmr`, `live_source()` maps the entry's `<script src="*.tsx">` to a
/// prebuilt `.superui/build/*.js` (loaded by the plain `JsLoader`, whose
/// `JsSource.css` is always empty) rather than routing it through `TsxLoader`,
/// which would defeat the point of this test.
#[cfg(feature = "hmr")]
#[test]
fn module_css_merges_after_link_stylesheet() {
    put("mcss_link.css", b".c { width: 999px; }");
    put("mcss_c.css", b".c { width: 10px; }");
    put("mcss_lib.tsx", b"import \"./mcss_c.css\";");
    put(
        "mcss_app.tsx",
        br#"import "./mcss_lib.tsx";
            var el = document.createElement('div');
            el.className = 'c';
            document.getElementById('host').appendChild(el);"#,
    );

    let mut app = app();
    let _root = spawn_root(&mut app, "<div id='host'></div>", "mcss_link.css", "mcss_app.tsx");
    tick(&mut app, 32);

    let width = {
        let mut q = app
            .world_mut()
            .query::<(&Node, &superui_css::prelude::ClassList)>();
        q.iter(app.world())
            .find(|(_, classes)| classes.contains("c"))
            .map(|(node, _)| node.width)
            .expect("the JS-created <div class='c'> should have reconciled")
    };
    assert_eq!(
        width,
        Val::Px(10.0),
        "the module's `.c` rule must win over the <link> sheet's `.c` rule, \
         proving the module CSS was merged in AFTER the link sheet"
    );
}

/// The `Val` width of the first reconciled `<div class="c">`, or `None` if it
/// hasn't reconciled. Shared by the module-CSS tests below.
fn class_c_width(app: &mut App) -> Option<Val> {
    let mut q = app
        .world_mut()
        .query::<(&Node, &superui_css::prelude::ClassList)>();
    q.iter(app.world())
        .find(|(_, classes)| classes.contains("c"))
        .map(|(node, _)| node.width)
}

/// Module-CSS HMR (the fix this file's `module_css_merges_after_link_stylesheet`
/// only covers at mount): editing a `.css` imported by a *dependency* module
/// must re-apply live, not just once at mount. Re-`put`s the module `.css` with a
/// new value and reloads the entry `.tsx` (the trigger a real file watcher would
/// deliver via `loader_dependents`, exactly as
/// `graph_wide_hmr_reloads_when_an_imported_module_changes` does for `.tsx`); the
/// styled width must track the edit.
#[cfg(feature = "hmr")]
#[test]
fn module_css_hot_reloads_when_its_css_file_changes() {
    put("mcss_hot_link.css", b"body { }");
    put("mcss_hot_c.css", b".c { width: 10px; }");
    put("mcss_hot_lib.tsx", b"import \"./mcss_hot_c.css\";");
    put(
        "mcss_hot_app.tsx",
        br#"import "./mcss_hot_lib.tsx";
            var el = document.createElement('div');
            el.className = 'c';
            document.getElementById('host').appendChild(el);"#,
    );

    let mut app = app();
    let _root = spawn_root(&mut app, "<div id='host'></div>", "mcss_hot_link.css", "mcss_hot_app.tsx");
    tick(&mut app, 32);
    assert_eq!(
        class_c_width(&mut app),
        Some(Val::Px(10.0)),
        "mount should apply the module's initial `.c` width"
    );

    // Edit the module `.css` and force the loader to re-walk the graph.
    put("mcss_hot_c.css", b".c { width: 20px; }");
    {
        let server = app.world().resource::<bevy::asset::AssetServer>().clone();
        server.reload("mcss_hot_app.tsx");
    }
    tick(&mut app, 32);

    assert_eq!(
        class_c_width(&mut app),
        Some(Val::Px(20.0)),
        "editing a module's `.css` must hot-reload the applied style"
    );
}

/// The other half of the old limitation: once module CSS is merged, editing the
/// entry `<link>` sheet must NOT drop the module CSS. Reloads the `<link>`
/// stylesheet (which flair reloads to pristine base, losing any in-slot merge)
/// and asserts the module's `.c` rule still wins — only possible if the applied
/// sheet is rebuilt from the fresh base plus the module CSS.
#[cfg(feature = "hmr")]
#[test]
fn editing_link_stylesheet_preserves_merged_module_css() {
    put("mcss_keep_link.css", b".c { width: 999px; }");
    put("mcss_keep_c.css", b".c { width: 10px; }");
    put("mcss_keep_lib.tsx", b"import \"./mcss_keep_c.css\";");
    put(
        "mcss_keep_app.tsx",
        br#"import "./mcss_keep_lib.tsx";
            var el = document.createElement('div');
            el.className = 'c';
            document.getElementById('host').appendChild(el);"#,
    );

    let mut app = app();
    let _root = spawn_root(&mut app, "<div id='host'></div>", "mcss_keep_link.css", "mcss_keep_app.tsx");
    tick(&mut app, 32);
    assert_eq!(
        class_c_width(&mut app),
        Some(Val::Px(10.0)),
        "mount should merge the module's `.c` rule over the <link> sheet"
    );

    // Edit the <link> sheet and reload it (flair replaces its asset with the
    // pristine, module-less base).
    put("mcss_keep_link.css", b".c { width: 888px; }");
    {
        let server = app.world().resource::<bevy::asset::AssetServer>().clone();
        server.reload("mcss_keep_link.css");
    }
    tick(&mut app, 32);

    assert_eq!(
        class_c_width(&mut app),
        Some(Val::Px(10.0)),
        "editing the <link> sheet must not drop the merged module CSS"
    );
}

/// Cross-operand specificity: merging is NOT a plain append where the module
/// always wins. A *higher*-specificity rule in the `<link>` sheet must still
/// beat a *lower*-specificity module rule, even though the module CSS is
/// concatenated AFTER the link sheet — the merge re-sorts the combined rules by
/// (layer, specificity), so `#host .c` (id + class, from the link sheet)
/// outranks `.c` (class only, from the module). Paired with
/// `module_css_merges_after_link_stylesheet` (which proves the module wins a
/// same-specificity tie), it pins the full cross-sheet cascade.
#[cfg(feature = "hmr")]
#[test]
fn higher_specificity_link_rule_beats_lower_specificity_module_rule() {
    put("mcss_spec_link.css", b"#host .c { width: 999px; }");
    put("mcss_spec_c.css", b".c { width: 10px; }");
    put("mcss_spec_lib.tsx", b"import \"./mcss_spec_c.css\";");
    put(
        "mcss_spec_app.tsx",
        br#"import "./mcss_spec_lib.tsx";
            var el = document.createElement('div');
            el.className = 'c';
            document.getElementById('host').appendChild(el);"#,
    );

    let mut app = app();
    let _root = spawn_root(&mut app, "<div id='host'></div>", "mcss_spec_link.css", "mcss_spec_app.tsx");
    tick(&mut app, 32);

    assert_eq!(
        class_c_width(&mut app),
        Some(Val::Px(999.0)),
        "the <link> sheet's higher-specificity `#host .c` rule must win over the \
         module's lower-specificity `.c` rule, even though module CSS is merged in \
         after the link sheet"
    );
}

/// Shipped (no-HMR) path: with `hmr` off, the mount seam loads the prebuilt
/// `.superui/build/<stem>.js` via the plain `JsLoader`, which must pick up the
/// sidecar `<stem>.css` that `supersolid::build` writes so module CSS still
/// applies in release builds. Stand in for a real `build.rs` run by `put`ting
/// the artifact + sidecar directly (the loader's sidecar read is the code under
/// test). Gated to the no-HMR config because with `hmr` the seam would load the
/// `.tsx` through `TsxLoader` instead.
#[cfg(not(feature = "hmr"))]
#[test]
fn shipped_build_applies_module_css_from_sidecar() {
    put("mcss_ship_link.css", b".c { width: 999px; }");
    put(
        ".superui/build/mcss_ship_app.js",
        b"var el = document.createElement('div'); el.className = 'c'; \
          document.getElementById('host').appendChild(el);",
    );
    put(".superui/build/mcss_ship_app.css", b".c { width: 10px; }");

    let mut app = app();
    // The entry references the `.tsx` source; the no-HMR seam maps it to the
    // prebuilt `.superui/build/mcss_ship_app.js` artifact above.
    let _root = spawn_root(&mut app, "<div id='host'></div>", "mcss_ship_link.css", "mcss_ship_app.tsx");
    tick(&mut app, 32);

    assert_eq!(
        class_c_width(&mut app),
        Some(Val::Px(10.0)),
        "the shipped build must apply module CSS from the generated sidecar"
    );
}

#[test]
fn html_hot_reload_despawns_old_entities_no_leak() {
    // Mount with 3 lis, then hot-reload HTML+JS to produce 1 li;
    // assert old 3 are gone (no leak), exactly 1 remains.
    put("leak.css", b"li { }");
    put(
        "leak.js",
        b"var h=document.getElementById('h'); \
          for(var i=0;i<3;i++){var li=document.createElement('li');h.appendChild(li);}",
    );

    let mut app = app();
    let _root = spawn_root_entry(&mut app, "leak.html", "<ul id='h'></ul>", "leak.css", "leak.js");
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
        if let Some(mut h) = assets.get_mut(&html_handle) {
            h.0 = entry_doc("<ul id='h'></ul>", "leak.css", "leak.js");
        };
    }
    // Also mutate JS so re-run produces only 1 li.
    let js_handle = {
        let server = app.world().resource::<bevy::asset::AssetServer>().clone();
        server.load::<superui::JsSource>("leak.js")
    };
    {
        let mut assets = app.world_mut().resource_mut::<Assets<superui::JsSource>>();
        if let Some(mut j) = assets.get_mut(&js_handle) {
            j.code = "var h=document.getElementById('h'); \
                   var li=document.createElement('li');h.appendChild(li);"
                .to_string();
        };
    }
    // Explicitly fire Modified for the HTML handle to trigger hot reload.
    app.world_mut().write_message(bevy::asset::AssetEvent::Modified {
        id: html_handle.id(),
    });
    tick(&mut app, 12);

    assert_eq!(
        count_li(&mut app),
        1,
        "after HTML hot-reload exactly 1 li must exist (old 3 must be despawned)"
    );
}

#[test]
fn despawning_root_tears_down_runtime_and_subtree() {
    // Idiomatic teardown: `commands.entity(root).despawn()` must remove the
    // UiRuntime and the reconciled subtree, without the "entity does not exist"
    // panic the bridge chain would otherwise hit next frame on a stale runtime.
    put("td.css", b"li { }");
    put(
        "td.js",
        b"var h=document.getElementById('h'); \
          for(var i=0;i<3;i++){var li=document.createElement('li');h.appendChild(li);}",
    );

    let mut app = app();
    let root = spawn_root(&mut app, "<ul id='h'></ul>", "td.css", "td.js");
    tick(&mut app, 32);

    assert!(app.world().contains_non_send::<UiRuntime>(), "UI should mount");
    let count_li = |app: &mut App| {
        let mut q = app.world_mut().query::<&superui_css::prelude::TypeName>();
        q.iter(app.world()).filter(|t| t.0 == "li").count()
    };
    assert_eq!(count_li(&mut app), 3, "initial JS should create 3 lis");

    app.world_mut().entity_mut(root).despawn();
    tick(&mut app, 8);

    assert!(
        !app.world().contains_non_send::<UiRuntime>(),
        "despawning the root must remove the UiRuntime"
    );
    assert_eq!(count_li(&mut app), 0, "the reconciled subtree must be despawned");
}

#[test]
fn a_fresh_root_mounts_after_teardown() {
    // Menu switch: the mount guard is keyed on the UiRuntime, which teardown
    // releases, so a newly spawned SuperUiRoot mounts on its own.
    put("sw.css", b"li { }");
    put(
        "sw.js",
        b"var h=document.getElementById('h'); var li=document.createElement('li'); \
          li.textContent='a'; h.appendChild(li);",
    );

    let mut app = app();
    let root = spawn_root(&mut app, "<ul id='h'></ul>", "sw.css", "sw.js");
    tick(&mut app, 32);
    assert!(app.world().contains_non_send::<UiRuntime>(), "first UI should mount");

    app.world_mut().entity_mut(root).despawn();
    tick(&mut app, 8);
    assert!(
        !app.world().contains_non_send::<UiRuntime>(),
        "teardown should release the runtime"
    );

    let _root2 = spawn_root(&mut app, "<ul id='h'></ul>", "sw.css", "sw.js");
    tick(&mut app, 32);
    assert!(
        app.world().contains_non_send::<UiRuntime>(),
        "a fresh root must mount after teardown"
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

    let _root = spawn_root(&mut app, "<button id='add'>Add</button><ul id='list'></ul>", "cap.css", "cap.js");
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
