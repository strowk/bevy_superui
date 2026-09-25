//! An index.html edit remounts and re-discovers subresources (state reset is expected).
mod support;
use support::*;

use superui_css::prelude::TypeName;
use superui_playground_web::apply_source_inner;

const JS_A: &str = r#"
    var h = document.getElementById('root');
    var s = document.createElement('span'); s.textContent = 'A'; h.appendChild(s);
"#;

#[test]
fn html_edit_remounts_with_new_manifest() {
    put("a.js", JS_A.as_bytes());
    put("h.css", b"");
    let mut app = app();
    // The entry references a.js; body has the #root mount point.
    let _root = spawn_root(&mut app, "pg_html.html", "<div id='root'></div>", "h.css", "a.js");
    tick(&mut app, 32);
    assert_eq!(label_text(&mut app), "A", "initial script ran");

    // Edit the entry HTML to a new document (still referencing a.js + h.css).
    let new_doc = entry_doc("<div id='root'></div><p id='mark'>x</p>", "h.css", "a.js");
    apply_source_inner("pg_html.html", &new_doc);
    tick(&mut app, 32);

    // Remounted: the new <p id=mark> exists (proves the manifest was re-read).
    let mut q = app.world_mut().query::<&TypeName>();
    let has_p = q.iter(app.world()).any(|t| t.0 == "p");
    assert!(has_p, "HTML remount re-parsed the new document");
}
