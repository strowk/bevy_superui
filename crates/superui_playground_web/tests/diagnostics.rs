//! Runtime JS errors from an edit reach the diagnostics sink for the console.
mod support;
use support::*;

use superui_playground_web::{apply_source_inner, poll_diagnostics_inner};

const GOOD_JS: &str = r#"
    var s = document.createElement('span'); s.textContent = '1';
    document.getElementById('root').appendChild(s);
"#;

#[test]
fn runtime_error_from_edit_reaches_poll() {
    put("d.js", GOOD_JS.as_bytes());
    put("d.css", b"");
    let mut app = app();
    let _root = spawn_root(&mut app, "pg_diag.html", "<div id='root'></div>", "d.css", "d.js");
    tick(&mut app, 32);
    let _ = poll_diagnostics_inner(); // clear any startup noise

    // Edit to a throwing script; the seam re-execs it, UiRuntime captures the throw.
    apply_source_inner("d.js", "throw new Error('kaboom');");
    tick(&mut app, 4);

    let diags = poll_diagnostics_inner();
    assert!(diags.contains("kaboom"), "runtime error surfaced: {diags}");
}
