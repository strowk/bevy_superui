//! `supersolid_runtime` — the Supersolid reactive core: Solid-like fine-grained
//! signals, effects, memos, lifecycle, and context, authored in JS and run in
//! Boa. Bevy-free and wasm-clean (unlike the `supersolid` transpiler crate, this
//! runs on every target — direction spec §5/§6). Only the author API is published
//! on `globalThis`; the graph internals stay closured.

use superui_js::{BoaEngine, JsEngine};

/// The reactive core, embedded at build time.
const RUNTIME_JS: &str = include_str!("runtime.js");
/// The render + control-flow layer, embedded at build time.
const RENDER_JS: &str = include_str!("render.js");

/// Install the Supersolid reactive core onto `engine`. Call once, after
/// `superui_api::install` and before evaluating author scripts. Publishes
/// `createSignal`/`createEffect`/`createMemo`/`onMount`/`onCleanup`/
/// `createContext`/`useContext` (+ `createRoot`/`untrack`/`batch`) as globals,
/// plus `$ss` (`el`/`txt`/`attr`/`child`/`on`/`bind`/`insert`/`cmp`/`frag`)
/// from the render layer, and author globals `render`/`Show`/`For`/`Index`/
/// `Switch`/`Match`.
pub fn install(engine: &mut BoaEngine) {
    engine
        .eval(RUNTIME_JS)
        .expect("supersolid_runtime: runtime.js must evaluate (internal invariant)");
    engine
        .eval(RENDER_JS)
        .expect("supersolid_runtime: render.js must evaluate (internal invariant)");
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

    /// Evaluate `expr` and read it back as a Rust String.
    fn text(e: &mut BoaEngine, expr: &str) -> String {
        let v = e
            .context_mut()
            .eval(boa_engine::Source::from_bytes(expr))
            .unwrap();
        v.to_string(e.context_mut()).unwrap().to_std_string_escaped()
    }

    #[test]
    fn on_signal_hook_receives_created_signals() {
        let mut e = engine();
        e.eval(
            r#"
            globalThis.collected = [];
            globalThis.$ssOnSignal = function (read, write) {
                globalThis.collected.push([read, write]);
            };
            var a = createSignal(1);
            var b = createSignal(2);
            globalThis.n = globalThis.collected.length;      // 2
            globalThis.v0 = globalThis.collected[0][0]();    // read a -> 1
            globalThis.collected[1][1](9);                   // write b -> 9
            globalThis.v1 = b[0]();                          // 9
            "#,
        )
        .unwrap();
        assert_eq!(num(&mut e, "globalThis.n"), 2.0);
        assert_eq!(num(&mut e, "globalThis.v0"), 1.0);
        assert_eq!(num(&mut e, "globalThis.v1"), 9.0);
    }

    #[test]
    fn on_signal_hook_absent_is_a_noop() {
        let mut e = engine();
        e.eval(r#"var s = createSignal(5); globalThis.v = s[0]();"#).unwrap();
        assert_eq!(num(&mut e, "globalThis.v"), 5.0);
    }

    #[test]
    fn on_mount_runs_exactly_once() {
        let mut e = engine();
        e.eval(
            r#"
            globalThis.mounts = 0;
            var m = createSignal(0);
            createRoot(function () {
                onMount(function () { globalThis.mounts++; m[0](); }); // reads m, but untracked
            });
            globalThis.m0 = globalThis.mounts;   // 1
            m[1](1);                             // one-shot + untracked -> no re-run
            globalThis.m1 = globalThis.mounts;   // 1
            "#,
        )
        .unwrap();
        assert_eq!(num(&mut e, "globalThis.m0"), 1.0);
        assert_eq!(num(&mut e, "globalThis.m1"), 1.0);
    }

    #[test]
    fn context_default_provided_and_nested() {
        let mut e = engine();
        e.eval(
            r#"
            var Ctx = createContext("default");
            globalThis.d0 = useContext(Ctx);     // "default" (no provider)
            globalThis.prov = null; globalThis.nested = null; globalThis.effCtx = null;
            $ssProvideContext(Ctx, "outer", function () {
                globalThis.prov = useContext(Ctx);          // "outer"
                $ssProvideContext(Ctx, "inner", function () {
                    globalThis.nested = useContext(Ctx);    // "inner"
                });
                createRoot(function () {
                    createEffect(function () { globalThis.effCtx = useContext(Ctx); }); // "outer"
                });
            });
            "#,
        )
        .unwrap();
        assert_eq!(text(&mut e, "globalThis.d0"), "default");
        assert_eq!(text(&mut e, "globalThis.prov"), "outer");
        assert_eq!(text(&mut e, "globalThis.nested"), "inner");
        assert_eq!(text(&mut e, "globalThis.effCtx"), "outer");
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

    #[test]
    fn self_writing_effect_converges_like_solid() {
        let mut e = engine();
        e.eval(
            r#"
            globalThis.runs = 0;
            var c = createSignal(0);
            createEffect(function () {
                globalThis.runs++;
                var v = c[0]();
                if (v < 3) c[1](v + 1);   // self-correcting: climb to 3
            });
            globalThis.finalVal = c[0]();          // 3
            globalThis.runCount = globalThis.runs; // 4 (v=0,1,2,3)
            "#,
        )
        .unwrap();
        assert_eq!(num(&mut e, "globalThis.finalVal"), 3.0);
        assert_eq!(num(&mut e, "globalThis.runCount"), 4.0);
    }

    #[test]
    fn effect_reading_memo_cascade_runs_once_and_is_glitch_free() {
        let mut e = engine();
        e.eval(
            r#"
            var a = createSignal(1);
            var b = createMemo(function () { return a[0]() * 10; }); // derive via MEMO
            globalThis.runs = 0; globalThis.observed = [];
            createEffect(function () {
                globalThis.runs++;
                globalThis.observed.push(a[0]() + "," + b()); // reads a AND derived b
            });
            globalThis.r0 = globalThis.runs;             // 1
            globalThis.first = globalThis.observed[0];   // "1,10"
            a[1](2);
            globalThis.r1 = globalThis.runs;             // 2 (once, glitch-free)
            globalThis.second = globalThis.observed[globalThis.observed.length - 1]; // "2,20"
            "#,
        )
        .unwrap();
        assert_eq!(num(&mut e, "globalThis.r0"), 1.0);
        assert_eq!(text(&mut e, "globalThis.first"), "1,10");
        assert_eq!(num(&mut e, "globalThis.r1"), 2.0);
        assert_eq!(text(&mut e, "globalThis.second"), "2,20");
    }

    #[test]
    fn nested_effect_is_disposed_and_recreated_across_owner_reruns() {
        let mut e = engine();
        e.eval(
            r#"
            globalThis.innerRuns = 0; globalThis.innerCleanups = 0;
            var show = createSignal(true);
            var val = createSignal(0);
            createRoot(function () {
                createEffect(function () {            // outer owns a conditional inner effect
                    if (show[0]()) {
                        createEffect(function () {
                            globalThis.innerRuns++;
                            val[0]();
                            onCleanup(function () { globalThis.innerCleanups++; });
                        });
                    }
                });
            });
            globalThis.i0 = globalThis.innerRuns;        // 1
            val[1](1);                                    // inner re-runs
            globalThis.i1 = globalThis.innerRuns;        // 2
            show[1](false);                               // outer re-runs -> disposes inner
            globalThis.c1 = globalThis.innerCleanups;    // 2
            val[1](2);                                     // inner gone -> no run
            globalThis.i2 = globalThis.innerRuns;        // still 2
            "#,
        )
        .unwrap();
        assert_eq!(num(&mut e, "globalThis.i0"), 1.0);
        assert_eq!(num(&mut e, "globalThis.i1"), 2.0);
        assert_eq!(num(&mut e, "globalThis.i2"), 2.0);
        assert_eq!(num(&mut e, "globalThis.c1"), 2.0);
    }

    #[test]
    fn create_root_with_explicit_owner_survives_a_sibling_scope_disposal() {
        let mut e = engine();
        e.eval(
            r#"
            globalThis.cleaned = 0;
            globalThis.host = null;
            // An outer root whose owner we capture and reuse for a detached child.
            createRoot(function (disposeOuter) {
                globalThis.host = $ssGetOwner();     // capture the outer owner
                globalThis.disposeOuter = disposeOuter;
            });
            // A throwaway scope: create a child root attached to `host`, NOT to this scope.
            createRoot(function (disposeThrow) {
                createRoot(function () {
                    onCleanup(function () { globalThis.cleaned++; });
                }, globalThis.host);                 // detached owner = host
                globalThis.disposeThrow = disposeThrow;
            });
            globalThis.disposeThrow();               // dispose the throwaway scope
            globalThis.afterThrow = globalThis.cleaned;   // 0 — child is owned by host, not the throwaway
            globalThis.disposeOuter();               // dispose host
            globalThis.afterOuter = globalThis.cleaned;   // 1
            "#,
        )
        .unwrap();
        assert_eq!(num(&mut e, "globalThis.afterThrow"), 0.0);
        assert_eq!(num(&mut e, "globalThis.afterOuter"), 1.0);
    }

    #[test]
    fn memo_of_memo_chain_propagates() {
        let mut e = engine();
        e.eval(
            r#"
            var a = createSignal(2);
            var b = createMemo(function () { return a[0]() + 1; });  // 3
            var c = createMemo(function () { return b() * 2; });     // 6
            globalThis.runs = 0;
            createEffect(function () { globalThis.runs++; globalThis.last = c(); });
            globalThis.r0 = globalThis.runs; globalThis.v0 = globalThis.last;  // 1, 6
            a[1](5);                                                          // b=6, c=12
            globalThis.r1 = globalThis.runs; globalThis.v1 = globalThis.last;  // 2, 12
            "#,
        )
        .unwrap();
        assert_eq!(num(&mut e, "globalThis.r0"), 1.0);
        assert_eq!(num(&mut e, "globalThis.v0"), 6.0);
        assert_eq!(num(&mut e, "globalThis.r1"), 2.0);
        assert_eq!(num(&mut e, "globalThis.v1"), 12.0);
    }
}

#[cfg(test)]
mod render_tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;
    use superui_dom::Dom;

    /// A BoaEngine with the DOM/Web API (superui_api) AND the reactive+render
    /// runtime installed — the full surface author `.tsx` runs against.
    fn render_engine() -> BoaEngine {
        let dom = Rc::new(RefCell::new(Dom::new()));
        let mut e = BoaEngine::new(dom);
        superui_api::install(&mut e);
        install(&mut e);
        e
    }

    /// Like `render_engine` but also returns the shared `Dom` so tests can
    /// resolve `NodeId`s (e.g. to call `BoaEngine::dispatch_event`).
    fn render_engine_with_dom() -> (BoaEngine, Rc<RefCell<Dom>>) {
        let dom = Rc::new(RefCell::new(Dom::new()));
        let mut e = BoaEngine::new(dom.clone());
        superui_api::install(&mut e);
        install(&mut e);
        (e, dom)
    }

    fn num(e: &mut BoaEngine, expr: &str) -> f64 {
        e.context_mut()
            .eval(boa_engine::Source::from_bytes(expr))
            .unwrap()
            .as_number()
            .unwrap_or(f64::NAN)
    }

    fn text(e: &mut BoaEngine, expr: &str) -> String {
        let v = e
            .context_mut()
            .eval(boa_engine::Source::from_bytes(expr))
            .unwrap();
        v.to_string(e.context_mut()).unwrap().to_std_string_escaped()
    }

    #[test]
    fn el_and_txt_and_child_build_dom() {
        let mut e = render_engine();
        e.eval(
            r#"
            globalThis.p = $ss.el("div");
            $ss.child(p, $ss.el("span"));
            $ss.child(p, $ss.txt("hi"));
            globalThis.count = p.childNodes.length;   // 2
            globalThis.tag0 = p.childNodes[0].tagName; // "SPAN"
            globalThis.txt1 = p.childNodes[1].data;    // "hi"
            "#,
        )
        .unwrap();
        assert_eq!(num(&mut e, "globalThis.count"), 2.0);
        assert_eq!(text(&mut e, "globalThis.tag0"), "SPAN");
        assert_eq!(text(&mut e, "globalThis.txt1"), "hi");
    }

    #[test]
    fn attr_sets_attribute_and_value_property() {
        let mut e = render_engine();
        e.eval(
            r#"
            globalThis.a = $ss.el("div");
            $ss.attr(a, "class", "box");
            globalThis.b = $ss.el("input");
            $ss.attr(b, "value", "typed");
            globalThis.cls = a.getAttribute ? a.getAttribute("class") : a.className;
            globalThis.val = b.value;   // property path
            "#,
        )
        .unwrap();
        // `class` reaches the class attribute (read back via className accessor).
        assert_eq!(text(&mut e, "globalThis.a.className"), "box");
        assert_eq!(text(&mut e, "globalThis.val"), "typed");
    }

    #[test]
    fn child_flattens_arrays() {
        let mut e = render_engine();
        e.eval(
            r#"
            globalThis.p = $ss.el("div");
            $ss.child(p, [ $ss.el("span"), $ss.txt("x") ]);
            globalThis.count = p.childNodes.length;   // 2
            "#,
        )
        .unwrap();
        assert_eq!(num(&mut e, "globalThis.count"), 2.0);
    }

    #[test]
    fn bind_updates_attribute_reactively() {
        let mut e = render_engine();
        e.eval(
            r#"
            var pair = createSignal("a");
            globalThis.get = pair[0]; globalThis.set = pair[1];
            globalThis.el = $ss.el("div");
            $ss.bind(el, "class", function () { return globalThis.get(); });
            globalThis.c0 = el.className;   // "a" — effect ran once on bind
            globalThis.set("b");
            globalThis.c1 = el.className;   // "b" — surgical re-run
            "#,
        )
        .unwrap();
        assert_eq!(text(&mut e, "globalThis.c0"), "a");
        assert_eq!(text(&mut e, "globalThis.c1"), "b");
    }

    #[test]
    fn insert_text_updates_in_place() {
        let mut e = render_engine();
        e.eval(
            r#"
            var pair = createSignal(1);
            globalThis.get = pair[0]; globalThis.set = pair[1];
            globalThis.p = $ss.el("div");
            $ss.child(p, $ss.txt("n="));
            $ss.insert(p, function () { return globalThis.get(); });
            globalThis.t0 = p.textContent;               // "n=1"
            globalThis.firstText = p.childNodes[1];      // the inserted text node
            globalThis.set(2);
            globalThis.t1 = p.textContent;               // "n=2"
            globalThis.sameNode = (p.childNodes[1] === globalThis.firstText); // true — surgical
            "#,
        )
        .unwrap();
        assert_eq!(text(&mut e, "globalThis.t0"), "n=1");
        assert_eq!(text(&mut e, "globalThis.t1"), "n=2");
        assert_eq!(text(&mut e, "globalThis.sameNode"), "true");
    }

    #[test]
    fn insert_null_renders_nothing_and_toggles_a_node() {
        let mut e = render_engine();
        e.eval(
            r#"
            var pair = createSignal(false);
            globalThis.get = pair[0]; globalThis.set = pair[1];
            globalThis.span = $ss.el("span");
            $ss.child(span, $ss.txt("shown"));
            globalThis.p = $ss.el("div");
            $ss.insert(p, function () { return globalThis.get() ? globalThis.span : null; });
            globalThis.t0 = p.textContent;   // "" — false renders nothing
            globalThis.set(true);
            globalThis.t1 = p.textContent;   // "shown"
            globalThis.set(false);
            globalThis.t2 = p.textContent;   // "" again
            "#,
        )
        .unwrap();
        assert_eq!(text(&mut e, "globalThis.t0"), "");
        assert_eq!(text(&mut e, "globalThis.t1"), "shown");
        assert_eq!(text(&mut e, "globalThis.t2"), "");
    }

    #[test]
    fn cmp_runs_component_once_and_inserts_its_nodes() {
        let mut e = render_engine();
        e.eval(
            r#"
            globalThis.calls = 0;
            function Box(props) {
                globalThis.calls++;
                var d = $ss.el("div");
                $ss.child(d, $ss.txt("boxed:"));
                $ss.insert(d, function () { return props.label; });
                return d;
            }
            var pair = createSignal("x");
            globalThis.set = pair[1];
            globalThis.p = $ss.el("main");
            $ss.insert(p, function () {
                return $ss.cmp(Box, { get label() { return pair[0](); } });
            });
            globalThis.t0 = p.textContent;   // "boxed:x"
            globalThis.callsAfter = globalThis.calls; // 1
            globalThis.set("y");
            globalThis.t1 = p.textContent;   // "boxed:y" — inner insert re-ran, body did not
            globalThis.callsAfter2 = globalThis.calls; // still 1 (runs once)
            "#,
        )
        .unwrap();
        assert_eq!(text(&mut e, "globalThis.t0"), "boxed:x");
        assert_eq!(num(&mut e, "globalThis.callsAfter"), 1.0);
        assert_eq!(text(&mut e, "globalThis.t1"), "boxed:y");
        assert_eq!(num(&mut e, "globalThis.callsAfter2"), 1.0);
    }

    #[test]
    fn frag_and_insert_flatten_into_the_parent() {
        let mut e = render_engine();
        e.eval(
            r#"
            globalThis.p = $ss.el("div");
            $ss.insert(p, function () {
                return $ss.frag([ $ss.el("a"), $ss.txt("mid"), $ss.el("b") ]);
            });
            globalThis.count = p.childNodes.length;  // 3 content + 1 anchor = 4
            globalThis.first = p.childNodes[0].tagName;  // "A"
            globalThis.mid = p.childNodes[1].data;       // "mid"
            globalThis.last = p.childNodes[2].tagName;   // "B"
            "#,
        )
        .unwrap();
        assert_eq!(num(&mut e, "globalThis.count"), 4.0);
        assert_eq!(text(&mut e, "globalThis.first"), "A");
        assert_eq!(text(&mut e, "globalThis.mid"), "mid");
        assert_eq!(text(&mut e, "globalThis.last"), "B");
    }

    #[test]
    fn on_fires_a_registered_click_handler() {
        // Events dispatch from the Rust side (BoaEngine::dispatch_event), not from JS.
        // Attach the button to the document so we can resolve its NodeId and dispatch.
        let (mut e, dom) = render_engine_with_dom();
        e.eval(
            r#"
            globalThis.clicks = 0;
            var b = $ss.el("button");
            b.setAttribute("id", "btn");
            document.appendChild(b);
            $ss.on(b, "click", function () { globalThis.clicks++; });
            "#,
        )
        .unwrap();
        let btn = { let d = dom.borrow(); d.get_element_by_id("btn").unwrap() };
        e.dispatch_event(btn, "click", None, true, true);
        assert_eq!(num(&mut e, "globalThis.clicks"), 1.0);
    }

    #[test]
    fn insert_array_reorders_reusing_nodes() {
        let mut e = render_engine();
        e.eval(
            r#"
            // Build three stable element nodes keyed by identity.
            globalThis.A = $ss.el("i"); A.setAttribute("k", "A");
            globalThis.B = $ss.el("i"); B.setAttribute("k", "B");
            globalThis.C = $ss.el("i"); C.setAttribute("k", "C");
            var pair = createSignal([A, B, C]);
            globalThis.set = pair[1];
            globalThis.p = $ss.el("div");
            $ss.insert(p, function () { return pair[0](); });
            function order() {
                var s = "";
                for (var i = 0; i < p.childNodes.length; i++) {
                    var n = p.childNodes[i];
                    if (n.getAttribute) { var k = n.getAttribute("k"); if (k) s += k; }
                }
                return s;
            }
            globalThis.o0 = order();          // "ABC"
            globalThis.set([C, A, B]);        // rotate
            globalThis.o1 = order();          // "CAB"
            globalThis.reusedA = (p.childNodes[1] === A); // A reused (identity)
            globalThis.set([B]);              // shrink, drop A and C
            globalThis.o2 = order();          // "B"
            "#,
        )
        .unwrap();
        assert_eq!(text(&mut e, "globalThis.o0"), "ABC");
        assert_eq!(text(&mut e, "globalThis.o1"), "CAB");
        assert_eq!(text(&mut e, "globalThis.reusedA"), "true");
        assert_eq!(text(&mut e, "globalThis.o2"), "B");
    }

    #[test]
    fn insert_array_full_reversal_reuses_all_nodes() {
        let mut e = render_engine();
        e.eval(
            r#"
            // Build four stable element nodes keyed by identity.
            globalThis.A = $ss.el("i"); A.setAttribute("k", "A");
            globalThis.B = $ss.el("i"); B.setAttribute("k", "B");
            globalThis.C = $ss.el("i"); C.setAttribute("k", "C");
            globalThis.D = $ss.el("i"); D.setAttribute("k", "D");
            var pair = createSignal([A, B, C, D]);
            globalThis.set = pair[1];
            globalThis.p = $ss.el("div");
            $ss.insert(p, function () { return pair[0](); });
            function order() {
                var s = "";
                for (var i = 0; i < p.childNodes.length; i++) {
                    var n = p.childNodes[i];
                    if (n.getAttribute) { var k = n.getAttribute("k"); if (k) s += k; }
                }
                return s;
            }
            globalThis.o0 = order();          // "ABCD"
            globalThis.set([D, C, B, A]);     // full reversal
            globalThis.o1 = order();          // "DCBA"
            // A is now at the last keyed position; D is at the first.
            // Both original node objects must still be present (reused by identity).
            globalThis.reusedA = (A.parentNode === p);
            globalThis.reusedD = (D.parentNode === p);
            "#,
        )
        .unwrap();
        assert_eq!(text(&mut e, "globalThis.o0"), "ABCD");
        assert_eq!(text(&mut e, "globalThis.o1"), "DCBA");
        assert_eq!(text(&mut e, "globalThis.reusedA"), "true");
        assert_eq!(text(&mut e, "globalThis.reusedD"), "true");
    }

    #[test]
    fn insert_array_appends_and_prepends() {
        let mut e = render_engine();
        e.eval(
            r#"
            globalThis.A = $ss.el("i"); A.setAttribute("k","A");
            globalThis.B = $ss.el("i"); B.setAttribute("k","B");
            globalThis.C = $ss.el("i"); C.setAttribute("k","C");
            var pair = createSignal([B]);
            globalThis.set = pair[1];
            globalThis.p = $ss.el("div");
            $ss.insert(p, function () { return pair[0](); });
            function order() {
                var s = "";
                for (var i=0;i<p.childNodes.length;i++){var n=p.childNodes[i];if(n.getAttribute){var k=n.getAttribute("k");if(k)s+=k;}}
                return s;
            }
            globalThis.o0 = order();      // "B"
            globalThis.set([A, B]);       // prepend A
            globalThis.o1 = order();      // "AB"
            globalThis.set([A, B, C]);    // append C
            globalThis.o2 = order();      // "ABC"
            "#,
        )
        .unwrap();
        assert_eq!(text(&mut e, "globalThis.o0"), "B");
        assert_eq!(text(&mut e, "globalThis.o1"), "AB");
        assert_eq!(text(&mut e, "globalThis.o2"), "ABC");
    }

    #[test]
    fn show_toggles_children_and_fallback() {
        let mut e = render_engine();
        e.eval(
            r#"
            var pair = createSignal(false);
            globalThis.set = pair[1];
            globalThis.p = $ss.el("div");
            $ss.insert(p, function () {
                return $ss.cmp(Show, {
                    get when() { return pair[0](); },
                    get children() { var s = $ss.el("span"); $ss.child(s, $ss.txt("yes")); return s; },
                    get fallback() { var f = $ss.el("em"); $ss.child(f, $ss.txt("no")); return f; },
                });
            });
            globalThis.t0 = p.textContent;   // "no"
            globalThis.set(true);
            globalThis.t1 = p.textContent;   // "yes"
            globalThis.set(false);
            globalThis.t2 = p.textContent;   // "no"
            "#,
        )
        .unwrap();
        assert_eq!(text(&mut e, "globalThis.t0"), "no");
        assert_eq!(text(&mut e, "globalThis.t1"), "yes");
        assert_eq!(text(&mut e, "globalThis.t2"), "no");
    }

    #[test]
    fn show_disposes_hidden_branch_effects() {
        let mut e = render_engine();
        e.eval(
            r#"
            globalThis.binds = 0;
            var when = createSignal(true);
            var label = createSignal("a");
            globalThis.setWhen = when[1]; globalThis.setLabel = label[1];
            globalThis.p = $ss.el("div");
            $ss.insert(p, function () {
                return $ss.cmp(Show, {
                    get when() { return when[0](); },
                    get children() {
                        var s = $ss.el("span");
                        $ss.bind(s, "class", function () { globalThis.binds++; return label[0](); });
                        return s;
                    },
                });
            });
            globalThis.b0 = globalThis.binds;     // 1 — bind ran once while shown
            globalThis.setWhen(false);            // hide: branch (and its effect) disposed
            globalThis.setLabel("b");             // must NOT re-run the disposed bind
            globalThis.b1 = globalThis.binds;     // still 1
            "#,
        )
        .unwrap();
        assert_eq!(num(&mut e, "globalThis.b0"), 1.0);
        assert_eq!(num(&mut e, "globalThis.b1"), 1.0);
    }

    #[test]
    fn for_renders_and_reorders_keyed_rows() {
        let mut e = render_engine();
        e.eval(
            r#"
            // Items are objects (identity keys).
            globalThis.a = { n: "a" }; globalThis.b = { n: "b" }; globalThis.c = { n: "c" };
            var pair = createSignal([globalThis.a, globalThis.b, globalThis.c]);
            globalThis.set = pair[1];
            globalThis.p = $ss.el("ul");
            $ss.insert(p, function () {
                return $ss.cmp(For, {
                    get each() { return pair[0](); },
                    get children() {
                        return function (item) {
                            var li = $ss.el("li");
                            $ss.child(li, $ss.txt(item.n));
                            return li;
                        };
                    },
                });
            });
            function order() {
                var s = "";
                for (var i=0;i<p.childNodes.length;i++){var n=p.childNodes[i];if(n.nodeType===1)s+=n.textContent;}
                return s;
            }
            globalThis.rowA = p.childNodes[0]; // <li>a</li>
            globalThis.o0 = order();           // "abc"
            globalThis.set([globalThis.c, globalThis.a, globalThis.b]);
            globalThis.o1 = order();           // "cab"
            globalThis.reusedA = (p.childNodes[1] === globalThis.rowA); // true — same <li> reused
            "#,
        )
        .unwrap();
        assert_eq!(text(&mut e, "globalThis.o0"), "abc");
        assert_eq!(text(&mut e, "globalThis.o1"), "cab");
        assert_eq!(text(&mut e, "globalThis.reusedA"), "true");
    }

    #[test]
    fn for_preserves_per_row_state_across_list_change() {
        let mut e = render_engine();
        e.eval(
            r#"
            globalThis.a = { n: "a" }; globalThis.b = { n: "b" };
            var pair = createSignal([globalThis.a, globalThis.b]);
            globalThis.set = pair[1];
            globalThis.p = $ss.el("ul");
            // Each row owns a private counter signal; reused rows must keep it.
            $ss.insert(p, function () {
                return $ss.cmp(For, {
                    get each() { return pair[0](); },
                    get children() {
                        return function (item) {
                            var c = createSignal(0);
                            item.inc = c[1]; item.read = c[0];
                            var li = $ss.el("li");
                            $ss.insert(li, function () { return c[0](); });
                            return li;
                        };
                    },
                });
            });
            globalThis.a.inc(5);                 // bump row a's private state
            globalThis.set([globalThis.b, globalThis.a]); // reorder (row a retained)
            globalThis.aState = globalThis.a.read();      // 5 — state preserved
            "#,
        )
        .unwrap();
        assert_eq!(num(&mut e, "globalThis.aState"), 5.0);
    }

    #[test]
    fn index_keys_by_position_and_updates_item_in_place() {
        let mut e = render_engine();
        e.eval(
            r#"
            var pair = createSignal(["x", "y"]);
            globalThis.set = pair[1];
            globalThis.p = $ss.el("ul");
            $ss.insert(p, function () {
                return $ss.cmp(Index, {
                    get each() { return pair[0](); },
                    get children() {
                        return function (item) {   // item is a SIGNAL getter
                            var li = $ss.el("li");
                            $ss.insert(li, function () { return item(); });
                            return li;
                        };
                    },
                });
            });
            function order(){var s="";for(var i=0;i<p.childNodes.length;i++){var n=p.childNodes[i];if(n.nodeType===1)s+=n.textContent;}return s;}
            globalThis.row0 = p.childNodes[0];
            globalThis.o0 = order();            // "xy"
            globalThis.set(["z", "y"]);         // position 0 value changes x->z
            globalThis.o1 = order();            // "zy"
            globalThis.sameRow0 = (p.childNodes[0] === globalThis.row0); // true — position reused
            "#,
        )
        .unwrap();
        assert_eq!(text(&mut e, "globalThis.o0"), "xy");
        assert_eq!(text(&mut e, "globalThis.o1"), "zy");
        assert_eq!(text(&mut e, "globalThis.sameRow0"), "true");
    }

    #[test]
    fn index_grows_when_list_lengthens() {
        let mut e = render_engine();
        e.eval(
            r#"
            var pair = createSignal(["x"]);
            globalThis.set = pair[1];
            globalThis.p = $ss.el("ul");
            $ss.insert(p, function () {
                return $ss.cmp(Index, {
                    get each() { return pair[0](); },
                    get children() {
                        return function (item) {
                            var li = $ss.el("li");
                            $ss.insert(li, function () { return item(); });
                            return li;
                        };
                    },
                });
            });
            function order() {
                var s = "";
                for (var i = 0; i < p.childNodes.length; i++) {
                    var n = p.childNodes[i];
                    if (n.nodeType === 1) s += n.textContent;
                }
                return s;
            }
            function elemCount() {
                var c = 0;
                for (var i = 0; i < p.childNodes.length; i++) {
                    if (p.childNodes[i].nodeType === 1) c++;
                }
                return c;
            }
            globalThis.o0 = order();          // "x"
            globalThis.c0 = elemCount();      // 1
            globalThis.set(["x", "y", "z"]); // grow from 1 to 3
            globalThis.o1 = order();          // "xyz"
            globalThis.c1 = elemCount();      // 3
            "#,
        )
        .unwrap();
        assert_eq!(text(&mut e, "globalThis.o0"), "x");
        assert_eq!(num(&mut e, "globalThis.c0"), 1.0);
        assert_eq!(text(&mut e, "globalThis.o1"), "xyz");
        assert_eq!(num(&mut e, "globalThis.c1"), 3.0);
    }

    #[test]
    fn index_shrinks_when_list_shortens() {
        let mut e = render_engine();
        e.eval(
            r#"
            var pair = createSignal(["a", "b", "c"]);
            globalThis.set = pair[1];
            globalThis.p = $ss.el("ul");
            $ss.insert(p, function () {
                return $ss.cmp(Index, {
                    get each() { return pair[0](); },
                    get children() {
                        return function (item) {
                            var li = $ss.el("li");
                            $ss.insert(li, function () { return item(); });
                            return li;
                        };
                    },
                });
            });
            function order() {
                var s = "";
                for (var i = 0; i < p.childNodes.length; i++) {
                    var n = p.childNodes[i];
                    if (n.nodeType === 1) s += n.textContent;
                }
                return s;
            }
            function elemCount() {
                var c = 0;
                for (var i = 0; i < p.childNodes.length; i++) {
                    if (p.childNodes[i].nodeType === 1) c++;
                }
                return c;
            }
            globalThis.o0 = order();     // "abc"
            globalThis.c0 = elemCount(); // 3
            globalThis.set(["a"]);       // shrink from 3 to 1: dispose and remove trailing rows
            globalThis.o1 = order();     // "a"
            globalThis.c1 = elemCount(); // 1
            "#,
        )
        .unwrap();
        assert_eq!(text(&mut e, "globalThis.o0"), "abc");
        assert_eq!(num(&mut e, "globalThis.c0"), 3.0);
        assert_eq!(text(&mut e, "globalThis.o1"), "a");
        assert_eq!(num(&mut e, "globalThis.c1"), 1.0);
    }

    #[test]
    fn keyed_reuses_rows_updates_fields_and_adds_removes() {
        let mut e = render_engine();
        e.eval(
            r#"
            var pair = createSignal([{ id: 1, v: "a" }, { id: 2, v: "b" }]);
            globalThis.set = pair[1];
            globalThis.p = $ss.el("div");
            $ss.insert(p, function () {
                return $ss.cmp(Keyed, {
                    get each() { return pair[0](); },
                    by: "id",
                    get children() {
                        return function (row) {
                            var s = $ss.el("span");
                            s.setAttribute("k", "" + row.id);
                            $ss.insert(s, function () { return row.v; });   // fine-grained field read
                            return s;
                        };
                    },
                });
            });
            function order() {
                var out = "";
                for (var i = 0; i < p.childNodes.length; i++) {
                    var n = p.childNodes[i];
                    if (n.getAttribute && n.getAttribute("k")) out += n.textContent;
                }
                return out;
            }
            globalThis.o0 = order();                 // "ab"
            globalThis.rowA = p.childNodes[0];        // span for id 1
            globalThis.set([{ id: 1, v: "A" }, { id: 2, v: "b" }]);   // update id1's field
            globalThis.o1 = order();                 // "Ab"
            globalThis.sameA = (p.childNodes[0] === globalThis.rowA); // true — row reused in place
            globalThis.set([{ id: 2, v: "b" }]);      // remove id1
            globalThis.o2 = order();                 // "b"
            globalThis.set([{ id: 2, v: "b" }, { id: 3, v: "c" }]);   // add id3
            globalThis.o3 = order();                 // "bc"
            "#,
        )
        .unwrap();
        assert_eq!(text(&mut e, "globalThis.o0"), "ab");
        assert_eq!(text(&mut e, "globalThis.o1"), "Ab");
        assert_eq!(text(&mut e, "globalThis.sameA"), "true");
        assert_eq!(text(&mut e, "globalThis.o2"), "b");
        assert_eq!(text(&mut e, "globalThis.o3"), "bc");
    }

    #[test]
    fn keyed_updates_only_the_changed_rows_binding() {
        let mut e = render_engine();
        e.eval(
            r#"
            var pair = createSignal([{ id: 1, x: 0 }, { id: 2, x: 0 }]);
            globalThis.set = pair[1];
            globalThis.runs = {};   // id -> style-binding run count
            globalThis.p = $ss.el("div");
            $ss.insert(p, function () {
                return $ss.cmp(Keyed, {
                    get each() { return pair[0](); },
                    by: "id",
                    get children() {
                        return function (row) {
                            var d = $ss.el("div");
                            $ss.bind(d, "style", function () {
                                var id = row.id;
                                globalThis.runs[id] = (globalThis.runs[id] || 0) + 1;
                                return "left:" + row.x + "px";
                            });
                            return d;
                        };
                    },
                });
            });
            globalThis.r1_0 = globalThis.runs[1]; globalThis.r2_0 = globalThis.runs[2]; // 1,1
            globalThis.set([{ id: 1, x: 5 }, { id: 2, x: 0 }]);   // only id1's x changes
            globalThis.r1_1 = globalThis.runs[1];   // 2 — id1 binding re-ran
            globalThis.r2_1 = globalThis.runs[2];   // 1 — id2 binding did NOT re-run
            "#,
        )
        .unwrap();
        assert_eq!(num(&mut e, "globalThis.r1_0"), 1.0);
        assert_eq!(num(&mut e, "globalThis.r2_0"), 1.0);
        assert_eq!(num(&mut e, "globalThis.r1_1"), 2.0);
        assert_eq!(num(&mut e, "globalThis.r2_1"), 1.0);
    }

    #[test]
    fn switch_picks_first_matching_branch() {
        let mut e = render_engine();
        e.eval(
            r#"
            var pair = createSignal(1);
            globalThis.set = pair[1];
            globalThis.p = $ss.el("div");
            $ss.insert(p, function () {
                return $ss.cmp(Switch, {
                    get fallback() { var f=$ss.el("em"); $ss.child(f,$ss.txt("none")); return f; },
                    get children() {
                        return [
                            $ss.cmp(Match, { get when(){ return pair[0]() === 1; },
                                get children(){ var s=$ss.el("span"); $ss.child(s,$ss.txt("one")); return s; } }),
                            $ss.cmp(Match, { get when(){ return pair[0]() === 2; },
                                get children(){ var s=$ss.el("span"); $ss.child(s,$ss.txt("two")); return s; } }),
                        ];
                    },
                });
            });
            globalThis.t0 = p.textContent;   // "one"
            globalThis.set(2);
            globalThis.t1 = p.textContent;   // "two"
            globalThis.set(9);
            globalThis.t2 = p.textContent;   // "none" (fallback)
            "#,
        )
        .unwrap();
        assert_eq!(text(&mut e, "globalThis.t0"), "one");
        assert_eq!(text(&mut e, "globalThis.t1"), "two");
        assert_eq!(text(&mut e, "globalThis.t2"), "none");
    }

    #[test]
    fn render_mounts_a_component_into_a_target() {
        let mut e = render_engine();
        e.eval(
            r#"
            function App() {
                var d = $ss.el("h1");
                $ss.child(d, $ss.txt("hello"));
                return d;
            }
            globalThis.root = $ss.el("main");
            globalThis.dispose = render(function () { return $ss.cmp(App, {}); }, globalThis.root);
            globalThis.t = root.textContent;             // "hello"
            globalThis.isFn = (typeof globalThis.dispose === "function"); // true
            "#,
        )
        .unwrap();
        assert_eq!(text(&mut e, "globalThis.t"), "hello");
        assert_eq!(text(&mut e, "globalThis.isFn"), "true");
    }

    // THE RED DRIVER for this task. Node wrappers are identity-stable, so the Task-4
    // replace-based stub yields the SAME final DOM as minimal-move (the two order
    // tests above pass under both). What distinguishes them is HOW MANY DOM ops a
    // reorder costs. Spy on the parent's insertBefore/removeChild and assert a single
    // item moving in a list of four costs only a couple of ops — a full rebuild would
    // be 4 removes + 4 inserts = 8.
    #[test]
    fn insert_array_reorder_is_minimal_moves() {
        let mut e = render_engine();
        e.eval(
            r#"
            globalThis.A=$ss.el("i"); globalThis.B=$ss.el("i");
            globalThis.C=$ss.el("i"); globalThis.D=$ss.el("i");
            var pair = createSignal([A, B, C, D]);
            globalThis.set = pair[1];
            globalThis.p = $ss.el("div");
            $ss.insert(p, function () { return pair[0](); });

            // Install op-count spies AFTER the initial render (shadow the proto methods).
            globalThis.ops = 0;
            var proto = Object.getPrototypeOf(p);
            p.insertBefore = function (n, r) { globalThis.ops++; return proto.insertBefore.call(p, n, r); };
            p.removeChild  = function (n)    { globalThis.ops++; return proto.removeChild.call(p, n); };

            globalThis.set([A, C, D, B]);   // move B from index 1 to the end
            globalThis.opsAfter = globalThis.ops;
            "#,
        )
        .unwrap();
        // Minimal-move handles this in <= 2 ops; the replace-based stub would use 8.
        let ops = num(&mut e, "globalThis.opsAfter");
        assert!(ops <= 2.0, "expected minimal moves (<=2 ops), got {ops}");
    }

    #[test]
    fn hot_tags_component_with_id() {
        let mut e = render_engine();
        e.eval(
            r#"
            function App() { return $ss.el("div"); }
            $ss.hot("app.tsx#App", App);
            globalThis.id = App.__ssId;                       // "app.tsx#App"
            globalThis.same = ($ss.hot("x#Y", App) === App);  // returns the fn
            "#,
        )
        .unwrap();
        assert_eq!(text(&mut e, "globalThis.id"), "app.tsx#App");
        assert_eq!(text(&mut e, "globalThis.same"), "true");
    }

    #[test]
    fn hmr_preserves_component_signal_value() {
        let mut e = render_engine();
        e.eval(
            r#"
            globalThis.__ssHmr = true;
            globalThis.root = $ss.el("main");
            function makeApp() {
                function Counter() {
                    var c = createSignal(0);
                    globalThis.__c = c;
                    var d = $ss.el("div");
                    $ss.insert(d, function () { return c[0](); });
                    return d;
                }
                Counter.__ssId = "app#Counter";
                return function () { return $ss.cmp(Counter, {}); };
            }
            render(makeApp(), root);
            globalThis.t0 = root.textContent;   // "0"
            globalThis.__c[1](5);
            globalThis.t1 = root.textContent;   // "5"
            render(makeApp(), root);            // hot reload: same mount node
            globalThis.t2 = root.textContent;   // "5" — preserved
            "#,
        )
        .unwrap();
        assert_eq!(text(&mut e, "globalThis.t0"), "0");
        assert_eq!(text(&mut e, "globalThis.t1"), "5");
        assert_eq!(text(&mut e, "globalThis.t2"), "5");
    }

    #[test]
    fn hmr_resets_on_shape_change() {
        let mut e = render_engine();
        e.eval(
            r#"
            globalThis.__ssHmr = true;
            globalThis.root = $ss.el("main");
            function makeApp(twoCells) {
                function Counter() {
                    if (twoCells) { createSignal(0); }   // extra leading cell -> shape change
                    var c = createSignal(0);
                    globalThis.__c = c;
                    var d = $ss.el("div");
                    $ss.insert(d, function () { return c[0](); });
                    return d;
                }
                Counter.__ssId = "app#Counter";
                return function () { return $ss.cmp(Counter, {}); };
            }
            render(makeApp(false), root);
            globalThis.__c[1](5);
            globalThis.t1 = root.textContent;   // "5"
            render(makeApp(true), root);        // reload with a DIFFERENT signal count
            globalThis.t2 = root.textContent;   // "0" — shape changed -> reset
            "#,
        )
        .unwrap();
        assert_eq!(text(&mut e, "globalThis.t1"), "5");
        assert_eq!(text(&mut e, "globalThis.t2"), "0");
    }

    #[test]
    fn hmr_keys_sibling_instances_separately() {
        let mut e = render_engine();
        e.eval(
            r#"
            globalThis.__ssHmr = true;
            globalThis.root = $ss.el("main");
            function makeApp() {
                globalThis.__cs = [];
                function Counter() {
                    var c = createSignal(0);
                    globalThis.__cs.push(c);
                    var d = $ss.el("i");
                    $ss.insert(d, function () { return c[0](); });
                    return d;
                }
                Counter.__ssId = "app#Counter";
                function App() {
                    var wrap = $ss.el("div");
                    $ss.insert(wrap, function () { return $ss.cmp(Counter, {}); });
                    $ss.insert(wrap, function () { return $ss.cmp(Counter, {}); });
                    return wrap;
                }
                App.__ssId = "app#App";
                return function () { return $ss.cmp(App, {}); };
            }
            render(makeApp(), root);
            globalThis.__cs[0][1](7);           // first sibling -> 7
            globalThis.__cs[1][1](3);           // second sibling -> 3
            globalThis.t1 = root.textContent;   // "73"
            render(makeApp(), root);            // reload
            globalThis.t2 = root.textContent;   // "73" — each sibling kept its own value
            "#,
        )
        .unwrap();
        assert_eq!(text(&mut e, "globalThis.t1"), "73");
        assert_eq!(text(&mut e, "globalThis.t2"), "73");
    }

    #[test]
    fn hmr_preserves_for_row_state_across_reorder() {
        let mut e = render_engine();
        e.eval(
            r#"
            globalThis.__ssHmr = true;
            globalThis.a = { n: "a" };
            globalThis.b = { n: "b" };
            globalThis.root = $ss.el("div");
            function build() {
                function App() {
                    var list = createSignal([globalThis.a, globalThis.b]);
                    globalThis.__list = list;
                    var ul = $ss.el("ul");
                    $ss.insert(ul, function () {
                        return $ss.cmp(For, {
                            get each() { return list[0](); },
                            get children() {
                                return function (item) {
                                    var cnt = createSignal(0);
                                    item.__cnt = cnt;            // expose row signal via the item
                                    var li = $ss.el("li");
                                    $ss.insert(li, function () { return item.n + ":" + cnt[0](); });
                                    return li;
                                };
                            },
                        });
                    });
                    return ul;
                }
                App.__ssId = "app#App";
                return function () { return $ss.cmp(App, {}); };
            }
            render(build(), root);
            globalThis.t0 = root.textContent;   // "a:0b:0"
            globalThis.a.__cnt[1](7);
            globalThis.b.__cnt[1](3);
            globalThis.t1 = root.textContent;   // "a:7b:3"
            globalThis.__list[1]([globalThis.b, globalThis.a]);   // runtime reorder (Plan-4 For)
            globalThis.tR = root.textContent;   // "b:3a:7"
            render(build(), root);              // hot reload
            globalThis.t2 = root.textContent;   // "b:3a:7" — list + per-row state preserved
            "#,
        )
        .unwrap();
        assert_eq!(text(&mut e, "globalThis.t0"), "a:0b:0");
        assert_eq!(text(&mut e, "globalThis.t1"), "a:7b:3");
        assert_eq!(text(&mut e, "globalThis.tR"), "b:3a:7");
        assert_eq!(text(&mut e, "globalThis.t2"), "b:3a:7");
    }

    #[test]
    fn hmr_preserves_index_row_state_by_position() {
        let mut e = render_engine();
        e.eval(
            r#"
            globalThis.__ssHmr = true;
            globalThis.root = $ss.el("div");
            function build() {
                function App() {
                    var list = createSignal(["x", "y"]);
                    var ul = $ss.el("ul");
                    $ss.insert(ul, function () {
                        return $ss.cmp(Index, {
                            get each() { return list[0](); },
                            get children() {
                                return function (item) {              // item: signal getter
                                    var tag = createSignal("");        // private per-position state
                                    globalThis.__tags = globalThis.__tags || [];
                                    globalThis.__tags.push(tag);
                                    var li = $ss.el("li");
                                    $ss.insert(li, function () { return item() + tag[0](); });
                                    return li;
                                };
                            },
                        });
                    });
                    return ul;
                }
                App.__ssId = "app#App";
                return function () { return $ss.cmp(App, {}); };
            }
            globalThis.__tags = [];
            render(build(), root);
            globalThis.t0 = root.textContent;   // "xy"
            globalThis.__tags[0][1]("!");        // position 0 private state
            globalThis.t1 = root.textContent;   // "x!y"
            globalThis.__tags = [];
            render(build(), root);              // hot reload
            globalThis.t2 = root.textContent;   // "x!y" — position 0 state preserved
            "#,
        )
        .unwrap();
        assert_eq!(text(&mut e, "globalThis.t0"), "xy");
        assert_eq!(text(&mut e, "globalThis.t1"), "x!y");
        assert_eq!(text(&mut e, "globalThis.t2"), "x!y");
    }

    #[test]
    fn hmr_stale_snapshot_not_reused_after_reload() {
        let mut e = render_engine();
        e.eval(
            r#"
        globalThis.__ssHmr = true;
        globalThis.root = $ss.el("div");
        function build() {
            function App() {
                var list = createSignal(["x"]);
                globalThis.__list = list;
                var ul = $ss.el("ul");
                $ss.insert(ul, function () {
                    return $ss.cmp(Index, {
                        get each() { return list[0](); },
                        get children() {
                            return function (item) {
                                var tag = createSignal("");
                                globalThis.__lastTag = tag;
                                var li = $ss.el("li");
                                $ss.insert(li, function () { return item() + tag[0](); });
                                return li;
                            };
                        },
                    });
                });
                return ul;
            }
            App.__ssId = "app#App";
            return function () { return $ss.cmp(App, {}); };
        }
        render(build(), root);
        globalThis.__lastTag[1]("!");        // position 0 private state
        globalThis.t1 = root.textContent;    // "x!"
        render(build(), root);               // reload -> position 0 legitimately rehydrated to "!"
        globalThis.t2 = root.textContent;    // "x!"
        // Post-reload churn: drop the row, then regrow a NEW row at position 0.
        globalThis.__list[1]([]);            // shrink -> row disposed
        globalThis.__list[1](["x"]);         // grow -> fresh row at position 0
        globalThis.t3 = root.textContent;    // MUST be "x" (fresh default), NOT stale "x!"
        "#,
        )
        .unwrap();
        assert_eq!(text(&mut e, "globalThis.t1"), "x!");
        assert_eq!(text(&mut e, "globalThis.t2"), "x!");
        assert_eq!(text(&mut e, "globalThis.t3"), "x");
    }

    /// DIAGNOSTIC micro-bench (not a correctness test): mirror the horde overlay
    /// workload in pure JS — N enemies, positions changing every frame, a churn
    /// rate of rows added/removed per frame — and time the reactive update for the
    /// `<Index>`-over-snapshot approach vs the entity-keyed `<Keyed>` approach (both
    /// the real primitive and a hand-written prototype), against the theoretical
    /// floor. Prints per-frame ms + binding re-run counts so the savings vs overhead
    /// are visible in seconds instead of a multi-minute Bevy profile.
    ///
    ///   cargo test --release -p supersolid_runtime overlay_microbench -- --ignored --nocapture
    #[test]
    #[ignore = "diagnostic micro-bench; run explicitly with --ignored --nocapture --release"]
    fn overlay_microbench() {
        use std::time::Instant;

        // Shared frame-data generator + counters, installed once per engine.
        const SETUP: &str = r#"
            globalThis.N = 300;
            if (typeof globalThis.CHURN !== "number") globalThis.CHURN = 8; // rows removed+added per frame
            globalThis.styleRuns = 0;
            globalThis.otherRuns = 0;       // data-id + np-fill binding re-runs
            globalThis.nextId = 0;
            globalThis.liveIds = [];
            for (var i = 0; i < globalThis.N; i++) globalThis.liveIds.push(globalThis.nextId++);
            globalThis.frameIdx = 0;
            globalThis.snapshot = function () {
                var fi = ++globalThis.frameIdx;
                // churn: drop the oldest CHURN, append CHURN fresh ids
                if (globalThis.CHURN > 0) {
                    globalThis.liveIds.splice(0, globalThis.CHURN);
                    for (var c = 0; c < globalThis.CHURN; c++) globalThis.liveIds.push(globalThis.nextId++);
                }
                var arr = new Array(globalThis.liveIds.length);
                for (var i = 0; i < globalThis.liveIds.length; i++) {
                    var id = globalThis.liveIds[i];
                    arr[i] = {
                        id: id,
                        sx: (id * 7 + fi * 3) % 1280,
                        sy: (id * 13 + fi * 5) % 720,
                        frac: 1.0,                 // constant: exercises the "unchanged" skip
                    };
                }
                return { enemies: arr };
            };
        "#;

        // Baseline: <Index> over the whole snapshot — the row's item is one signal
        // (`e()`), so every position write re-runs all of the row's bindings.
        fn approach_index() -> String {
            format!(
                r#"{SETUP}
                var frame = createSignal({{ enemies: globalThis.snapshot() }});
                globalThis.root = $ss.el("div");
                $ss.insert(globalThis.root, function () {{
                    return $ss.cmp(Index, {{
                        get each() {{ return frame[0]().enemies; }},
                        get children() {{
                            return function (e) {{
                                var d = $ss.el("div");
                                $ss.bind(d, "data-id", function () {{ globalThis.otherRuns++; return e().id; }});
                                $ss.bind(d, "style", function () {{ globalThis.styleRuns++; return "left:" + Math.round(e().sx - 22) + "px;top:" + Math.round(e().sy - 30) + "px"; }});
                                var fll = $ss.el("div");
                                $ss.bind(fll, "style", function () {{ globalThis.otherRuns++; return "width:" + Math.round(e().frac * 100) + "%"; }});
                                $ss.child(d, fll);
                                return d;
                            }};
                        }},
                    }});
                }});
                globalThis.step = function () {{ frame[1](globalThis.snapshot()); }};
                "#,
                SETUP = SETUP
            )
        }

        let warmup = 20usize;
        let measured = 200usize;
        let run = |label: &str, setup: String| {
            let mut e = render_engine();
            e.eval(&setup).unwrap();
            for _ in 0..warmup {
                e.eval("globalThis.step();").unwrap();
            }
            e.eval("globalThis.styleRuns = 0; globalThis.otherRuns = 0;").unwrap();
            let t = Instant::now();
            for _ in 0..measured {
                e.eval("globalThis.step();").unwrap();
            }
            let ms = t.elapsed().as_secs_f64() * 1000.0;
            let mut e2 = e;
            let style_runs = num(&mut e2, "globalThis.styleRuns");
            let other_runs = num(&mut e2, "globalThis.otherRuns");
            println!(
                "{label:12}  {:.3} ms/frame  |  style runs/f = {:.1}  |  data-id+npfill runs/f = {:.1}",
                ms / measured as f64,
                style_runs / measured as f64,
                other_runs / measured as f64
            );
        };

        // Theoretical FLOOR: 300 rows built once; each frame directly write per-row
        // sx/sy signals (no list, no diff, no churn). This is the cheapest any
        // fine-grained design could be — pure "positions changed" cost.
        let approach_floor = format!(
            r#"{SETUP}
            globalThis.CHURN = 0;
            globalThis.root = $ss.el("div");
            globalThis.setters = [];
            var snap0 = globalThis.snapshot();
            for (var i = 0; i < snap0.enemies.length; i++) {{
                (function (item) {{
                    var sxp = createSignal(item.sx), syp = createSignal(item.sy), frp = createSignal(item.frac);
                    globalThis.setters.push([sxp[1], syp[1], frp[1]]);
                    var d = $ss.el("div");
                    $ss.bind(d, "data-id", function () {{ globalThis.otherRuns++; return item.id; }});
                    $ss.bind(d, "style", function () {{ globalThis.styleRuns++; return "left:" + Math.round(sxp[0]() - 22) + "px;top:" + Math.round(syp[0]() - 30) + "px"; }});
                    var fll = $ss.el("div");
                    $ss.bind(fll, "style", function () {{ globalThis.otherRuns++; return "width:" + Math.round(frp[0]() * 100) + "%"; }});
                    $ss.child(d, fll);
                    $ss.child(globalThis.root, d);
                }})(snap0.enemies[i]);
            }}
            globalThis.step = function () {{
                var snap = globalThis.snapshot();
                batch(function () {{
                    for (var i = 0; i < snap.enemies.length; i++) {{
                        var it = snap.enemies[i], s = globalThis.setters[i];
                        s[0](it.sx); s[1](it.sy); s[2](it.frac);
                    }}
                }});
            }};
            "#,
            SETUP = SETUP
        );

        // IDEAL keyed store: owns its DOM list, keyed by id. Per frame: batched
        // incremental diff — write changed per-field signals for surviving rows,
        // create DOM only for new keys, remove DOM only for gone keys. No generic
        // <For>, no list-signal rewrite, no full re-diff. This is the ceiling a
        // purpose-built store primitive could hit.
        fn approach_keyed() -> String {
            format!(
                r#"{SETUP}
                globalThis.root = $ss.el("div");
                globalThis.rows = new Map();     // id -> {{ el, sx, sy, fr }}
                function makeRow(item) {{
                    var sx = createSignal(item.sx), sy = createSignal(item.sy), fr = createSignal(item.frac);
                    var d = $ss.el("div");
                    $ss.bind(d, "data-id", function () {{ globalThis.otherRuns++; return item.id; }});
                    $ss.bind(d, "style", function () {{ globalThis.styleRuns++; return "left:" + Math.round(sx[0]() - 22) + "px;top:" + Math.round(sy[0]() - 30) + "px"; }});
                    var fll = $ss.el("div");
                    $ss.bind(fll, "style", function () {{ globalThis.otherRuns++; return "width:" + Math.round(fr[0]() * 100) + "%"; }});
                    $ss.child(d, fll);
                    return {{ el: d, sx: sx[1], sy: sy[1], fr: fr[1] }};
                }}
                globalThis.step = function () {{
                    var snap = globalThis.snapshot();
                    var arr = snap.enemies;
                    batch(function () {{
                        var seen = new Set();
                        for (var i = 0; i < arr.length; i++) {{
                            var it = arr[i], row = globalThis.rows.get(it.id);
                            if (row) {{
                                row.sx(it.sx); row.sy(it.sy); row.fr(it.frac);
                            }} else {{
                                row = makeRow(it);
                                globalThis.rows.set(it.id, row);
                                $ss.child(globalThis.root, row.el);   // append (order approx; ok for bench)
                            }}
                            seen.add(it.id);
                        }}
                        // remove gone rows
                        globalThis.rows.forEach(function (row, id) {{
                            if (!seen.has(id)) {{
                                globalThis.root.removeChild(row.el);
                                globalThis.rows.delete(id);
                            }}
                        }});
                    }});
                }};
                globalThis.step();  // build initial rows
                "#,
                SETUP = SETUP
            )
        }

        // The REAL <Keyed> primitive driven through insert/cmp, fed by a frame signal.
        fn approach_keyed_real() -> String {
            format!(
                r#"{SETUP}
                var pair = createSignal(globalThis.snapshot().enemies);
                var frame = pair[0], setFrame = pair[1];
                globalThis.root = $ss.el("div");
                $ss.insert(globalThis.root, function () {{
                    return $ss.cmp(Keyed, {{
                        get each() {{ return frame(); }},
                        by: "id",
                        get children() {{
                            return function (e) {{
                                var d = $ss.el("div");
                                $ss.bind(d, "data-id", function () {{ globalThis.otherRuns++; return e.id; }});
                                $ss.bind(d, "style", function () {{ globalThis.styleRuns++; return "left:" + Math.round(e.sx - 22) + "px;top:" + Math.round(e.sy - 30) + "px"; }});
                                var fll = $ss.el("div");
                                $ss.bind(fll, "style", function () {{ globalThis.otherRuns++; return "width:" + Math.round(e.frac * 100) + "%"; }});
                                $ss.child(d, fll);
                                return d;
                            }};
                        }},
                    }});
                }});
                globalThis.step = function () {{ setFrame(globalThis.snapshot().enemies); }};
                "#,
                SETUP = SETUP
            )
        }

        for churn in [0usize, 8usize] {
            println!("\n--- overlay micro-bench (N=300, churn={churn}/frame, {measured} frames) ---");
            let inject = format!("globalThis.CHURN = {churn};\n");
            run("index", format!("{inject}{}", approach_index()));
            run("keyed-proto", format!("{inject}{}", approach_keyed()));
            run("Keyed(real)", format!("{inject}{}", approach_keyed_real()));
        }
        println!("\n--- floor: 300 static rows, direct per-row signal writes (no list/diff/churn) ---");
        run("floor", approach_floor);
    }

    #[test]
    fn hmr_off_does_not_preserve() {
        let mut e = render_engine();
        e.eval(
            r#"
            globalThis.__ssHmr = false;         // gate OFF
            globalThis.root = $ss.el("main");
            function makeApp() {
                function Counter() {
                    var c = createSignal(0);
                    globalThis.__c = c;
                    var d = $ss.el("div");
                    $ss.insert(d, function () { return c[0](); });
                    return d;
                }
                Counter.__ssId = "app#Counter";
                return function () { return $ss.cmp(Counter, {}); };
            }
            render(makeApp(), root);
            globalThis.__c[1](5);               // bump the first render's signal
            render(makeApp(), root);            // second render (gate off)
            globalThis.v = globalThis.__c[0](); // the SECOND render's fresh signal -> 0
            "#,
        )
        .unwrap();
        assert_eq!(num(&mut e, "globalThis.v"), 0.0);
    }
}
