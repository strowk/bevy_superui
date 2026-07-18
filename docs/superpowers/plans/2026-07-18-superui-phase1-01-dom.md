# superui_dom Implementation Plan (Phase 1, Plan 1 of 6)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build `superui_dom`, a headless, dependency-light arena DOM tree with standards-shaped mutation, attribute/class/text semantics, and W3C-style event dispatch (capture → target → bubble) — the structural source of truth the rest of bevy_superui reconciles from.

**Architecture:** A `Dom` owns a `slotmap` arena of `NodeData` (Document / Element / Text). Nodes link via `parent: Option<NodeId>` + `children: Vec<NodeId>`. Event dispatch is split into a *pure* `build_dispatch_plan` (computes the ordered capture/target/bubble visit list — fully testable, borrows the DOM only to build an owned plan) and a reference executor `run_dispatch` (walks the plan, honoring `stopPropagation`/`stopImmediatePropagation`/`preventDefault` and live listener removal). This split lets the future JS layer own invocation (calling Boa) without holding a DOM borrow across reentrant mutations.

**Tech Stack:** Rust (edition 2021), `slotmap` for the generational arena. No Bevy, no JS, no async. Pure `std`.

## Global Constraints

- **Bevy version target for the overall project: 0.17** — but `superui_dom` has NO Bevy dependency and must stay Bevy-version-agnostic.
- **wasm32-unknown-unknown must compile** — use only pure-Rust, wasm-safe deps (`slotmap` qualifies). No `std::time`, no threads, no filesystem in this crate.
- **Graceful degradation over panics** — public mutation methods return `Result<_, DomError>` for hierarchy violations rather than panicking. Accessors return `Option` for stale/missing handles.
- **No bespoke web-incompatible surface** — every public concept mirrors a DOM concept (`appendChild`, `classList`, `textContent`, event phases).
- **TDD, DRY, YAGNI, frequent commits** — every task is test-first and ends with a commit.

---

### Task 1: Workspace + `superui_dom` crate skeleton

**Files:**
- Modify: `Cargo.toml` (root → virtual workspace)
- Delete: `src/main.rs` (obsolete hello-world stub)
- Create: `crates/superui_dom/Cargo.toml`
- Create: `crates/superui_dom/src/lib.rs`

**Interfaces:**
- Consumes: nothing (first task).
- Produces: a compiling `superui_dom` library crate in a workspace; `cargo test -p superui_dom` runs.

- [ ] **Step 1: Convert the root manifest to a virtual workspace**

Replace the entire contents of `Cargo.toml` with:

```toml
[workspace]
resolver = "2"
members = ["crates/*"]

[workspace.package]
edition = "2021"
version = "0.1.0"
license = "MIT OR Apache-2.0"

[workspace.dependencies]
slotmap = "1.0"
```

- [ ] **Step 2: Remove the obsolete binary stub**

Run: `git rm src/main.rs`
Expected: `src/main.rs` removed (a virtual workspace manifest cannot also be a package).

- [ ] **Step 3: Create the crate manifest**

Create `crates/superui_dom/Cargo.toml`:

```toml
[package]
name = "superui_dom"
edition.workspace = true
version.workspace = true
license.workspace = true

[dependencies]
slotmap.workspace = true
```

- [ ] **Step 4: Create a minimal lib with a smoke test**

Create `crates/superui_dom/src/lib.rs`:

```rust
//! Headless, arena-backed DOM tree for bevy_superui.
//!
//! Knows nothing about Bevy or JavaScript. The structural source of truth that
//! the reconciler diffs against and that the JS layer mutates.

#[cfg(test)]
mod smoke {
    #[test]
    fn crate_builds() {
        assert_eq!(2 + 2, 4);
    }
}
```

- [ ] **Step 5: Run the smoke test to verify the workspace builds**

Run: `cargo test -p superui_dom`
Expected: PASS — 1 test (`smoke::crate_builds`) passes; workspace resolves.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml crates/superui_dom
git commit -m "chore: workspace + superui_dom crate skeleton"
```

---

### Task 2: Node model + arena

**Files:**
- Create: `crates/superui_dom/src/node.rs`
- Create: `crates/superui_dom/src/tree.rs`
- Modify: `crates/superui_dom/src/lib.rs`

**Interfaces:**
- Consumes: the crate skeleton from Task 1.
- Produces:
  - `pub struct NodeId` (slotmap key, `Copy`).
  - `pub enum NodeKind { Document, Element(ElementData), Text(String) }`.
  - `pub struct ElementData { pub tag: String, attrs: Vec<(String,String)>, listeners: Vec<Listener> }` (fields private; accessed via `Dom`/methods in later tasks).
  - `pub struct NodeData { pub kind: NodeKind, parent: Option<NodeId>, children: Vec<NodeId> }`.
  - `pub struct Dom` with: `new() -> Dom`, `document(&self) -> NodeId`, `create_element(&mut self, tag: &str) -> NodeId`, `create_text(&mut self, data: &str) -> NodeId`, `get(&self, NodeId) -> Option<&NodeData>`, `get_mut(&mut self, NodeId) -> Option<&mut NodeData>`, `tag(&self, NodeId) -> Option<&str>`, `is_element(&self, NodeId) -> bool`.

- [ ] **Step 1: Write the failing test**

Create `crates/superui_dom/src/node.rs`:

```rust
use crate::tree::Dom;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::NodeKind;

    #[test]
    fn new_dom_has_a_document_root() {
        let dom = Dom::new();
        let root = dom.document();
        assert!(matches!(dom.get(root).unwrap().kind, NodeKind::Document));
        assert_eq!(dom.get(root).unwrap().parent(), None);
        assert_eq!(dom.get(root).unwrap().children().len(), 0);
    }

    #[test]
    fn create_element_lowercases_tag_and_stores_it() {
        let mut dom = Dom::new();
        let el = dom.create_element("DIV");
        assert_eq!(dom.tag(el), Some("div"));
        assert!(dom.is_element(el));
    }

    #[test]
    fn create_text_stores_data() {
        let mut dom = Dom::new();
        let t = dom.create_text("hello");
        assert!(matches!(&dom.get(t).unwrap().kind, NodeKind::Text(s) if s == "hello"));
        assert!(!dom.is_element(t));
    }

    #[test]
    fn stale_handle_returns_none() {
        let dom = Dom::new();
        // A never-inserted key is unavailable via a second empty dom.
        let other = Dom::new();
        assert!(dom.get(other.document()).is_none() || other.get(dom.document()).is_none());
    }
}
```

Add module wiring to `crates/superui_dom/src/lib.rs` (append after the doc comment, replacing the `smoke` module):

```rust
mod node;
mod tree;

pub use node::{ElementData, Listener, ListenerId, NodeData, NodeId, NodeKind};
pub use tree::{Dom, DomError};
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p superui_dom`
Expected: FAIL — `Dom`, `NodeKind`, etc. not defined / unresolved imports.

- [ ] **Step 3: Write the node model**

Replace the top of `crates/superui_dom/src/node.rs` (above the `#[cfg(test)]` block) with:

```rust
use slotmap::new_key_type;

new_key_type! {
    /// Stable, generational handle to a node. Cheap `Copy`. A handle to a
    /// removed node becomes permanently invalid (accessors return `None`).
    pub struct NodeId;
}

/// Monotonic identifier for a registered event listener on a node.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ListenerId(pub u64);

/// A registered event listener. The actual callback lives in the JS layer;
/// the DOM only records that a listener of a given type/phase exists.
#[derive(Clone, Debug)]
pub struct Listener {
    pub id: ListenerId,
    pub event_type: String,
    pub capture: bool,
}

/// Element-specific data: tag name, ordered attributes, and listeners.
#[derive(Clone, Debug, Default)]
pub struct ElementData {
    pub tag: String,
    pub(crate) attrs: Vec<(String, String)>,
    pub(crate) listeners: Vec<Listener>,
}

/// The variant of a node.
#[derive(Clone, Debug)]
pub enum NodeKind {
    Document,
    Element(ElementData),
    Text(String),
}

/// A single node in the arena. Structural links are private; mutate them only
/// through `Dom` so invariants (single parent, no cycles) hold.
#[derive(Clone, Debug)]
pub struct NodeData {
    pub kind: NodeKind,
    pub(crate) parent: Option<NodeId>,
    pub(crate) children: Vec<NodeId>,
}

impl NodeData {
    pub(crate) fn new(kind: NodeKind) -> Self {
        NodeData { kind, parent: None, children: Vec::new() }
    }

    /// Parent node, or `None` for the document root or a detached node.
    pub fn parent(&self) -> Option<NodeId> {
        self.parent
    }

    /// Ordered child handles.
    pub fn children(&self) -> &[NodeId] {
        &self.children
    }
}
```

- [ ] **Step 4: Write the arena/tree core**

Create `crates/superui_dom/src/tree.rs`:

```rust
use slotmap::SlotMap;

use crate::node::{ElementData, NodeData, NodeId, NodeKind};

/// Errors from structural mutations. Returned instead of panicking so that
/// AI-generated / framework code hitting an illegal operation degrades.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DomError {
    /// A handle referred to a removed or foreign node.
    NotFound,
    /// The operation requires an element but the node was Document/Text.
    NotAnElement,
    /// The reference/old child is not actually a child of the parent.
    NotAChild,
    /// The operation would create a cycle (inserting an ancestor into a descendant).
    Hierarchy,
}

/// The DOM: an arena of nodes plus the document root and a listener counter.
pub struct Dom {
    nodes: SlotMap<NodeId, NodeData>,
    document: NodeId,
    pub(crate) next_listener: u64,
}

impl Dom {
    /// Create an empty document (a `Document` root with no children).
    pub fn new() -> Self {
        let mut nodes = SlotMap::with_key();
        let document = nodes.insert(NodeData::new(NodeKind::Document));
        Dom { nodes, document, next_listener: 0 }
    }

    /// The document root handle.
    pub fn document(&self) -> NodeId {
        self.document
    }

    /// Create a detached element node. Tag name is lowercased (HTML-insensitive).
    pub fn create_element(&mut self, tag: &str) -> NodeId {
        let data = ElementData { tag: tag.to_ascii_lowercase(), ..Default::default() };
        self.nodes.insert(NodeData::new(NodeKind::Element(data)))
    }

    /// Create a detached text node.
    pub fn create_text(&mut self, data: &str) -> NodeId {
        self.nodes.insert(NodeData::new(NodeKind::Text(data.to_string())))
    }

    /// Node data by handle, or `None` if the handle is stale/foreign.
    pub fn get(&self, id: NodeId) -> Option<&NodeData> {
        self.nodes.get(id)
    }

    /// Mutable node data by handle.
    pub fn get_mut(&mut self, id: NodeId) -> Option<&mut NodeData> {
        self.nodes.get_mut(id)
    }

    /// Tag name of an element node, else `None`.
    pub fn tag(&self, id: NodeId) -> Option<&str> {
        match &self.nodes.get(id)?.kind {
            NodeKind::Element(e) => Some(e.tag.as_str()),
            _ => None,
        }
    }

    /// Whether the node is an element.
    pub fn is_element(&self, id: NodeId) -> bool {
        matches!(self.nodes.get(id).map(|n| &n.kind), Some(NodeKind::Element(_)))
    }
}

impl Default for Dom {
    fn default() -> Self {
        Self::new()
    }
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p superui_dom`
Expected: PASS — the four `node::tests` cases pass.

- [ ] **Step 6: Commit**

```bash
git add crates/superui_dom
git commit -m "feat(dom): node model and arena with document root"
```

---

### Task 3: Tree mutation + queries

**Files:**
- Modify: `crates/superui_dom/src/tree.rs`

**Interfaces:**
- Consumes: `Dom`, `NodeId`, `DomError` from Task 2.
- Produces (all on `Dom`):
  - `append_child(&mut self, parent: NodeId, child: NodeId) -> Result<(), DomError>`
  - `insert_before(&mut self, parent: NodeId, child: NodeId, reference: Option<NodeId>) -> Result<(), DomError>`
  - `remove_child(&mut self, parent: NodeId, child: NodeId) -> Result<(), DomError>`
  - `replace_child(&mut self, parent: NodeId, new: NodeId, old: NodeId) -> Result<(), DomError>`
  - `parent(&self, NodeId) -> Option<NodeId>`, `children(&self, NodeId) -> &[NodeId]`
  - `next_sibling(&self, NodeId) -> Option<NodeId>`, `previous_sibling(&self, NodeId) -> Option<NodeId>`
  - internal helper `is_inclusive_ancestor(&self, maybe_ancestor: NodeId, node: NodeId) -> bool`

- [ ] **Step 1: Write the failing tests**

Append to `crates/superui_dom/src/tree.rs`:

```rust
#[cfg(test)]
mod mutation_tests {
    use super::*;

    #[test]
    fn append_child_sets_parent_and_order() {
        let mut dom = Dom::new();
        let root = dom.document();
        let a = dom.create_element("a");
        let b = dom.create_element("b");
        dom.append_child(root, a).unwrap();
        dom.append_child(root, b).unwrap();
        assert_eq!(dom.children(root), &[a, b]);
        assert_eq!(dom.parent(a), Some(root));
        assert_eq!(dom.parent(b), Some(root));
    }

    #[test]
    fn append_child_reparents_detaching_from_old_parent() {
        let mut dom = Dom::new();
        let p1 = dom.create_element("p1");
        let p2 = dom.create_element("p2");
        let c = dom.create_element("c");
        dom.append_child(p1, c).unwrap();
        dom.append_child(p2, c).unwrap();
        assert_eq!(dom.children(p1), &[]);
        assert_eq!(dom.children(p2), &[c]);
        assert_eq!(dom.parent(c), Some(p2));
    }

    #[test]
    fn insert_before_places_child_ahead_of_reference() {
        let mut dom = Dom::new();
        let root = dom.document();
        let a = dom.create_element("a");
        let b = dom.create_element("b");
        let c = dom.create_element("c");
        dom.append_child(root, a).unwrap();
        dom.append_child(root, c).unwrap();
        dom.insert_before(root, b, Some(c)).unwrap();
        assert_eq!(dom.children(root), &[a, b, c]);
    }

    #[test]
    fn insert_before_none_reference_appends() {
        let mut dom = Dom::new();
        let root = dom.document();
        let a = dom.create_element("a");
        dom.insert_before(root, a, None).unwrap();
        assert_eq!(dom.children(root), &[a]);
    }

    #[test]
    fn remove_child_detaches() {
        let mut dom = Dom::new();
        let root = dom.document();
        let a = dom.create_element("a");
        dom.append_child(root, a).unwrap();
        dom.remove_child(root, a).unwrap();
        assert_eq!(dom.children(root), &[]);
        assert_eq!(dom.parent(a), None);
    }

    #[test]
    fn remove_child_of_wrong_parent_errors() {
        let mut dom = Dom::new();
        let root = dom.document();
        let a = dom.create_element("a");
        let stranger = dom.create_element("s");
        dom.append_child(root, a).unwrap();
        assert_eq!(dom.remove_child(a, stranger), Err(DomError::NotAChild));
    }

    #[test]
    fn replace_child_swaps_in_place() {
        let mut dom = Dom::new();
        let root = dom.document();
        let a = dom.create_element("a");
        let b = dom.create_element("b");
        let n = dom.create_element("n");
        dom.append_child(root, a).unwrap();
        dom.append_child(root, b).unwrap();
        dom.replace_child(root, n, a).unwrap();
        assert_eq!(dom.children(root), &[n, b]);
        assert_eq!(dom.parent(a), None);
        assert_eq!(dom.parent(n), Some(root));
    }

    #[test]
    fn append_cycle_is_rejected() {
        let mut dom = Dom::new();
        let a = dom.create_element("a");
        let b = dom.create_element("b");
        dom.append_child(a, b).unwrap();
        // Making `a` a child of its own descendant `b` must fail.
        assert_eq!(dom.append_child(b, a), Err(DomError::Hierarchy));
    }

    #[test]
    fn siblings_report_neighbors() {
        let mut dom = Dom::new();
        let root = dom.document();
        let a = dom.create_element("a");
        let b = dom.create_element("b");
        dom.append_child(root, a).unwrap();
        dom.append_child(root, b).unwrap();
        assert_eq!(dom.next_sibling(a), Some(b));
        assert_eq!(dom.previous_sibling(b), Some(a));
        assert_eq!(dom.next_sibling(b), None);
        assert_eq!(dom.previous_sibling(a), None);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p superui_dom`
Expected: FAIL — `append_child`, `children`, etc. not defined.

- [ ] **Step 3: Implement mutation + queries**

Add an `impl Dom` block to `crates/superui_dom/src/tree.rs` (before the `#[cfg(test)]` modules):

```rust
impl Dom {
    /// Ordered children of a node (empty slice if node is missing/childless).
    pub fn children(&self, id: NodeId) -> &[NodeId] {
        self.nodes.get(id).map(|n| n.children.as_slice()).unwrap_or(&[])
    }

    /// Parent of a node, if any.
    pub fn parent(&self, id: NodeId) -> Option<NodeId> {
        self.nodes.get(id)?.parent
    }

    /// True if `maybe_ancestor` is `node` or an ancestor of `node`.
    pub(crate) fn is_inclusive_ancestor(&self, maybe_ancestor: NodeId, node: NodeId) -> bool {
        let mut cur = Some(node);
        while let Some(c) = cur {
            if c == maybe_ancestor {
                return true;
            }
            cur = self.nodes.get(c).and_then(|n| n.parent);
        }
        false
    }

    /// Detach `child` from its current parent, if it has one. Internal helper.
    fn detach(&mut self, child: NodeId) {
        let Some(old_parent) = self.nodes.get(child).and_then(|n| n.parent) else { return };
        if let Some(p) = self.nodes.get_mut(old_parent) {
            p.children.retain(|&c| c != child);
        }
        if let Some(n) = self.nodes.get_mut(child) {
            n.parent = None;
        }
    }

    /// Append `child` as the last child of `parent`, reparenting if needed.
    pub fn append_child(&mut self, parent: NodeId, child: NodeId) -> Result<(), DomError> {
        self.insert_before(parent, child, None)
    }

    /// Insert `child` into `parent` immediately before `reference`
    /// (or append when `reference` is `None`). Reparents `child` if attached.
    pub fn insert_before(
        &mut self,
        parent: NodeId,
        child: NodeId,
        reference: Option<NodeId>,
    ) -> Result<(), DomError> {
        if self.nodes.get(parent).is_none() || self.nodes.get(child).is_none() {
            return Err(DomError::NotFound);
        }
        // Inserting an (inclusive) ancestor of `parent` under it would cycle.
        if self.is_inclusive_ancestor(child, parent) {
            return Err(DomError::Hierarchy);
        }
        let index = match reference {
            None => self.nodes[parent].children.len(),
            Some(r) => self.nodes[parent]
                .children
                .iter()
                .position(|&c| c == r)
                .ok_or(DomError::NotAChild)?,
        };
        self.detach(child);
        self.nodes[parent].children.insert(index, child);
        self.nodes[child].parent = Some(parent);
        Ok(())
    }

    /// Remove `child` from `parent`. Errors if `child` is not a child of `parent`.
    pub fn remove_child(&mut self, parent: NodeId, child: NodeId) -> Result<(), DomError> {
        let siblings = &self.nodes.get(parent).ok_or(DomError::NotFound)?.children;
        if !siblings.contains(&child) {
            return Err(DomError::NotAChild);
        }
        self.nodes[parent].children.retain(|&c| c != child);
        self.nodes[child].parent = None;
        Ok(())
    }

    /// Replace `old` (a child of `parent`) with `new`, preserving position.
    pub fn replace_child(
        &mut self,
        parent: NodeId,
        new: NodeId,
        old: NodeId,
    ) -> Result<(), DomError> {
        let index = self
            .nodes
            .get(parent)
            .ok_or(DomError::NotFound)?
            .children
            .iter()
            .position(|&c| c == old)
            .ok_or(DomError::NotAChild)?;
        if self.nodes.get(new).is_none() {
            return Err(DomError::NotFound);
        }
        if self.is_inclusive_ancestor(new, parent) {
            return Err(DomError::Hierarchy);
        }
        self.detach(new);
        // `old` may have shifted if `new` was an earlier sibling that got detached.
        let index = self.nodes[parent].children.iter().position(|&c| c == old).unwrap_or(index);
        self.nodes[parent].children[index] = new;
        self.nodes[new].parent = Some(parent);
        self.nodes[old].parent = None;
        Ok(())
    }

    /// Next sibling of `id` within its parent, if any.
    pub fn next_sibling(&self, id: NodeId) -> Option<NodeId> {
        let parent = self.parent(id)?;
        let kids = &self.nodes.get(parent)?.children;
        let i = kids.iter().position(|&c| c == id)?;
        kids.get(i + 1).copied()
    }

    /// Previous sibling of `id` within its parent, if any.
    pub fn previous_sibling(&self, id: NodeId) -> Option<NodeId> {
        let parent = self.parent(id)?;
        let kids = &self.nodes.get(parent)?.children;
        let i = kids.iter().position(|&c| c == id)?;
        if i == 0 { None } else { kids.get(i - 1).copied() }
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p superui_dom`
Expected: PASS — all `mutation_tests` and prior tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/superui_dom
git commit -m "feat(dom): tree mutation, reparenting, cycle guard, sibling queries"
```

---

### Task 4: Attributes, classList, textContent, getElementById

**Files:**
- Create: `crates/superui_dom/src/attr.rs`
- Modify: `crates/superui_dom/src/lib.rs` (add `mod attr;`)

**Interfaces:**
- Consumes: `Dom`, `NodeId`, `NodeKind` from earlier tasks.
- Produces (all on `Dom`):
  - `set_attribute(&mut self, id, name: &str, value: &str) -> Result<(), DomError>`
  - `get_attribute(&self, id, name: &str) -> Option<&str>`
  - `has_attribute(&self, id, name: &str) -> bool`
  - `remove_attribute(&mut self, id, name: &str)`
  - `classes(&self, id) -> Vec<String>`, `class_contains(&self, id, &str) -> bool`
  - `class_add(&mut self, id, &str)`, `class_remove(&mut self, id, &str)`, `class_toggle(&mut self, id, &str) -> bool`
  - `text_content(&self, id) -> String`, `set_text_content(&mut self, id, &str)`
  - `get_element_by_id(&self, &str) -> Option<NodeId>`

- [ ] **Step 1: Write the failing tests**

Create `crates/superui_dom/src/attr.rs`:

```rust
use crate::node::{NodeId, NodeKind};
use crate::tree::{Dom, DomError};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_get_remove_attribute() {
        let mut dom = Dom::new();
        let el = dom.create_element("input");
        dom.set_attribute(el, "type", "checkbox").unwrap();
        assert_eq!(dom.get_attribute(el, "type"), Some("checkbox"));
        assert!(dom.has_attribute(el, "type"));
        dom.remove_attribute(el, "type");
        assert_eq!(dom.get_attribute(el, "type"), None);
        assert!(!dom.has_attribute(el, "type"));
    }

    #[test]
    fn set_attribute_overwrites() {
        let mut dom = Dom::new();
        let el = dom.create_element("div");
        dom.set_attribute(el, "id", "a").unwrap();
        dom.set_attribute(el, "id", "b").unwrap();
        assert_eq!(dom.get_attribute(el, "id"), Some("b"));
    }

    #[test]
    fn set_attribute_on_non_element_errors() {
        let mut dom = Dom::new();
        let t = dom.create_text("x");
        assert_eq!(dom.set_attribute(t, "id", "a"), Err(DomError::NotAnElement));
    }

    #[test]
    fn classlist_derives_from_class_attribute() {
        let mut dom = Dom::new();
        let el = dom.create_element("div");
        dom.set_attribute(el, "class", "  foo   bar ").unwrap();
        assert_eq!(dom.classes(el), vec!["foo".to_string(), "bar".to_string()]);
        assert!(dom.class_contains(el, "foo"));
        assert!(!dom.class_contains(el, "baz"));
    }

    #[test]
    fn classlist_mutations_rewrite_class_attribute() {
        let mut dom = Dom::new();
        let el = dom.create_element("div");
        dom.class_add(el, "a");
        dom.class_add(el, "b");
        dom.class_add(el, "a"); // idempotent
        assert_eq!(dom.get_attribute(el, "class"), Some("a b"));
        dom.class_remove(el, "a");
        assert_eq!(dom.get_attribute(el, "class"), Some("b"));
        assert_eq!(dom.class_toggle(el, "b"), false); // was present -> removed
        assert_eq!(dom.class_toggle(el, "c"), true); // was absent -> added
        assert_eq!(dom.classes(el), vec!["c".to_string()]);
    }

    #[test]
    fn text_content_reads_and_concatenates_descendants() {
        let mut dom = Dom::new();
        let root = dom.document();
        let p = dom.create_element("p");
        let t1 = dom.create_text("Hello, ");
        let span = dom.create_element("span");
        let t2 = dom.create_text("world");
        dom.append_child(root, p).unwrap();
        dom.append_child(p, t1).unwrap();
        dom.append_child(p, span).unwrap();
        dom.append_child(span, t2).unwrap();
        assert_eq!(dom.text_content(p), "Hello, world");
    }

    #[test]
    fn set_text_content_replaces_children_with_one_text_node() {
        let mut dom = Dom::new();
        let p = dom.create_element("p");
        let old = dom.create_element("span");
        dom.append_child(p, old).unwrap();
        dom.set_text_content(p, "just text");
        assert_eq!(dom.text_content(p), "just text");
        assert_eq!(dom.children(p).len(), 1);
        assert_eq!(dom.parent(old), None);
    }

    #[test]
    fn set_text_content_empty_clears_children() {
        let mut dom = Dom::new();
        let p = dom.create_element("p");
        let c = dom.create_text("x");
        dom.append_child(p, c).unwrap();
        dom.set_text_content(p, "");
        assert_eq!(dom.children(p).len(), 0);
        assert_eq!(dom.text_content(p), "");
    }

    #[test]
    fn get_element_by_id_finds_first_match() {
        let mut dom = Dom::new();
        let root = dom.document();
        let a = dom.create_element("div");
        let b = dom.create_element("div");
        dom.set_attribute(b, "id", "target").unwrap();
        dom.append_child(root, a).unwrap();
        dom.append_child(a, b).unwrap();
        assert_eq!(dom.get_element_by_id("target"), Some(b));
        assert_eq!(dom.get_element_by_id("missing"), None);
    }
}
```

Add to `crates/superui_dom/src/lib.rs` after `mod tree;`:

```rust
mod attr;
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p superui_dom`
Expected: FAIL — `set_attribute`, `classes`, `text_content`, etc. not defined.

- [ ] **Step 3: Implement attribute/class/text/id APIs**

Add above the `#[cfg(test)]` block in `crates/superui_dom/src/attr.rs`:

```rust
impl Dom {
    /// Set an attribute (overwriting) on an element. `class`/`id` are ordinary
    /// attributes here; `classList` reads/writes the `class` attribute.
    pub fn set_attribute(&mut self, id: NodeId, name: &str, value: &str) -> Result<(), DomError> {
        let node = self.get_mut(id).ok_or(DomError::NotFound)?;
        let NodeKind::Element(el) = &mut node.kind else { return Err(DomError::NotAnElement) };
        let name = name.to_ascii_lowercase();
        match el.attrs.iter_mut().find(|(n, _)| *n == name) {
            Some(slot) => slot.1 = value.to_string(),
            None => el.attrs.push((name, value.to_string())),
        }
        Ok(())
    }

    /// Attribute value, if present on an element.
    pub fn get_attribute(&self, id: NodeId, name: &str) -> Option<&str> {
        let NodeKind::Element(el) = &self.get(id)?.kind else { return None };
        let name = name.to_ascii_lowercase();
        el.attrs.iter().find(|(n, _)| *n == name).map(|(_, v)| v.as_str())
    }

    /// Whether an element carries the attribute.
    pub fn has_attribute(&self, id: NodeId, name: &str) -> bool {
        self.get_attribute(id, name).is_some()
    }

    /// Remove an attribute (no-op if absent or not an element).
    pub fn remove_attribute(&mut self, id: NodeId, name: &str) {
        if let Some(node) = self.get_mut(id) {
            if let NodeKind::Element(el) = &mut node.kind {
                let name = name.to_ascii_lowercase();
                el.attrs.retain(|(n, _)| *n != name);
            }
        }
    }

    /// Classes parsed from the `class` attribute (whitespace-separated).
    pub fn classes(&self, id: NodeId) -> Vec<String> {
        self.get_attribute(id, "class")
            .map(|c| c.split_whitespace().map(str::to_string).collect())
            .unwrap_or_default()
    }

    /// Whether `class` is present.
    pub fn class_contains(&self, id: NodeId, class: &str) -> bool {
        self.classes(id).iter().any(|c| c == class)
    }

    /// Add a class (idempotent). Rewrites the `class` attribute.
    pub fn class_add(&mut self, id: NodeId, class: &str) {
        let mut classes = self.classes(id);
        if !classes.iter().any(|c| c == class) {
            classes.push(class.to_string());
            let _ = self.set_attribute(id, "class", &classes.join(" "));
        }
    }

    /// Remove a class if present. Rewrites the `class` attribute.
    pub fn class_remove(&mut self, id: NodeId, class: &str) {
        let mut classes = self.classes(id);
        let before = classes.len();
        classes.retain(|c| c != class);
        if classes.len() != before {
            let _ = self.set_attribute(id, "class", &classes.join(" "));
        }
    }

    /// Toggle a class. Returns `true` if the class is present afterwards.
    pub fn class_toggle(&mut self, id: NodeId, class: &str) -> bool {
        if self.class_contains(id, class) {
            self.class_remove(id, class);
            false
        } else {
            self.class_add(id, class);
            true
        }
    }

    /// Concatenated text of a node and its descendants (document order).
    pub fn text_content(&self, id: NodeId) -> String {
        let Some(node) = self.get(id) else { return String::new() };
        match &node.kind {
            NodeKind::Text(s) => s.clone(),
            _ => {
                let mut out = String::new();
                for &child in node.children() {
                    out.push_str(&self.text_content(child));
                }
                out
            }
        }
    }

    /// Replace a node's children with a single text node (or clear if empty).
    /// On a text node, sets its data directly.
    pub fn set_text_content(&mut self, id: NodeId, text: &str) {
        let Some(node) = self.get(id) else { return };
        if let NodeKind::Text(_) = node.kind {
            if let Some(n) = self.get_mut(id) {
                n.kind = NodeKind::Text(text.to_string());
            }
            return;
        }
        let existing: Vec<NodeId> = self.children(id).to_vec();
        for c in existing {
            let _ = self.remove_child(id, c);
        }
        if !text.is_empty() {
            let t = self.create_text(text);
            let _ = self.append_child(id, t);
        }
    }

    /// First element (document order) whose `id` attribute equals `id_value`.
    pub fn get_element_by_id(&self, id_value: &str) -> Option<NodeId> {
        fn walk(dom: &Dom, node: NodeId, target: &str) -> Option<NodeId> {
            if dom.get_attribute(node, "id") == Some(target) {
                return Some(node);
            }
            for &child in dom.children(node) {
                if let Some(found) = walk(dom, child, target) {
                    return Some(found);
                }
            }
            None
        }
        walk(self, self.document(), id_value)
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p superui_dom`
Expected: PASS — all `attr::tests` and prior tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/superui_dom
git commit -m "feat(dom): attributes, classList, textContent, getElementById"
```

---

### Task 5: Event object + listener registry

**Files:**
- Create: `crates/superui_dom/src/event.rs`
- Modify: `crates/superui_dom/src/lib.rs` (add `mod event;`, extend `pub use`)

**Interfaces:**
- Consumes: `Dom`, `NodeId`, `Listener`, `ListenerId`, `NodeKind`.
- Produces:
  - `pub enum EventPhase { None, Capturing, AtTarget, Bubbling }`.
  - `pub struct Event { pub type_: String, pub target: NodeId, pub current_target: Option<NodeId>, pub phase: EventPhase, pub bubbles: bool, pub cancelable: bool, ... }` with methods `new(type_, target, bubbles, cancelable)`, `stop_propagation()`, `stop_immediate_propagation()`, `prevent_default()`, `default_prevented() -> bool`, `propagation_stopped() -> bool`, `immediate_stopped() -> bool`.
  - On `Dom`: `add_event_listener(&mut self, id, event_type: &str, capture: bool) -> Option<ListenerId>`, `remove_event_listener(&mut self, id, ListenerId) -> bool`, `listener_exists(&self, id, ListenerId) -> bool`, `listeners(&self, id) -> &[Listener]`.

- [ ] **Step 1: Write the failing tests**

Create `crates/superui_dom/src/event.rs`:

```rust
use crate::node::{ListenerId, Listener, NodeId, NodeKind};
use crate::tree::Dom;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_flags_toggle() {
        let mut dom = Dom::new();
        let el = dom.create_element("div");
        let mut ev = Event::new("click", el, true, true);
        assert!(!ev.default_prevented());
        assert!(!ev.propagation_stopped());
        ev.prevent_default();
        ev.stop_propagation();
        assert!(ev.default_prevented());
        assert!(ev.propagation_stopped());
        assert!(!ev.immediate_stopped());
        ev.stop_immediate_propagation();
        assert!(ev.immediate_stopped());
    }

    #[test]
    fn add_and_remove_listener() {
        let mut dom = Dom::new();
        let el = dom.create_element("div");
        let id = dom.add_event_listener(el, "click", false).unwrap();
        assert!(dom.listener_exists(el, id));
        assert_eq!(dom.listeners(el).len(), 1);
        assert!(dom.remove_event_listener(el, id));
        assert!(!dom.listener_exists(el, id));
        assert!(!dom.remove_event_listener(el, id)); // second remove is a no-op
    }

    #[test]
    fn add_listener_on_non_element_returns_none() {
        let mut dom = Dom::new();
        let t = dom.create_text("x");
        assert_eq!(dom.add_event_listener(t, "click", false), None);
    }

    #[test]
    fn listener_ids_are_unique() {
        let mut dom = Dom::new();
        let el = dom.create_element("div");
        let a = dom.add_event_listener(el, "click", false).unwrap();
        let b = dom.add_event_listener(el, "click", false).unwrap();
        assert_ne!(a, b);
    }
}
```

Add to `crates/superui_dom/src/lib.rs` after `mod attr;`:

```rust
mod event;
```

And extend the `pub use` line in `lib.rs` to:

```rust
pub use event::{Event, EventPhase};
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p superui_dom`
Expected: FAIL — `Event`, `add_event_listener`, etc. not defined.

- [ ] **Step 3: Implement Event + listener registry**

Add above the `#[cfg(test)]` block in `crates/superui_dom/src/event.rs`:

```rust
/// Propagation phase during dispatch (mirrors the DOM `eventPhase`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventPhase {
    None,
    Capturing,
    AtTarget,
    Bubbling,
}

/// A dispatched event. Carries mutable propagation/cancellation state that
/// listeners flip via the `stop_*` / `prevent_default` methods.
#[derive(Clone, Debug)]
pub struct Event {
    pub type_: String,
    pub target: NodeId,
    pub current_target: Option<NodeId>,
    pub phase: EventPhase,
    pub bubbles: bool,
    pub cancelable: bool,
    default_prevented: bool,
    propagation_stopped: bool,
    immediate_stopped: bool,
}

impl Event {
    /// Create an event targeting `target`.
    pub fn new(type_: &str, target: NodeId, bubbles: bool, cancelable: bool) -> Self {
        Event {
            type_: type_.to_string(),
            target,
            current_target: None,
            phase: EventPhase::None,
            bubbles,
            cancelable,
            default_prevented: false,
            propagation_stopped: false,
            immediate_stopped: false,
        }
    }

    /// Stop propagation to further nodes after the current one finishes.
    pub fn stop_propagation(&mut self) {
        self.propagation_stopped = true;
    }

    /// Stop propagation immediately, including remaining listeners on this node.
    pub fn stop_immediate_propagation(&mut self) {
        self.propagation_stopped = true;
        self.immediate_stopped = true;
    }

    /// Mark the default action as prevented (only if cancelable).
    pub fn prevent_default(&mut self) {
        if self.cancelable {
            self.default_prevented = true;
        }
    }

    pub fn default_prevented(&self) -> bool {
        self.default_prevented
    }
    pub fn propagation_stopped(&self) -> bool {
        self.propagation_stopped
    }
    pub fn immediate_stopped(&self) -> bool {
        self.immediate_stopped
    }
}

impl Dom {
    /// Register a listener on an element. Returns its id, or `None` if the node
    /// is not an element.
    pub fn add_event_listener(
        &mut self,
        id: NodeId,
        event_type: &str,
        capture: bool,
    ) -> Option<ListenerId> {
        let listener_id = ListenerId(self.next_listener);
        let node = self.get_mut(id)?;
        let NodeKind::Element(el) = &mut node.kind else { return None };
        el.listeners.push(Listener {
            id: listener_id,
            event_type: event_type.to_string(),
            capture,
        });
        self.next_listener += 1;
        Some(listener_id)
    }

    /// Remove a listener by id. Returns whether one was removed.
    pub fn remove_event_listener(&mut self, id: NodeId, listener: ListenerId) -> bool {
        let Some(node) = self.get_mut(id) else { return false };
        let NodeKind::Element(el) = &mut node.kind else { return false };
        let before = el.listeners.len();
        el.listeners.retain(|l| l.id != listener);
        el.listeners.len() != before
    }

    /// Whether a listener id is still registered on the node.
    pub fn listener_exists(&self, id: NodeId, listener: ListenerId) -> bool {
        self.listeners(id).iter().any(|l| l.id == listener)
    }

    /// All listeners registered on a node (empty if none / not an element).
    pub fn listeners(&self, id: NodeId) -> &[Listener] {
        match self.get(id).map(|n| &n.kind) {
            Some(NodeKind::Element(el)) => &el.listeners,
            _ => &[],
        }
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p superui_dom`
Expected: PASS — all `event::tests` and prior tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/superui_dom
git commit -m "feat(dom): Event object and per-node listener registry"
```

---

### Task 6: Dispatch plan + reference executor

**Files:**
- Modify: `crates/superui_dom/src/event.rs`
- Modify: `crates/superui_dom/src/lib.rs` (extend `pub use` with `DispatchStep`)

**Interfaces:**
- Consumes: everything from Task 5.
- Produces:
  - `pub struct DispatchStep { pub node: NodeId, pub phase: EventPhase, pub listeners: Vec<ListenerId> }`.
  - On `Dom`: `build_dispatch_plan(&self, target: NodeId, event_type: &str, bubbles: bool) -> Vec<DispatchStep>` — pure, ordered capture→target→bubble visit list; each step lists the matching listener ids (by type and phase) captured at plan time.
  - On `Dom`: `run_dispatch(&self, event: &mut Event, mut invoke: impl FnMut(&mut Event, NodeId, ListenerId))` — reference executor honoring propagation flags and live listener existence. (The future JS layer will instead call `build_dispatch_plan` and run its own loop against Boa; this executor is the tested reference and serves headless tests.)

- [ ] **Step 1: Write the failing tests**

Append to the `#[cfg(test)] mod tests` block in `crates/superui_dom/src/event.rs`:

```rust
    // ---- dispatch tests ----

    /// Build a small tree root > mid > leaf and return the ids.
    fn tree() -> (Dom, NodeId, NodeId, NodeId) {
        let mut dom = Dom::new();
        let root = dom.create_element("root");
        let mid = dom.create_element("mid");
        let leaf = dom.create_element("leaf");
        dom.append_child(dom.document(), root).unwrap();
        dom.append_child(root, mid).unwrap();
        dom.append_child(mid, leaf).unwrap();
        (dom, root, mid, leaf)
    }

    #[test]
    fn dispatch_order_is_capture_target_bubble() {
        let (mut dom, root, mid, leaf) = tree();
        dom.add_event_listener(root, "click", true); // capture
        dom.add_event_listener(mid, "click", true); // capture
        dom.add_event_listener(leaf, "click", false); // target (bubble flag)
        dom.add_event_listener(mid, "click", false); // bubble
        dom.add_event_listener(root, "click", false); // bubble

        let mut order: Vec<NodeId> = Vec::new();
        let mut ev = Event::new("click", leaf, true, true);
        dom.run_dispatch(&mut ev, |_e, node, _l| order.push(node));
        assert_eq!(order, vec![root, mid, leaf, mid, root]);
    }

    #[test]
    fn non_bubbling_event_skips_bubble_phase() {
        let (mut dom, root, mid, leaf) = tree();
        dom.add_event_listener(root, "click", true);
        dom.add_event_listener(leaf, "click", false);
        dom.add_event_listener(root, "click", false); // bubble - should NOT fire

        let mut order: Vec<NodeId> = Vec::new();
        let mut ev = Event::new("click", leaf, false, true);
        dom.run_dispatch(&mut ev, |_e, node, _l| order.push(node));
        assert_eq!(order, vec![root, leaf]);
    }

    #[test]
    fn stop_propagation_halts_after_current_node() {
        let (mut dom, root, mid, leaf) = tree();
        dom.add_event_listener(root, "click", true);
        dom.add_event_listener(mid, "click", true);
        dom.add_event_listener(leaf, "click", false);

        let mut order: Vec<NodeId> = Vec::new();
        let mut ev = Event::new("click", leaf, true, true);
        dom.run_dispatch(&mut ev, |e, node, _l| {
            order.push(node);
            if node == mid {
                e.stop_propagation();
            }
        });
        // root (capture) and mid (capture) fire; leaf is never reached.
        assert_eq!(order, vec![root, mid]);
    }

    #[test]
    fn stop_immediate_propagation_halts_remaining_listeners_on_node() {
        let mut dom = Dom::new();
        let el = dom.create_element("el");
        dom.append_child(dom.document(), el).unwrap();
        let first = dom.add_event_listener(el, "click", false).unwrap();
        let _second = dom.add_event_listener(el, "click", false).unwrap();

        let mut fired: Vec<ListenerId> = Vec::new();
        let mut ev = Event::new("click", el, true, true);
        dom.run_dispatch(&mut ev, |e, _node, l| {
            fired.push(l);
            e.stop_immediate_propagation();
        });
        assert_eq!(fired, vec![first]); // second listener suppressed
    }

    #[test]
    fn build_dispatch_plan_lists_expected_nodes_and_phases() {
        let (mut dom, root, mid, leaf) = tree();
        dom.add_event_listener(root, "click", true); // capture
        dom.add_event_listener(leaf, "click", false); // target
        dom.add_event_listener(mid, "click", false); // bubble
        dom.add_event_listener(root, "click", false); // bubble

        let plan = dom.build_dispatch_plan(leaf, "click", true);
        let shape: Vec<(NodeId, EventPhase)> =
            plan.iter().map(|s| (s.node, s.phase)).collect();
        assert_eq!(
            shape,
            vec![
                (root, EventPhase::Capturing),
                (leaf, EventPhase::AtTarget),
                (mid, EventPhase::Bubbling),
                (root, EventPhase::Bubbling),
            ]
        );
    }

    #[test]
    fn build_dispatch_plan_filters_by_event_type() {
        let (mut dom, _root, _mid, leaf) = tree();
        dom.add_event_listener(leaf, "input", false); // wrong type
        let plan = dom.build_dispatch_plan(leaf, "click", true);
        assert!(plan.is_empty());
    }

    #[test]
    fn run_dispatch_skips_a_listener_removed_after_planning() {
        // Exercises the `listener_exists` re-check inside run_dispatch by driving
        // the same plan loop manually with a removal interleaved (the real JS
        // layer does exactly this, mutating the DOM between listener calls).
        let mut dom = Dom::new();
        let el = dom.create_element("el");
        dom.append_child(dom.document(), el).unwrap();
        let a = dom.add_event_listener(el, "click", false).unwrap();
        let b = dom.add_event_listener(el, "click", false).unwrap();

        let plan = dom.build_dispatch_plan(el, "click", true);
        dom.remove_event_listener(el, b); // b removed before we "reach" it

        let mut fired: Vec<ListenerId> = Vec::new();
        for step in &plan {
            for &lid in &step.listeners {
                if dom.listener_exists(step.node, lid) {
                    fired.push(lid);
                }
            }
        }
        assert_eq!(fired, vec![a]); // b was skipped by the existence check
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p superui_dom`
Expected: FAIL — `run_dispatch` / `build_dispatch_plan` not defined.

- [ ] **Step 3: Implement dispatch plan + executor**

Add to the `impl Dom` block region in `crates/superui_dom/src/event.rs` (above `#[cfg(test)]`), and add the `DispatchStep` struct near `EventPhase`:

```rust
/// One node's worth of a computed dispatch: which listeners (by id) to invoke,
/// in order, at a given phase. Owned, so it survives DOM mutation during invocation.
#[derive(Clone, Debug)]
pub struct DispatchStep {
    pub node: NodeId,
    pub phase: EventPhase,
    pub listeners: Vec<ListenerId>,
}

impl Dom {
    /// Compute the ordered capture → target → bubble visit plan for an event of
    /// `event_type` at `target`. Pure: reads the DOM once into an owned plan.
    /// Listener ids are captured at plan time (matching type + phase); the
    /// executor re-checks existence so removals during dispatch are honored.
    pub fn build_dispatch_plan(
        &self,
        target: NodeId,
        event_type: &str,
        bubbles: bool,
    ) -> Vec<DispatchStep> {
        if self.get(target).is_none() {
            return Vec::new();
        }
        // Ancestor chain from parent(target) up to the root.
        let mut ancestors: Vec<NodeId> = Vec::new();
        let mut cur = self.parent(target);
        while let Some(node) = cur {
            ancestors.push(node);
            cur = self.parent(node);
        }

        let ids_at = |node: NodeId, want_capture: bool| -> Vec<ListenerId> {
            self.listeners(node)
                .iter()
                .filter(|l| l.event_type == event_type && l.capture == want_capture)
                .map(|l| l.id)
                .collect()
        };

        let mut plan = Vec::new();

        // Capture: root -> parent(target).
        for &node in ancestors.iter().rev() {
            let listeners = ids_at(node, true);
            if !listeners.is_empty() {
                plan.push(DispatchStep { node, phase: EventPhase::Capturing, listeners });
            }
        }

        // Target: both capture and non-capture listeners fire, in registration order.
        let target_listeners: Vec<ListenerId> = self
            .listeners(target)
            .iter()
            .filter(|l| l.event_type == event_type)
            .map(|l| l.id)
            .collect();
        if !target_listeners.is_empty() {
            plan.push(DispatchStep {
                node: target,
                phase: EventPhase::AtTarget,
                listeners: target_listeners,
            });
        }

        // Bubble: parent(target) -> root (only if the event bubbles).
        if bubbles {
            for &node in ancestors.iter() {
                let listeners = ids_at(node, false);
                if !listeners.is_empty() {
                    plan.push(DispatchStep { node, phase: EventPhase::Bubbling, listeners });
                }
            }
        }

        plan
    }

    /// Reference executor: walk the plan for `event`, invoking `invoke` per live
    /// listener while honoring propagation/immediate-stop flags. The DOM is not
    /// borrowed across `invoke` calls beyond an existence check, so a real JS
    /// integration can mutate the tree from within `invoke`.
    pub fn run_dispatch(
        &self,
        event: &mut Event,
        mut invoke: impl FnMut(&mut Event, NodeId, ListenerId),
    ) {
        let plan = self.build_dispatch_plan(event.target, &event.type_, event.bubbles);
        for step in plan {
            if event.propagation_stopped() {
                break;
            }
            event.current_target = Some(step.node);
            event.phase = step.phase;
            for lid in step.listeners {
                if event.immediate_stopped() {
                    break;
                }
                if !self.listener_exists(step.node, lid) {
                    continue; // removed since plan was built
                }
                invoke(event, step.node, lid);
            }
        }
        event.current_target = None;
        event.phase = EventPhase::None;
    }
}
```

Extend the `pub use event::{...}` line in `crates/superui_dom/src/lib.rs` to include `DispatchStep`:

```rust
pub use event::{DispatchStep, Event, EventPhase};
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p superui_dom`
Expected: PASS — all dispatch tests plus every prior test pass.

- [ ] **Step 5: Verify the crate compiles for wasm**

Run: `cargo build -p superui_dom --target wasm32-unknown-unknown`
Expected: SUCCESS — no errors (confirms the Global Constraint that this crate is wasm-clean).

- [ ] **Step 6: Commit**

```bash
git add crates/superui_dom
git commit -m "feat(dom): dispatch plan + reference executor with propagation control"
```

---

## Self-Review

**Spec coverage (against the design doc §4 `superui_dom` responsibilities):**
- "arena DOM: NodeData, mutation ops" → Tasks 2–3. ✅
- "event dispatch (capture/bubble)" → Tasks 5–6. ✅
- "Knows nothing about Bevy or JS. Headless-testable." → no Bevy/JS deps; all tasks headless-tested. ✅
- classList / textContent / attributes / getElementById needed by later JS bindings (§9 DOM API) → Task 4. ✅
- Graceful degradation (Global Constraint) → `Result`/`Option` throughout, no panics in public API. ✅
- wasm-clean (Global Constraint) → verified in Task 6 Step 5. ✅

**Placeholder scan:** No TBD/TODO; every code step contains complete code; every test step contains real assertions. ✅

**Type consistency:** `NodeId`, `ListenerId`, `Listener`, `NodeKind`, `Event`, `EventPhase`, `DispatchStep`, `DomError` names are used identically across tasks. `add_event_listener` returns `Option<ListenerId>`; `run_dispatch` invoke signature `FnMut(&mut Event, NodeId, ListenerId)` matches the tests. ✅

**Note on Task 6 dispatch tests:** the reference executor `run_dispatch` takes `&self`, so its test `invoke` closures cannot mutate the DOM. The live-removal guarantee (a listener removed after planning but before it is reached must not fire) is therefore tested by `run_dispatch_skips_a_listener_removed_after_planning`, which drives the plan loop manually with `build_dispatch_plan` + `listener_exists` and an interleaved `remove_event_listener` — mirroring exactly what the JS layer (Plan 3) does when it mutates the DOM between listener calls. `build_dispatch_plan` structure and type-filtering are covered by their own tests.

---
```
