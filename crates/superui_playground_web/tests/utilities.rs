//! In-browser utilities: apply_utilities generates CSS for scanned classes and combines
//! it with the authored stylesheet.
#![cfg(feature = "utilities")]
mod support;
use support::*;

use bevy::prelude::Assets;
use superui_css::style::StyleSheet;
use superui_playground_web::{apply_source_inner, apply_utilities_inner};

const COUNTER_JS: &str = r#"
    function Counter() {
        var c = createSignal(3); globalThis.__c = c;
        var wrap = $ss.el("div"); wrap.className = "flex";
        var label = $ss.el("span");
        $ss.insert(label, function () { return c[0](); });
        $ss.child(wrap, label);
        return wrap;
    }
    $ss.hot("u.js#Counter", Counter);
    render(function () { return $ss.cmp(Counter, {}); }, document.getElementById("root"));
"#;

#[test]
fn apply_utilities_combines_generated_and_authored() {
    put("u.js", COUNTER_JS.as_bytes());
    put("u.css", b"span { color: red }");
    let mut app = app();
    let _root = spawn_root(&mut app, "pg_util.html", "<div id='root'></div>", "u.css", "u.js");
    tick(&mut app, 32);

    // Seed authored CSS, then generate utilities from the source that uses `flex`.
    apply_source_inner("u.css", "span { color: red }");
    let out = apply_utilities_inner("[\"<div class=\\\"flex\\\">\"]");
    assert!(out.contains("\"ok\":true"), "apply_utilities ok: {out}");
    tick(&mut app, 4);

    // The stylesheet asset is overwritten in place, not replaced wholesale, and a
    // restyle preserves component state. This does not check WHAT the sheet
    // contains -- see `utilities_survive_a_later_authored_css_edit` for that.
    assert_eq!(app.world().resource::<Assets<StyleSheet>>().len(), 1, "overwritten in place");
    apply_source_inner("u.css", "span { color: blue }");
    tick(&mut app, 4);
    assert_eq!(label_text(&mut app), "3", "state preserved across restyles");
}

#[test]
fn bad_json_reports_without_panicking() {
    let out = apply_utilities_inner("not json");
    assert!(out.contains("\"ok\":false"), "bad JSON -> ok:false: {out}");
}

#[test]
fn utilities_survive_a_later_authored_css_edit() {
    let _ = superui_playground_web::drain_queue(); // isolate the queue on this thread

    // Seed authored CSS, then generate utilities from a flex-using source.
    apply_source_inner("x.css", "span { color: red }");
    apply_utilities_inner("[\"<div class=\\\"flex\\\">\"]");

    // The enqueued combined sheet carries BOTH the generated utility and the authored rule.
    let combined = last_css_edit(superui_playground_web::drain_queue());
    assert!(combined.contains("display: flex"), "utilities present: {combined}");
    assert!(combined.contains("color: red"), "authored present: {combined}");

    // A LATER authored edit must NOT drop the utilities (the wipe hazard).
    apply_source_inner("x.css", "span { color: blue }");
    let combined2 = last_css_edit(superui_playground_web::drain_queue());
    assert!(combined2.contains("display: flex"), "utilities survive a later authored edit: {combined2}");
    assert!(combined2.contains("color: blue"), "new authored rule applied: {combined2}");
}

fn last_css_edit(edits: Vec<superui_playground_web::Edit>) -> String {
    edits
        .into_iter()
        .rev()
        .find_map(|e| match e {
            superui_playground_web::Edit::Css(s) => Some(s),
            _ => None,
        })
        .expect("an Edit::Css was enqueued")
}
