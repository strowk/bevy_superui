//! `superui` integration: `<img>` loads from assets and resolves `src`.
mod support;
use support::*;

use superui::UiRuntime;

#[test]
fn mount_records_entry_directory_as_base_dir() {
    put("bd.css", b"img { }");
    put("bd.js", b"");
    let mut app = app();
    // css/js are root-absolute so they resolve to the root `put` names regardless
    // of the subdir entry; only the `<img src>` exercises base_dir-relative joins.
    let _root = spawn_root_at(&mut app, "ui/x/index.html", "<img src='pic.png'>", "/bd.css", "/bd.js");
    tick(&mut app, 32);

    let rt = app
        .world()
        .get_non_send::<UiRuntime>()
        .expect("runtime mounted");
    assert_eq!(rt.base_dir, "ui/x", "base_dir must be the entry HTML's parent dir");
}

#[test]
fn mounted_img_resolves_and_gets_image_node() {
    use bevy::prelude::*;
    use superui_css::prelude::TypeName;

    put("e2e.css", b"img { }");
    put("e2e.js", b"");
    let mut app = app();
    let _root = spawn_root_at(
        &mut app,
        "ui/e2e/index.html",
        "<img id='pic' src='logo.png'>",
        "/e2e.css",
        "/e2e.js",
    );
    tick(&mut app, 32);

    // Find the img entity by TypeName and assert its ImageNode handle path.
    let mut q = app.world_mut().query::<(&TypeName, &ImageNode)>();
    let path = q
        .iter(app.world())
        .find(|(t, _)| t.0 == "img")
        .and_then(|(_, n)| n.image.path())
        .map(|p| p.path().to_string_lossy().into_owned());
    assert_eq!(path.as_deref(), Some("ui/e2e/logo.png"));
}
