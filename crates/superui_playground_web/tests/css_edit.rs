//! A queued CSS edit reparses and restyles without losing signal state.
mod support;
use support::*;

use bevy::prelude::Assets;
use superui_bridge::UiRuntime;
use superui_css::style::StyleSheet;
use superui_playground_web::apply_source_inner;

const COUNTER_JS: &str = r#"
    function Counter() {
        var c = createSignal(7);
        globalThis.__c = c;
        var wrap = $ss.el("div");
        var label = $ss.el("span");
        $ss.insert(label, function () { return c[0](); });
        $ss.child(wrap, label);
        return wrap;
    }
    $ss.hot("counter.js#Counter", Counter);
    render(function () { return $ss.cmp(Counter, {}); }, document.getElementById("root"));
"#;

#[test]
fn css_edit_restyles_and_preserves_state() {
    put("cnt.js", COUNTER_JS.as_bytes());
    put("s.css", b"span { color: red }");
    let mut app = app();
    let _root = spawn_root(&mut app, "pg_css.html", "<div id='root'></div>", "s.css", "cnt.js");
    tick(&mut app, 32);
    assert_eq!(label_text(&mut app), "7");

    let sheets_before = app.world().resource::<Assets<StyleSheet>>().len();
    apply_source_inner("s.css", "span { color: blue }");
    tick(&mut app, 4);

    // Signal state survives a restyle; the stylesheet asset was replaced in place.
    assert_eq!(label_text(&mut app), "7", "restyle preserves state");
    assert_eq!(app.world().resource::<Assets<StyleSheet>>().len(), sheets_before);
}

#[test]
fn malformed_css_reports_and_keeps_running() {
    let _ = superui_playground_web::drain_queue();
    put("m.js", COUNTER_JS.as_bytes());
    put("m.css", b"span { color: red }");
    let mut app = app();
    let _root = spawn_root(&mut app, "pg_cssbad.html", "<div id='root'></div>", "m.css", "m.js");
    tick(&mut app, 32);

    apply_source_inner("m.css", "span { color: ; @@@ }");
    tick(&mut app, 4);

    // No panic; a diagnostic was recorded; UI still shows the counter.
    let diags = superui_playground_web::poll_diagnostics_inner();
    assert!(diags.contains("message"), "a parse diagnostic was recorded: {diags}");
    assert_eq!(label_text(&mut app), "7");
}
