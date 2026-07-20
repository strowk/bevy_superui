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

    #[test]
    fn memo_is_lazy_then_memoized() {
        let mut e = engine();
        e.eval(
            r#"
            globalThis.memoRuns = 0;
            var x = createSignal(10);
            var m = createMemo(function () { globalThis.memoRuns++; return x[0]() * 2; });
            globalThis.beforeRead = globalThis.memoRuns;   // 0 — lazy, not computed yet
            globalThis.v1 = m();                           // 20 — computes now
            globalThis.afterRead = globalThis.memoRuns;    // 1
            globalThis.v2 = m();                           // 20 — cached
            globalThis.afterRead2 = globalThis.memoRuns;   // 1 — memoized, no recompute
            "#,
        )
        .unwrap();
        assert_eq!(num(&mut e, "globalThis.beforeRead"), 0.0);
        assert_eq!(num(&mut e, "globalThis.v1"), 20.0);
        assert_eq!(num(&mut e, "globalThis.afterRead"), 1.0);
        assert_eq!(num(&mut e, "globalThis.v2"), 20.0);
        assert_eq!(num(&mut e, "globalThis.afterRead2"), 1.0);
    }

    #[test]
    fn memo_value_equality_gates_downstream_effects() {
        let mut e = engine();
        e.eval(
            r#"
            globalThis.effRuns = 0;
            var n = createSignal(4);
            var even = createMemo(function () { return n[0]() % 2 === 0; });
            createEffect(function () { globalThis.effRuns++; even(); });
            globalThis.e0 = globalThis.effRuns;   // 1
            n[1](6);                              // even stays true -> no downstream re-run
            globalThis.e1 = globalThis.effRuns;   // 1
            n[1](7);                              // even flips to false -> re-run
            globalThis.e2 = globalThis.effRuns;   // 2
            "#,
        )
        .unwrap();
        assert_eq!(num(&mut e, "globalThis.e0"), 1.0);
        assert_eq!(num(&mut e, "globalThis.e1"), 1.0);
        assert_eq!(num(&mut e, "globalThis.e2"), 2.0);
    }

    #[test]
    fn diamond_dependency_reruns_effect_exactly_once() {
        let mut e = engine();
        e.eval(
            r#"
            globalThis.dRuns = 0;
            var a = createSignal(1);
            var b = createMemo(function () { return a[0]() * 2; });
            var c = createMemo(function () { return a[0]() + 1; });
            createEffect(function () { globalThis.dRuns++; return b() + c(); });
            globalThis.after1 = globalThis.dRuns;   // 1
            a[1](2);                                // one change to A ...
            globalThis.after2 = globalThis.dRuns;   // ... D runs once, not twice
            "#,
        )
        .unwrap();
        assert_eq!(num(&mut e, "globalThis.after1"), 1.0);
        assert_eq!(num(&mut e, "globalThis.after2"), 2.0);
    }

    #[test]
    fn on_cleanup_runs_before_each_effect_rerun() {
        let mut e = engine();
        e.eval(
            r#"
            globalThis.cleanups = 0;
            var a = createSignal(0);
            createEffect(function () {
                a[0]();
                onCleanup(function () { globalThis.cleanups++; });
            });
            globalThis.c0 = globalThis.cleanups;   // 0 — nothing to clean before first re-run
            a[1](1);
            globalThis.c1 = globalThis.cleanups;   // 1 — prior run's cleanup fired
            a[1](2);
            globalThis.c2 = globalThis.cleanups;   // 2
            "#,
        )
        .unwrap();
        assert_eq!(num(&mut e, "globalThis.c0"), 0.0);
        assert_eq!(num(&mut e, "globalThis.c1"), 1.0);
        assert_eq!(num(&mut e, "globalThis.c2"), 2.0);
    }

    #[test]
    fn create_root_dispose_runs_cleanups_and_stops_effects() {
        let mut e = engine();
        e.eval(
            r#"
            globalThis.rootRuns = 0; globalThis.rootCleanups = 0;
            createRoot(function (dispose) {
                var x = createSignal(0);
                globalThis.setInner = x[1];
                globalThis.disposeRoot = dispose;
                createEffect(function () {
                    globalThis.rootRuns++;
                    x[0]();
                    onCleanup(function () { globalThis.rootCleanups++; });
                });
            });
            globalThis.r0 = globalThis.rootRuns;         // 1
            globalThis.setInner(1);
            globalThis.r1 = globalThis.rootRuns;         // 2
            globalThis.disposeRoot();                    // tear down
            globalThis.rc = globalThis.rootCleanups;     // 2 (re-run cleanup + dispose cleanup)
            globalThis.setInner(2);                      // disposed -> effect must not run
            globalThis.r2 = globalThis.rootRuns;         // still 2
            "#,
        )
        .unwrap();
        assert_eq!(num(&mut e, "globalThis.r0"), 1.0);
        assert_eq!(num(&mut e, "globalThis.r1"), 2.0);
        assert_eq!(num(&mut e, "globalThis.rc"), 2.0);
        assert_eq!(num(&mut e, "globalThis.r2"), 2.0);
    }
}
