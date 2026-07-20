//! `supersolid_runtime` — the Supersolid reactive core: Solid-like fine-grained
//! signals, effects, memos, lifecycle, and context, authored in JS and run in
//! Boa. Bevy-free and wasm-clean (unlike the `supersolid` transpiler crate, this
//! runs on every target — direction spec §5/§6). Only the author API is published
//! on `globalThis`; the graph internals stay closured.

use superui_js::{BoaEngine, JsEngine};

/// The reactive core, embedded at build time.
const RUNTIME_JS: &str = include_str!("runtime.js");

/// Install the Supersolid reactive core onto `engine`. Call once, after
/// `superui_api::install` and before evaluating author scripts. Publishes
/// `createSignal`/`createEffect`/`createMemo`/`onMount`/`onCleanup`/
/// `createContext`/`useContext` (+ `createRoot`/`untrack`/`batch`) as globals.
pub fn install(engine: &mut BoaEngine) {
    engine
        .eval(RUNTIME_JS)
        .expect("supersolid_runtime: runtime.js must evaluate (internal invariant)");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;
    use superui_dom::Dom;

    fn engine() -> BoaEngine {
        let dom = Rc::new(RefCell::new(Dom::new()));
        let mut e = BoaEngine::new(dom);
        install(&mut e);
        e
    }

    /// Evaluate `expr` and read it back as an f64 (reads `globalThis.*` snapshots).
    fn num(e: &mut BoaEngine, expr: &str) -> f64 {
        e.context_mut()
            .eval(boa_engine::Source::from_bytes(expr))
            .unwrap()
            .as_number()
            .unwrap_or(f64::NAN)
    }

    #[test]
    fn effect_runs_on_creation_and_reruns_on_dependency_change() {
        let mut e = engine();
        e.eval(
            r#"
            globalThis.runs = 0; var last = 0;
            var pair = createSignal(1);
            var count = pair[0], setCount = pair[1];
            createEffect(function () { globalThis.runs++; last = count(); });
            globalThis.runsAfterCreate = globalThis.runs;   // 1
            globalThis.lastAfterCreate = last;              // 1
            setCount(5);
            globalThis.runsAfterSet = globalThis.runs;      // 2
            globalThis.lastAfterSet = last;                 // 5
            "#,
        )
        .unwrap();
        assert_eq!(num(&mut e, "globalThis.runsAfterCreate"), 1.0);
        assert_eq!(num(&mut e, "globalThis.lastAfterCreate"), 1.0);
        assert_eq!(num(&mut e, "globalThis.runsAfterSet"), 2.0);
        assert_eq!(num(&mut e, "globalThis.lastAfterSet"), 5.0);
    }

    #[test]
    fn effect_does_not_rerun_for_unrelated_signal() {
        let mut e = engine();
        e.eval(
            r#"
            globalThis.runs = 0;
            var a = createSignal(0), b = createSignal(0);
            createEffect(function () { globalThis.runs++; a[0](); }); // reads a only
            b[1](99);                                                 // write b
            globalThis.runsAfterUnrelated = globalThis.runs;          // still 1
            a[1](1);                                                  // write a
            globalThis.runsAfterRelated = globalThis.runs;            // 2
            "#,
        )
        .unwrap();
        assert_eq!(num(&mut e, "globalThis.runsAfterUnrelated"), 1.0);
        assert_eq!(num(&mut e, "globalThis.runsAfterRelated"), 2.0);
    }

    #[test]
    fn signal_equals_default_and_updater_form() {
        let mut e = engine();
        e.eval(
            r#"
            globalThis.runs = 0;
            var s = createSignal(1);
            createEffect(function () { globalThis.runs++; s[0](); });
            s[1](1);                              // Object.is equal -> no notify
            globalThis.runsAfterSame = globalThis.runs;   // 1
            s[1](function (prev) { return prev + 1; });   // updater -> 2
            globalThis.updated = s[0]();                  // 2
            globalThis.runsAfterUpdate = globalThis.runs; // 2
            "#,
        )
        .unwrap();
        assert_eq!(num(&mut e, "globalThis.runsAfterSame"), 1.0);
        assert_eq!(num(&mut e, "globalThis.updated"), 2.0);
        assert_eq!(num(&mut e, "globalThis.runsAfterUpdate"), 2.0);
    }

    #[test]
    fn signal_equals_false_always_notifies() {
        let mut e = engine();
        e.eval(
            r#"
            globalThis.runs = 0;
            var s = createSignal(0, { equals: false });
            createEffect(function () { globalThis.runs++; s[0](); });
            s[1](0);   // same value, but equals:false -> notify
            globalThis.runsAfter = globalThis.runs;   // 2
            "#,
        )
        .unwrap();
        assert_eq!(num(&mut e, "globalThis.runsAfter"), 2.0);
    }

    #[test]
    fn untrack_reads_do_not_subscribe() {
        let mut e = engine();
        e.eval(
            r#"
            globalThis.runs = 0;
            var u = createSignal(1);
            createEffect(function () {
                globalThis.runs++;
                untrack(function () { u[0](); });   // read but do not subscribe
            });
            u[1](2);
            globalThis.runsAfter = globalThis.runs;   // still 1
            "#,
        )
        .unwrap();
        assert_eq!(num(&mut e, "globalThis.runsAfter"), 1.0);
    }

    #[test]
    fn batch_coalesces_writes_into_one_effect_run() {
        let mut e = engine();
        e.eval(
            r#"
            globalThis.runs = 0;
            var p = createSignal(0), q = createSignal(0);
            createEffect(function () { globalThis.runs++; p[0](); q[0](); });
            batch(function () { p[1](1); q[1](1); });   // one combined run
            globalThis.runsAfterBatch = globalThis.runs;   // 2
            p[1](2);                                       // outside batch
            globalThis.runsAfterSingle = globalThis.runs;  // 3
            "#,
        )
        .unwrap();
        assert_eq!(num(&mut e, "globalThis.runsAfterBatch"), 2.0);
        assert_eq!(num(&mut e, "globalThis.runsAfterSingle"), 3.0);
    }
}
