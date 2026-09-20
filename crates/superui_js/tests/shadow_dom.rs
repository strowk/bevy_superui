//! Drives `js/dom.js` through a bare Boa context: runs author JS that mutates
//! the JS-side shadow DOM, calls `__ss_flush()`, and decodes the returned byte
//! array with [`OpBatch::decode`] to assert the exact recorded ops. Also checks
//! that structural/attribute reads are served from JS state and that
//! `replaceChild` decomposes into insertBefore + removeChild.

#![cfg(feature = "engine-boa")]

use boa_engine::{js_string, Context, JsValue, Source};

use superui_js::opwire::{Op, OpBatch};

const DOM_JS: &str = include_str!("../js/dom.js");

/// Fresh Boa context with `dom.js` evaluated (globals installed).
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

/// Call `__ss_flush()` and read the returned array-of-bytes into a `Vec<u8>`.
fn flush(ctx: &mut Context) -> Vec<u8> {
    let value = eval(ctx, "__ss_flush()");
    let obj = value.as_object().expect("__ss_flush returns an array/object");
    let len = obj
        .get(js_string!("length"), ctx)
        .unwrap()
        .to_u32(ctx)
        .unwrap();
    let mut bytes = Vec::with_capacity(len as usize);
    for i in 0..len {
        let v = obj.get(i, ctx).unwrap();
        bytes.push(v.to_u32(ctx).unwrap() as u8);
    }
    bytes
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

/// Position of an already-interned string in a decoded batch's pool.
fn pos(batch: &OpBatch, s: &str) -> u32 {
    batch
        .strings
        .iter()
        .position(|existing| existing == s)
        .unwrap_or_else(|| panic!("string {s:?} not in pool {:?}", batch.strings)) as u32
}

#[test]
fn author_mutations_emit_the_exact_ops() {
    let mut ctx = ctx();
    eval(
        &mut ctx,
        r#"
        const d = document.createElement('div');
        d.setAttribute('class', 'row');
        __ss_root.appendChild(d);
        const t = document.createTextNode('hello');
        d.appendChild(t);
        "#,
    );
    let batch = OpBatch::decode(&flush(&mut ctx)).expect("decodes");

    // ids: div = 2, text = 3. string pool interned in emission order.
    let div = pos(&batch, "div");
    let class = pos(&batch, "class");
    let row = pos(&batch, "row");
    let hello = pos(&batch, "hello");

    assert_eq!(
        batch.ops,
        vec![
            Op::CreateElement { id: 2, tag: div },
            Op::SetAttribute { id: 2, name: class, value: row },
            Op::InsertBefore { parent: 1, node: 2, reference: 0 },
            Op::CreateText { id: 3, data: hello },
            Op::InsertBefore { parent: 2, node: 3, reference: 0 },
        ]
    );
}

#[test]
fn insert_before_uses_reference_id() {
    let mut ctx = ctx();
    eval(
        &mut ctx,
        r#"
        const a = document.createElement('a');
        const b = document.createElement('b');
        __ss_root.appendChild(a);
        __ss_root.insertBefore(b, a);
        "#,
    );
    let batch = OpBatch::decode(&flush(&mut ctx)).expect("decodes");
    // a = 2, b = 3. insertBefore(b, a) references a's id (2).
    assert_eq!(
        batch.ops.last(),
        Some(&Op::InsertBefore { parent: 1, node: 3, reference: 2 })
    );
}

#[test]
fn property_and_style_and_text_ops() {
    let mut ctx = ctx();
    eval(
        &mut ctx,
        r#"
        const inp = document.createElement('input');
        __ss_root.appendChild(inp);
        inp.value = 'typed';
        inp.checked = true;
        inp.style.color = 'red';
        const t = document.createTextNode('a');
        inp.appendChild(t);
        t.data = 'b';
        inp.textContent = 'c';
        inp.removeAttribute('class');
        "#,
    );
    let batch = OpBatch::decode(&flush(&mut ctx)).expect("decodes");

    let has = |op: Op| assert!(batch.ops.contains(&op), "missing op {op:?} in {:?}", batch.ops);
    let s = |v: &str| pos(&batch, v);
    has(Op::SetProperty { id: 2, name: s("value"), value: s("typed") });
    has(Op::SetProperty { id: 2, name: s("checked"), value: s("true") });
    has(Op::SetStyle { id: 2, prop: s("color"), value: s("red") });
    has(Op::SetText { id: 3, data: s("b") });
    has(Op::SetText { id: 2, data: s("c") });
    has(Op::RemoveAttribute { id: 2, name: s("class") });
}

#[test]
fn reads_served_from_js_state() {
    let mut ctx = ctx();
    eval(
        &mut ctx,
        r#"
        globalThis.d = document.createElement('div');
        d.setAttribute('id', 'main');
        d.setAttribute('class', 'row');
        __ss_root.appendChild(d);
        globalThis.t = document.createTextNode('hi');
        d.appendChild(t);
        "#,
    );
    // flush so reads are proven independent of the op queue
    let _ = flush(&mut ctx);

    assert_eq!(eval_string(&mut ctx, "d.getAttribute('class')"), "row");
    assert_eq!(eval_string(&mut ctx, "d.getAttribute('missing') === null ? 'NULL' : 'x'"), "NULL");
    assert_eq!(eval_string(&mut ctx, "'' + d.childNodes.length"), "1");
    assert!(eval_bool(&mut ctx, "d.childNodes[0] === t"));
    assert!(eval_bool(&mut ctx, "t.parentNode === d"));
    assert_eq!(eval_string(&mut ctx, "'' + t.nodeType"), "3");
    assert_eq!(eval_string(&mut ctx, "'' + d.nodeType"), "1");
    assert_eq!(eval_string(&mut ctx, "t.data"), "hi");
    assert_eq!(eval_string(&mut ctx, "d.textContent"), "hi");
    assert!(eval_bool(&mut ctx, "document.getElementById('main') === d"));
}

#[test]
fn next_sibling_reads_from_js_state() {
    let mut ctx = ctx();
    eval(
        &mut ctx,
        r#"
        globalThis.p = document.createElement('p');
        globalThis.a = document.createElement('a');
        globalThis.b = document.createElement('b');
        __ss_root.appendChild(p);
        p.appendChild(a);
        p.appendChild(b);
        "#,
    );
    assert!(eval_bool(&mut ctx, "a.nextSibling === b"));
    assert!(eval_bool(&mut ctx, "b.nextSibling === null"));
}

#[test]
fn replace_child_decomposes_into_insert_then_remove() {
    let mut ctx = ctx();
    eval(
        &mut ctx,
        r#"
        globalThis.p = document.createElement('p');
        globalThis.a = document.createElement('a');
        globalThis.b = document.createElement('b');
        __ss_root.appendChild(p);
        p.appendChild(a);
        p.appendChild(b);
        "#,
    );
    let _ = flush(&mut ctx); // drop setup ops
    eval(
        &mut ctx,
        r#"
        globalThis.c = document.createElement('c');
        p.replaceChild(c, a);
        "#,
    );
    let batch = OpBatch::decode(&flush(&mut ctx)).expect("decodes");
    // p=2, a=3, b=4, c=5. replaceChild(c, a) => insertBefore(c, a) then removeChild(a).
    assert_eq!(
        batch.ops,
        vec![
            Op::CreateElement { id: 5, tag: pos(&batch, "c") },
            Op::InsertBefore { parent: 2, node: 5, reference: 3 },
            Op::RemoveChild { parent: 2, node: 3 },
        ]
    );
    // JS tree now [c, b]
    assert!(eval_bool(&mut ctx, "p.childNodes.length === 2 && p.childNodes[0] === c && p.childNodes[1] === b"));
    assert!(eval_bool(&mut ctx, "c.parentNode === p && a.parentNode === null"));
}

#[test]
fn text_content_setter_clears_children() {
    let mut ctx = ctx();
    eval(
        &mut ctx,
        r#"
        globalThis.p = document.createElement('p');
        globalThis.a = document.createElement('a');
        __ss_root.appendChild(p);
        p.appendChild(a);
        p.textContent = 'gone';
        "#,
    );
    let _ = flush(&mut ctx);
    assert_eq!(eval_string(&mut ctx, "p.textContent"), "gone");
    assert_eq!(eval_string(&mut ctx, "'' + p.childNodes.length"), "0");
}

#[test]
fn bounding_rect_is_zero_without_measure_host() {
    let mut ctx = ctx();
    eval(&mut ctx, "globalThis.d = document.createElement('div'); __ss_root.appendChild(d);");
    assert_eq!(eval_string(&mut ctx, "'' + d.getBoundingClientRect().width"), "0");
    assert_eq!(eval_string(&mut ctx, "'' + d.offsetWidth"), "0");
    assert_eq!(eval_string(&mut ctx, "'' + d.offsetHeight"), "0");
}

#[test]
fn flush_clears_the_queue() {
    let mut ctx = ctx();
    eval(&mut ctx, "__ss_root.appendChild(document.createElement('div'));");
    assert!(!flush(&mut ctx).is_empty());
    let second = OpBatch::decode(&flush(&mut ctx)).expect("decodes");
    assert!(second.ops.is_empty());
    assert!(second.strings.is_empty());
}

#[test]
fn non_ascii_strings_survive_roundtrip() {
    for s in ["caf\u{e9}", "\u{65e5}\u{672c}\u{8a9e}", "a\u{1f600}b"] {
        let mut ctx = ctx();
        eval(
            &mut ctx,
            &format!("__ss_root.appendChild(document.createTextNode(\"{s}\"));"),
        );
        let batch = OpBatch::decode(&flush(&mut ctx)).expect("decodes");
        assert!(
            batch.strings.iter().any(|p| p == s),
            "pool {:?} missing {s:?}",
            batch.strings
        );
    }
}

#[test]
fn lone_surrogate_becomes_replacement_char_not_decode_failure() {
    let mut ctx = ctx();
    // A lone high surrogate (0xD83D) is not valid UTF-8; the encoder must emit
    // U+FFFD so decode succeeds and the frame is not dropped.
    eval(
        &mut ctx,
        r#"__ss_root.appendChild(document.createTextNode("x" + String.fromCharCode(0xD83D) + "y"));"#,
    );
    let batch = OpBatch::decode(&flush(&mut ctx)).expect("decodes despite lone surrogate");
    assert!(
        batch.strings.iter().any(|p| p == "x\u{FFFD}y"),
        "pool {:?} missing replacement-char string",
        batch.strings
    );
}

#[test]
fn removing_subtree_prunes_the_node_registry() {
    let mut ctx = ctx();
    eval(
        &mut ctx,
        r#"
        globalThis.p = document.createElement('p');
        globalThis.a = document.createElement('a');
        globalThis.b = document.createElement('b');
        __ss_root.appendChild(p);
        p.appendChild(a);
        a.appendChild(b);
        globalThis.pid = p.id; globalThis.aid = a.id; globalThis.bid = b.id;
        "#,
    );
    assert!(eval_bool(&mut ctx, "__ss_hasNode(pid) && __ss_hasNode(aid) && __ss_hasNode(bid)"));
    eval(&mut ctx, "__ss_root.removeChild(p);");
    assert!(eval_bool(
        &mut ctx,
        "!__ss_hasNode(pid) && !__ss_hasNode(aid) && !__ss_hasNode(bid)"
    ));
}

#[test]
fn text_content_setter_prunes_cleared_children() {
    let mut ctx = ctx();
    eval(
        &mut ctx,
        r#"
        globalThis.p = document.createElement('p');
        globalThis.a = document.createElement('a');
        __ss_root.appendChild(p);
        p.appendChild(a);
        globalThis.aid = a.id;
        "#,
    );
    assert!(eval_bool(&mut ctx, "__ss_hasNode(aid)"));
    eval(&mut ctx, "p.textContent = 'gone';");
    assert!(eval_bool(&mut ctx, "!__ss_hasNode(aid)"));
}

#[test]
fn insert_before_foreign_reference_appends_and_emits_zero() {
    let mut ctx = ctx();
    eval(
        &mut ctx,
        r#"
        globalThis.p = document.createElement('p');
        globalThis.q = document.createElement('q');
        globalThis.foreign = document.createElement('x');
        globalThis.n = document.createElement('n');
        __ss_root.appendChild(p);
        __ss_root.appendChild(q);
        q.appendChild(foreign);
        "#,
    );
    let _ = flush(&mut ctx); // drop setup ops
    // `foreign` is q's child, not p's; the local fallback appends, so the emitted
    // op must also append (reference 0) to keep Rust in sync.
    eval(&mut ctx, "p.insertBefore(n, foreign);");
    let batch = OpBatch::decode(&flush(&mut ctx)).expect("decodes");
    // p=2, q=3, foreign=4, n=5.
    assert_eq!(
        batch.ops.last(),
        Some(&Op::InsertBefore { parent: 2, node: 5, reference: 0 })
    );
    assert!(eval_bool(&mut ctx, "p.childNodes.length === 1 && p.childNodes[0] === n"));
}

#[test]
fn remove_child_updates_sibling_pointers_at_each_position() {
    let mut ctx = ctx();
    eval(
        &mut ctx,
        r#"
        globalThis.p = document.createElement('p');
        globalThis.a = document.createElement('a');
        globalThis.b = document.createElement('b');
        globalThis.c = document.createElement('c');
        __ss_root.appendChild(p);
        p.appendChild(a); p.appendChild(b); p.appendChild(c);
        "#,
    );
    // Remove the middle: a <-> c must relink both directions.
    eval(&mut ctx, "p.removeChild(b);");
    assert!(eval_bool(&mut ctx, "a.nextSibling === c && c.previousSibling === a"));
    assert!(eval_bool(&mut ctx, "b.parentNode === null && b.nextSibling === null && b.previousSibling === null"));
    // Remove the first: firstChild advances, new first has no previous.
    eval(&mut ctx, "p.removeChild(a);");
    assert!(eval_bool(&mut ctx, "p.firstChild === c && c.previousSibling === null"));
    // Remove the last: lastChild retreats to null (now empty).
    eval(&mut ctx, "p.removeChild(c);");
    assert!(eval_bool(&mut ctx, "p.firstChild === null && p.lastChild === null"));
}

#[test]
fn next_sibling_after_mid_list_insert() {
    let mut ctx = ctx();
    eval(
        &mut ctx,
        r#"
        globalThis.p = document.createElement('p');
        globalThis.a = document.createElement('a');
        globalThis.c = document.createElement('c');
        globalThis.b = document.createElement('b');
        __ss_root.appendChild(p);
        p.appendChild(a); p.appendChild(c);
        p.insertBefore(b, c);
        "#,
    );
    // Order must be a, b, c with fully consistent forward/backward links.
    assert!(eval_bool(&mut ctx, "a.nextSibling === b && b.nextSibling === c && c.nextSibling === null"));
    assert!(eval_bool(&mut ctx, "c.previousSibling === b && b.previousSibling === a && a.previousSibling === null"));
    assert!(eval_bool(&mut ctx, "p.firstChild === a && p.lastChild === c"));
}

#[test]
fn large_build_then_clear_stays_consistent() {
    // Correctness at scale for the O(1) structural ops, not a perf test. Build and
    // clear run in one eval, reporting counts on globals: Boa iterates a top-level
    // for-loop only once when it runs in a later, separate eval.
    let mut ctx = ctx();
    eval(
        &mut ctx,
        r#"
        globalThis.p = document.createElement('p');
        __ss_root.appendChild(p);
        for (var i = 0; i < 500; i++) p.appendChild(document.createElement('div'));
        globalThis.builtCount = p.childNodes.length;
        // clear via childNodes snapshot + removeChild, as render.js's clearChildren does
        var ks = p.childNodes;
        for (var j = 0; j < ks.length; j++) p.removeChild(ks[j]);
        globalThis.clearedCount = p.childNodes.length;
        globalThis.endsNull = p.firstChild === null && p.lastChild === null;
        "#,
    );
    assert_eq!(eval_string(&mut ctx, "'' + builtCount"), "500");
    assert_eq!(eval_string(&mut ctx, "'' + clearedCount"), "0");
    assert!(eval_bool(&mut ctx, "endsNull"));
}

#[test]
fn data_property_is_ignored_on_elements() {
    let mut ctx = ctx();
    eval(
        &mut ctx,
        "globalThis.d = document.createElement('div'); __ss_root.appendChild(d); d.data = 'x';",
    );
    let batch = OpBatch::decode(&flush(&mut ctx)).expect("decodes");
    assert!(
        !batch.ops.iter().any(|op| matches!(op, Op::SetText { .. })),
        "element .data must not emit SetText, got {:?}",
        batch.ops
    );
}
