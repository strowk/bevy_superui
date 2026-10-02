//! `superui` integration: `<img>` loads from assets and resolves `src`.
mod support;
use support::*;

use bevy::prelude::*;
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
        .get_non_send_resource::<UiRuntime>()
        .expect("runtime mounted");
    assert_eq!(rt.base_dir, "ui/x", "base_dir must be the entry HTML's parent dir");
}
