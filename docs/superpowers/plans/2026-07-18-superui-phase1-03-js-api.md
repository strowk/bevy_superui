# superui_js + superui_api Implementation Plan (Phase 1, Plan 3 of 6)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the two headless JS crates — `superui_js` (a `JsEngine` trait + a Boa backend that shares the retained `superui_dom` tree with native functions and marshals DOM handles to/from JS) and `superui_api` (the standards-shaped `document`/`Node`/`Element`/`Event`/`classList`/`style` surface, `console`, timers, and a `fetch` warn-reject stub installed onto a `BoaEngine`) — so that plain author JavaScript can read and mutate the DOM synchronously and receive dispatched DOM events, all headlessly and wasm-clean.

**Architecture:** The DOM is shared as `Rc<RefCell<superui_dom::Dom>>`. Boa's `HostDefined` realm slot holds a GC-managed `HostState { dom, wrappers, listeners, protos, timers }`; every native binding reaches the DOM through it and mutates it synchronously, so `el.appendChild(x); x.parentNode === el` holds immediately. Each JS `Node`/`Element` is a `JsObject` carrying a `superui_dom::NodeId` (never a Bevy `Entity`) with a per-kind shared prototype; a `NodeId → JsObject` cache gives stable object identity. `superui_js` owns the engine, the marshalling toolkit (`wrap_node`/`node_id_of`/`dom_of`), and the coarse `JsEngine` trait the future Bevy layers consume (`eval`, `dispatch_event`, `run_timers`); `superui_api` builds the fine-grained web APIs on top using Boa directly (permitted by design §4).

**Tech Stack:** Rust (edition 2021). `boa_engine` 0.21 + `boa_gc` 0.21 (JS engine, pure-Rust, wasm-capable). `superui_dom` (Plan 1) as the retained tree. No Bevy, no JS frameworks, no async runtime, no threads.

## Global Constraints

- **Bevy version target for the overall project: 0.17** — but `superui_js` and `superui_api` have **NO Bevy dependency** and must stay Bevy-version-agnostic (design §4 boundary discipline). They are Boa-facing only.
- **`wasm32-unknown-unknown` must compile.** Boa pulls `getrandom` 0.3, which needs the JS backend on wasm. This requires **both** (design §5, verified in an API spike): (a) a direct `getrandom = { version = "0.3", features = ["wasm_js"] }` dependency (scoped to the wasm target), and (b) a repo-root `.cargo/config.toml` with `[target.wasm32-unknown-unknown] rustflags = ['--cfg', 'getrandom_backend="wasm_js"']`. Removing either breaks the wasm build.
- **Boa is single-threaded** (`Context`/`JsValue`/GC types are `!Send + !Sync`). Shared DOM state is `Rc<RefCell<Dom>>`, never `Arc<Mutex>`. Keep the engine on one thread.
- **`window.bevy` is DEFERRED to Plan 5** (scope decision for this plan). This plan ships **zero** Bevy bridge — only standards-shaped DOM/Web APIs. The plan-series README is updated accordingly in the final task.
- **Graceful degradation over throwing** (design §1): native bindings that receive a bad `this`, a stale handle, or wrong argument types **return `Ok(JsValue::undefined())` / null** rather than raising, so AI-generated code touching an unimplemented corner keeps running. `fetch` is the one deliberate rejecter (warn + rejected promise).
- **No bespoke web-incompatible surface** — public JS API mirrors the browser (`appendChild`, `getElementById`, `addEventListener`, `textContent`, …). The only Rust-facing non-web surface is the coarse `JsEngine` trait, which is internal plumbing, not exposed to author JS.
- **TDD, DRY, YAGNI, frequent commits** — every task is test-first and ends with a commit.

**Verified Boa 0.21.1 API reference (used verbatim below; all confirmed compiling in a spike).** If the compiler reports a re-export path differently, follow its suggestion — the *types/methods* are correct, only the module a type is re-exported through may differ (same rule as Plan 2).
- Imports: `use boa_engine::{js_string, JsArgs, JsData, JsError, JsNativeError, JsObject, JsResult, JsValue, JsString, NativeFunction, Source, Context, object::{FunctionObjectBuilder, builtins::{JsArray, JsFunction, JsPromise}}, property::{Attribute, PropertyDescriptor}};` and `use boa_gc::{Finalize, Trace};`.
- Eval: `context.eval(Source::from_bytes("1+2"))? ` → `JsValue`; readback `v.as_i32()`, `v.to_string(ctx)?.to_std_string_escaped()`.
- Native fn pointer: `fn(&JsValue, &[JsValue], &mut Context) -> JsResult<JsValue>`; args via `args.get_or_undefined(0)`.
- Build a `JsFunction`: `FunctionObjectBuilder::new(context.realm(), NativeFunction::from_fn_ptr(f)).name(js_string!("m")).length(N).build()`.
- Method on a prototype: `proto.set(js_string!("m"), func, false, context)?`.
- Accessor on a prototype: `let d = PropertyDescriptor::builder().get(getter).set(setter).enumerable(true).configurable(true).build(); proto.define_property_or_throw(js_string!("value"), d, context)?;` (`.get`/`.set` take `Into<JsValue>`; a `JsFunction` works directly; omit `.set` for read-only).
- Object with native data: `JsObject::from_proto_and_data(proto_jsobject, DataStruct { .. })` where `DataStruct: #[derive(Trace, Finalize, JsData)]` with `#[unsafe_ignore_trace]` on non-`Trace` fields.
- Read native data off `this`: `this.as_object().and_then(|o| o.downcast_ref::<DataStruct>())` → `Ref<DataStruct>`; mutable: `downcast_mut::<DataStruct>()`.
- Fresh ordinary prototype object: `JsObject::with_object_proto(context.intrinsics())`.
- Register a global (object or fn value): `context.register_global_property(js_string!("document"), value, Attribute::all())?;` (`JsObject`/`JsFunction`/`JsValue` all `Into<JsValue>`). Register a global callable: `context.register_global_callable(js_string!("setTimeout"), arity, NativeFunction::from_fn_ptr(f))?;`.
- Build a JS array: `JsArray::from_iter(vec_of_jsvalue, context)`; return `arr.into()`.
- Rejected promise: `JsPromise::reject(JsNativeError::typ().with_message("…"), context)`; return `promise.into()`. **Promise reactions are lazy** — a `.catch`/`.then` only runs after `context.run_jobs()`.
- HostDefined: insert `context.realm().host_defined_mut().insert(state)`; read `context.realm().host_defined().get::<T>()`; mutate `context.realm().host_defined_mut().get_mut::<T>()`. `T: Trace + Finalize + JsData + 'static`. **Drop the guard before other `&mut context` use** (scope it in a block, or clone the needed handle out).

**Contingency note (one place only):** the code stores GC handles in `std::collections::HashMap<u64, JsObject/JsFunction>` fields of `HostState`. `boa_gc` implements `Trace` for `HashMap<K, V>` where `K: Trace, V: Trace` (`u64` is `Trace`); if this specific version does not, replace those two fields with `Vec<(u64, JsObject)>` / `Vec<(u64, JsFunction)>` (both `Trace`) and linear-scan — behavior is identical at Phase-1 scale. Do not change any other design detail.

---

## File Structure

```
Cargo.toml                         # workspace deps: add boa_engine, boa_gc
.cargo/config.toml                 # NEW: wasm getrandom rustflag
crates/superui_dom/src/
  node.rs                          # + NodeId::to_ffi/from_ffi; ElementData.value/checked fields
  props.rs                         # NEW: Dom::value/set_value/checked/set_checked
  selector.rs                      # NEW: query_selector / query_selector_all (basic selectors)
  lib.rs                           # + mod props; mod selector;
crates/superui_js/
  Cargo.toml                       # NEW
  src/lib.rs                       # NEW: re-exports; JsEngine trait
  src/engine.rs                    # NEW: BoaEngine (context + dom + HostState)
  src/state.rs                     # NEW: HostState, NodeHandle, Protos, Timer, marshalling toolkit
crates/superui_api/
  Cargo.toml                       # NEW
  src/lib.rs                       # NEW: install(engine); mod console; document; node; element; events; timers; fetch
  src/console.rs                   # NEW: console.{log,warn,error,info}
  src/fetch.rs                     # NEW: fetch warn-reject stub
  src/document.rs                  # NEW: document object + methods; builds the 3 node protos
  src/node.rs                      # NEW: structural methods + parentNode/childNodes/... accessors
  src/element.rs                   # NEW: attributes/props + classList + style
  src/events.rs                    # NEW: event proto + addEventListener/removeEventListener
  src/timers.rs                    # NEW: setTimeout/setInterval/clear*
```

---

### Task 1: `superui_dom` — `NodeId` ffi round-trip + `.value`/`.checked` property model

**Files:**
- Modify: `crates/superui_dom/src/node.rs` (add `NodeId::to_ffi`/`from_ffi`; add `value`/`checked` fields to `ElementData`)
- Create: `crates/superui_dom/src/props.rs`
- Modify: `crates/superui_dom/src/lib.rs` (add `mod props;`)

**Interfaces:**
- Consumes: existing `Dom`, `NodeId`, `NodeKind`, `ElementData`.
- Produces:
  - `impl NodeId { pub fn to_ffi(self) -> u64; pub fn from_ffi(v: u64) -> NodeId }`
  - `impl Dom { pub fn value(&self, id: NodeId) -> String; pub fn set_value(&mut self, id: NodeId, value: &str); pub fn checked(&self, id: NodeId) -> bool; pub fn set_checked(&mut self, id: NodeId, checked: bool) }`
  - `.value` defaults to the `value` attribute until set; `.checked` defaults to presence of the `checked` attribute until set.

- [ ] **Step 1: Write the failing tests** — append to `crates/superui_dom/src/node.rs` inside a `#[cfg(test)] mod ffi_tests` and create the props test file.

Append to `crates/superui_dom/src/node.rs`:

```rust
#[cfg(test)]
mod ffi_tests {
    use crate::Dom;

    #[test]
    fn node_id_round_trips_through_ffi() {
        let mut dom = Dom::new();
        let el = dom.create_element("div");
        let raw = el.to_ffi();
        assert_eq!(crate::NodeId::from_ffi(raw), el);
    }
}
```

Create `crates/superui_dom/src/props.rs`:

```rust
use crate::node::{NodeId, NodeKind};
use crate::tree::Dom;

impl Dom {
    /// The `.value` IDL property: the live value, defaulting to the `value`
    /// attribute until JS explicitly sets it. Empty string for non-elements.
    pub fn value(&self, id: NodeId) -> String {
        let Some(NodeKind::Element(el)) = self.get(id).map(|n| &n.kind) else {
            return String::new();
        };
        if let Some(v) = &el.value {
            return v.clone();
        }
        self.get_attribute(id, "value").unwrap_or("").to_string()
    }

    /// Set the `.value` IDL property (does not change the `value` attribute,
    /// mirroring browser input-value semantics; a Phase-1 simplification).
    pub fn set_value(&mut self, id: NodeId, value: &str) {
        if let Some(node) = self.get_mut(id) {
            if let NodeKind::Element(el) = &mut node.kind {
                el.value = Some(value.to_string());
            }
        }
    }

    /// The `.checked` IDL property: defaults to presence of the `checked`
    /// attribute until JS explicitly sets it.
    pub fn checked(&self, id: NodeId) -> bool {
        let Some(NodeKind::Element(el)) = self.get(id).map(|n| &n.kind) else {
            return false;
        };
        if let Some(c) = el.checked {
            return c;
        }
        self.has_attribute(id, "checked")
    }

    /// Set the `.checked` IDL property.
    pub fn set_checked(&mut self, id: NodeId, checked: bool) {
        if let Some(node) = self.get_mut(id) {
            if let NodeKind::Element(el) = &mut node.kind {
                el.checked = Some(checked);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::Dom;

    #[test]
    fn value_defaults_to_attribute_then_reflects_set() {
        let mut dom = Dom::new();
        let input = dom.create_element("input");
        assert_eq!(dom.value(input), "");
        dom.set_attribute(input, "value", "default").unwrap();
        assert_eq!(dom.value(input), "default");
        dom.set_value(input, "typed");
        assert_eq!(dom.value(input), "typed");
        // Setting the property does not rewrite the attribute.
        assert_eq!(dom.get_attribute(input, "value"), Some("default"));
    }

    #[test]
    fn checked_defaults_to_attribute_presence_then_reflects_set() {
        let mut dom = Dom::new();
        let input = dom.create_element("input");
        assert!(!dom.checked(input));
        dom.set_attribute(input, "checked", "").unwrap();
        assert!(dom.checked(input));
        dom.set_checked(input, false);
        assert!(!dom.checked(input));
        dom.set_checked(input, true);
        assert!(dom.checked(input));
    }

    #[test]
    fn value_and_checked_on_non_element_are_defaults() {
        let mut dom = Dom::new();
        let t = dom.create_text("x");
        assert_eq!(dom.value(t), "");
        assert!(!dom.checked(t));
    }
}
```

- [ ] **Step 2: Add the `NodeId` ffi impl and the `ElementData` fields**

In `crates/superui_dom/src/node.rs`, directly after the `new_key_type! { ... }` block that defines `NodeId`, add:

```rust
use slotmap::Key;

impl NodeId {
    /// Encode this handle as a stable `u64` (for marshalling to the JS layer).
    pub fn to_ffi(self) -> u64 {
        self.data().as_ffi()
    }

    /// Reconstruct a handle from [`NodeId::to_ffi`]. The result is only valid if
    /// the original node still exists; accessors return `None` otherwise.
    pub fn from_ffi(v: u64) -> Self {
        slotmap::KeyData::from_ffi(v).into()
    }
}
```

In the same file, extend `ElementData` with two fields (keep existing fields):

```rust
#[derive(Clone, Debug, Default)]
pub struct ElementData {
    pub tag: String,
    pub(crate) attrs: Vec<(String, String)>,
    pub(crate) listeners: Vec<Listener>,
    /// `.value` IDL property once set by JS (`None` = derive from attribute).
    pub(crate) value: Option<String>,
    /// `.checked` IDL property once set by JS (`None` = derive from attribute).
    pub(crate) checked: Option<bool>,
}
```

In `crates/superui_dom/src/lib.rs`, add the module (after the existing `mod attr;` line):

```rust
mod props;
```

- [ ] **Step 3: Run the tests**

Run: `cargo test -p superui_dom`
Expected: PASS — all existing tests plus `ffi_tests::node_id_round_trips_through_ffi` and the three `props::tests` cases pass.

- [ ] **Step 4: Commit**

```bash
git add crates/superui_dom
git commit -m "feat(dom): NodeId ffi round-trip + .value/.checked property model"
```

---

### Task 2: `superui_dom` — CSS-subset selector engine (`query_selector`/`query_selector_all`)

**Files:**
- Create: `crates/superui_dom/src/selector.rs`
- Modify: `crates/superui_dom/src/lib.rs` (add `mod selector;`)

**Interfaces:**
- Consumes: `Dom`, `NodeId`, `NodeKind`.
- Produces:
  - `impl Dom { pub fn query_selector(&self, root: NodeId, selector: &str) -> Option<NodeId>; pub fn query_selector_all(&self, root: NodeId, selector: &str) -> Vec<NodeId> }`
  - Supports: type (`div`, `*`), id (`#foo`), class (`.bar`), compound (`input.toggle`, `li.completed`), and the descendant combinator (whitespace, e.g. `.todo-list li`). Matches descendants of `root` (excludes `root` itself), document order. Unparseable selectors yield no matches (graceful degradation).

- [ ] **Step 1: Write the failing tests** — create the file with tests first.

Create `crates/superui_dom/src/selector.rs`:

```rust
use crate::node::NodeId;
use crate::tree::Dom;

/// A single compound selector: an optional type, an optional id, and zero or
/// more required classes (e.g. `input.toggle` → tag=input, classes=[toggle]).
struct Compound {
    tag: Option<String>,
    id: Option<String>,
    classes: Vec<String>,
}

/// Parse one compound selector (no combinators). Returns `None` if malformed.
fn parse_compound(sel: &str) -> Option<Compound> {
    let mut tag = None;
    let mut id = None;
    let mut classes = Vec::new();

    let mut rest = sel;
    // Optional leading type selector (runs until the first '.' or '#').
    let type_end = rest.find(['.', '#']).unwrap_or(rest.len());
    if type_end > 0 {
        let t = &rest[..type_end];
        if t != "*" {
            tag = Some(t.to_ascii_lowercase());
        }
        rest = &rest[type_end..];
    }
    // Then a sequence of `.class` / `#id` tokens.
    while !rest.is_empty() {
        let marker = rest.as_bytes()[0];
        let after = &rest[1..];
        let end = after.find(['.', '#']).unwrap_or(after.len());
        let name = &after[..end];
        if name.is_empty() {
            return None;
        }
        match marker {
            b'.' => classes.push(name.to_string()),
            b'#' => id = Some(name.to_string()),
            _ => return None,
        }
        rest = &after[end..];
    }
    Some(Compound { tag, id, classes })
}

/// Parse a full selector into its whitespace-separated compound sequence.
fn parse_selector(selector: &str) -> Option<Vec<Compound>> {
    let compounds: Option<Vec<Compound>> =
        selector.split_whitespace().map(parse_compound).collect();
    match compounds {
        Some(v) if !v.is_empty() => Some(v),
        _ => None,
    }
}

/// Whether `node` (an element) matches a single compound selector.
fn matches_compound(dom: &Dom, node: NodeId, c: &Compound) -> bool {
    if !dom.is_element(node) {
        return false;
    }
    if let Some(t) = &c.tag {
        if dom.tag(node) != Some(t.as_str()) {
            return false;
        }
    }
    if let Some(id) = &c.id {
        if dom.get_attribute(node, "id") != Some(id.as_str()) {
            return false;
        }
    }
    for class in &c.classes {
        if !dom.class_contains(node, class) {
            return false;
        }
    }
    true
}

/// Whether `node` matches the full descendant-combinator selector: it must match
/// the rightmost compound, and the preceding compounds must match ancestors in
/// order (not necessarily contiguous).
fn matches_selector(dom: &Dom, node: NodeId, compounds: &[Compound]) -> bool {
    let last = compounds.len() - 1;
    if !matches_compound(dom, node, &compounds[last]) {
        return false;
    }
    if last == 0 {
        return true;
    }
    // Match compounds[last-1..=0] up the ancestor chain, right to left.
    let mut ci = last - 1;
    let mut cur = dom.parent(node);
    while let Some(a) = cur {
        if matches_compound(dom, a, &compounds[ci]) {
            if ci == 0 {
                return true;
            }
            ci -= 1;
        }
        cur = dom.parent(a);
    }
    false
}

impl Dom {
    /// All elements in `root`'s subtree (excluding `root`) matching `selector`,
    /// in document order. Empty for an unparseable selector.
    pub fn query_selector_all(&self, root: NodeId, selector: &str) -> Vec<NodeId> {
        let Some(compounds) = parse_selector(selector) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        fn walk(dom: &Dom, node: NodeId, compounds: &[Compound], out: &mut Vec<NodeId>) {
            for &child in dom.children(node) {
                if matches_selector(dom, child, compounds) {
                    out.push(child);
                }
                walk(dom, child, compounds, out);
            }
        }
        walk(self, root, &compounds, &mut out);
        out
    }

    /// The first element (document order) in `root`'s subtree matching
    /// `selector`, or `None`.
    pub fn query_selector(&self, root: NodeId, selector: &str) -> Option<NodeId> {
        self.query_selector_all(root, selector).into_iter().next()
    }
}

#[cfg(test)]
mod tests {
    use crate::Dom;

    /// Build: document > section.todoapp > ul.todo-list > (li.completed > label, li > label)
    fn fixture() -> (Dom, super::NodeId, super::NodeId, super::NodeId) {
        let mut dom = Dom::new();
        let doc = dom.document();
        let section = dom.create_element("section");
        dom.set_attribute(section, "class", "todoapp").unwrap();
        let ul = dom.create_element("ul");
        dom.set_attribute(ul, "class", "todo-list").unwrap();
        let li1 = dom.create_element("li");
        dom.set_attribute(li1, "class", "completed").unwrap();
        let label1 = dom.create_element("label");
        let li2 = dom.create_element("li");
        let label2 = dom.create_element("label");
        dom.append_child(doc, section).unwrap();
        dom.append_child(section, ul).unwrap();
        dom.append_child(ul, li1).unwrap();
        dom.append_child(li1, label1).unwrap();
        dom.append_child(ul, li2).unwrap();
        dom.append_child(li2, label2).unwrap();
        (dom, section, li1, li2)
    }

    #[test]
    fn type_class_id_selectors() {
        let (mut dom, section, _, _) = fixture();
        dom.set_attribute(section, "id", "app").unwrap();
        let root = dom.document();
        assert_eq!(dom.query_selector(root, "section"), Some(section));
        assert_eq!(dom.query_selector(root, ".todoapp"), Some(section));
        assert_eq!(dom.query_selector(root, "#app"), Some(section));
        assert_eq!(dom.query_selector(root, "section.todoapp"), Some(section));
        assert_eq!(dom.query_selector(root, ".nope"), None);
    }

    #[test]
    fn descendant_combinator_and_query_all() {
        let (dom, _, li1, li2) = fixture();
        let root = dom.document();
        let lis = dom.query_selector_all(root, ".todo-list li");
        assert_eq!(lis, vec![li1, li2]);
        assert_eq!(dom.query_selector(root, "li.completed"), Some(li1));
        assert_eq!(dom.query_selector_all(root, "label").len(), 2);
    }

    #[test]
    fn scoped_to_root_excludes_root_itself() {
        let (dom, section, _, _) = fixture();
        // Searching within `section` never returns `section`.
        assert_eq!(dom.query_selector(section, "section"), None);
        assert!(dom.query_selector(section, ".todo-list").is_some());
    }

    #[test]
    fn malformed_selector_yields_nothing() {
        let (dom, _, _, _) = fixture();
        let root = dom.document();
        assert_eq!(dom.query_selector_all(root, ".").len(), 0);
        assert_eq!(dom.query_selector(root, ""), None);
    }
}
```

- [ ] **Step 2: Wire the module** — in `crates/superui_dom/src/lib.rs`, add after `mod props;`:

```rust
mod selector;
```

- [ ] **Step 3: Run the tests**

Run: `cargo test -p superui_dom`
Expected: PASS — the four `selector::tests` cases pass alongside everything else.

- [ ] **Step 4: Commit**

```bash
git add crates/superui_dom
git commit -m "feat(dom): CSS-subset selector engine (query_selector/all)"
```

---

### Task 3: `superui_js` — crate skeleton, Boa deps, wasm config, `JsEngine` trait, `BoaEngine`, `HostState` types

**Files:**
- Modify: `Cargo.toml` (workspace deps: add `boa_engine`, `boa_gc`)
- Create: `.cargo/config.toml`
- Create: `crates/superui_js/Cargo.toml`
- Create: `crates/superui_js/src/lib.rs`
- Create: `crates/superui_js/src/state.rs`
- Create: `crates/superui_js/src/engine.rs`

**Interfaces:**
- Consumes: `superui_dom::{Dom, NodeId}`; Boa 0.21.
- Produces:
  - `pub trait JsEngine { fn eval(&mut self, script: &str) -> Result<(), String>; }` (extended in Tasks 8–9).
  - `pub struct BoaEngine { pub(crate) context: Context, pub(crate) dom: Rc<RefCell<Dom>> }` with `pub fn new(dom: Rc<RefCell<Dom>>) -> BoaEngine`, `pub fn context_mut(&mut self) -> &mut Context`, `pub fn dom(&self) -> Rc<RefCell<Dom>>`, and `impl JsEngine for BoaEngine`.
  - `pub struct HostState { pub dom: Rc<RefCell<Dom>>, pub wrappers: HashMap<u64, JsObject>, pub listeners: HashMap<u64, JsFunction>, pub protos: Protos, pub timers: Vec<Timer>, pub now_ms: f64, pub next_timer_id: u64 }` (GC-managed via HostDefined).
  - `pub struct Protos { document, element, text, event, token_list, style: Option<JsObject> }` (`Default`).
  - `pub struct NodeHandle { pub node: NodeId }`, `pub struct Timer { .. }` (defined here, used later).

- [ ] **Step 1: Add workspace dependencies** — in the root `Cargo.toml` `[workspace.dependencies]` table (keep existing lines), add:

```toml
boa_engine = "0.21"
boa_gc = "0.21"
```

- [ ] **Step 2: Create the wasm getrandom config** — create `.cargo/config.toml` at the repo root:

```toml
[target.wasm32-unknown-unknown]
rustflags = ['--cfg', 'getrandom_backend="wasm_js"']
```

- [ ] **Step 3: Create the crate manifest** — create `crates/superui_js/Cargo.toml`:

```toml
[package]
name = "superui_js"
edition.workspace = true
version.workspace = true
license.workspace = true

[dependencies]
superui_dom = { path = "../superui_dom" }
boa_engine.workspace = true
boa_gc.workspace = true

# Boa pulls getrandom 0.3, which needs the JS backend on wasm. Scope the direct
# dep to the wasm target so native builds are unaffected. Pair with the
# `.cargo/config.toml` rustflag (repo root).
[target.'cfg(target_arch = "wasm32")'.dependencies]
getrandom = { version = "0.3", features = ["wasm_js"] }
```

- [ ] **Step 4: Create `state.rs` with the host-state types** — create `crates/superui_js/src/state.rs`:

```rust
//! State shared between the Boa engine and all native DOM bindings.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use boa_engine::object::builtins::JsFunction;
use boa_engine::{JsData, JsObject};
use boa_gc::{Finalize, Trace};

use superui_dom::{Dom, NodeId};

/// Native data carried by every JS `Node`/`Element` wrapper: the arena handle.
/// Never a Bevy `Entity` (design §3).
#[derive(Trace, Finalize, JsData)]
pub struct NodeHandle {
    #[unsafe_ignore_trace]
    pub node: NodeId,
}

/// The per-interface shared prototypes, installed once by `superui_api::install`.
#[derive(Trace, Finalize, Default)]
pub struct Protos {
    pub document: Option<JsObject>,
    pub element: Option<JsObject>,
    pub text: Option<JsObject>,
    pub event: Option<JsObject>,
    pub token_list: Option<JsObject>,
    pub style: Option<JsObject>,
}

/// A scheduled timer callback (Task 9).
#[derive(Trace, Finalize)]
pub struct Timer {
    pub id: u64,
    pub callback: JsFunction,
    pub due_ms: f64,
    /// `Some(period)` for `setInterval`, `None` for `setTimeout`.
    pub interval_ms: Option<f64>,
}

/// GC-managed state stored in Boa's `HostDefined` realm slot. Every native
/// binding reaches the DOM and registries through it.
#[derive(Trace, Finalize, JsData)]
pub struct HostState {
    /// The live retained DOM. Plain Rust (not GC-managed), so ignored by the tracer.
    #[unsafe_ignore_trace]
    pub dom: Rc<RefCell<Dom>>,
    /// `NodeId.to_ffi()` → JS wrapper, giving stable object identity.
    pub wrappers: HashMap<u64, JsObject>,
    /// `ListenerId.0` → JS callback for registered DOM listeners.
    pub listeners: HashMap<u64, JsFunction>,
    /// Shared interface prototypes.
    pub protos: Protos,
    /// Pending timers.
    pub timers: Vec<Timer>,
    /// Monotonic clock (milliseconds) advanced by `run_timers`.
    #[unsafe_ignore_trace]
    pub now_ms: f64,
    /// Next timer id to hand out.
    pub next_timer_id: u64,
}

impl HostState {
    pub fn new(dom: Rc<RefCell<Dom>>) -> Self {
        HostState {
            dom,
            wrappers: HashMap::new(),
            listeners: HashMap::new(),
            protos: Protos::default(),
            timers: Vec::new(),
            now_ms: 0.0,
            next_timer_id: 1,
        }
    }
}
```

- [ ] **Step 5: Create `engine.rs`** — create `crates/superui_js/src/engine.rs`:

```rust
//! The Boa-backed [`JsEngine`] implementation.

use std::cell::RefCell;
use std::rc::Rc;

use boa_engine::{Context, Source};

use superui_dom::Dom;

use crate::state::HostState;
use crate::JsEngine;

/// A Boa JS context wired to a shared [`Dom`]. Single-threaded.
pub struct BoaEngine {
    pub(crate) context: Context,
    pub(crate) dom: Rc<RefCell<Dom>>,
}

impl BoaEngine {
    /// Build an engine sharing `dom`. Installs [`HostState`] into the realm's
    /// `HostDefined` slot; call `superui_api::install` before evaluating author
    /// scripts to populate the DOM/Web API surface.
    pub fn new(dom: Rc<RefCell<Dom>>) -> Self {
        let mut context = Context::default();
        context
            .realm()
            .host_defined_mut()
            .insert(HostState::new(dom.clone()));
        BoaEngine { context, dom }
    }

    /// Mutable access to the underlying Boa context (used by `superui_api` to
    /// install bindings).
    pub fn context_mut(&mut self) -> &mut Context {
        &mut self.context
    }

    /// A clone of the shared DOM handle.
    pub fn dom(&self) -> Rc<RefCell<Dom>> {
        self.dom.clone()
    }
}

impl JsEngine for BoaEngine {
    fn eval(&mut self, script: &str) -> Result<(), String> {
        self.context
            .eval(Source::from_bytes(script))
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}
```

- [ ] **Step 6: Create `lib.rs`** — create `crates/superui_js/src/lib.rs`:

```rust
//! JS engine boundary + Boa backend for bevy_superui.
//!
//! Owns the retained-DOM ↔ JS marshalling. Knows nothing about Bevy.
//! Headless-testable.

mod engine;
mod state;

pub use engine::BoaEngine;
pub use state::{HostState, NodeHandle, Protos, Timer};

/// The coarse boundary the Bevy layers consume so they never name Boa. Fine-
/// grained DOM bindings live in `superui_api`, not here. Extended with
/// `dispatch_event` (Task 8) and `run_timers` (Task 9).
pub trait JsEngine {
    /// Evaluate a script against the current context. `Err` carries a message.
    fn eval(&mut self, script: &str) -> Result<(), String>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;
    use superui_dom::Dom;

    #[test]
    fn eval_runs_and_shares_the_dom_handle() {
        let dom = Rc::new(RefCell::new(Dom::new()));
        let mut engine = BoaEngine::new(dom.clone());
        engine.eval("var x = 1 + 2;").expect("eval ok");
        // The engine holds the same DOM Rc we passed in.
        assert_eq!(Rc::strong_count(&dom), 3); // caller + engine.dom + HostState.dom
    }

    #[test]
    fn eval_reports_syntax_errors_without_panicking() {
        let dom = Rc::new(RefCell::new(Dom::new()));
        let mut engine = BoaEngine::new(dom);
        assert!(engine.eval("this is not valid )(").is_err());
    }
}
```

- [ ] **Step 7: Run the tests**

Run: `cargo test -p superui_js`
Expected: PASS — `eval_runs_and_shares_the_dom_handle` and `eval_reports_syntax_errors_without_panicking` pass. (If the `Rc::strong_count` assertion is off by the exact number, adjust it to the observed count — the point is that the DOM is shared, not the precise count; but 3 is expected: the local `dom`, `BoaEngine.dom`, and `HostState.dom`.)

- [ ] **Step 8: Verify the wasm build**

Run: `cargo build -p superui_js --target wasm32-unknown-unknown`
Expected: SUCCESS. If it fails on getrandom (`inner_u32`/`backends`), confirm both the `.cargo/config.toml` rustflag and the wasm-target `getrandom` dep with the `wasm_js` feature are present.

- [ ] **Step 9: Commit**

```bash
git add Cargo.toml .cargo crates/superui_js
git commit -m "feat(js): superui_js skeleton — Boa engine, HostState, JsEngine trait; wasm-clean"
```

---

### Task 4: `superui_js` — DOM↔JS marshalling toolkit (`dom_of`, `node_id_of`, `wrap_node`)

**Files:**
- Modify: `crates/superui_js/src/state.rs` (add the toolkit functions + tests)
- Modify: `crates/superui_js/src/lib.rs` (re-export the toolkit)

**Interfaces:**
- Consumes: `HostState`, `NodeHandle`, `Protos`, Boa `Context`/`JsObject`/`JsValue`/`JsString`, `superui_dom::{NodeId, NodeKind}`.
- Produces (all `pub`, re-exported from crate root):
  - `pub fn dom_of(context: &mut Context) -> Rc<RefCell<Dom>>`
  - `pub fn node_id_of(this: &JsValue) -> Option<NodeId>`
  - `pub fn wrap_node(context: &mut Context, node: NodeId) -> JsObject` — cached, kind-dispatched wrapper; stable identity.
  - `pub fn wrap_opt_node(context: &mut Context, node: Option<NodeId>) -> JsValue` — wrapper or `null`.
  - `pub fn jsstr(s: &str) -> JsValue`
  - `pub fn with_host_state<R>(context: &mut Context, f: impl FnOnce(&HostState) -> R) -> R`
  - `pub fn with_host_state_mut<R>(context: &mut Context, f: impl FnOnce(&mut HostState) -> R) -> R`

- [ ] **Step 1: Write the failing test** — append to `crates/superui_js/src/state.rs`:

```rust
#[cfg(test)]
mod toolkit_tests {
    use super::*;
    use boa_engine::{Context, JsObject};

    /// Insert a HostState with a minimal element prototype so `wrap_node` works.
    fn ctx_with_state(dom: Rc<RefCell<Dom>>) -> Context {
        let mut context = Context::default();
        context.realm().host_defined_mut().insert(HostState::new(dom));
        let element_proto = JsObject::with_object_proto(context.intrinsics());
        let text_proto = JsObject::with_object_proto(context.intrinsics());
        with_host_state_mut(&mut context, |s| {
            s.protos.element = Some(element_proto);
            s.protos.text = Some(text_proto);
        });
        context
    }

    #[test]
    fn wrap_node_is_identity_stable_and_round_trips_the_id() {
        let dom = Rc::new(RefCell::new(Dom::new()));
        let el = dom.borrow_mut().create_element("div");
        let mut context = ctx_with_state(dom);

        let a = wrap_node(&mut context, el);
        let b = wrap_node(&mut context, el);
        // Same NodeId → same JS object (===).
        assert!(JsObject::equals(&a, &b));
        // The wrapper carries the original NodeId.
        assert_eq!(node_id_of(&a.clone().into()), Some(el));
    }

    #[test]
    fn wrap_opt_node_maps_none_to_null() {
        let dom = Rc::new(RefCell::new(Dom::new()));
        let mut context = ctx_with_state(dom);
        assert!(wrap_opt_node(&mut context, None).is_null());
    }

    #[test]
    fn node_id_of_non_node_is_none() {
        assert_eq!(node_id_of(&booa_undefined()), None);
    }

    fn booa_undefined() -> boa_engine::JsValue {
        boa_engine::JsValue::undefined()
    }
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p superui_js`
Expected: FAIL — `dom_of`/`node_id_of`/`wrap_node`/`wrap_opt_node`/`with_host_state*` are undefined.

- [ ] **Step 3: Implement the toolkit** — add to `crates/superui_js/src/state.rs` (before the `#[cfg(test)]` modules), and add the needed imports at the top:

Add imports at the top of `state.rs` (merge with existing `use` lines):

```rust
use boa_engine::{Context, JsString, JsValue};
use superui_dom::NodeKind;
```

Add the functions:

```rust
/// Run `f` with a shared borrow of the realm's [`HostState`].
pub fn with_host_state<R>(context: &mut Context, f: impl FnOnce(&HostState) -> R) -> R {
    let host = context.realm().host_defined();
    let state = host.get::<HostState>().expect("HostState installed");
    f(state)
}

/// Run `f` with a mutable borrow of the realm's [`HostState`]. Do not call other
/// `context` methods inside `f` (the realm is borrowed).
pub fn with_host_state_mut<R>(context: &mut Context, f: impl FnOnce(&mut HostState) -> R) -> R {
    let mut host = context.realm().host_defined_mut();
    let state = host.get_mut::<HostState>().expect("HostState installed");
    f(state)
}

/// A clone of the shared DOM handle (guard dropped before returning).
pub fn dom_of(context: &mut Context) -> Rc<RefCell<Dom>> {
    with_host_state(context, |s| s.dom.clone())
}

/// The `NodeId` carried by a JS node wrapper, or `None` for any other value.
pub fn node_id_of(this: &JsValue) -> Option<NodeId> {
    this.as_object()
        .and_then(|o| o.downcast_ref::<NodeHandle>().map(|h| h.node))
}

/// A `JsValue` string (for returning DOM strings to JS).
pub fn jsstr(s: &str) -> JsValue {
    JsValue::from(JsString::from(s))
}

/// The stable JS wrapper for `node`, creating and caching it on first use. The
/// prototype is chosen by node kind (document/element/text), so the wrapper has
/// the right methods. Panics only if the protos were not installed first.
pub fn wrap_node(context: &mut Context, node: NodeId) -> JsObject {
    let key = node.to_ffi();
    if let Some(existing) = with_host_state(context, |s| s.wrappers.get(&key).cloned()) {
        return existing;
    }
    // Choose the prototype by node kind (short DOM borrow, then drop it).
    let proto = {
        let dom = dom_of(context);
        let dom = dom.borrow();
        let kind = dom.get(node).map(|n| match &n.kind {
            NodeKind::Text(_) => "text",
            NodeKind::Document => "document",
            NodeKind::Element(_) => "element",
        });
        with_host_state(context, |s| match kind {
            Some("text") => s.protos.text.clone(),
            Some("document") => s.protos.document.clone(),
            _ => s.protos.element.clone(),
        })
        .expect("node prototypes installed before wrapping")
    };
    let obj = JsObject::from_proto_and_data(proto, NodeHandle { node });
    with_host_state_mut(context, |s| {
        s.wrappers.insert(key, obj.clone());
    });
    obj
}

/// `wrap_node(node)` if `Some`, else JS `null`.
pub fn wrap_opt_node(context: &mut Context, node: Option<NodeId>) -> JsValue {
    match node {
        Some(id) => wrap_node(context, id).into(),
        None => JsValue::null(),
    }
}
```

- [ ] **Step 4: Re-export from the crate root** — in `crates/superui_js/src/lib.rs`, extend the `state` re-export line:

```rust
pub use state::{
    dom_of, jsstr, node_id_of, with_host_state, with_host_state_mut, wrap_node, wrap_opt_node,
    HostState, NodeHandle, Protos, Timer,
};
```

- [ ] **Step 5: Run the tests**

Run: `cargo test -p superui_js`
Expected: PASS — the three `toolkit_tests` cases pass. (`JsObject::equals(&a, &b)` is object identity; if the exact helper name differs, use `JsValue::from(a).strict_equals(&JsValue::from(b))` or compare raw pointers via `JsObject::equals` per the compiler's guidance — the assertion is "same object".)

- [ ] **Step 6: Commit**

```bash
git add crates/superui_js
git commit -m "feat(js): DOM<->JS marshalling toolkit (wrap_node identity, node_id_of, dom_of)"
```

---

### Task 5: `superui_api` — crate skeleton, `install()` scaffold, `console`, `fetch` stub, node prototypes

**Files:**
- Create: `crates/superui_api/Cargo.toml`
- Create: `crates/superui_api/src/lib.rs`
- Create: `crates/superui_api/src/console.rs`
- Create: `crates/superui_api/src/fetch.rs`

**Interfaces:**
- Consumes: `superui_js::{BoaEngine, with_host_state_mut, jsstr}`; Boa APIs.
- Produces:
  - `pub fn install(engine: &mut BoaEngine)` — builds and stores the document/element/text/event/token_list/style prototypes into `HostState.protos`, then registers `console`, `fetch` (Tasks 6–9 extend it with the DOM surface).
  - `console.{log,warn,error,info}` (each stringifies args, space-joined, into a test-visible sink); `fetch(...)` warns and returns a rejected promise.
- For assertion in headless tests, console output is appended to a `Vec<String>` reachable from Rust. This is stored in `HostState`? No — to avoid widening `HostState`, console writes go to a process-local test sink guarded behind a helper. Implementation below uses a `thread_local!` buffer in `console.rs`, exposed via `console_take()` for tests.

- [ ] **Step 1: Create the crate manifest** — create `crates/superui_api/Cargo.toml`:

```toml
[package]
name = "superui_api"
edition.workspace = true
version.workspace = true
license.workspace = true

[dependencies]
superui_dom = { path = "../superui_dom" }
superui_js = { path = "../superui_js" }
boa_engine.workspace = true
boa_gc.workspace = true

[target.'cfg(target_arch = "wasm32")'.dependencies]
getrandom = { version = "0.3", features = ["wasm_js"] }
```

- [ ] **Step 2: Create `console.rs`** — create `crates/superui_api/src/console.rs`:

```rust
//! `console.{log,warn,error,info}` bindings.

use std::cell::RefCell;

use boa_engine::{
    js_string, object::FunctionObjectBuilder, property::Attribute, Context, JsArgs, JsObject,
    JsResult, JsValue, NativeFunction,
};

thread_local! {
    /// Test-visible sink of console output lines (level-prefixed).
    static SINK: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// Drain and return the console lines captured so far (for tests).
pub fn console_take() -> Vec<String> {
    SINK.with(|s| std::mem::take(&mut *s.borrow_mut()))
}

fn stringify_args(args: &[JsValue], context: &mut Context) -> String {
    args.iter()
        .map(|a| {
            a.to_string(context)
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_else(|_| "<unprintable>".to_string())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn log_at(level: &str, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let line = format!("{level}: {}", stringify_args(args, context));
    SINK.with(|s| s.borrow_mut().push(line));
    Ok(JsValue::undefined())
}

fn method(context: &mut Context, name: &str, f: fn(&JsValue, &[JsValue], &mut Context) -> JsResult<JsValue>) -> JsValue {
    FunctionObjectBuilder::new(context.realm(), NativeFunction::from_fn_ptr(f))
        .name(js_string!(name))
        .length(1)
        .build()
        .into()
}

/// Install the `console` global.
pub fn install_console(context: &mut Context) {
    let console = JsObject::with_object_proto(context.intrinsics());
    let log = method(context, "log", |_, a, c| log_at("log", a, c));
    let warn = method(context, "warn", |_, a, c| log_at("warn", a, c));
    let error = method(context, "error", |_, a, c| log_at("error", a, c));
    let info = method(context, "info", |_, a, c| log_at("info", a, c));
    console.set(js_string!("log"), log, false, context).unwrap();
    console.set(js_string!("warn"), warn, false, context).unwrap();
    console.set(js_string!("error"), error, false, context).unwrap();
    console.set(js_string!("info"), info, false, context).unwrap();
    context
        .register_global_property(js_string!("console"), console, Attribute::all())
        .unwrap();
}
```

- [ ] **Step 3: Create `fetch.rs`** — create `crates/superui_api/src/fetch.rs`:

```rust
//! `fetch` — a deliberate warn-and-reject stub (design §2: network is ⛔ forever).

use boa_engine::{
    js_string, object::builtins::JsPromise, Context, JsArgs, JsNativeError, JsResult, JsValue,
    NativeFunction,
};

fn fetch_impl(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let url = args
        .get_or_undefined(0)
        .to_string(context)
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();
    // Warn (via console if present) and reject — never actually hit the network.
    let msg = format!("fetch is not supported (network is out of scope): {url}");
    let promise = JsPromise::reject(JsNativeError::typ().with_message(msg), context);
    Ok(promise.into())
}

/// Install the `fetch` global.
pub fn install_fetch(context: &mut Context) {
    context
        .register_global_callable(js_string!("fetch"), 1, NativeFunction::from_fn_ptr(fetch_impl))
        .unwrap();
}
```

- [ ] **Step 4: Create `lib.rs` with `install()` and the failing test** — create `crates/superui_api/src/lib.rs`:

```rust
//! Standards-shaped DOM/Web API surface installed onto a `superui_js::BoaEngine`.
//!
//! Uses Boa directly (design §4 permits this crate to depend on Boa). Knows
//! nothing about Bevy. Headless-testable.

mod console;
mod fetch;

pub use console::console_take;

use boa_engine::{Context, JsObject};
use superui_js::{with_host_state_mut, BoaEngine};

/// Build the six shared interface prototypes as empty ordinary objects and store
/// them in `HostState.protos`. Later phases (this task's callers, Tasks 6–9)
/// attach methods/accessors to these same proto objects.
fn build_protos(context: &mut Context) {
    let document = JsObject::with_object_proto(context.intrinsics());
    let element = JsObject::with_object_proto(context.intrinsics());
    let text = JsObject::with_object_proto(context.intrinsics());
    let event = JsObject::with_object_proto(context.intrinsics());
    let token_list = JsObject::with_object_proto(context.intrinsics());
    let style = JsObject::with_object_proto(context.intrinsics());
    with_host_state_mut(context, |s| {
        s.protos.document = Some(document);
        s.protos.element = Some(element);
        s.protos.text = Some(text);
        s.protos.event = Some(event);
        s.protos.token_list = Some(token_list);
        s.protos.style = Some(style);
    });
}

/// Install the full DOM/Web API surface onto `engine`. Call once, after
/// `BoaEngine::new` and before evaluating author scripts.
pub fn install(engine: &mut BoaEngine) {
    let context = engine.context_mut();
    build_protos(context);
    console::install_console(context);
    fetch::install_fetch(context);
    // Tasks 6–9 extend install() with document/node/element/events/timers.
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;
    use superui_dom::Dom;
    use superui_js::JsEngine;

    fn engine() -> BoaEngine {
        let dom = Rc::new(RefCell::new(Dom::new()));
        let mut e = BoaEngine::new(dom);
        install(&mut e);
        e
    }

    #[test]
    fn console_log_reaches_the_rust_sink() {
        let mut e = engine();
        e.eval("console.log('hello', 42); console.warn('careful');").unwrap();
        let lines = console_take();
        assert_eq!(lines, vec!["log: hello 42".to_string(), "warn: careful".to_string()]);
    }

    #[test]
    fn fetch_rejects_and_runs_the_catch() {
        let mut e = engine();
        e.eval("globalThis.caught = null; fetch('http://x').catch(err => { globalThis.caught = String(err); });")
            .unwrap();
        // Promise reactions are lazy — pump the job queue.
        e.context_mut().run_jobs().unwrap();
        let caught = e
            .context_mut()
            .eval(boa_engine::Source::from_bytes("globalThis.caught"))
            .unwrap()
            .to_string(e.context_mut())
            .unwrap()
            .to_std_string_escaped();
        assert!(caught.contains("fetch is not supported"), "got: {caught}");
    }
}
```

- [ ] **Step 5: Run the tests**

Run: `cargo test -p superui_api`
Expected: PASS — `console_log_reaches_the_rust_sink` and `fetch_rejects_and_runs_the_catch` pass. (If `run_jobs()` returns a different type, adjust `.unwrap()` per the compiler — it returns `JsResult<()>` in this version.)

- [ ] **Step 6: Commit**

```bash
git add crates/superui_api
git commit -m "feat(api): superui_api skeleton — install scaffold, console, fetch stub, node protos"
```

---

### Task 6: `superui_api` — `document` object + methods (getElementById, createElement, createTextNode, querySelector/All)

**Files:**
- Create: `crates/superui_api/src/document.rs`
- Modify: `crates/superui_api/src/lib.rs` (`mod document;`, call `document::install_document` in `install`)

**Interfaces:**
- Consumes: `superui_js::{wrap_node, wrap_opt_node, node_id_of, dom_of, with_host_state, jsstr}`.
- Produces: `pub fn install_document(context: &mut Context)` — attaches methods to the `document` proto, wraps the DOM's document node with that proto, and registers it as the `document` global.
  - `document.getElementById(id) -> Element|null`
  - `document.createElement(tag) -> Element`
  - `document.createTextNode(data) -> Text`
  - `document.querySelector(sel) -> Element|null`
  - `document.querySelectorAll(sel) -> Array<Element>`

- [ ] **Step 1: Write the failing test** — append to `crates/superui_api/src/lib.rs` `tests` module:

```rust
    #[test]
    fn document_queries_and_factories() {
        // Seed a DOM: document > div#root > span.item ("hi")
        let dom = Rc::new(RefCell::new(Dom::new()));
        {
            let mut d = dom.borrow_mut();
            let doc = d.document();
            let root = d.create_element("div");
            d.set_attribute(root, "id", "root").unwrap();
            let span = d.create_element("span");
            d.set_attribute(span, "class", "item").unwrap();
            let t = d.create_text("hi");
            d.append_child(doc, root).unwrap();
            d.append_child(root, span).unwrap();
            d.append_child(span, t).unwrap();
        }
        let mut e = BoaEngine::new(dom);
        install(&mut e);

        e.eval(
            r#"
            var byId = document.getElementById('root');
            globalThis.foundById = (byId !== null);
            globalThis.idStable = (document.getElementById('root') === byId);
            globalThis.qs = (document.querySelector('.item') !== null);
            globalThis.qsaLen = document.querySelectorAll('span').length;
            var made = document.createElement('p');
            globalThis.madeIsObject = (typeof made === 'object' && made !== null);
            var txt = document.createTextNode('yo');
            globalThis.txtIsObject = (typeof txt === 'object' && txt !== null);
            globalThis.missing = (document.getElementById('nope') === null);
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
        assert_eq!(check(&mut e, "globalThis.foundById"), "true");
        assert_eq!(check(&mut e, "globalThis.idStable"), "true");
        assert_eq!(check(&mut e, "globalThis.qs"), "true");
        assert_eq!(check(&mut e, "globalThis.qsaLen"), "1");
        assert_eq!(check(&mut e, "globalThis.madeIsObject"), "true");
        assert_eq!(check(&mut e, "globalThis.txtIsObject"), "true");
        assert_eq!(check(&mut e, "globalThis.missing"), "true");
    }
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p superui_api document_queries_and_factories`
Expected: FAIL — `document` is undefined in JS (not yet installed).

- [ ] **Step 3: Create `document.rs`** — create `crates/superui_api/src/document.rs`:

```rust
//! The `document` object and its factory/query methods.

use boa_engine::{
    js_string, object::builtins::JsArray, object::FunctionObjectBuilder, property::Attribute,
    Context, JsArgs, JsResult, JsValue, NativeFunction,
};
use superui_js::{dom_of, jsstr, node_id_of, with_host_state, wrap_node, wrap_opt_node};

fn get_element_by_id(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let id = args.get_or_undefined(0).to_string(context)?.to_std_string_escaped();
    let found = dom_of(context).borrow().get_element_by_id(&id);
    Ok(wrap_opt_node(context, found))
}

fn create_element(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let tag = args.get_or_undefined(0).to_string(context)?.to_std_string_escaped();
    let id = dom_of(context).borrow_mut().create_element(&tag);
    Ok(wrap_node(context, id).into())
}

fn create_text_node(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let data = args.get_or_undefined(0).to_string(context)?.to_std_string_escaped();
    let id = dom_of(context).borrow_mut().create_text(&data);
    Ok(wrap_node(context, id).into())
}

fn query_selector(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let sel = args.get_or_undefined(0).to_string(context)?.to_std_string_escaped();
    let dom = dom_of(context);
    let found = { let d = dom.borrow(); d.query_selector(d.document(), &sel) };
    Ok(wrap_opt_node(context, found))
}

fn query_selector_all(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let sel = args.get_or_undefined(0).to_string(context)?.to_std_string_escaped();
    let dom = dom_of(context);
    let matches = { let d = dom.borrow(); d.query_selector_all(d.document(), &sel) };
    let items: Vec<JsValue> = matches.into_iter().map(|id| wrap_node(context, id).into()).collect();
    Ok(JsArray::from_iter(items, context).into())
}

/// Attach a native method `name` to `proto`.
pub(crate) fn set_method(
    proto: &boa_engine::JsObject,
    name: &str,
    arity: usize,
    f: fn(&JsValue, &[JsValue], &mut Context) -> JsResult<JsValue>,
    context: &mut Context,
) {
    let func = FunctionObjectBuilder::new(context.realm(), NativeFunction::from_fn_ptr(f))
        .name(js_string!(name))
        .length(arity)
        .build();
    proto.set(js_string!(name), func, false, context).unwrap();
}

/// Build the document proto's methods, wrap the document node, and register it
/// as the `document` global.
pub fn install_document(context: &mut Context) {
    let proto = with_host_state(context, |s| s.protos.document.clone()).expect("document proto");
    set_method(&proto, "getElementById", 1, get_element_by_id, context);
    set_method(&proto, "createElement", 1, create_element, context);
    set_method(&proto, "createTextNode", 1, create_text_node, context);
    set_method(&proto, "querySelector", 1, query_selector, context);
    set_method(&proto, "querySelectorAll", 1, query_selector_all, context);

    let doc_id = dom_of(context).borrow().document();
    let document = wrap_node(context, doc_id);
    context
        .register_global_property(js_string!("document"), document, Attribute::all())
        .unwrap();
    let _ = (jsstr, node_id_of); // (kept in scope for later tasks; no-op)
}
```

Note: the trailing `let _ = (jsstr, node_id_of);` line only exists to avoid unused-import churn while later tasks are pending; **delete it** once Task 7 uses those imports. If it causes a warning, simply remove the two imports instead.

- [ ] **Step 4: Wire into `install`** — in `crates/superui_api/src/lib.rs`:

Add `mod document;` near the other module declarations, and in `install()` add after `fetch::install_fetch(context);`:

```rust
    document::install_document(context);
```

- [ ] **Step 5: Run the tests**

Run: `cargo test -p superui_api`
Expected: PASS — `document_queries_and_factories` and the Task 5 tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/superui_api
git commit -m "feat(api): document object — getElementById/createElement/createTextNode/querySelector(All)"
```

---

### Task 7: `superui_api` — Node/Element structural bindings (methods + parentNode/childNodes/... accessors)

**Files:**
- Create: `crates/superui_api/src/node.rs`
- Modify: `crates/superui_api/src/lib.rs` (`mod node;`, call `node::install_node` in `install`)

**Interfaces:**
- Consumes: the `element`/`text`/`document` protos (from Task 5/6), `superui_js` toolkit, `superui_dom` structural ops.
- Produces: `pub fn install_node(context: &mut Context)` attaching to the **element** proto (and, where shared, the document proto):
  - Methods: `appendChild(child) -> child`, `removeChild(child) -> child`, `insertBefore(new, ref) -> new`, `replaceChild(new, old) -> old`.
  - Accessors (getters): `parentNode`, `firstChild`, `nextSibling`, `previousSibling`, `childNodes` (Array incl. text), `children` (Array, elements only), `nodeType` (1 element / 3 text / 9 document), `tagName` (uppercase, elements).
- Uses a shared `set_accessor` helper (added here) and the `set_method` from `document.rs`.

- [ ] **Step 1: Write the failing test** — append to `crates/superui_api/src/lib.rs` `tests`:

```rust
    #[test]
    fn structural_mutation_and_navigation() {
        let dom = Rc::new(RefCell::new(Dom::new()));
        let mut e = BoaEngine::new(dom);
        install(&mut e);
        e.eval(
            r#"
            var root = document.createElement('div');
            document.getElementById; // smoke: document usable
            var a = document.createElement('span');
            var b = document.createElement('span');
            root.appendChild(a);
            root.appendChild(b);
            globalThis.parentOk = (a.parentNode === root);
            globalThis.count = root.childNodes.length;
            globalThis.firstIsA = (root.firstChild === a);
            globalThis.nextIsB = (a.nextSibling === b);
            root.insertBefore(document.createElement('em'), b);
            globalThis.count2 = root.children.length;
            root.removeChild(a);
            globalThis.count3 = root.children.length;
            globalThis.tag = root.tagName;
            globalThis.ntype = a.nodeType;
            "#,
        )
        .unwrap();
        let check = |e: &mut BoaEngine, expr: &str| -> String {
            e.context_mut().eval(boa_engine::Source::from_bytes(expr)).unwrap()
                .to_string(e.context_mut()).unwrap().to_std_string_escaped()
        };
        assert_eq!(check(&mut e, "globalThis.parentOk"), "true");
        assert_eq!(check(&mut e, "globalThis.count"), "2");
        assert_eq!(check(&mut e, "globalThis.firstIsA"), "true");
        assert_eq!(check(&mut e, "globalThis.nextIsB"), "true");
        assert_eq!(check(&mut e, "globalThis.count2"), "3");
        assert_eq!(check(&mut e, "globalThis.count3"), "2");
        assert_eq!(check(&mut e, "globalThis.tag"), "DIV");
        assert_eq!(check(&mut e, "globalThis.ntype"), "1");
    }
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p superui_api structural_mutation_and_navigation`
Expected: FAIL — `appendChild`/`parentNode`/etc. are undefined.

- [ ] **Step 3: Create `node.rs`** — create `crates/superui_api/src/node.rs`:

```rust
//! Structural `Node`/`Element` bindings: appendChild/removeChild/insertBefore/
//! replaceChild + parentNode/childNodes/children/... navigation accessors.

use boa_engine::{
    js_string, object::builtins::JsArray, object::FunctionObjectBuilder, property::PropertyDescriptor,
    Context, JsArgs, JsObject, JsResult, JsValue, NativeFunction,
};
use superui_dom::NodeKind;
use superui_js::{dom_of, jsstr, node_id_of, with_host_state, wrap_node, wrap_opt_node};

use crate::document::set_method;

/// Attach a getter-only accessor `name` to `proto`.
pub(crate) fn set_getter(
    proto: &JsObject,
    name: &str,
    getter: fn(&JsValue, &[JsValue], &mut Context) -> JsResult<JsValue>,
    context: &mut Context,
) {
    let g = FunctionObjectBuilder::new(context.realm(), NativeFunction::from_fn_ptr(getter))
        .name(js_string!(name))
        .length(0)
        .build();
    let desc = PropertyDescriptor::builder().get(g).enumerable(true).configurable(true).build();
    proto.define_property_or_throw(js_string!(name), desc, context).unwrap();
}

/// Attach a getter+setter accessor `name` to `proto`.
pub(crate) fn set_accessor(
    proto: &JsObject,
    name: &str,
    getter: fn(&JsValue, &[JsValue], &mut Context) -> JsResult<JsValue>,
    setter: fn(&JsValue, &[JsValue], &mut Context) -> JsResult<JsValue>,
    context: &mut Context,
) {
    let g = FunctionObjectBuilder::new(context.realm(), NativeFunction::from_fn_ptr(getter))
        .name(js_string!(name)).length(0).build();
    let s = FunctionObjectBuilder::new(context.realm(), NativeFunction::from_fn_ptr(setter))
        .name(js_string!(name)).length(1).build();
    let desc = PropertyDescriptor::builder().get(g).set(s).enumerable(true).configurable(true).build();
    proto.define_property_or_throw(js_string!(name), desc, context).unwrap();
}

// ---- structural methods ----

fn append_child(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let (Some(parent), Some(child)) = (node_id_of(this), node_id_of(args.get_or_undefined(0))) else {
        return Ok(JsValue::undefined());
    };
    let _ = dom_of(context).borrow_mut().append_child(parent, child);
    Ok(args.get_or_undefined(0).clone())
}

fn remove_child(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let (Some(parent), Some(child)) = (node_id_of(this), node_id_of(args.get_or_undefined(0))) else {
        return Ok(JsValue::undefined());
    };
    let _ = dom_of(context).borrow_mut().remove_child(parent, child);
    Ok(args.get_or_undefined(0).clone())
}

fn insert_before(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let parent = node_id_of(this);
    let new = node_id_of(args.get_or_undefined(0));
    let reference = node_id_of(args.get_or_undefined(1)); // may be null -> None -> append
    if let (Some(parent), Some(new)) = (parent, new) {
        let _ = dom_of(context).borrow_mut().insert_before(parent, new, reference);
    }
    Ok(args.get_or_undefined(0).clone())
}

fn replace_child(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let parent = node_id_of(this);
    let new = node_id_of(args.get_or_undefined(0));
    let old = node_id_of(args.get_or_undefined(1));
    if let (Some(parent), Some(new), Some(old)) = (parent, new, old) {
        let _ = dom_of(context).borrow_mut().replace_child(parent, new, old);
    }
    Ok(args.get_or_undefined(1).clone())
}

// ---- navigation accessors ----

fn parent_node(this: &JsValue, _a: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let Some(n) = node_id_of(this) else { return Ok(JsValue::null()) };
    let p = dom_of(context).borrow().parent(n);
    Ok(wrap_opt_node(context, p))
}

fn first_child(this: &JsValue, _a: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let Some(n) = node_id_of(this) else { return Ok(JsValue::null()) };
    let c = dom_of(context).borrow().children(n).first().copied();
    Ok(wrap_opt_node(context, c))
}

fn next_sibling(this: &JsValue, _a: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let Some(n) = node_id_of(this) else { return Ok(JsValue::null()) };
    let s = dom_of(context).borrow().next_sibling(n);
    Ok(wrap_opt_node(context, s))
}

fn previous_sibling(this: &JsValue, _a: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let Some(n) = node_id_of(this) else { return Ok(JsValue::null()) };
    let s = dom_of(context).borrow().previous_sibling(n);
    Ok(wrap_opt_node(context, s))
}

fn child_nodes(this: &JsValue, _a: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let Some(n) = node_id_of(this) else { return Ok(JsArray::from_iter(Vec::new(), context).into()) };
    let kids: Vec<_> = dom_of(context).borrow().children(n).to_vec();
    let items: Vec<JsValue> = kids.into_iter().map(|id| wrap_node(context, id).into()).collect();
    Ok(JsArray::from_iter(items, context).into())
}

fn children(this: &JsValue, _a: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let Some(n) = node_id_of(this) else { return Ok(JsArray::from_iter(Vec::new(), context).into()) };
    let dom = dom_of(context);
    let kids: Vec<_> = { let d = dom.borrow(); d.children(n).iter().copied().filter(|&c| d.is_element(c)).collect() };
    let items: Vec<JsValue> = kids.into_iter().map(|id| wrap_node(context, id).into()).collect();
    Ok(JsArray::from_iter(items, context).into())
}

fn node_type(this: &JsValue, _a: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let Some(n) = node_id_of(this) else { return Ok(JsValue::from(0)) };
    let dom = dom_of(context);
    let d = dom.borrow();
    let t = match d.get(n).map(|nd| &nd.kind) {
        Some(NodeKind::Element(_)) => 1,
        Some(NodeKind::Text(_)) => 3,
        Some(NodeKind::Document) => 9,
        None => 0,
    };
    Ok(JsValue::from(t))
}

fn tag_name(this: &JsValue, _a: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let Some(n) = node_id_of(this) else { return Ok(JsValue::undefined()) };
    let tag = dom_of(context).borrow().tag(n).map(|t| t.to_ascii_uppercase());
    Ok(match tag {
        Some(t) => jsstr(&t),
        None => JsValue::undefined(),
    })
}

/// Install structural methods + navigation accessors on the element proto (and
/// the shared subset — parentNode/childNodes/etc. — on document too).
pub fn install_node(context: &mut Context) {
    let element = with_host_state(context, |s| s.protos.element.clone()).expect("element proto");
    let document = with_host_state(context, |s| s.protos.document.clone()).expect("document proto");

    for proto in [element.clone(), document.clone()] {
        set_method(&proto, "appendChild", 1, append_child, context);
        set_method(&proto, "removeChild", 1, remove_child, context);
        set_method(&proto, "insertBefore", 2, insert_before, context);
        set_method(&proto, "replaceChild", 2, replace_child, context);
        set_getter(&proto, "parentNode", parent_node, context);
        set_getter(&proto, "firstChild", first_child, context);
        set_getter(&proto, "nextSibling", next_sibling, context);
        set_getter(&proto, "previousSibling", previous_sibling, context);
        set_getter(&proto, "childNodes", child_nodes, context);
        set_getter(&proto, "children", children, context);
        set_getter(&proto, "nodeType", node_type, context);
    }
    // tagName is element-only.
    set_getter(&element, "tagName", tag_name, context);
}
```

- [ ] **Step 4: Wire into `install`** — in `crates/superui_api/src/lib.rs`, add `mod node;` and, in `install()` after `document::install_document(context);`:

```rust
    node::install_node(context);
```

- [ ] **Step 5: Run the tests**

Run: `cargo test -p superui_api`
Expected: PASS — `structural_mutation_and_navigation` plus prior tests pass. (If `set_method` is not visible from `node.rs`, ensure it is `pub(crate)` in `document.rs` as written.)

- [ ] **Step 6: Commit**

```bash
git add crates/superui_api
git commit -m "feat(api): Node/Element structural bindings + navigation accessors"
```

---

### Task 8: `superui_api` — Element attributes, content, and property accessors (+ classList, style)

**Files:**
- Create: `crates/superui_api/src/element.rs`
- Modify: `crates/superui_api/src/lib.rs` (`mod element;`, call `element::install_element` in `install`)

**Interfaces:**
- Consumes: element/token_list/style protos, `superui_js` toolkit, `superui_dom` attr/class/props ops, `set_method`/`set_getter`/`set_accessor` helpers.
- Produces: `pub fn install_element(context: &mut Context)`:
  - Methods on element proto: `getAttribute`, `setAttribute`, `removeAttribute`, `hasAttribute`.
  - Accessors on element proto: `id`, `className`, `textContent`, `innerText`, `value`, `checked`, `classList` (returns a DOMTokenList object), `style` (returns a style object).
  - DOMTokenList proto methods: `add`, `remove`, `toggle`, `contains`.
  - Style proto methods: `setProperty(name, value)`, `getPropertyValue(name)` (backed by the `style` attribute; open-ended `style.camelCase = x` is 🟡 Roadmap, not Phase 1).

- [ ] **Step 1: Write the failing test** — append to `crates/superui_api/src/lib.rs` `tests`:

```rust
    #[test]
    fn element_attributes_content_and_props() {
        let dom = Rc::new(RefCell::new(Dom::new()));
        let mut e = BoaEngine::new(dom);
        install(&mut e);
        e.eval(
            r#"
            var el = document.createElement('input');
            el.setAttribute('type', 'checkbox');
            globalThis.attr = el.getAttribute('type');
            globalThis.hasIt = el.hasAttribute('type');
            el.removeAttribute('type');
            globalThis.gone = (el.getAttribute('type') === null);

            el.id = 'todo-1';
            globalThis.id = el.id;
            el.className = 'a b';
            globalThis.cls = el.className;
            el.classList.add('c');
            el.classList.toggle('a');       // removes a
            globalThis.hasC = el.classList.contains('c');
            globalThis.hasA = el.classList.contains('a');

            var p = document.createElement('p');
            p.textContent = 'hello';
            globalThis.text = p.textContent;

            el.value = 'typed';
            globalThis.value = el.value;
            el.checked = true;
            globalThis.checked = el.checked;

            el.style.setProperty('display', 'none');
            globalThis.disp = el.style.getPropertyValue('display');
            "#,
        )
        .unwrap();
        let check = |e: &mut BoaEngine, expr: &str| -> String {
            e.context_mut().eval(boa_engine::Source::from_bytes(expr)).unwrap()
                .to_string(e.context_mut()).unwrap().to_std_string_escaped()
        };
        assert_eq!(check(&mut e, "globalThis.attr"), "checkbox");
        assert_eq!(check(&mut e, "globalThis.hasIt"), "true");
        assert_eq!(check(&mut e, "globalThis.gone"), "true");
        assert_eq!(check(&mut e, "globalThis.id"), "todo-1");
        assert_eq!(check(&mut e, "globalThis.cls"), "a b");
        assert_eq!(check(&mut e, "globalThis.hasC"), "true");
        assert_eq!(check(&mut e, "globalThis.hasA"), "false");
        assert_eq!(check(&mut e, "globalThis.text"), "hello");
        assert_eq!(check(&mut e, "globalThis.value"), "typed");
        assert_eq!(check(&mut e, "globalThis.checked"), "true");
        assert_eq!(check(&mut e, "globalThis.disp"), "none");
    }
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p superui_api element_attributes_content_and_props`
Expected: FAIL — `setAttribute`/`classList`/`value`/etc. undefined.

- [ ] **Step 3: Create `element.rs`** — create `crates/superui_api/src/element.rs`:

```rust
//! Element attribute methods, content/value/checked accessors, classList, style.

use boa_engine::{
    js_string, Context, JsArgs, JsObject, JsResult, JsValue, NativeFunction,
};
use boa_engine::object::FunctionObjectBuilder;
use superui_js::{dom_of, jsstr, node_id_of, with_host_state, NodeHandle};

use crate::document::set_method;
use crate::node::{set_accessor, set_getter};

// ---- attribute methods ----

fn get_attribute(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let Some(n) = node_id_of(this) else { return Ok(JsValue::null()) };
    let name = args.get_or_undefined(0).to_string(context)?.to_std_string_escaped();
    let v = dom_of(context).borrow().get_attribute(n, &name).map(|s| s.to_string());
    Ok(v.map(|s| jsstr(&s)).unwrap_or(JsValue::null()))
}

fn set_attribute(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let name = args.get_or_undefined(0).to_string(context)?.to_std_string_escaped();
    let value = args.get_or_undefined(1).to_string(context)?.to_std_string_escaped();
    if let Some(n) = node_id_of(this) {
        let _ = dom_of(context).borrow_mut().set_attribute(n, &name, &value);
    }
    Ok(JsValue::undefined())
}

fn remove_attribute(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let name = args.get_or_undefined(0).to_string(context)?.to_std_string_escaped();
    if let Some(n) = node_id_of(this) {
        dom_of(context).borrow_mut().remove_attribute(n, &name);
    }
    Ok(JsValue::undefined())
}

fn has_attribute(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let Some(n) = node_id_of(this) else { return Ok(JsValue::from(false)) };
    let name = args.get_or_undefined(0).to_string(context)?.to_std_string_escaped();
    Ok(JsValue::from(dom_of(context).borrow().has_attribute(n, &name)))
}

// ---- string-attribute-backed accessors (id, className) ----

fn attr_getter(this: &JsValue, context: &mut Context, name: &str) -> JsValue {
    let Some(n) = node_id_of(this) else { return jsstr("") };
    let v = dom_of(context).borrow().get_attribute(n, name).unwrap_or("").to_string();
    jsstr(&v)
}
fn attr_setter(this: &JsValue, args: &[JsValue], context: &mut Context, name: &str) -> JsResult<JsValue> {
    let v = args.get_or_undefined(0).to_string(context)?.to_std_string_escaped();
    if let Some(n) = node_id_of(this) {
        let _ = dom_of(context).borrow_mut().set_attribute(n, name, &v);
    }
    Ok(JsValue::undefined())
}

fn id_get(this: &JsValue, _a: &[JsValue], c: &mut Context) -> JsResult<JsValue> { Ok(attr_getter(this, c, "id")) }
fn id_set(this: &JsValue, a: &[JsValue], c: &mut Context) -> JsResult<JsValue> { attr_setter(this, a, c, "id") }
fn class_name_get(this: &JsValue, _a: &[JsValue], c: &mut Context) -> JsResult<JsValue> { Ok(attr_getter(this, c, "class")) }
fn class_name_set(this: &JsValue, a: &[JsValue], c: &mut Context) -> JsResult<JsValue> { attr_setter(this, a, c, "class") }

// ---- textContent / innerText ----

fn text_content_get(this: &JsValue, _a: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let Some(n) = node_id_of(this) else { return Ok(jsstr("")) };
    let t = dom_of(context).borrow().text_content(n);
    Ok(jsstr(&t))
}
fn text_content_set(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let text = args.get_or_undefined(0).to_string(context)?.to_std_string_escaped();
    if let Some(n) = node_id_of(this) {
        dom_of(context).borrow_mut().set_text_content(n, &text);
    }
    Ok(JsValue::undefined())
}

// ---- value / checked ----

fn value_get(this: &JsValue, _a: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let Some(n) = node_id_of(this) else { return Ok(jsstr("")) };
    let v = dom_of(context).borrow().value(n);
    Ok(jsstr(&v))
}
fn value_set(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let v = args.get_or_undefined(0).to_string(context)?.to_std_string_escaped();
    if let Some(n) = node_id_of(this) { dom_of(context).borrow_mut().set_value(n, &v); }
    Ok(JsValue::undefined())
}
fn checked_get(this: &JsValue, _a: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let Some(n) = node_id_of(this) else { return Ok(JsValue::from(false)) };
    Ok(JsValue::from(dom_of(context).borrow().checked(n)))
}
fn checked_set(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let b = args.get_or_undefined(0).to_boolean();
    if let Some(n) = node_id_of(this) { dom_of(context).borrow_mut().set_checked(n, b); }
    Ok(JsValue::undefined())
}

// ---- classList ----

fn make_token_list(context: &mut Context, owner: superui_dom::NodeId) -> JsObject {
    let proto = with_host_state(context, |s| s.protos.token_list.clone()).expect("token_list proto");
    JsObject::from_proto_and_data(proto, NodeHandle { node: owner })
}
fn class_list_get(this: &JsValue, _a: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let Some(n) = node_id_of(this) else { return Ok(JsValue::null()) };
    Ok(make_token_list(context, n).into())
}
fn cl_add(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let c = args.get_or_undefined(0).to_string(context)?.to_std_string_escaped();
    if let Some(n) = node_id_of(this) { dom_of(context).borrow_mut().class_add(n, &c); }
    Ok(JsValue::undefined())
}
fn cl_remove(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let c = args.get_or_undefined(0).to_string(context)?.to_std_string_escaped();
    if let Some(n) = node_id_of(this) { dom_of(context).borrow_mut().class_remove(n, &c); }
    Ok(JsValue::undefined())
}
fn cl_toggle(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let c = args.get_or_undefined(0).to_string(context)?.to_std_string_escaped();
    let Some(n) = node_id_of(this) else { return Ok(JsValue::from(false)) };
    Ok(JsValue::from(dom_of(context).borrow_mut().class_toggle(n, &c)))
}
fn cl_contains(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let c = args.get_or_undefined(0).to_string(context)?.to_std_string_escaped();
    let Some(n) = node_id_of(this) else { return Ok(JsValue::from(false)) };
    Ok(JsValue::from(dom_of(context).borrow().class_contains(n, &c)))
}

// ---- style (setProperty / getPropertyValue, backed by the `style` attribute) ----

fn parse_style(s: &str) -> Vec<(String, String)> {
    s.split(';').filter_map(|decl| {
        let (k, v) = decl.split_once(':')?;
        let (k, v) = (k.trim(), v.trim());
        if k.is_empty() { None } else { Some((k.to_ascii_lowercase(), v.to_string())) }
    }).collect()
}
fn serialize_style(decls: &[(String, String)]) -> String {
    decls.iter().map(|(k, v)| format!("{k}: {v}")).collect::<Vec<_>>().join("; ")
}
fn make_style(context: &mut Context, owner: superui_dom::NodeId) -> JsObject {
    let proto = with_host_state(context, |s| s.protos.style.clone()).expect("style proto");
    JsObject::from_proto_and_data(proto, NodeHandle { node: owner })
}
fn style_get(this: &JsValue, _a: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let Some(n) = node_id_of(this) else { return Ok(JsValue::null()) };
    Ok(make_style(context, n).into())
}
fn style_set_property(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let name = args.get_or_undefined(0).to_string(context)?.to_std_string_escaped().to_ascii_lowercase();
    let value = args.get_or_undefined(1).to_string(context)?.to_std_string_escaped();
    if let Some(n) = node_id_of(this) {
        let dom = dom_of(context);
        let mut d = dom.borrow_mut();
        let mut decls = parse_style(d.get_attribute(n, "style").unwrap_or(""));
        match decls.iter_mut().find(|(k, _)| *k == name) {
            Some(slot) => slot.1 = value,
            None => decls.push((name, value)),
        }
        let _ = d.set_attribute(n, "style", &serialize_style(&decls));
    }
    Ok(JsValue::undefined())
}
fn style_get_property_value(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let name = args.get_or_undefined(0).to_string(context)?.to_std_string_escaped().to_ascii_lowercase();
    let Some(n) = node_id_of(this) else { return Ok(jsstr("")) };
    let dom = dom_of(context);
    let d = dom.borrow();
    let val = parse_style(d.get_attribute(n, "style").unwrap_or(""))
        .into_iter().find(|(k, _)| *k == name).map(|(_, v)| v).unwrap_or_default();
    Ok(jsstr(&val))
}

/// Install element attribute methods, content/value/checked accessors,
/// classList and style.
pub fn install_element(context: &mut Context) {
    let element = with_host_state(context, |s| s.protos.element.clone()).expect("element proto");
    let token_list = with_host_state(context, |s| s.protos.token_list.clone()).expect("token_list proto");
    let style = with_host_state(context, |s| s.protos.style.clone()).expect("style proto");

    set_method(&element, "getAttribute", 1, get_attribute, context);
    set_method(&element, "setAttribute", 2, set_attribute, context);
    set_method(&element, "removeAttribute", 1, remove_attribute, context);
    set_method(&element, "hasAttribute", 1, has_attribute, context);

    set_accessor(&element, "id", id_get, id_set, context);
    set_accessor(&element, "className", class_name_get, class_name_set, context);
    set_accessor(&element, "textContent", text_content_get, text_content_set, context);
    set_accessor(&element, "innerText", text_content_get, text_content_set, context);
    set_accessor(&element, "value", value_get, value_set, context);
    set_accessor(&element, "checked", checked_get, checked_set, context);
    set_getter(&element, "classList", class_list_get, context);
    set_getter(&element, "style", style_get, context);

    set_method(&token_list, "add", 1, cl_add, context);
    set_method(&token_list, "remove", 1, cl_remove, context);
    set_method(&token_list, "toggle", 1, cl_toggle, context);
    set_method(&token_list, "contains", 1, cl_contains, context);

    set_method(&style, "setProperty", 2, style_set_property, context);
    set_method(&style, "getPropertyValue", 1, style_get_property_value, context);
}
```

- [ ] **Step 4: Wire into `install`** — in `crates/superui_api/src/lib.rs`, add `mod element;` and, in `install()` after `node::install_node(context);`:

```rust
    element::install_element(context);
```

Also update `node.rs`'s helper visibility if needed: ensure `set_getter` and `set_accessor` are `pub(crate)` (they are, as written).

- [ ] **Step 5: Run the tests**

Run: `cargo test -p superui_api`
Expected: PASS — `element_attributes_content_and_props` plus all prior tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/superui_api
git commit -m "feat(api): Element attributes/content/value/checked + classList + style"
```

---

### Task 9: Events — `EventData`, event proto, addEventListener/removeEventListener, `BoaEngine::dispatch_event`

**Files:**
- Modify: `crates/superui_js/src/state.rs` (add `EventData` native-data type)
- Modify: `crates/superui_js/src/engine.rs` (add `dispatch_event`; borrow-safe manual dispatch loop + JS event object)
- Modify: `crates/superui_js/src/lib.rs` (extend `JsEngine` trait with `dispatch_event`; re-export `EventData`)
- Create: `crates/superui_api/src/events.rs` (event proto methods/accessors + addEventListener/removeEventListener)
- Modify: `crates/superui_api/src/lib.rs` (`mod events;`, call `events::install_events`)

**Interfaces:**
- Consumes: `superui_dom::{Event, EventPhase, ListenerId}` dispatch engine (`add_event_listener`, `remove_event_listener`, `build_dispatch_plan`, `listener_exists`), toolkit, event proto.
- Produces:
  - `superui_js`: `pub struct EventData { pub inner: Rc<RefCell<superui_dom::Event>> }`; `BoaEngine::dispatch_event(&mut self, target: NodeId, event_type: &str, bubbles: bool, cancelable: bool) -> bool` (returns `default_prevented`); `JsEngine::dispatch_event` trait method.
  - `superui_api`: `install_events(context)` — event proto (`preventDefault`, `stopPropagation`, `stopImmediatePropagation`; accessors `type`, `target`, `currentTarget`, `defaultPrevented`) + element-proto `addEventListener(type, cb, capture?)` / `removeEventListener` (stores the JS callback in `HostState.listeners` keyed by the `superui_dom` `ListenerId`).

**Design note (borrow safety):** `Dom::run_dispatch` holds `&Dom` across the invoke callback, which would conflict with a JS listener's `borrow_mut()`. So `dispatch_event` instead: (1) borrows the DOM only to `build_dispatch_plan` (owned `Vec<DispatchStep>`) and drops the borrow; (2) iterates the plan itself, taking only short-lived borrows for `listener_exists`, and calls each JS callback with **no DOM borrow held** — so listeners may freely mutate the DOM. This mirrors the `run_dispatch_skips_a_listener_removed_after_planning` test in `superui_dom`.

- [ ] **Step 1: Write the failing test** — append to `crates/superui_api/src/lib.rs` `tests`:

```rust
    #[test]
    fn events_dispatch_capture_target_bubble_and_prevent_default() {
        use superui_js::JsEngine;
        let dom = Rc::new(RefCell::new(Dom::new()));
        let (root, mid, leaf) = {
            let mut d = dom.borrow_mut();
            let doc = d.document();
            let root = d.create_element("div");
            let mid = d.create_element("div");
            let leaf = d.create_element("button");
            d.append_child(doc, root).unwrap();
            d.append_child(root, mid).unwrap();
            d.append_child(mid, leaf).unwrap();
            (root, mid, leaf)
        };
        let mut e = BoaEngine::new(dom);
        install(&mut e);
        // Expose the three nodes to JS by id for listener wiring.
        {
            let mut d = e.dom();
            let mut d = d.borrow_mut();
            d.set_attribute(root, "id", "root").unwrap();
            d.set_attribute(mid, "id", "mid").unwrap();
            d.set_attribute(leaf, "id", "leaf").unwrap();
        }
        e.eval(
            r#"
            globalThis.order = [];
            document.getElementById('root').addEventListener('click', e => { order.push('root-capture'); }, true);
            document.getElementById('mid').addEventListener('click', function(e){ order.push('mid-bubble'); });
            document.getElementById('leaf').addEventListener('click', function(e){ order.push('leaf'); e.preventDefault(); });
            "#,
        )
        .unwrap();

        let default_prevented = e.dispatch_event(leaf, "click", true, true);
        assert!(default_prevented);

        let order = e
            .context_mut()
            .eval(boa_engine::Source::from_bytes("globalThis.order.join(',')"))
            .unwrap()
            .to_string(e.context_mut())
            .unwrap()
            .to_std_string_escaped();
        assert_eq!(order, "root-capture,leaf,mid-bubble");
    }

    #[test]
    fn remove_event_listener_stops_delivery() {
        use superui_js::JsEngine;
        let dom = Rc::new(RefCell::new(Dom::new()));
        let btn = { let mut d = dom.borrow_mut(); let doc = d.document(); let b = d.create_element("button"); d.append_child(doc, b).unwrap(); d.set_attribute(b, "id", "b").unwrap(); b };
        let mut e = BoaEngine::new(dom);
        install(&mut e);
        e.eval(
            r#"
            globalThis.hits = 0;
            globalThis.h = function(){ globalThis.hits++; };
            document.getElementById('b').addEventListener('click', globalThis.h);
            document.getElementById('b').removeEventListener('click', globalThis.h);
            "#,
        ).unwrap();
        e.dispatch_event(btn, "click", true, true);
        let hits = e.context_mut().eval(boa_engine::Source::from_bytes("globalThis.hits")).unwrap().as_i32().unwrap_or(-1);
        assert_eq!(hits, 0);
    }
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p superui_api events_dispatch_capture_target_bubble_and_prevent_default`
Expected: FAIL — `addEventListener` and `dispatch_event` do not exist yet.

- [ ] **Step 3: Add `EventData` to `superui_js` state** — append to `crates/superui_js/src/state.rs` (and re-export in Step 6):

```rust
/// Native data for a JS `Event` object: the shared dispatch state that
/// `preventDefault`/`stopPropagation` mutate and the dispatch loop reads.
#[derive(Trace, Finalize, JsData)]
pub struct EventData {
    #[unsafe_ignore_trace]
    pub inner: Rc<RefCell<superui_dom::Event>>,
}
```

- [ ] **Step 4: Add `dispatch_event` to `BoaEngine`** — in `crates/superui_js/src/engine.rs`, extend imports and add the method + a JS-event-object builder.

Extend the top-of-file imports:

```rust
use boa_engine::{js_string, Context, JsObject, JsValue, Source};
use superui_dom::{Dom, Event, NodeId};
use crate::state::{with_host_state, wrap_node, EventData, HostState};
```

Add inside `impl BoaEngine` (after `dom`):

```rust
    /// Build the JS `Event` object for a dispatch, backed by shared `inner`.
    fn make_event_object(&mut self, inner: &std::rc::Rc<std::cell::RefCell<Event>>, target: NodeId) -> JsObject {
        let proto = with_host_state(&mut self.context, |s| s.protos.event.clone())
            .expect("event proto installed");
        let obj = JsObject::from_proto_and_data(proto, EventData { inner: inner.clone() });
        let type_ = inner.borrow().type_.clone();
        let target_obj = wrap_node(&mut self.context, target);
        obj.set(js_string!("type"), crate::state::jsstr(&type_), false, &mut self.context).ok();
        obj.set(js_string!("target"), target_obj, false, &mut self.context).ok();
        obj
    }
```

Add the trait-method implementation in `impl JsEngine for BoaEngine` (below `eval`):

```rust
    fn dispatch_event(
        &mut self,
        target: NodeId,
        event_type: &str,
        bubbles: bool,
        cancelable: bool,
    ) -> bool {
        // 1. Build the ordered plan from a *short* DOM borrow, then drop it.
        let plan = self.dom.borrow().build_dispatch_plan(target, event_type, bubbles);

        // 2. Shared event state + its JS mirror object.
        let inner = std::rc::Rc::new(std::cell::RefCell::new(Event::new(
            event_type, target, bubbles, cancelable,
        )));
        let event_obj = self.make_event_object(&inner, target);

        // 3. Walk the plan ourselves so no DOM borrow is held across a JS call.
        for step in plan {
            if inner.borrow().propagation_stopped() {
                break;
            }
            {
                let mut ev = inner.borrow_mut();
                ev.current_target = Some(step.node);
                ev.phase = step.phase;
            }
            let current = wrap_node(&mut self.context, step.node);
            event_obj
                .set(js_string!("currentTarget"), current.clone(), false, &mut self.context)
                .ok();
            for lid in step.listeners {
                if inner.borrow().immediate_stopped() {
                    break;
                }
                if !self.dom.borrow().listener_exists(step.node, lid) {
                    continue;
                }
                let cb = with_host_state(&mut self.context, |s| s.listeners.get(&lid.0).cloned());
                if let Some(cb) = cb {
                    let _ = cb.call(
                        &JsValue::from(current.clone()),
                        &[JsValue::from(event_obj.clone())],
                        &mut self.context,
                    );
                }
            }
        }
        let prevented = inner.borrow().default_prevented();
        prevented
    }
```

- [ ] **Step 5: Extend the `JsEngine` trait** — in `crates/superui_js/src/lib.rs`, add to the trait and re-export `EventData`:

```rust
pub trait JsEngine {
    /// Evaluate a script against the current context. `Err` carries a message.
    fn eval(&mut self, script: &str) -> Result<(), String>;

    /// Dispatch a DOM event of `event_type` at `target` (W3C capture→target→
    /// bubble). Returns whether `preventDefault()` was called.
    fn dispatch_event(
        &mut self,
        target: superui_dom::NodeId,
        event_type: &str,
        bubbles: bool,
        cancelable: bool,
    ) -> bool;
}
```

And extend the `pub use state::{...}` list to include `EventData`.

- [ ] **Step 6: Create `events.rs` in `superui_api`** — create `crates/superui_api/src/events.rs`:

```rust
//! `addEventListener`/`removeEventListener` + the JS `Event` object surface.

use boa_engine::{
    js_string, object::builtins::JsFunction, Context, JsArgs, JsResult, JsValue, NativeFunction,
};
use superui_js::{dom_of, jsstr, node_id_of, with_host_state, with_host_state_mut, wrap_opt_node, EventData};

use crate::document::set_method;
use crate::node::set_getter;

// ---- Event object methods/accessors (operate on EventData via `this`) ----

fn with_event<R>(this: &JsValue, f: impl FnOnce(&mut superui_dom::Event) -> R) -> Option<R> {
    let obj = this.as_object()?;
    let data = obj.downcast_ref::<EventData>()?;
    let mut ev = data.inner.borrow_mut();
    Some(f(&mut ev))
}

fn prevent_default(this: &JsValue, _a: &[JsValue], _c: &mut Context) -> JsResult<JsValue> {
    with_event(this, |e| e.prevent_default());
    Ok(JsValue::undefined())
}
fn stop_propagation(this: &JsValue, _a: &[JsValue], _c: &mut Context) -> JsResult<JsValue> {
    with_event(this, |e| e.stop_propagation());
    Ok(JsValue::undefined())
}
fn stop_immediate(this: &JsValue, _a: &[JsValue], _c: &mut Context) -> JsResult<JsValue> {
    with_event(this, |e| e.stop_immediate_propagation());
    Ok(JsValue::undefined())
}
fn ev_type(this: &JsValue, _a: &[JsValue], _c: &mut Context) -> JsResult<JsValue> {
    let obj = this.as_object();
    let t = obj.as_ref().and_then(|o| o.downcast_ref::<EventData>()).map(|d| d.inner.borrow().type_.clone());
    Ok(t.map(|s| jsstr(&s)).unwrap_or(JsValue::undefined()))
}
fn ev_default_prevented(this: &JsValue, _a: &[JsValue], _c: &mut Context) -> JsResult<JsValue> {
    let v = with_event(this, |e| e.default_prevented()).unwrap_or(false);
    Ok(JsValue::from(v))
}
fn ev_target(this: &JsValue, _a: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let obj = this.as_object();
    let n = obj.as_ref().and_then(|o| o.downcast_ref::<EventData>()).map(|d| d.inner.borrow().target);
    Ok(wrap_opt_node(context, n))
}
fn ev_current_target(this: &JsValue, _a: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let obj = this.as_object();
    let n = obj.as_ref().and_then(|o| o.downcast_ref::<EventData>()).and_then(|d| d.inner.borrow().current_target);
    Ok(wrap_opt_node(context, n))
}

// ---- addEventListener / removeEventListener (element proto) ----

fn add_event_listener(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let Some(n) = node_id_of(this) else { return Ok(JsValue::undefined()) };
    let ty = args.get_or_undefined(0).to_string(context)?.to_std_string_escaped();
    let Some(cb_obj) = args.get_or_undefined(1).as_object() else { return Ok(JsValue::undefined()) };
    let Some(cb) = JsFunction::from_object(cb_obj.clone()) else { return Ok(JsValue::undefined()) };
    let capture = args.get_or_undefined(2).to_boolean();

    let listener_id = dom_of(context).borrow_mut().add_event_listener(n, &ty, capture);
    if let Some(lid) = listener_id {
        with_host_state_mut(context, |s| { s.listeners.insert(lid.0, cb); });
    }
    Ok(JsValue::undefined())
}

fn remove_event_listener(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let Some(n) = node_id_of(this) else { return Ok(JsValue::undefined()) };
    let ty = args.get_or_undefined(0).to_string(context)?.to_std_string_escaped();
    let Some(cb_obj) = args.get_or_undefined(1).as_object() else { return Ok(JsValue::undefined()) };
    let capture = args.get_or_undefined(2).to_boolean();

    // Find the listener id whose stored callback === the given function AND whose
    // (type, capture) match, then remove from the DOM and the registry.
    let target_fn = JsFunction::from_object(cb_obj.clone());
    let Some(target_fn) = target_fn else { return Ok(JsValue::undefined()) };

    let candidates: Vec<u64> = {
        let dom = dom_of(context);
        let d = dom.borrow();
        d.listeners(n).iter()
            .filter(|l| l.event_type == ty && l.capture == capture)
            .map(|l| l.id.0)
            .collect()
    };
    let mut to_remove = None;
    for lid in candidates {
        let same = with_host_state(context, |s| {
            s.listeners.get(&lid).map(|f| JsFunction::equals(f, &target_fn)).unwrap_or(false)
        });
        if same { to_remove = Some(lid); break; }
    }
    if let Some(lid) = to_remove {
        dom_of(context).borrow_mut().remove_event_listener(n, superui_dom::ListenerId(lid));
        with_host_state_mut(context, |s| { s.listeners.remove(&lid); });
    }
    Ok(JsValue::undefined())
}

/// Install the event proto surface + element-proto listener methods.
pub fn install_events(context: &mut Context) {
    let event = with_host_state(context, |s| s.protos.event.clone()).expect("event proto");
    set_method(&event, "preventDefault", 0, prevent_default, context);
    set_method(&event, "stopPropagation", 0, stop_propagation, context);
    set_method(&event, "stopImmediatePropagation", 0, stop_immediate, context);
    set_getter(&event, "type", ev_type, context);
    set_getter(&event, "target", ev_target, context);
    set_getter(&event, "currentTarget", ev_current_target, context);
    set_getter(&event, "defaultPrevented", ev_default_prevented, context);

    let element = with_host_state(context, |s| s.protos.element.clone()).expect("element proto");
    set_method(&element, "addEventListener", 3, add_event_listener, context);
    set_method(&element, "removeEventListener", 3, remove_event_listener, context);
}
```

Note: `JsFunction::equals(&a, &b)` compares object identity; if the exact helper differs, compare via the underlying `JsObject` (`a.as_ref() == b.as_ref()`) per the compiler — the intent is "same function object".

- [ ] **Step 7: Wire into `install`** — in `crates/superui_api/src/lib.rs`, add `mod events;` and, in `install()` after `element::install_element(context);`:

```rust
    events::install_events(context);
```

- [ ] **Step 8: Run the tests**

Run: `cargo test -p superui_api && cargo test -p superui_js`
Expected: PASS — the two new event tests pass, and `superui_js` still builds/tests green. (If a borrow panic occurs during dispatch, verify no `self.dom.borrow()` is held across `cb.call` — the loop above drops each borrow before the call.)

- [ ] **Step 9: Commit**

```bash
git add crates/superui_js crates/superui_api
git commit -m "feat(events): addEventListener/removeEventListener + borrow-safe dispatch_event"
```

---

### Task 10: Timers, integration test, wasm check, ledger/README update

**Files:**
- Create: `crates/superui_api/src/timers.rs`
- Modify: `crates/superui_js/src/engine.rs` (add `run_timers`), `crates/superui_js/src/lib.rs` (trait method)
- Modify: `crates/superui_api/src/lib.rs` (`mod timers;`, call `timers::install_timers`; integration test)
- Modify: `docs/superpowers/plans/README.md` (mark Plan 3 done; note window.bevy moved to Plan 5)

**Interfaces:**
- Produces:
  - `superui_api`: `install_timers(context)` — `setTimeout(cb, ms)`, `setInterval(cb, ms)`, `clearTimeout(id)`, `clearInterval(id)` (store into `HostState.timers`; return a numeric id).
  - `superui_js`: `BoaEngine::run_timers(&mut self, now_ms: f64)` + `JsEngine::run_timers` — advance the clock, fire due timers (reschedule intervals), pump the microtask queue.

- [ ] **Step 1: Write the failing test** — append to `crates/superui_api/src/lib.rs` `tests`:

```rust
    #[test]
    fn timers_fire_when_due() {
        use superui_js::JsEngine;
        let dom = Rc::new(RefCell::new(Dom::new()));
        let mut e = BoaEngine::new(dom);
        install(&mut e);
        e.eval(
            r#"
            globalThis.fired = 0;
            setTimeout(function(){ globalThis.fired += 1; }, 100);
            "#,
        ).unwrap();
        e.run_timers(50.0);   // not due yet
        let before = e.context_mut().eval(boa_engine::Source::from_bytes("globalThis.fired")).unwrap().as_i32().unwrap();
        assert_eq!(before, 0);
        e.run_timers(150.0);  // now due
        let after = e.context_mut().eval(boa_engine::Source::from_bytes("globalThis.fired")).unwrap().as_i32().unwrap();
        assert_eq!(after, 1);
        e.run_timers(300.0);  // one-shot does not refire
        let again = e.context_mut().eval(boa_engine::Source::from_bytes("globalThis.fired")).unwrap().as_i32().unwrap();
        assert_eq!(again, 1);
    }
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p superui_api timers_fire_when_due`
Expected: FAIL — `setTimeout` and `run_timers` do not exist.

- [ ] **Step 3: Create `timers.rs`** — create `crates/superui_api/src/timers.rs`:

```rust
//! `setTimeout`/`setInterval`/`clearTimeout`/`clearInterval`.

use boa_engine::{
    js_string, object::builtins::JsFunction, Context, JsArgs, JsResult, JsValue, NativeFunction,
};
use superui_js::{with_host_state, with_host_state_mut, Timer};

fn schedule(args: &[JsValue], context: &mut Context, repeating: bool) -> JsResult<JsValue> {
    let Some(cb_obj) = args.get_or_undefined(0).as_object() else { return Ok(JsValue::from(0)) };
    let Some(cb) = JsFunction::from_object(cb_obj.clone()) else { return Ok(JsValue::from(0)) };
    let delay = args.get_or_undefined(1).to_number(context)?.max(0.0);
    let now = with_host_state(context, |s| s.now_ms);
    let id = with_host_state_mut(context, |s| {
        let id = s.next_timer_id;
        s.next_timer_id += 1;
        s.timers.push(Timer {
            id,
            callback: cb,
            due_ms: now + delay,
            interval_ms: if repeating { Some(delay) } else { None },
        });
        id
    });
    Ok(JsValue::from(id as u32))
}

fn set_timeout(_t: &JsValue, a: &[JsValue], c: &mut Context) -> JsResult<JsValue> { schedule(a, c, false) }
fn set_interval(_t: &JsValue, a: &[JsValue], c: &mut Context) -> JsResult<JsValue> { schedule(a, c, true) }

fn clear(args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let id = args.get_or_undefined(0).to_number(context).unwrap_or(0.0) as u64;
    with_host_state_mut(context, |s| s.timers.retain(|t| t.id != id));
    Ok(JsValue::undefined())
}
fn clear_timeout(_t: &JsValue, a: &[JsValue], c: &mut Context) -> JsResult<JsValue> { clear(a, c) }
fn clear_interval(_t: &JsValue, a: &[JsValue], c: &mut Context) -> JsResult<JsValue> { clear(a, c) }

/// Install the four timer globals.
pub fn install_timers(context: &mut Context) {
    context.register_global_callable(js_string!("setTimeout"), 2, NativeFunction::from_fn_ptr(set_timeout)).unwrap();
    context.register_global_callable(js_string!("setInterval"), 2, NativeFunction::from_fn_ptr(set_interval)).unwrap();
    context.register_global_callable(js_string!("clearTimeout"), 1, NativeFunction::from_fn_ptr(clear_timeout)).unwrap();
    context.register_global_callable(js_string!("clearInterval"), 1, NativeFunction::from_fn_ptr(clear_interval)).unwrap();
}
```

- [ ] **Step 4: Add `run_timers` to `BoaEngine`** — in `crates/superui_js/src/engine.rs`, add inside `impl JsEngine for BoaEngine`:

```rust
    fn run_timers(&mut self, now_ms: f64) {
        with_host_state(&mut self.context, |_s| ()); // ensure state exists
        crate::state::with_host_state_mut(&mut self.context, |s| s.now_ms = now_ms);
        loop {
            // Pop the earliest due timer (short mutable borrow), fire outside it.
            let due = crate::state::with_host_state_mut(&mut self.context, |s| {
                let idx = s.timers.iter().enumerate()
                    .filter(|(_, t)| t.due_ms <= now_ms)
                    .min_by(|(_, a), (_, b)| a.due_ms.total_cmp(&b.due_ms))
                    .map(|(i, _)| i);
                idx.map(|i| {
                    let t = &s.timers[i];
                    let cb = t.callback.clone();
                    match t.interval_ms {
                        Some(period) => { let due = t.due_ms + period.max(1.0); s.timers[i].due_ms = due; }
                        None => { s.timers.remove(i); }
                    }
                    cb
                })
            });
            match due {
                Some(cb) => { let _ = cb.call(&JsValue::undefined(), &[], &mut self.context); }
                None => break,
            }
        }
        let _ = self.context.run_jobs(); // drain any microtasks the callbacks queued
    }
```

Add `Source` is already imported; ensure `use crate::state::with_host_state;` is present (it is, from Task 9). If `run_jobs()` returns `JsResult<()>`, the `let _ =` is correct.

- [ ] **Step 5: Extend the `JsEngine` trait** — in `crates/superui_js/src/lib.rs`, add to the trait:

```rust
    /// Advance the timer clock to `now_ms` and fire all due timers (intervals
    /// reschedule). Pumps the microtask queue afterward.
    fn run_timers(&mut self, now_ms: f64);
```

- [ ] **Step 6: Wire timers into `install`** — in `crates/superui_api/src/lib.rs`, add `mod timers;` and, in `install()` after `events::install_events(context);`:

```rust
    timers::install_timers(context);
```

- [ ] **Step 7: Write the integration test** — append to `crates/superui_api/src/lib.rs` `tests`. It parses TodoMVC-shaped HTML via `superui_html`, installs the API, runs an app-style script that adds/toggles/removes todos and dispatches synthetic events, then asserts DOM state.

Add `superui_html` as a **dev-dependency** in `crates/superui_api/Cargo.toml`:

```toml
[dev-dependencies]
superui_html = { path = "../superui_html" }
```

Append the test:

```rust
    #[test]
    fn todomvc_shaped_integration() {
        use superui_js::JsEngine;
        // Bootstrap the DOM from HTML (as the real loader will).
        let dom = Rc::new(RefCell::new(superui_html::parse_document(
            r#"<section class="todoapp">
                 <input class="new-todo" id="new-todo">
                 <ul class="todo-list" id="list"></ul>
                 <span class="todo-count" id="count"></span>
               </section>"#,
        )));
        let mut e = BoaEngine::new(dom.clone());
        install(&mut e);

        // A tiny "app.js": addTodo(text) appends <li><input.toggle><label>; the
        // toggle click toggles completed; renderCount updates the counter.
        e.eval(r#"
            var list = document.getElementById('list');
            var count = document.getElementById('count');
            function renderCount() {
                var items = list.querySelectorAll ? [] : []; // (element.querySelectorAll not required)
                var remaining = 0;
                var lis = list.childNodes;
                for (var i = 0; i < lis.length; i++) {
                    var li = lis[i];
                    if (!li.classList.contains('completed')) remaining++;
                }
                count.textContent = remaining + ' items left';
            }
            globalThis.addTodo = function(text) {
                var li = document.createElement('li');
                var toggle = document.createElement('input');
                toggle.setAttribute('type', 'checkbox');
                toggle.className = 'toggle';
                var label = document.createElement('label');
                label.textContent = text;
                toggle.addEventListener('click', function() {
                    if (toggle.checked) li.classList.add('completed');
                    else li.classList.remove('completed');
                    renderCount();
                });
                li.appendChild(toggle);
                li.appendChild(label);
                list.appendChild(li);
                renderCount();
            };
            addTodo('Taste JS');
            addTodo('Buy a unicorn');
        "#).unwrap();

        // Two todos, counter shows 2 left.
        let count_text = |e: &mut BoaEngine| e.context_mut()
            .eval(boa_engine::Source::from_bytes("document.getElementById('count').textContent"))
            .unwrap().to_string(e.context_mut()).unwrap().to_std_string_escaped();
        assert_eq!(count_text(&mut e), "2 items left");

        // Toggle the first todo's checkbox: set checked, dispatch click.
        let first_toggle = { let d = dom.borrow(); d.query_selector(d.document(), ".todo-list .toggle").unwrap() };
        dom.borrow_mut().set_checked(first_toggle, true);
        e.dispatch_event(first_toggle, "click", true, true);
        assert_eq!(count_text(&mut e), "1 items left");

        // The first <li> is now completed.
        let completed = { let d = dom.borrow(); d.query_selector_all(d.document(), "li.completed").len() };
        assert_eq!(completed, 1);
    }
```

- [ ] **Step 8: Run the full suite**

Run: `cargo test -p superui_api && cargo test -p superui_js && cargo test -p superui_dom`
Expected: PASS — every test including `timers_fire_when_due` and `todomvc_shaped_integration`.

- [ ] **Step 9: Verify the wasm build for both crates**

Run: `cargo build -p superui_js -p superui_api --target wasm32-unknown-unknown`
Expected: SUCCESS — confirms Boa + the API layer stay wasm-clean with the getrandom config.

- [ ] **Step 10: Mark Plan 3 done and record the scope change** — in `docs/superpowers/plans/README.md`:

Replace the Plan 3 row:

```markdown
| 3 | `superui_js` + `superui_api` | Boa engine behind a `JsEngine` trait; broad DOM/Web API bindings (document, Node/Element, events, classList, style, console, timers, `fetch` warn-stub) + the `window.bevy` bridge. | ⬜ Not started |
```

with:

```markdown
| 3 | `superui_js` + `superui_api` | Boa engine behind a `JsEngine` trait; broad DOM/Web API bindings (document, Node/Element, events, classList, style, console, timers, `fetch` warn-stub). **`window.bevy` moved to Plan 5.** | ✅ Done — merged to `main` ([plan](./2026-07-18-superui-phase1-03-js-api.md)) |
```

And update the Plan 5 row to note it now also owns the JS-facing `window.bevy` surface — replace:

```markdown
| 5 | `superui_bridge` + `superui` | Reconciler (DOM diff → Bevy ECS commands; picking/input → DOM events), `SuperUiPlugin`, asset loaders, hot reload via `AssetEvent::Modified`, observer-based `window.bevy`. | ⬜ Not started |
```

with:

```markdown
| 5 | `superui_bridge` + `superui` | Reconciler (DOM diff → Bevy ECS commands; picking/input → DOM events), `SuperUiPlugin`, asset loaders, hot reload via `AssetEvent::Modified`, **the full `window.bevy` bridge (JS-facing `bevy.send`/`bevy.on` global + observer wiring, deferred from Plan 3)**. | ⬜ Not started |
```

Also update the "Resuming in a fresh session" block to target Plan 4.

- [ ] **Step 11: Commit**

```bash
git add crates docs/superpowers/plans/README.md
git commit -m "feat(timers): setTimeout/Interval + run_timers; TodoMVC integration; wasm-clean; mark Plan 3 done"
```

---

## Self-Review

**Spec coverage (design §4, §9 JS/DOM API list, §11 testing):**
- "JsEngine trait + Boa backend; DOM↔JS handle marshalling" → Tasks 3–4 (`JsEngine`, `BoaEngine`, `wrap_node`/`node_id_of`/`dom_of`, identity cache). ✅
- "document: getElementById, querySelector/All, createElement, createTextNode" → Task 6. ✅
- "Node/Element: appendChild, removeChild, insertBefore, replaceChild, parentNode, childNodes, children, textContent, innerText, setAttribute/getAttribute, classList, .value, .checked, .style.*" → Tasks 7 (structural + navigation) & 8 (attrs/content/value/checked/classList/style). ✅
- "Events: addEventListener/removeEventListener, capture/bubble dispatch, event object with target/currentTarget/preventDefault/stopPropagation; types click/input/change/keydown/keyup/submit" → Task 9 (dispatch is type-agnostic; any `event_type` string works, so all listed types are covered). ✅
- "console.*, setTimeout/setInterval/clear*, fetch warn-and-reject stub" → console+fetch Task 5, timers Task 10. ✅
- "window.bevy: minimal send/on" → **deliberately deferred to Plan 5** per the scope decision; README updated (Task 10). ✅ (documented, not an omission)
- "Unsupported features degrade gracefully" → every native binding returns undefined/null on bad `this`/args instead of throwing; `fetch` is the one intentional rejecter; malformed selectors return empty. ✅
- Headless tests running JS snippets and asserting DOM mutations (§11) → every API task. wasm build check (§11) → Tasks 3 & 10. ✅
- Bevy-agnostic + wasm-clean (§4/§5) → no Bevy deps in either crate; getrandom config verified. ✅

**Placeholder scan:** No TBD/TODO. Every code step contains complete code; every test step contains real assertions. The two "if the exact helper name differs, follow the compiler" notes (`JsObject::equals`, `JsFunction::equals`, `run_jobs` return) are precise re-export/identity contingencies of the Plan-2 kind, not vague hand-waving — the semantics are fixed. The one HashMap-`Trace` fallback is stated once in Global Constraints with an exact replacement. ✅

**Type consistency:** `HostState`/`NodeHandle`/`Protos`/`Timer`/`EventData` (superui_js) are used consistently by superui_api; the toolkit signatures (`wrap_node(&mut Context, NodeId) -> JsObject`, `node_id_of(&JsValue) -> Option<NodeId>`, `dom_of(&mut Context) -> Rc<RefCell<Dom>>`, `wrap_opt_node`, `jsstr`, `with_host_state[_mut]`) match every call site. The shared helpers `set_method` (document.rs), `set_getter`/`set_accessor` (node.rs) are `pub(crate)` and imported where used. `JsEngine` gains `dispatch_event` (Task 9) and `run_timers` (Task 10) exactly as the tests call them. All `superui_dom` methods invoked (`create_element`, `create_text`, `append_child`, `insert_before`, `remove_child`, `replace_child`, `parent`, `children`, `next_sibling`, `previous_sibling`, `is_element`, `tag`, `get_attribute`, `set_attribute`, `remove_attribute`, `has_attribute`, `class_add`/`remove`/`toggle`/`contains`, `text_content`, `set_text_content`, `value`/`set_value`/`checked`/`set_checked`, `get_element_by_id`, `query_selector`/`_all`, `add_event_listener`/`remove_event_listener`/`listeners`/`build_dispatch_plan`/`listener_exists`, `NodeId::to_ffi`/`from_ffi`) exist in Plan 1 or are added in Tasks 1–2. ✅

**Boa API fidelity:** every Boa call (`Context::default`, `eval`, `register_global_property`/`register_global_callable`, `FunctionObjectBuilder`, `NativeFunction::from_fn_ptr`, `JsObject::with_object_proto`/`from_proto_and_data`/`set`/`define_property_or_throw`/`downcast_ref`, `PropertyDescriptor::builder().get/set/build`, `JsArray::from_iter`, `JsPromise::reject`, `JsFunction::from_object`/`call`, `host_defined()/host_defined_mut()`, `run_jobs`) was verified compiling against boa_engine 0.21.1 in three spikes. ✅
