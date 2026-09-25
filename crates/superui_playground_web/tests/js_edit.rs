//! A queued JS edit drives the seam to a state-preserving re-exec.
mod support;
use support::*;

use superui_bridge::UiRuntime;
use superui_playground_web::apply_source_inner;

// A hand-written $ss counter module (what the transpiler emits), tagged for HMR.
const COUNTER_JS: &str = r#"
    function Counter() {
        var c = createSignal(0);
        globalThis.__c = c;
        var wrap = $ss.el("div");
        var label = $ss.el("span");
        $ss.insert(label, function () { return c[0](); });
        $ss.child(wrap, label);
        return wrap;
    }
    $ss.hot("counter.js#Counter", Counter);
    render(function () { return $ss.cmp(Counter, {}); },
           document.getElementById("root"));
"#;

// The "edited" module: same signal + same $ss.hot id (so the cell rehydrates),
// but the reactive binding now prefixes the value. Only a genuine re-exec can
// make "v2:" appear in the DOM; only rehydration can make the value stay "5"
// instead of resetting to "0" — so `"v2:5"` proves both at once.
const EDITED_COUNTER_JS: &str = r#"
    function Counter() {
        var c = createSignal(0);
        globalThis.__c = c;
        var wrap = $ss.el("div");
        var label = $ss.el("span");
        $ss.insert(label, function () { return "v2:" + c[0](); });
        $ss.child(wrap, label);
        return wrap;
    }
    $ss.hot("counter.js#Counter", Counter);
    render(function () { return $ss.cmp(Counter, {}); },
           document.getElementById("root"));
"#;

#[test]
fn js_edit_reexecs_and_preserves_signal() {
    put("counter.js", COUNTER_JS.as_bytes());
    put("c.css", b"");
    let mut app = app();
    let _root = spawn_root(&mut app, "pg_js.html", "<div id='root'></div>", "c.css", "counter.js");
    tick(&mut app, 32);
    assert_eq!(label_text(&mut app), "0", "initial reconcile");

    // Bump the signal to 5.
    app.world_mut().non_send_mut::<UiRuntime>().run_script("globalThis.__c[1](5);");
    tick(&mut app, 2);
    assert_eq!(label_text(&mut app), "5");

    // Edit the SAME module through the bridge: the reactive binding now prefixes
    // the value, so the edit is observable in the DOM only via a real re-exec.
    assert!(
        apply_source_inner("counter.js", EDITED_COUNTER_JS).contains("\"ok\":true"),
        "edit should enqueue cleanly"
    );
    tick(&mut app, 4);

    assert_eq!(
        label_text(&mut app),
        "v2:5",
        "\"v2:\" proves the edited module re-executed; \"5\" (not \"0\") proves the signal was rehydrated"
    );
}

#[test]
fn unknown_path_and_premmount_edit_do_not_panic() {
    let _ = superui_playground_web::drain_queue();
    // Unknown extension: reported not-ok, enqueues nothing harmful.
    let out = apply_source_inner("notes.txt", "hi");
    assert!(out.contains("\"ok\":false"));
    // A JS edit enqueued before any UI mounts must drain without panicking.
    apply_source_inner("counter.js", "var x=1;");
    let mut app = app();
    tick(&mut app, 3); // no SuperUiRoot spawned; drain sees no subresources
}
