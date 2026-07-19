# Supersolid Phase 2 — Plan 1: Lower-level DOM/JS prerequisites — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the two small, already-mostly-there DOM/JS capabilities the Supersolid runtime depends on — mutating a **text node's** value from JS, and a *proven* guarantee that node **reorder/reparent** reconciles into Bevy — without building any of the framework itself.

**Architecture:** Two isolated changes to existing Phase-1 crates.
1. **`superui_api`** — install `data` / `nodeValue` / `textContent` accessors on the **text** prototype. The element proto already has `textContent` (`element.rs:188`); the text proto (`lib.rs:32`) has *no* methods installed, so a text node can't be updated from JS today. The existing `text_content_get`/`text_content_set` already handle text nodes correctly (they call `superui_dom::set_text_content`/`text_content`, which store/read `NodeKind::Text(String)` directly) — so this is pure wiring: expose those two functions and bind them on the text proto.
2. **`superui_bridge`** — add characterization tests proving the reconciler's `sync_children` + `replace_children` (`reconcile.rs:144`) already reorders siblings and reparents a moved node correctly. This is the exact behavior Supersolid's keyed `<For>` will rely on; we lock it with tests, not new code.

**Tech Stack:** Rust, Bevy 0.17, Boa 0.21 (`boa_engine`), the `superui_*` crates. No new dependencies.

## Global Constraints

- **Bevy 0.17**, edition 2021.
- `superui_dom` / `superui_js` / `superui_api` stay **Bevy-free and wasm-clean**; only `superui_bridge` touches Bevy.
- **TDD** throughout; **frequent commits**; execute on a feature branch merged to `main`.
- Public DOM surface **mirrors web standards** (`.data`/`.nodeValue`/`.textContent` are the real DOM names).
- **Out of scope for this plan (deferred):** `document.createDocumentFragment` (adding a `NodeKind` variant ripples through many match sites; the Supersolid render layer will instead insert around anchor text nodes, and this is revisited in the render-layer plan only if measurements demand it). Keyboard `event.key`/`code` and `focus()`/`:focus` are Phase-3 browser-compat items, not runtime prerequisites — the Supersolid TodoMVC mirrors the existing example (Add button, no Enter), so it does not need them.

---

## Task 1: Text-node value accessors (`data` / `nodeValue` / `textContent`)

**Files:**
- Modify: `crates/superui_api/src/element.rs` (make `text_content_get`/`text_content_set` `pub(crate)`)
- Create: `crates/superui_api/src/text.rs` (install accessors on the text proto)
- Modify: `crates/superui_api/src/lib.rs` (register `mod text;` + call `text::install_text`)
- Modify: `docs/support/js-dom.md` (ledger row)
- Test: `crates/superui_api/src/lib.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: `superui_api::element::text_content_get` / `text_content_set` — signature `fn(&JsValue, &[JsValue], &mut Context) -> JsResult<JsValue>`; `superui_api::node::set_accessor(proto, name, getter, setter, context)`; `superui_js::with_host_state`; `HostState.protos.text`.
- Produces: `superui_api::text::install_text(context: &mut Context)` — installs `data`/`nodeValue`/`textContent` on the text proto; called from `install()`.

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests` block in `crates/superui_api/src/lib.rs`:

```rust
#[test]
fn text_node_value_accessors_write_through_to_dom() {
    // Seed: document > div#host > (text "a"). We keep the text NodeId so we can
    // assert the accessor writes reach the real DOM, not a stray JS own-property.
    let dom = Rc::new(RefCell::new(Dom::new()));
    let text_id = {
        let mut d = dom.borrow_mut();
        let doc = d.document();
        let host = d.create_element("div");
        d.set_attribute(host, "id", "host").unwrap();
        let t = d.create_text("a");
        d.append_child(doc, host).unwrap();
        d.append_child(host, t).unwrap();
        t
    };
    let mut e = BoaEngine::new(dom.clone());
    install(&mut e);

    e.eval(
        r#"
        var host = document.getElementById('host');
        var t = host.childNodes[0];      // the text node
        globalThis.g0 = t.data;          // getter reads the DOM -> "a"
        t.data = 'b';                    // setter writes the DOM
        globalThis.g1 = t.nodeValue;     // -> "b"
        t.textContent = 'c';             // last write wins
        "#,
    )
    .unwrap();

    let check = |e: &mut BoaEngine, expr: &str| -> String {
        e.context_mut()
            .eval(boa_engine::Source::from_bytes(expr))
            .unwrap()
            .to_string(e.context_mut())
            .unwrap()
            .to_std_string_escaped()
    };
    assert_eq!(check(&mut e, "globalThis.g0"), "a");
    assert_eq!(check(&mut e, "globalThis.g1"), "b");
    // The accessor wrote through to the arena DOM (read back via the known id).
    assert_eq!(dom.borrow().text_content(text_id), "c");
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p superui_api text_node_value_accessors_write_through_to_dom`
Expected: FAIL — with no accessor on the text proto, `t.data` is `undefined` so `g0 == "undefined"` (not `"a"`), and `t.data = 'b'` sets a plain JS own-property instead of the DOM, so the final `text_content(text_id)` is still `"a"` (not `"c"`).

- [ ] **Step 3: Make the two content functions crate-visible**

In `crates/superui_api/src/element.rs`, change both signatures (currently `fn`) to `pub(crate) fn`:

```rust
pub(crate) fn text_content_get(this: &JsValue, _a: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let Some(n) = node_id_of(this) else { return Ok(jsstr("")) };
    let t = dom_of(context).borrow().text_content(n);
    Ok(jsstr(&t))
}
pub(crate) fn text_content_set(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let text = args.get_or_undefined(0).to_string(context)?.to_std_string_escaped();
    if let Some(n) = node_id_of(this) {
        dom_of(context).borrow_mut().set_text_content(n, &text);
    }
    Ok(JsValue::undefined())
}
```

- [ ] **Step 4: Create the text-proto installer**

Create `crates/superui_api/src/text.rs`:

```rust
//! Text-node value accessors (`data` / `nodeValue` / `textContent`) on the shared
//! text prototype. All three map to the node's text data, which `superui_dom`
//! stores directly in `NodeKind::Text` — so they reuse the element crate's
//! `text_content_get`/`text_content_set` (correct for text nodes as-is).

use boa_engine::Context;
use superui_js::with_host_state;

use crate::element::{text_content_get, text_content_set};
use crate::node::set_accessor;

/// Install text-node value accessors onto the text proto.
pub fn install_text(context: &mut Context) {
    let text = with_host_state(context, |s| s.protos.text.clone()).expect("text proto");
    set_accessor(&text, "textContent", text_content_get, text_content_set, context);
    set_accessor(&text, "nodeValue", text_content_get, text_content_set, context);
    set_accessor(&text, "data", text_content_get, text_content_set, context);
}
```

- [ ] **Step 5: Register the module and call the installer**

In `crates/superui_api/src/lib.rs`, add the module declaration next to the others (after `mod node;`):

```rust
mod node;
mod text;
mod timers;
```

And in `install()`, add the call after `element::install_element(context);`:

```rust
    element::install_element(context);
    text::install_text(context);
    events::install_events(context);
```

- [ ] **Step 6: Run the test to verify it passes**

Run: `cargo test -p superui_api text_node_value_accessors_write_through_to_dom`
Expected: PASS

- [ ] **Step 7: Update the capability ledger**

In `docs/support/js-dom.md`, in the **`## Element — attributes / content / state`** table, add a row immediately after the `textContent / innerText` row:

```markdown
| text node `.data` / `.nodeValue` / `.textContent` (get/set) | ✅ | T0 | mutate a Text node's value from JS — Supersolid text bindings |
```

- [ ] **Step 8: Run the whole api crate's tests (no regressions) and commit**

Run: `cargo test -p superui_api`
Expected: PASS (all existing tests + the new one)

```bash
git add crates/superui_api/src/element.rs crates/superui_api/src/text.rs crates/superui_api/src/lib.rs docs/support/js-dom.md
git commit -m "feat(api): text-node value accessors (data/nodeValue/textContent) for Supersolid

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 2: Reconciler reorder / reparent characterization tests

These lock in existing correct behavior that Supersolid's keyed `<For>` depends on. They are **expected to pass** against the current reconciler (which rebuilds child order from DOM order and calls `replace_children`). If either fails, it has found a real reconciler bug — stop and fix `sync_children`'s child-ordering before proceeding; based on the current code (`reconcile.rs:89-145`) both should pass.

**Files:**
- Test: `crates/superui_bridge/tests/reconcile.rs` (add two tests + one import)

**Interfaces:**
- Consumes: `support::{test_app, mount, child_count}`; `superui_bridge::UiRuntime` with `entity_for(node) -> Option<Entity>` and public `dirty: bool`; `superui_dom::Dom::{query_selector, children, insert_before, append_child, document}`; `superui_html::parse_document`; Bevy `Children` / `Text`.

- [ ] **Step 1: Add the `UiRuntime` import**

At the top of `crates/superui_bridge/tests/reconcile.rs`, ensure this import is present (add it if not already there):

```rust
use superui_bridge::UiRuntime;
```

- [ ] **Step 2: Write the reorder test**

Append to `crates/superui_bridge/tests/reconcile.rs`:

```rust
#[test]
fn insert_before_reorders_children_on_reconcile() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<ul><li>one</li><li>two</li></ul>",
    )));
    let mut app = test_app();
    let _root = mount(&mut app, dom.clone());
    app.update(); // initial reconcile

    let (ul, li_one, li_two) = {
        let d = dom.borrow();
        let ul = d.query_selector(d.document(), "ul").unwrap();
        let kids = d.children(ul).to_vec();
        (ul, kids[0], kids[1])
    };

    // Move the second <li> before the first — a keyed-<For> style reorder.
    dom.borrow_mut()
        .insert_before(ul, li_two, Some(li_one))
        .unwrap();
    app.world_mut().non_send_resource_mut::<UiRuntime>().dirty = true;
    app.update();

    // The <ul> entity's children now read ["two", "one"].
    let ul_entity = app
        .world()
        .non_send_resource::<UiRuntime>()
        .entity_for(ul)
        .unwrap();
    let li_entities = app.world().get::<Children>(ul_entity).unwrap().to_vec();
    let labels: Vec<String> = li_entities
        .iter()
        .map(|&li| {
            let text_entity = app.world().get::<Children>(li).unwrap()[0];
            app.world().get::<Text>(text_entity).unwrap().0.clone()
        })
        .collect();
    assert_eq!(labels, vec!["two".to_string(), "one".to_string()]);
}
```

- [ ] **Step 3: Write the reparent test**

Append to the same file:

```rust
#[test]
fn append_child_reparents_entity_on_reconcile() {
    let dom = Rc::new(RefCell::new(superui_html::parse_document(
        "<div id='a'><span>x</span></div><div id='b'></div>",
    )));
    let mut app = test_app();
    let _root = mount(&mut app, dom.clone());
    app.update(); // initial reconcile

    let (a, b, span) = {
        let d = dom.borrow();
        let doc = d.document();
        (
            d.query_selector(doc, "#a").unwrap(),
            d.query_selector(doc, "#b").unwrap(),
            d.query_selector(doc, "span").unwrap(),
        )
    };

    // Move <span> from #a to #b (append reparents an attached node).
    dom.borrow_mut().append_child(b, span).unwrap();
    app.world_mut().non_send_resource_mut::<UiRuntime>().dirty = true;
    app.update();

    let (a_entity, b_entity) = {
        let rt = app.world().non_send_resource::<UiRuntime>();
        (rt.entity_for(a).unwrap(), rt.entity_for(b).unwrap())
    };
    assert_eq!(child_count(&mut app, a_entity), 0);
    assert_eq!(child_count(&mut app, b_entity), 1);
}
```

- [ ] **Step 4: Run both tests to verify they pass**

Run: `cargo test -p superui_bridge --test reconcile insert_before_reorders_children_on_reconcile append_child_reparents_entity_on_reconcile`
Expected: PASS (both). If either FAILS, the reconciler does not correctly reorder/reparent — fix `sync_children` in `crates/superui_bridge/src/reconcile.rs` so the rebuilt child-entity vector follows DOM order and `replace_children` is applied, then re-run.

- [ ] **Step 5: Run the bridge crate's reconcile tests and commit**

Run: `cargo test -p superui_bridge --test reconcile`
Expected: PASS (existing + two new)

```bash
git add crates/superui_bridge/tests/reconcile.rs
git commit -m "test(bridge): lock reconciler reorder + reparent for Supersolid keyed <For>

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Done-when

- `cargo test -p superui_api` and `cargo test -p superui_bridge` both green.
- A text node's value is settable from JS via `.data` / `.nodeValue` / `.textContent`, writing through to the arena DOM (Task 1 test).
- Reorder-via-`insertBefore` and reparent-via-`appendChild` provably reconcile into correct Bevy child order/parentage (Task 2 tests).
- `docs/support/js-dom.md` marks text-node value mutation ✅.
- `wasm32-unknown-unknown` remains unaffected: only `superui_api` (already wasm-clean) and a `superui_bridge` test changed; no new deps. (Optional check: `cargo build -p superui_api --target wasm32-unknown-unknown`.)

## Self-review (author)

- **Spec coverage:** implements the two runtime prerequisites the direction spec §5/§11 imply for the render layer (text updates + keyed-list moves); `createDocumentFragment` and keyboard/focus are explicitly deferred above with rationale.
- **No placeholders:** every step has real code, exact paths, exact commands, and expected output.
- **Type consistency:** `text_content_get`/`text_content_set` names, the `set_accessor(proto, name, getter, setter, context)` arity, `install_text(context)`, `UiRuntime::entity_for`/`dirty`, and `Dom::{insert_before, append_child, query_selector, children, document}` all match the crates as read.
