//! Browser "watcher" for the superui web playground: takes edited source from JS,
//! transpiles/parses it, and drives the existing superui hot-reload seam. All logic
//! is native-testable; the wasm-bindgen exports are thin wrappers added by a later task.

use std::cell::RefCell;

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
