//! Drives the JS-side W3C event dispatch in `js/dom.js`: builds a shadow tree,
//! registers listeners, calls `__ss_dispatch`, and asserts phase ordering,
//! `preventDefault` return value, and `stopPropagation` semantics.

#![cfg(feature = "engine-boa")]

use boa_engine::{Context, JsValue, Source};

const DOM_JS: &str = include_str!("../js/dom.js");

fn ctx() -> Context {
    let mut ctx = Context::default();
    ctx.eval(Source::from_bytes(DOM_JS))
        .expect("dom.js evaluates cleanly");
    ctx
}

fn eval(ctx: &mut Context, src: &str) -> JsValue {
    ctx.eval(Source::from_bytes(src))
        .unwrap_or_else(|e| panic!("eval failed for `{src}`: {e}"))
}

fn eval_bool(ctx: &mut Context, src: &str) -> bool {
    eval(ctx, src).as_boolean().expect("boolean result")
}

fn eval_string(ctx: &mut Context, src: &str) -> String {
    eval(ctx, src)
        .to_string(ctx)
        .expect("string result")
        .to_std_string_escaped()
}

/// parent > child under the root; a shared `globalThis.ran` order log.
fn tree(ctx: &mut Context) {
    eval(
        ctx,
        r#"
        globalThis.parent = document.createElement('div');
        globalThis.child = document.createElement('div');
        __ss_root.appendChild(parent);
        parent.appendChild(child);
        globalThis.ran = [];
        "#,
    );
}

#[test]
fn bubbling_listener_runs_and_prevent_default_is_reported() {
    let mut ctx = ctx();
    tree(&mut ctx);
    eval(
        &mut ctx,
        r#"
        parent.addEventListener('click', function (e) { ran.push('parent'); });
        child.addEventListener('click', function (e) { ran.push('child'); e.preventDefault(); });
        "#,
    );
    let prevented = eval_bool(&mut ctx, "__ss_dispatch(child.id, 'click', null, true, true)");
    assert!(prevented, "preventDefault() should be reported as prevented");
    assert_eq!(eval_string(&mut ctx, "ran.join(',')"), "child,parent");
}

#[test]
fn stop_propagation_prevents_parent() {
    let mut ctx = ctx();
    tree(&mut ctx);
    eval(
        &mut ctx,
        r#"
        parent.addEventListener('click', function (e) { ran.push('parent'); });
        child.addEventListener('click', function (e) { ran.push('child'); e.stopPropagation(); });
        "#,
    );
    let prevented = eval_bool(&mut ctx, "__ss_dispatch(child.id, 'click', null, true, true)");
    assert!(!prevented);
    assert_eq!(eval_string(&mut ctx, "ran.join(',')"), "child");
}

#[test]
fn capture_phase_runs_root_to_target() {
    let mut ctx = ctx();
    tree(&mut ctx);
    eval(
        &mut ctx,
        r#"
        parent.addEventListener('click', function (e) { ran.push('parent-capture'); }, true);
        parent.addEventListener('click', function (e) { ran.push('parent-bubble'); });
        child.addEventListener('click', function (e) { ran.push('child'); });
        "#,
    );
    let _ = eval_bool(&mut ctx, "__ss_dispatch(child.id, 'click', null, true, true)");
    // capture (root->target, excl target) then target then bubble (target->root).
    assert_eq!(
        eval_string(&mut ctx, "ran.join(',')"),
        "parent-capture,child,parent-bubble"
    );
}

#[test]
fn non_bubbling_skips_bubble_phase() {
    let mut ctx = ctx();
    tree(&mut ctx);
    eval(
        &mut ctx,
        r#"
        parent.addEventListener('click', function (e) { ran.push('parent-capture'); }, true);
        parent.addEventListener('click', function (e) { ran.push('parent-bubble'); });
        child.addEventListener('click', function (e) { ran.push('child'); });
        "#,
    );
    let _ = eval_bool(&mut ctx, "__ss_dispatch(child.id, 'click', null, false, true)");
    assert_eq!(eval_string(&mut ctx, "ran.join(',')"), "parent-capture,child");
}

#[test]
fn stop_immediate_propagation_halts_remaining_listeners_on_node() {
    let mut ctx = ctx();
    tree(&mut ctx);
    eval(
        &mut ctx,
        r#"
        child.addEventListener('click', function (e) { ran.push('child1'); e.stopImmediatePropagation(); });
        child.addEventListener('click', function (e) { ran.push('child2'); });
        parent.addEventListener('click', function (e) { ran.push('parent'); });
        "#,
    );
    let _ = eval_bool(&mut ctx, "__ss_dispatch(child.id, 'click', null, true, true)");
    assert_eq!(eval_string(&mut ctx, "ran.join(',')"), "child1");
}

#[test]
fn removed_listener_does_not_fire() {
    let mut ctx = ctx();
    tree(&mut ctx);
    eval(
        &mut ctx,
        r#"
        globalThis.h = function (e) { ran.push('parent'); };
        parent.addEventListener('click', h);
        parent.removeEventListener('click', h);
        child.addEventListener('click', function (e) { ran.push('child'); });
        "#,
    );
    let _ = eval_bool(&mut ctx, "__ss_dispatch(child.id, 'click', null, true, true)");
    assert_eq!(eval_string(&mut ctx, "ran.join(',')"), "child");
}

#[test]
fn event_object_exposes_target_and_type_and_key() {
    let mut ctx = ctx();
    tree(&mut ctx);
    eval(
        &mut ctx,
        r#"
        globalThis.seen = {};
        child.addEventListener('keydown', function (e) {
            seen.type = e.type;
            seen.key = e.key;
            seen.targetIsChild = e.target === child;
            seen.currentIsChild = e.currentTarget === child;
        });
        parent.addEventListener('keydown', function (e) {
            seen.currentIsParent = e.currentTarget === parent;
        });
        "#,
    );
    let _ = eval_bool(&mut ctx, "__ss_dispatch(child.id, 'keydown', 'Enter', true, true)");
    assert_eq!(eval_string(&mut ctx, "seen.type"), "keydown");
    assert_eq!(eval_string(&mut ctx, "seen.key"), "Enter");
    assert!(eval_bool(&mut ctx, "seen.targetIsChild"));
    assert!(eval_bool(&mut ctx, "seen.currentIsChild"));
    assert!(eval_bool(&mut ctx, "seen.currentIsParent"));
}

#[test]
fn prevent_default_ignored_when_not_cancelable() {
    let mut ctx = ctx();
    tree(&mut ctx);
    eval(
        &mut ctx,
        "child.addEventListener('click', function (e) { e.preventDefault(); });",
    );
    let prevented = eval_bool(&mut ctx, "__ss_dispatch(child.id, 'click', null, true, false)");
    assert!(!prevented, "preventDefault on a non-cancelable event has no effect");
}
