# superui_html Implementation Plan (Phase 1, Plan 2 of 6)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build `superui_html`, a headless crate that parses an HTML document string into a `superui_dom::Dom` tree using `html5ever`, producing the implied `html > head + body` structure with elements, text, and attributes — the on-load bootstrap for the DOM the rest of bevy_superui reconciles from.

**Architecture:** A single `HtmlSink` implements `html5ever`'s `TreeSink` trait and builds directly into a `superui_dom::Dom`. Because every `TreeSink` method takes `&self`, the mutable DOM lives behind a `RefCell<Dom>`. The sink's `Handle` type is a small `Clone` struct `{ id: NodeId, name: QualName }`: it carries the arena handle plus the element's qualified name, so `elem_name` can hand html5ever back a borrowed `&QualName` (which implements `ElemName`) without borrowing through the `RefCell`. Comments, the doctype, and processing instructions are **not** part of our DOM subset: `create_comment`/`create_pi` allocate a throwaway detached node whose id is recorded in an `ignored` set, and every append operation skips ignored children so they never enter the tree; `append_doctype_to_document` is a no-op. The one public entry point is `parse_document(html: &str) -> Dom`.

**Tech Stack:** Rust (edition 2021), `html5ever` 0.39 (pure-Rust, wasm-clean HTML5 parser) driving `superui_dom` from Plan 1. No Bevy, no JS, no async.

## Global Constraints

- **Bevy version target for the overall project: 0.17** — but `superui_html` has NO Bevy dependency and must stay Bevy-version-agnostic.
- **wasm32-unknown-unknown must compile** — `html5ever` / `markup5ever` / `tendril` are pure Rust and wasm-clean (design §5). No `std::time`, no threads, no filesystem in this crate.
- **Graceful degradation over panics** — malformed HTML must parse without panicking; unknown tags become plain elements; unknown/namespaced attributes keep their local name; comments/doctype/PIs are dropped, not fatal.
- **No bespoke web-incompatible surface** — the public API mirrors the browser (`parse_document`); the produced tree mirrors the HTML5 tree-construction result within our node subset.
- **TDD, DRY, YAGNI, frequent commits** — every task is test-first and ends with a commit. Only `parse_document` is public; fragment/`innerHTML` parsing is out of scope for Phase 1 (YAGNI).

**Note on `html5ever` import paths:** the code below uses the module paths expected for 0.39 (`html5ever::interface::{TreeSink, NodeOrText, ElementFlags, QuirksMode}`, `html5ever::{QualName, Attribute, LocalName, Namespace}`, `html5ever::tendril::{StrTendril, TendrilSink}`, `html5ever::driver::{parse_document, ParseOpts}`). These are re-exports from `markup5ever`/`tendril`. If the compiler reports a path as wrong, follow its suggestion — the *types* are correct; only the module a given type is re-exported through may differ. Do not change the design to work around an import path.

---

### Task 1: `superui_html` crate skeleton + dependency

**Files:**
- Modify: `Cargo.toml` (root workspace — add `html5ever` to `[workspace.dependencies]`)
- Create: `crates/superui_html/Cargo.toml`
- Create: `crates/superui_html/src/lib.rs`

**Interfaces:**
- Consumes: the `superui_dom` crate from Plan 1 (path dependency).
- Produces: a compiling `superui_html` library crate in the workspace; `cargo test -p superui_html` runs.

- [ ] **Step 1: Add `html5ever` to the workspace dependency table**

Edit the root `Cargo.toml` `[workspace.dependencies]` section so it reads (add the `html5ever` line; keep the existing `slotmap` line):

```toml
[workspace.dependencies]
slotmap = "1.0"
html5ever = "0.39"
```

- [ ] **Step 2: Create the crate manifest**

Create `crates/superui_html/Cargo.toml`:

```toml
[package]
name = "superui_html"
edition.workspace = true
version.workspace = true
license.workspace = true

[dependencies]
superui_dom = { path = "../superui_dom" }
html5ever.workspace = true
```

- [ ] **Step 3: Create a minimal lib with a smoke test**

Create `crates/superui_html/src/lib.rs`:

```rust
//! HTML-document parsing for bevy_superui.
//!
//! Parses an HTML string into a [`superui_dom::Dom`] via `html5ever`. Knows
//! nothing about Bevy or JavaScript. Headless-testable.

#[cfg(test)]
mod smoke {
    #[test]
    fn crate_builds() {
        assert_eq!(2 + 2, 4);
    }
}
```

- [ ] **Step 4: Run the smoke test to verify the crate resolves and builds**

Run: `cargo test -p superui_html`
Expected: PASS — 1 test (`smoke::crate_builds`) passes; `html5ever` and `superui_dom` resolve.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/superui_html
git commit -m "chore(html): superui_html crate skeleton + html5ever dep"
```

---

### Task 2: `HtmlSink` + `TreeSink` impl + `parse_document`

**Files:**
- Create: `crates/superui_html/src/sink.rs`
- Modify: `crates/superui_html/src/lib.rs`

**Interfaces:**
- Consumes: `superui_dom::{Dom, NodeId, NodeKind}`; `html5ever` `TreeSink`/driver APIs.
- Produces:
  - `pub(crate) struct Handle { pub(crate) id: NodeId, name: QualName }` implementing `Clone`.
  - `pub(crate) struct HtmlSink { dom: RefCell<Dom>, ignored: RefCell<HashSet<NodeId>> }` with `pub(crate) fn new() -> HtmlSink` and a full `impl TreeSink for HtmlSink` (`type Handle = Handle`, `type Output = Dom`, `type ElemName<'a> = &'a QualName`).
  - `pub fn parse_document(html: &str) -> superui_dom::Dom` on the crate root.

- [ ] **Step 1: Write the failing tests**

Add to `crates/superui_html/src/lib.rs`, replacing the `smoke` module with the module wiring and a test module:

```rust
mod sink;

pub use sink::parse_document;

#[cfg(test)]
mod parse_tests {
    use super::parse_document;
    use superui_dom::{Dom, NodeId};

    /// First element (document order) with the given tag name.
    fn first_by_tag(dom: &Dom, tag: &str) -> Option<NodeId> {
        fn walk(dom: &Dom, node: NodeId, tag: &str) -> Option<NodeId> {
            if dom.tag(node) == Some(tag) {
                return Some(node);
            }
            for &c in dom.children(node) {
                if let Some(found) = walk(dom, c, tag) {
                    return Some(found);
                }
            }
            None
        }
        walk(dom, dom.document(), tag)
    }

    #[test]
    fn parses_implied_html_head_body_structure() {
        let dom = parse_document("<div></div>");
        let html = first_by_tag(&dom, "html").expect("implied <html>");
        let head = first_by_tag(&dom, "head").expect("implied <head>");
        let body = first_by_tag(&dom, "body").expect("implied <body>");
        assert_eq!(dom.parent(html), Some(dom.document()));
        assert_eq!(dom.parent(head), Some(html));
        assert_eq!(dom.parent(body), Some(html));
        let div = first_by_tag(&dom, "div").expect("<div>");
        assert_eq!(dom.parent(div), Some(body));
    }

    #[test]
    fn parses_nested_elements() {
        let dom = parse_document("<ul><li></li></ul>");
        let ul = first_by_tag(&dom, "ul").expect("<ul>");
        let li = first_by_tag(&dom, "li").expect("<li>");
        assert_eq!(dom.parent(li), Some(ul));
    }

    #[test]
    fn text_becomes_a_text_node() {
        let dom = parse_document("<p>hello</p>");
        let p = first_by_tag(&dom, "p").expect("<p>");
        assert_eq!(dom.text_content(p), "hello");
    }

    #[test]
    fn attributes_are_parsed() {
        let dom = parse_document(r#"<input type="checkbox" id="done">"#);
        let input = first_by_tag(&dom, "input").expect("<input>");
        assert_eq!(dom.get_attribute(input, "type"), Some("checkbox"));
        assert_eq!(dom.get_attribute(input, "id"), Some("done"));
    }

    #[test]
    fn tag_names_are_lowercased() {
        let dom = parse_document("<DIV></DIV>");
        assert!(first_by_tag(&dom, "div").is_some());
        assert!(first_by_tag(&dom, "DIV").is_none());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p superui_html`
Expected: FAIL — `sink` module / `parse_document` do not exist (unresolved import).

- [ ] **Step 3: Write the sink and `TreeSink` impl**

Create `crates/superui_html/src/sink.rs`:

```rust
use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashSet;

use html5ever::driver::{parse_document as parse_html_document, ParseOpts};
use html5ever::interface::{ElementFlags, NodeOrText, QuirksMode, TreeSink};
use html5ever::tendril::{StrTendril, TendrilSink};
use html5ever::{Attribute, LocalName, Namespace, QualName};

use superui_dom::{Dom, NodeId, NodeKind};

/// A handle to a node in the DOM being built. Carries the arena id plus the
/// element's qualified name, so `elem_name` can hand html5ever a borrowed
/// `&QualName` (which implements `ElemName`) without borrowing through the
/// `RefCell`. Non-element handles (document/text/comment) get a placeholder name.
#[derive(Clone)]
pub(crate) struct Handle {
    pub(crate) id: NodeId,
    name: QualName,
}

/// A `QualName` for nodes that are not elements (never meaningfully queried).
fn placeholder_name() -> QualName {
    QualName::new(None, Namespace::from(""), LocalName::from(""))
}

/// html5ever tree sink that builds a [`superui_dom::Dom`].
///
/// All `TreeSink` methods take `&self`, so the mutable DOM lives behind a
/// `RefCell`. Comment / doctype / processing-instruction nodes are outside our
/// DOM subset: they get a throwaway detached node whose id is recorded in
/// `ignored`, and append operations skip ignored children so they never enter
/// the rendered tree.
pub(crate) struct HtmlSink {
    dom: RefCell<Dom>,
    ignored: RefCell<HashSet<NodeId>>,
}

impl HtmlSink {
    pub(crate) fn new() -> Self {
        HtmlSink {
            dom: RefCell::new(Dom::new()),
            ignored: RefCell::new(HashSet::new()),
        }
    }

    fn is_ignored(&self, id: NodeId) -> bool {
        self.ignored.borrow().contains(&id)
    }

    /// Allocate a detached placeholder node for a dropped comment/PI and record
    /// it as ignored. Returns a handle html5ever can keep referencing safely.
    fn ignored_handle(&self) -> Handle {
        let id = self.dom.borrow_mut().create_text("");
        self.ignored.borrow_mut().insert(id);
        Handle { id, name: placeholder_name() }
    }

    /// Append `text` as a child of `parent`, merging into a trailing text
    /// sibling if present (html5ever expects adjacent text to coalesce).
    fn append_text(&self, parent: NodeId, text: &str) {
        let mut dom = self.dom.borrow_mut();
        if let Some(&last) = dom.children(parent).last() {
            if let Some(node) = dom.get_mut(last) {
                if let NodeKind::Text(s) = &mut node.kind {
                    s.push_str(text);
                    return;
                }
            }
        }
        let t = dom.create_text(text);
        let _ = dom.append_child(parent, t);
    }
}

impl TreeSink for HtmlSink {
    type Handle = Handle;
    type Output = Dom;
    type ElemName<'a> = &'a QualName where Self: 'a;

    fn finish(self) -> Dom {
        self.dom.into_inner()
    }

    fn parse_error(&self, _msg: Cow<'static, str>) {}

    fn get_document(&self) -> Handle {
        let id = self.dom.borrow().document();
        Handle { id, name: placeholder_name() }
    }

    fn elem_name<'a>(&'a self, target: &'a Handle) -> &'a QualName {
        &target.name
    }

    fn create_element(&self, name: QualName, attrs: Vec<Attribute>, _flags: ElementFlags) -> Handle {
        let mut dom = self.dom.borrow_mut();
        let id = dom.create_element(&name.local);
        for attr in attrs {
            let _ = dom.set_attribute(id, &attr.name.local, &attr.value);
        }
        Handle { id, name }
    }

    fn create_comment(&self, _text: StrTendril) -> Handle {
        self.ignored_handle()
    }

    fn create_pi(&self, _target: StrTendril, _data: StrTendril) -> Handle {
        self.ignored_handle()
    }

    fn append(&self, parent: &Handle, child: NodeOrText<Handle>) {
        match child {
            NodeOrText::AppendNode(node) => {
                if self.is_ignored(node.id) {
                    return;
                }
                let _ = self.dom.borrow_mut().append_child(parent.id, node.id);
            }
            NodeOrText::AppendText(text) => self.append_text(parent.id, &text),
        }
    }

    fn append_based_on_parent_node(
        &self,
        element: &Handle,
        prev_element: &Handle,
        child: NodeOrText<Handle>,
    ) {
        let has_parent = self.dom.borrow().parent(element.id).is_some();
        if has_parent {
            self.append_before_sibling(element, child);
        } else {
            self.append(prev_element, child);
        }
    }

    fn append_doctype_to_document(&self, _name: StrTendril, _public: StrTendril, _system: StrTendril) {}

    fn get_template_contents(&self, target: &Handle) -> Handle {
        // Our subset has no separate template document fragment; template
        // children just live under the element itself (graceful degradation).
        target.clone()
    }

    fn same_node(&self, x: &Handle, y: &Handle) -> bool {
        x.id == y.id
    }

    fn set_quirks_mode(&self, _mode: QuirksMode) {}

    fn append_before_sibling(&self, sibling: &Handle, new_node: NodeOrText<Handle>) {
        // Copy the parent out first so the immutable borrow is dropped before we
        // take a mutable borrow in the arms below.
        let parent = self.dom.borrow().parent(sibling.id);
        let parent = match parent {
            Some(p) => p,
            None => return, // detached sibling; nothing to do
        };
        match new_node {
            NodeOrText::AppendNode(node) => {
                if self.is_ignored(node.id) {
                    return;
                }
                let _ = self
                    .dom
                    .borrow_mut()
                    .insert_before(parent, node.id, Some(sibling.id));
            }
            NodeOrText::AppendText(text) => {
                let mut dom = self.dom.borrow_mut();
                if let Some(prev) = dom.previous_sibling(sibling.id) {
                    if let Some(node) = dom.get_mut(prev) {
                        if let NodeKind::Text(s) = &mut node.kind {
                            s.push_str(&text);
                            return;
                        }
                    }
                }
                let t = dom.create_text(&text);
                let _ = dom.insert_before(parent, t, Some(sibling.id));
            }
        }
    }

    fn add_attrs_if_missing(&self, target: &Handle, attrs: Vec<Attribute>) {
        let mut dom = self.dom.borrow_mut();
        for attr in attrs {
            if dom.get_attribute(target.id, &attr.name.local).is_none() {
                let _ = dom.set_attribute(target.id, &attr.name.local, &attr.value);
            }
        }
    }

    fn remove_from_parent(&self, target: &Handle) {
        let mut dom = self.dom.borrow_mut();
        if let Some(parent) = dom.parent(target.id) {
            let _ = dom.remove_child(parent, target.id);
        }
    }

    fn reparent_children(&self, node: &Handle, new_parent: &Handle) {
        let mut dom = self.dom.borrow_mut();
        let kids: Vec<NodeId> = dom.children(node.id).to_vec();
        for child in kids {
            // append_child detaches from the old parent, preserving order.
            let _ = dom.append_child(new_parent.id, child);
        }
    }
}

/// Parse a full HTML document into a fresh [`superui_dom::Dom`].
///
/// Produces the implied `html > head + body` structure like a browser. Unknown
/// tags become plain elements; comments, the doctype, and processing
/// instructions are dropped (outside our DOM subset). Never panics on malformed
/// input.
pub fn parse_document(html: &str) -> Dom {
    let sink = HtmlSink::new();
    parse_html_document(sink, ParseOpts::default()).one(html)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p superui_html`
Expected: PASS — all five `parse_tests` cases pass. (If the build fails on an `html5ever` import path, adjust the `use` line per the compiler's suggestion — see the Global Constraints note — without changing the design.)

- [ ] **Step 5: Commit**

```bash
git add crates/superui_html
git commit -m "feat(html): TreeSink-backed parse_document into superui_dom"
```

---

### Task 3: Subset fidelity — comments/doctype dropped, text coalescing, void & unknown tags

**Files:**
- Modify: `crates/superui_html/src/lib.rs` (extend the `parse_tests` module)

**Interfaces:**
- Consumes: `parse_document` + the `first_by_tag` test helper from Task 2.
- Produces: no new public API — this task pins down the subset/degradation behavior of the Task 2 sink. Any failure here is a real behavior gap (e.g. a comment leaking into the tree, or uncoalesced text), which a reviewer could reject independently of Task 2's basic-parse deliverable.

- [ ] **Step 1: Write the failing tests**

Append these tests inside the `parse_tests` module in `crates/superui_html/src/lib.rs` (after the existing tests, before the closing `}`):

```rust
    /// Count elements (document order) with the given tag name.
    fn count_by_tag(dom: &Dom, tag: &str) -> usize {
        fn walk(dom: &Dom, node: NodeId, tag: &str, acc: &mut usize) {
            if dom.tag(node) == Some(tag) {
                *acc += 1;
            }
            for &c in dom.children(node) {
                walk(dom, c, tag, acc);
            }
        }
        let mut n = 0;
        walk(dom, dom.document(), tag, &mut n);
        n
    }

    #[test]
    fn comments_are_dropped() {
        let dom = parse_document("<div><!-- a comment --></div>");
        let div = first_by_tag(&dom, "div").expect("<div>");
        assert_eq!(dom.children(div).len(), 0);
        assert_eq!(dom.text_content(div), "");
    }

    #[test]
    fn doctype_is_dropped() {
        let dom = parse_document("<!DOCTYPE html><html><head></head><body></body></html>");
        // The document's only child is <html>; no doctype node was added.
        assert_eq!(dom.children(dom.document()).len(), 1);
        let html = first_by_tag(&dom, "html").expect("<html>");
        assert_eq!(dom.children(dom.document()), &[html]);
    }

    #[test]
    fn plain_text_is_a_single_text_node() {
        let dom = parse_document("<p>Hello, world</p>");
        let p = first_by_tag(&dom, "p").expect("<p>");
        assert_eq!(dom.children(p).len(), 1);
        assert_eq!(dom.text_content(p), "Hello, world");
    }

    #[test]
    fn text_split_by_a_dropped_comment_is_coalesced() {
        // html5ever emits text "a", a comment, then text "b". The comment is
        // dropped, and "b" must merge into the "a" text node left behind.
        let dom = parse_document("<p>a<!--x-->b</p>");
        let p = first_by_tag(&dom, "p").expect("<p>");
        assert_eq!(dom.children(p).len(), 1);
        assert_eq!(dom.text_content(p), "ab");
    }

    #[test]
    fn void_element_has_no_children_and_next_is_a_sibling() {
        let dom = parse_document("<input><span></span>");
        let input = first_by_tag(&dom, "input").expect("<input>");
        let span = first_by_tag(&dom, "span").expect("<span>");
        assert_eq!(dom.children(input).len(), 0);
        // input and span are siblings (span is NOT a child of the void input).
        assert_eq!(dom.parent(input), dom.parent(span));
    }

    #[test]
    fn unknown_tag_becomes_a_plain_element() {
        let dom = parse_document("<my-widget></my-widget>");
        assert_eq!(count_by_tag(&dom, "my-widget"), 1);
    }

    #[test]
    fn boolean_attribute_is_present_with_empty_value() {
        let dom = parse_document(r#"<input type="checkbox" checked>"#);
        let input = first_by_tag(&dom, "input").expect("<input>");
        assert!(dom.has_attribute(input, "checked"));
        assert_eq!(dom.get_attribute(input, "checked"), Some(""));
    }
```

- [ ] **Step 2: Run tests to verify status**

Run: `cargo test -p superui_html`
Expected: The new tests exercise behavior the Task 2 sink already implements (comment ignore-set, text coalescing, html5ever's own void-element and tag handling). They should PASS. If any FAIL, that is a real defect in the sink — fix `sink.rs` (do not weaken the test): e.g. confirm `append_text`/`append_before_sibling` merge into a trailing/preceding text node, and that `append` early-returns for ignored handles.

- [ ] **Step 3: Commit**

```bash
git add crates/superui_html
git commit -m "test(html): pin subset fidelity — comments/doctype dropped, text coalescing, void/unknown tags"
```

---

### Task 4: Robustness, integration, wasm check, ledger status

**Files:**
- Modify: `crates/superui_html/src/lib.rs` (extend the `parse_tests` module)
- Modify: `docs/superpowers/plans/README.md` (mark Plan 2 done)

**Interfaces:**
- Consumes: `parse_document`, `first_by_tag`, `count_by_tag` helpers.
- Produces: no new public API — malformed-input robustness, a `getElementById`-on-parsed-tree check, a TodoMVC-shaped integration test, a verified wasm build, and the plan-series status update.

- [ ] **Step 1: Write the failing tests**

Append these tests inside the `parse_tests` module in `crates/superui_html/src/lib.rs` (after the Task 3 tests, before the closing `}`):

```rust
    #[test]
    fn unclosed_tags_recover_without_panicking() {
        // Two <li> with no closing tags: the tree builder auto-closes the first.
        let dom = parse_document("<ul><li>a<li>b</ul>");
        assert_eq!(count_by_tag(&dom, "li"), 2);
    }

    #[test]
    fn mis_nested_tags_do_not_panic() {
        // Adoption-agency territory; we only require that it parses and a body
        // exists (exercises reparent_children / remove_from_parent paths).
        let dom = parse_document("<b><i>x</b>y</i>");
        assert!(first_by_tag(&dom, "body").is_some());
        assert!(first_by_tag(&dom, "b").is_some());
        assert!(first_by_tag(&dom, "i").is_some());
    }

    #[test]
    fn get_element_by_id_works_on_the_parsed_tree() {
        let dom = parse_document(r#"<div><input id="new-todo"></div>"#);
        let by_id = dom.get_element_by_id("new-todo").expect("element with id");
        assert_eq!(dom.tag(by_id), Some("input"));
        assert_eq!(dom.get_element_by_id("missing"), None);
    }

    #[test]
    fn parses_a_todomvc_shaped_fragment() {
        let html = r#"
            <section class="todoapp">
              <header class="header">
                <h1>todos</h1>
                <input class="new-todo" placeholder="What needs to be done?">
              </header>
              <ul class="todo-list">
                <li class="completed">
                  <div class="view">
                    <input class="toggle" type="checkbox" checked>
                    <label>Taste JavaScript</label>
                    <button class="destroy"></button>
                  </div>
                </li>
                <li>
                  <div class="view">
                    <input class="toggle" type="checkbox">
                    <label>Buy a unicorn</label>
                  </div>
                </li>
              </ul>
            </section>
        "#;
        let dom = parse_document(html);

        let section = first_by_tag(&dom, "section").expect("<section>");
        assert!(dom.class_contains(section, "todoapp"));

        // Two todo items.
        assert_eq!(count_by_tag(&dom, "li"), 2);

        // The first toggle input is checked; the second is not.
        let toggles: Vec<NodeId> = {
            let mut v = Vec::new();
            fn walk(dom: &Dom, node: NodeId, v: &mut Vec<NodeId>) {
                if dom.tag(node) == Some("input") && dom.class_contains(node, "toggle") {
                    v.push(node);
                }
                for &c in dom.children(node) {
                    walk(dom, c, v);
                }
            }
            walk(&dom, dom.document(), &mut v);
            v
        };
        assert_eq!(toggles.len(), 2);
        assert!(dom.has_attribute(toggles[0], "checked"));
        assert!(!dom.has_attribute(toggles[1], "checked"));

        // Labels carry the expected text.
        let labels: Vec<String> = {
            let mut v = Vec::new();
            fn walk(dom: &Dom, node: NodeId, v: &mut Vec<String>) {
                if dom.tag(node) == Some("label") {
                    v.push(dom.text_content(node));
                }
                for &c in dom.children(node) {
                    walk(dom, c, v);
                }
            }
            walk(&dom, dom.document(), &mut v);
            v
        };
        assert_eq!(labels, vec!["Taste JavaScript".to_string(), "Buy a unicorn".to_string()]);
    }
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test -p superui_html`
Expected: PASS — every `parse_tests` case (Tasks 2–4) passes. If a robustness test panics, that is a defect: ensure the `TreeSink` mutation methods use `superui_dom`'s `Result`-returning ops with `let _ =` (never `.unwrap()`), so hierarchy violations degrade instead of aborting.

- [ ] **Step 3: Verify the crate compiles for wasm**

Run: `cargo build -p superui_html --target wasm32-unknown-unknown`
Expected: SUCCESS — no errors (confirms the Global Constraint that `html5ever` keeps this crate wasm-clean).

- [ ] **Step 4: Mark Plan 2 done in the plan-series index**

In `docs/superpowers/plans/README.md`, replace the Plan 2 table row:

```markdown
| 2 | `superui_html` | HTML subset → DOM, via `html5ever`. | ⬜ Not started |
```

with:

```markdown
| 2 | `superui_html` | HTML subset → DOM, via `html5ever`. | ✅ Done — merged to `main` ([plan](./2026-07-18-superui-phase1-02-html.md)) |
```

- [ ] **Step 5: Commit**

```bash
git add crates/superui_html docs/superpowers/plans/README.md
git commit -m "feat(html): robustness + TodoMVC-shaped integration test; wasm-clean; mark Plan 2 done"
```

---

## Self-Review

**Spec coverage (against design doc §4 `superui_html` responsibilities and §9 HTML subset):**
- "HTML subset → DOM, via html5ever" → Tasks 2–4 (full `TreeSink` → `Dom`). ✅
- "`div`, `span`, `p`, `ul`/`li`, `button`, `input`, `label`, `h1`–`h6`, text nodes" → covered generically (any tag becomes an element) and exercised by the TodoMVC fragment (Task 4) + nested/text tests (Task 2). ✅
- "attributes `class`/`id`/`type`/`value`/`placeholder`/`checked`" → attribute parsing (Task 2) + boolean `checked` (Task 3) + TodoMVC fragment (Task 4). ✅
- "Unknown tags render as plain boxes; unknown attributes ignored" → `unknown_tag_becomes_a_plain_element` (Task 3); namespaced/unknown attributes keep their local name via `attr.name.local`. ✅
- "Knows nothing about Bevy or JS. Headless-testable." → no Bevy/JS deps; all tests headless. ✅
- "Unsupported features degrade gracefully … keep running, not crash" (design §1) → comments/doctype/PI dropped (Task 3), malformed/mis-nested input recovers without panic (Task 4), all mutation ops use `let _ =` on `Result`. ✅
- wasm-clean (Global Constraint) → verified in Task 4 Step 3. ✅
- YAGNI: only `parse_document` is public; fragment/`innerHTML` parsing deliberately omitted (not in the Phase 1 DOM API list). ✅

**Placeholder scan:** No TBD/TODO; every code step contains complete code; every test step contains real assertions. The only conditional guidance is the `html5ever` import-path note, which points to the compiler for the exact re-export module — the types themselves are fixed. ✅

**Type consistency:** `Handle { id, name }`, `HtmlSink`, `parse_document`, and the `superui_dom` API used (`create_element`, `set_attribute`, `get_attribute`, `has_attribute`, `class_contains`, `children`, `parent`, `previous_sibling`, `insert_before`, `append_child`, `remove_child`, `create_text`, `text_content`, `get_mut`, `tag`, `get_element_by_id`, `NodeKind::Text`) all match the surface produced by Plan 1 (`superui_dom`). `type Output = Dom` matches `finish`'s return and `parse_document`'s return. `type ElemName<'a> = &'a QualName` matches `elem_name`'s signature and relies on `&QualName: ElemName` (html5ever 0.39). ✅

**Note on the ignore-set:** `create_comment`/`create_pi` allocate a real detached `Text("")` node so every handle is a valid, unique `NodeId` (keeping `same_node` correct); membership in `ignored` makes `append`/`append_before_sibling` skip it, so it never links into the tree. The orphaned nodes remain unreferenced in the arena for the life of the parse — negligible and never rendered.

---
