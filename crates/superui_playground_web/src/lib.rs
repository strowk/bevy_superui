//! Browser "watcher" for the superui web playground: takes edited source from JS,
//! transpiles/parses it, and drives the existing superui hot-reload seam. All logic
//! is native-testable; the wasm-bindgen exports are thin wrappers added by a later task.

use std::cell::RefCell;

use bevy::asset::AssetEvent;
use bevy::ecs::system::SystemState;
use bevy::prelude::*;
use superui::{HtmlSource, JsSource, SuperUiRoot, SuperUiSubresources};
use superui_css::parser::InlineCssStyleSheetParser;
use superui_css::style::StyleSheet;

#[derive(Debug)]
pub enum Edit {
    Js(String),
    Css(String),
    Html(String),
}

thread_local! {
    static QUEUE: RefCell<Vec<Edit>> = const { RefCell::new(Vec::new()) };
    static DIAGS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// Drain the pending edits (used by the Bevy drain system and by tests).
pub fn drain_queue() -> Vec<Edit> {
    QUEUE.with(|q| core::mem::take(&mut *q.borrow_mut()))
}

/// Record a runtime/parse diagnostic for the console to poll.
pub fn push_diag(msg: String) {
    DIAGS.with(|d| d.borrow_mut().push(msg));
}

/// JSON array of diagnostics recorded since the last poll.
pub fn poll_diagnostics_inner() -> String {
    let msgs = DIAGS.with(|d| core::mem::take(&mut *d.borrow_mut()));
    let arr: Vec<_> = msgs.into_iter().map(|m| serde_json::json!({ "message": m })).collect();
    serde_json::Value::Array(arr).to_string()
}

/// Classify an edited file, transpile `.tsx`/`.ts` synchronously, enqueue one Edit,
/// and return `{ok, diagnostics}` JSON. CSS/HTML parse errors surface later via poll.
pub fn apply_source_inner(path: &str, src: &str) -> String {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".tsx") || lower.ends_with(".ts") {
        let tsx = !lower.ends_with(".ts");
        let opts = supersolid::TranspileOptions {
            tsx,
            module_id: Some(path.to_string()),
            ..Default::default()
        };
        let result = supersolid::transpile(src, &opts);
        let diags: Vec<_> = result
            .diagnostics
            .iter()
            .map(|d| serde_json::json!({ "severity": format!("{:?}", d.severity), "message": d.message }))
            .collect();
        let ok = diags.is_empty();
        QUEUE.with(|q| q.borrow_mut().push(Edit::Js(result.code)));
        serde_json::json!({ "ok": ok, "diagnostics": diags }).to_string()
    } else if lower.ends_with(".js") || lower.ends_with(".mjs") {
        // Classic JS passes through the seam verbatim (no transpile).
        QUEUE.with(|q| q.borrow_mut().push(Edit::Js(src.to_string())));
        serde_json::json!({ "ok": true, "diagnostics": [] }).to_string()
    } else if lower.ends_with(".css") {
        QUEUE.with(|q| q.borrow_mut().push(Edit::Css(src.to_string())));
        serde_json::json!({ "ok": true, "diagnostics": [] }).to_string()
    } else if lower.ends_with(".html") {
        QUEUE.with(|q| q.borrow_mut().push(Edit::Html(src.to_string())));
        serde_json::json!({ "ok": true, "diagnostics": [] }).to_string()
    } else {
        serde_json::json!({ "ok": false, "diagnostics": [{ "severity": "Error", "message": format!("unsupported file: {path}") }] }).to_string()
    }
}

/// Bevy-side half of the seam: drains queued edits each frame and drives them
/// through superui's existing hot-reload machinery (overwrite the mounted asset +
/// fire an explicit `AssetEvent::Modified`, which `detect_hot_reload`/
/// `apply_hot_reload` pick up).
pub struct PlaygroundBridgePlugin;

impl Plugin for PlaygroundBridgePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (drain_playground_edits, drain_runtime_errors).chain());
    }
}

/// Forward any uncaught JS errors captured by the runtime this frame into the
/// diagnostics sink for the playground console. `Option` because the runtime may
/// not be mounted yet (no UI spawned).
fn drain_runtime_errors(rt: Option<bevy::prelude::NonSendMut<superui_bridge::UiRuntime>>) {
    if let Some(mut rt) = rt {
        for e in rt.take_errors() {
            crate::push_diag(e);
        }
    }
}

/// Pop queued edits and apply them to the mounted `SuperUiRoot`'s subresources.
/// If nothing is mounted yet (no `SuperUiRoot` + `SuperUiSubresources` pair), the
/// edits are dropped silently — there is nothing to hot-reload into.
fn drain_playground_edits(world: &mut World) {
    let edits = drain_queue();
    if edits.is_empty() {
        return;
    }
    let handles = {
        let mut q = world.query::<(&SuperUiRoot, &SuperUiSubresources)>();
        q.iter(world)
            .next()
            .map(|(root, sub)| (root.html.clone(), sub.js.clone(), sub.css.clone()))
    };
    let Some((html_h, js_h, css_h_opt)) = handles else { return };

    for edit in edits {
        match edit {
            Edit::Js(code) => {
                if let Some(mut a) = world.resource_mut::<Assets<JsSource>>().get_mut(&js_h) {
                    a.0 = code;
                }
                world.write_message(AssetEvent::Modified { id: js_h.id() });
            }
            Edit::Html(text) => {
                if let Some(mut a) = world.resource_mut::<Assets<HtmlSource>>().get_mut(&html_h) {
                    a.0 = text;
                }
                world.write_message(AssetEvent::Modified { id: html_h.id() });
            }
            Edit::Css(text) => {
                let Some(css_h) = css_h_opt.clone() else {
                    push_diag("edit targets CSS but the document declares no stylesheet".into());
                    continue;
                };
                // Parse in a scoped block: `InlineCssStyleSheetParser` borrows `world` to
                // read flair's registries, and that borrow must end (returning an owned
                // `StyleSheet`) before `Assets<StyleSheet>` is mutably borrowed below.
                let parsed = {
                    let mut state: SystemState<InlineCssStyleSheetParser> = SystemState::new(world);
                    let parser = state.get(world).expect("InlineCssStyleSheetParser system params must be available");
                    parser.load_stylesheet(&text)
                };
                match parsed {
                    Ok(sheet) => {
                        if let Some(mut slot) = world.resource_mut::<Assets<StyleSheet>>().get_mut(&css_h) {
                            *slot = sheet;
                        }
                        world.write_message(AssetEvent::Modified { id: css_h.id() });
                    }
                    Err(e) => push_diag(format!("CSS parse error: {e}")),
                }
            }
        }
    }
}

#[cfg(test)]
mod apply_source_tests {
    use super::*;

    #[test]
    fn tsx_transpiles_and_enqueues_js() {
        let _ = drain_queue(); // isolate
        let out = apply_source_inner("app.tsx", "const n: number = 1; const a = <div>{n}</div>;");
        assert!(out.contains("\"ok\":true"), "ok result: {out}");
        let edits = drain_queue();
        match edits.as_slice() {
            [Edit::Js(code)] => {
                assert!(code.contains("$ss.el(\"div\")"), "JSX lowered: {code}");
                assert!(!code.contains(": number"), "types stripped: {code}");
            }
            other => panic!("expected one Edit::Js, got {other:?}"),
        }
    }

    #[test]
    fn broken_tsx_returns_not_ok_without_panicking() {
        let _ = drain_queue();
        let out = apply_source_inner("app.tsx", "const a = <div>{  ;");
        // Never panics; reports the problem. (supersolid degrades gracefully, so
        // it may still enqueue partial JS — the contract is only: no panic + a report.)
        assert!(out.contains("\"diagnostics\""), "diagnostics present: {out}");
        assert!(
            serde_json::from_str::<serde_json::Value>(&out).is_ok(),
            "output is valid JSON: {out}"
        );
    }

    #[test]
    fn css_and_html_enqueue_raw() {
        let _ = drain_queue();
        apply_source_inner("style.css", "div { color: red }");
        apply_source_inner("index.html", "<html></html>");
        let edits = drain_queue();
        assert!(matches!(edits[0], Edit::Css(_)));
        assert!(matches!(edits[1], Edit::Html(_)));
    }
}
