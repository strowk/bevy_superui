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
/// plus `$ss` (`el`/`txt`/`attr`/`child`) from the render layer.
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
}
