# superui_dom Tier-1 Benches Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the Tier-1 Criterion micro-bench suite for `superui_dom` — tree construction, mutation, querying, and event-dispatch path building — establishing the regression-tracking baseline for the arena DOM.

**Architecture:** A single Criterion bench target (`benches/dom_ops.rs`) with four benchmark groups. Trees are built programmatically from the crate's own mutation API (no external fixtures — this is why `superui_dom` is the standalone foundational plan). Each group is parameterized over tree size / chain depth so we watch scaling curves, not single points. A small `#[test]` guards the dispatch helper's own correctness (capture→bubble order) so the harness can't silently measure a broken setup.

**Tech Stack:** Rust, Criterion 0.5 (`harness = false`), `superui_dom` (pure Rust arena DOM, no Bevy / no Boa).

## Global Constraints

- **Bevy 0.17** target overall; **this crate and its benches pull in no Bevy and no Boa** — pure Rust, headless.
- Benches are **native-only dev tooling**, never on the wasm path.
- **Criterion**, declared `[[bench]]` with `harness = false`; criterion in `[dev-dependencies]`.
- **Determinism:** bench bodies use no wall-clock and no unseeded randomness. `superui_dom` tree ops are inherently deterministic; tree builders here use fixed fan-out/label patterns, never random.
- **Reported statistics:** steady-state groups report mean ± stddev; the event-dispatch group is the closest to a latency scenario but is still deterministic traversal, so mean is fine. Cold vs warm separation is N/A for this crate (no parse step).

## Execution Prerequisite

`superui_dom` must expose the Phase-1 API listed in **Interfaces → Consumes** below. If the real
crate names differ, update the bench call sites only — the scenarios are fixed. If the crate is
not yet implemented, this plan is "ready to run when it lands"; you may temporarily stub the API
to validate the harness shape, but the recorded baseline is meaningless until the real impl exists.

---

### Task 1: Bench target scaffolding + tree-builder helper

**Files:**
- Modify: `crates/superui_dom/Cargo.toml`
- Create: `crates/superui_dom/benches/dom_ops.rs`

**Interfaces:**
- Consumes (assumed `superui_dom` Phase-1 API — the contract this plan is written against):
  - `pub struct Dom;`
  - `pub struct NodeId(/* generational id */);` — `Copy`
  - `pub struct ListenerId(/* generational id */);` — `Copy`
  - `impl Dom { pub fn new() -> Dom }`
  - `pub fn create_element(&mut self, tag: &str) -> NodeId`
  - `pub fn create_text(&mut self, text: &str) -> NodeId`
  - `pub fn set_attribute(&mut self, node: NodeId, name: &str, value: &str)`
  - `pub fn append_child(&mut self, parent: NodeId, child: NodeId)`
  - `pub fn insert_before(&mut self, parent: NodeId, child: NodeId, reference: Option<NodeId>)`
  - `pub fn remove_child(&mut self, parent: NodeId, child: NodeId)`
  - `pub fn root(&self) -> NodeId` — the document root, present after `Dom::new()`
  - `pub fn query_selector(&self, selector: &str) -> Option<NodeId>`
  - `pub fn query_selector_all(&self, selector: &str) -> Vec<NodeId>`
  - `pub fn add_event_listener(&mut self, node: NodeId, event_type: &str, capture: bool) -> ListenerId`
  - `pub fn dispatch_event(&self, target: NodeId, event_type: &str) -> Vec<ListenerId>` — returns listeners in fire order (capture root→target, then bubble target→root); JS-agnostic, invokes nothing.
- Produces (for Tasks 2–5 in this file): the module-private helper `fn build_wide_tree(dom: &mut Dom, n: usize) -> Vec<NodeId>` and `fn build_deep_chain(dom: &mut Dom, depth: usize) -> Vec<NodeId>`.

- [ ] **Step 1: Add the bench declaration and criterion dev-dependency**

In `crates/superui_dom/Cargo.toml`, add (create the sections if absent):

```toml
[dev-dependencies]
criterion = { version = "0.5", features = ["html_reports"] }

[[bench]]
name = "dom_ops"
harness = false
```

- [ ] **Step 2: Create the bench file with the shared tree-builder helpers and an empty Criterion runner**

Create `crates/superui_dom/benches/dom_ops.rs`:

```rust
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::hint::black_box;
use superui_dom::{Dom, NodeId};

/// Node counts used across every scaling group.
const SIZES: [usize; 3] = [100, 1_000, 5_000];
/// Capture/bubble chain depths for the event-dispatch group.
const DEPTHS: [usize; 3] = [10, 50, 200];

/// Build a flat-ish tree of exactly `n` element nodes under the root.
///
/// Deterministic: fan-out is fixed at 8, ids/classes derive from the index.
/// Returns every created node id (in creation order) so callers can pick
/// targets without re-querying.
fn build_wide_tree(dom: &mut Dom, n: usize) -> Vec<NodeId> {
    let root = dom.root();
    let mut nodes = Vec::with_capacity(n);
    let mut parents = vec![root];
    let mut next_parent = 0usize;
    for i in 0..n {
        let parent = parents[next_parent.min(parents.len() - 1)];
        let el = dom.create_element("div");
        dom.set_attribute(el, "id", &format!("n{i}"));
        dom.set_attribute(el, "class", if i % 2 == 0 { "even item" } else { "odd item" });
        dom.append_child(parent, el);
        nodes.push(el);
        // Every 8th node becomes a parent for the next tier -> fan-out 8.
        if i % 8 == 0 {
            parents.push(el);
        }
        if i % 8 == 7 {
            next_parent += 1;
        }
    }
    nodes
}

/// Build a single linear chain of `depth` elements root->...->leaf.
/// Returns the chain top-to-bottom; last element is the leaf.
fn build_deep_chain(dom: &mut Dom, depth: usize) -> Vec<NodeId> {
    let mut parent = dom.root();
    let mut chain = Vec::with_capacity(depth);
    for i in 0..depth {
        let el = dom.create_element("div");
        dom.set_attribute(el, "class", "link");
        dom.append_child(parent, el);
        chain.push(el);
        parent = el;
    }
    chain
}

fn benches(_c: &mut Criterion) {
    // Groups are added in Tasks 2-5.
}

criterion_group!(dom_ops, benches);
criterion_main!(dom_ops);
```

- [ ] **Step 3: Verify the target compiles and runs (empty)**

Run: `cargo bench --bench dom_ops -- --quick`
Expected: compiles; Criterion runs with no benchmarks and exits 0 (no timings yet). If it fails
to compile, the assumed `superui_dom` API (Task 1 Consumes block) is not yet satisfied — see the
Execution Prerequisite.

- [ ] **Step 4: Commit**

```bash
git add crates/superui_dom/Cargo.toml crates/superui_dom/benches/dom_ops.rs
git commit -m "bench(dom): scaffold dom_ops criterion target + tree builders"
```

---

### Task 2: Tree-construction scaling bench

**Files:**
- Modify: `crates/superui_dom/benches/dom_ops.rs`

**Interfaces:**
- Consumes: `build_wide_tree` (Task 1), `Dom::new`, `Dom::root`, `create_element`, `set_attribute`, `append_child`.
- Produces: `fn bench_construction(c: &mut Criterion)`.

- [ ] **Step 1: Add the construction group**

Add above `fn benches` in `dom_ops.rs`:

```rust
/// How much does it cost to build a tree of N nodes from scratch?
/// Guards against O(n^2) regressions in append/id-map maintenance.
fn bench_construction(c: &mut Criterion) {
    let mut group = c.benchmark_group("dom/construction");
    for &n in &SIZES {
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            b.iter(|| {
                let mut dom = Dom::new();
                let nodes = build_wide_tree(&mut dom, black_box(n));
                black_box(nodes.len());
            });
        });
    }
    group.finish();
}
```

- [ ] **Step 2: Register the group in `benches`**

Replace the body of `fn benches`:

```rust
fn benches(c: &mut Criterion) {
    bench_construction(c);
}
```

- [ ] **Step 3: Run and confirm timings are emitted for all three sizes**

Run: `cargo bench --bench dom_ops -- dom/construction --quick`
Expected: PASS; three reported measurements (`dom/construction/100`, `/1000`, `/5000`), each a
time value. A near-linear ratio across sizes is the healthy signal (roughly ~10x from 100→1000).

- [ ] **Step 4: Commit**

```bash
git add crates/superui_dom/benches/dom_ops.rs
git commit -m "bench(dom): tree construction scaling (100/1k/5k)"
```

---

### Task 3: Mutation bench (append / insert / remove)

**Files:**
- Modify: `crates/superui_dom/benches/dom_ops.rs`

**Interfaces:**
- Consumes: `build_wide_tree`, `Dom::{append_child, insert_before, remove_child, create_element, root}`.
- Produces: `fn bench_mutation(c: &mut Criterion)`.

- [ ] **Step 1: Add the mutation group**

Add above `fn benches`:

```rust
/// Single-mutation cost on an already-large tree — this is the hot path the
/// reconciler's "single mutation" scenario ultimately drives. Uses
/// `iter_batched` so the tree build is NOT counted in the measured time.
fn bench_mutation(c: &mut Criterion) {
    use criterion::BatchSize;
    let mut group = c.benchmark_group("dom/mutation");

    // append one child to the root of an N-node tree
    for &n in &SIZES {
        group.bench_with_input(BenchmarkId::new("append", n), &n, |b, &n| {
            b.iter_batched(
                || {
                    let mut dom = Dom::new();
                    build_wide_tree(&mut dom, n);
                    dom
                },
                |mut dom| {
                    let root = dom.root();
                    let el = dom.create_element("div");
                    dom.append_child(root, black_box(el));
                    black_box(&dom);
                },
                BatchSize::SmallInput,
            );
        });
    }

    // insert_before the first child of the root
    for &n in &SIZES {
        group.bench_with_input(BenchmarkId::new("insert_before", n), &n, |b, &n| {
            b.iter_batched(
                || {
                    let mut dom = Dom::new();
                    let nodes = build_wide_tree(&mut dom, n);
                    (dom, nodes[0])
                },
                |(mut dom, first)| {
                    let root = dom.root();
                    let el = dom.create_element("div");
                    dom.insert_before(root, el, black_box(Some(first)));
                    black_box(&dom);
                },
                BatchSize::SmallInput,
            );
        });
    }

    // remove_child of a known leaf
    for &n in &SIZES {
        group.bench_with_input(BenchmarkId::new("remove", n), &n, |b, &n| {
            b.iter_batched(
                || {
                    let mut dom = Dom::new();
                    let nodes = build_wide_tree(&mut dom, n);
                    let last = *nodes.last().unwrap();
                    (dom, last)
                },
                |(mut dom, last)| {
                    let root = dom.root();
                    dom.remove_child(root, black_box(last));
                    black_box(&dom);
                },
                BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}
```

Note: `remove_child(root, last)` assumes `remove_child` tolerates a non-direct-child target by
locating the actual parent, OR that `last` is a direct child of root. If the real API requires
the true parent, change the setup closure to return the leaf's parent alongside it. This is the
one place the assumed API is thin; adjust at execution time.

- [ ] **Step 2: Register the group**

```rust
fn benches(c: &mut Criterion) {
    bench_construction(c);
    bench_mutation(c);
}
```

- [ ] **Step 3: Run and confirm**

Run: `cargo bench --bench dom_ops -- dom/mutation --quick`
Expected: PASS; nine measurements (append/insert_before/remove × 100/1k/5k). Healthy signal:
`append` and `remove` should be roughly **flat** across sizes (O(1)-ish); a rising curve flags an
O(n) parent scan worth a Tier-3 look later.

- [ ] **Step 4: Commit**

```bash
git add crates/superui_dom/benches/dom_ops.rs
git commit -m "bench(dom): single-mutation append/insert/remove on large trees"
```

---

### Task 4: Query bench (querySelector / querySelectorAll)

**Files:**
- Modify: `crates/superui_dom/benches/dom_ops.rs`

**Interfaces:**
- Consumes: `build_wide_tree`, `Dom::{query_selector, query_selector_all}`.
- Produces: `fn bench_query(c: &mut Criterion)`.

- [ ] **Step 1: Add the query group**

Add above `fn benches`:

```rust
/// Selector matching cost by selector kind, over growing trees.
/// build_wide_tree assigns id="n{i}", class "even item"/"odd item", tag "div".
fn bench_query(c: &mut Criterion) {
    let mut group = c.benchmark_group("dom/query");
    for &n in &SIZES {
        // id lookup — should be O(1) via the id map, flat across sizes.
        group.bench_with_input(BenchmarkId::new("by_id", n), &n, |b, &n| {
            let mut dom = Dom::new();
            build_wide_tree(&mut dom, n);
            b.iter(|| black_box(dom.query_selector(black_box("#n0"))));
        });

        // single class match (first hit).
        group.bench_with_input(BenchmarkId::new("by_class_first", n), &n, |b, &n| {
            let mut dom = Dom::new();
            build_wide_tree(&mut dom, n);
            b.iter(|| black_box(dom.query_selector(black_box(".odd"))));
        });

        // query_selector_all class — returns ~n/2 nodes, tests full traversal + collect.
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::new("all_by_class", n), &n, |b, &n| {
            let mut dom = Dom::new();
            build_wide_tree(&mut dom, n);
            b.iter(|| black_box(dom.query_selector_all(black_box(".item"))));
        });

        // descendant selector — exercises the combinator path.
        group.bench_with_input(BenchmarkId::new("descendant", n), &n, |b, &n| {
            let mut dom = Dom::new();
            build_wide_tree(&mut dom, n);
            b.iter(|| black_box(dom.query_selector_all(black_box("div .even"))));
        });
    }
    group.finish();
}
```

- [ ] **Step 2: Register the group**

```rust
fn benches(c: &mut Criterion) {
    bench_construction(c);
    bench_mutation(c);
    bench_query(c);
}
```

- [ ] **Step 3: Run and confirm**

Run: `cargo bench --bench dom_ops -- dom/query --quick`
Expected: PASS; measurements for by_id / by_class_first / all_by_class / descendant × 3 sizes.
Healthy signal: `by_id` flat across sizes; `all_by_class`/`descendant` scale ~linearly.

- [ ] **Step 4: Commit**

```bash
git add crates/superui_dom/benches/dom_ops.rs
git commit -m "bench(dom): querySelector/All by id/class/descendant across sizes"
```

---

### Task 5: Event-dispatch path bench + correctness guard

**Files:**
- Modify: `crates/superui_dom/benches/dom_ops.rs`

**Interfaces:**
- Consumes: `build_deep_chain` (Task 1), `Dom::{add_event_listener, dispatch_event}`, `ListenerId`.
- Produces: `fn bench_dispatch(c: &mut Criterion)`; module test `dispatch_order_is_capture_then_bubble`.

- [ ] **Step 1: Add the dispatch group**

Add above `fn benches`:

```rust
/// Cost of computing the propagation path and collecting listeners in
/// capture->target->bubble order, over chains of increasing depth. One
/// capturing + one bubbling listener per node, so a dispatch collects ~2*depth
/// listeners. This is the DOM-side half of event handling; JS invocation lives
/// in superui_api and is benched there.
fn bench_dispatch(c: &mut Criterion) {
    let mut group = c.benchmark_group("dom/dispatch");
    for &depth in &DEPTHS {
        group.throughput(Throughput::Elements(depth as u64));
        group.bench_with_input(BenchmarkId::from_parameter(depth), &depth, |b, &depth| {
            let mut dom = Dom::new();
            let chain = build_deep_chain(&mut dom, depth);
            for &node in &chain {
                dom.add_event_listener(node, "click", true); // capture
                dom.add_event_listener(node, "click", false); // bubble
            }
            let leaf = *chain.last().unwrap();
            b.iter(|| black_box(dom.dispatch_event(black_box(leaf), black_box("click"))));
        });
    }
    group.finish();
}
```

- [ ] **Step 2: Add a correctness guard for the dispatch helper**

Append at the end of `dom_ops.rs` (Criterion bench files can carry `#[cfg(test)]` modules):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // Guards the harness setup: dispatch must fire capture (root->leaf) then
    // bubble (leaf->root). If this breaks, the dispatch bench is measuring the
    // wrong traversal and its numbers are meaningless.
    #[test]
    fn dispatch_order_is_capture_then_bubble() {
        let mut dom = Dom::new();
        let chain = build_deep_chain(&mut dom, 3);
        let mut capture_ids = Vec::new();
        for &node in &chain {
            capture_ids.push(dom.add_event_listener(node, "click", true));
        }
        let mut bubble_ids = Vec::new();
        for &node in &chain {
            bubble_ids.push(dom.add_event_listener(node, "click", false));
        }
        let leaf = *chain.last().unwrap();
        let fired = dom.dispatch_event(leaf, "click");

        // Expected order: capture top->leaf, then bubble leaf->top.
        let mut expected = capture_ids.clone(); // root..leaf
        let mut bubble_rev = bubble_ids.clone();
        bubble_rev.reverse(); // leaf..root
        expected.extend(bubble_rev);
        assert_eq!(fired, expected);
    }
}
```

- [ ] **Step 3: Register the group**

```rust
fn benches(c: &mut Criterion) {
    bench_construction(c);
    bench_mutation(c);
    bench_query(c);
    bench_dispatch(c);
}
```

- [ ] **Step 4: Run the correctness guard, then the bench**

Run: `cargo test --bench dom_ops`
Expected: `dispatch_order_is_capture_then_bubble` PASSES. If it fails, fix the assumed dispatch
ordering contract (or the crate's implementation) before trusting the bench.

Run: `cargo bench --bench dom_ops -- dom/dispatch --quick`
Expected: PASS; three measurements (depth 10/50/200), scaling ~linearly with depth.

- [ ] **Step 5: Commit**

```bash
git add crates/superui_dom/benches/dom_ops.rs
git commit -m "bench(dom): event-dispatch path build (capture/bubble) + order guard"
```

---

### Task 6: Establish the baseline and document the run command

**Files:**
- Modify: `crates/superui_dom/benches/dom_ops.rs` (header doc comment only)

**Interfaces:**
- Consumes: all four groups.
- Produces: a saved Criterion baseline named `main` (local artifact, not committed).

- [ ] **Step 1: Add a header doc comment to the bench file**

At the very top of `dom_ops.rs`, prepend:

```rust
//! Tier-1 micro-benches for `superui_dom` (arena DOM).
//!
//! Groups: dom/construction, dom/mutation, dom/query, dom/dispatch.
//!
//! Run all:            cargo bench --bench dom_ops
//! Quick iterate:      cargo bench --bench dom_ops -- --quick
//! Save a baseline:    cargo bench --bench dom_ops -- --save-baseline main
//! Compare to it:      cargo bench --bench dom_ops -- --baseline main
//! Pretty diff (2 saved baselines): critcmp before after
//!
//! See docs/superpowers/specs/2026-07-18-bevy-superui-performance-strategy.md.
```

- [ ] **Step 2: Run the full suite and save the baseline**

Run: `cargo bench --bench dom_ops -- --save-baseline main`
Expected: all four groups run to completion (full sample sizes, no `--quick`); Criterion writes
`target/criterion/**` and a `main` baseline. This is the reference future runs compare against.

- [ ] **Step 3: Commit the doc comment**

```bash
git add crates/superui_dom/benches/dom_ops.rs
git commit -m "bench(dom): document run/baseline commands"
```

> Note: `target/criterion/` baselines are build artifacts — confirm `target/` is git-ignored; do
> not commit baseline data. Trend storage is Plan 11's concern.

---

## Self-Review

**Spec coverage (strategy §3 superui_dom bullet + §4 scenarios):**
- "tree ops: appendChild/insertBefore/removeChild" → Task 3. ✅
- "querySelector" → Task 4 (querySelector + querySelectorAll, multiple selector kinds). ✅
- "event-dispatch (capture/bubble) through a chain of depth N" → Task 5. ✅
- §4 scaling at 100/1k/5k → `SIZES` used in Tasks 2–4; depth 10/50/200 in Task 5. ✅
- §4 determinism (no random/clock) → tree builders are fixed-pattern; Global Constraints. ✅
- §3 "assert cost, not just command set" → all groups measure time; N/A command sets here. ✅
- Baseline/"faster than before?" workflow → Task 6 documents save/compare/critcmp. ✅

**Placeholder scan:** No TBD/TODO/"handle edge cases". The one soft spot (`remove_child` parent
assumption in Task 3 Step 1) is called out explicitly with the concrete adjustment, not left vague. ✅

**Type consistency:** `Dom`, `NodeId`, `ListenerId`, `SIZES`, `DEPTHS`, `build_wide_tree`,
`build_deep_chain`, `bench_construction`/`_mutation`/`_query`/`_dispatch` are named identically at
definition and every call site. `benches` accumulates all four groups by Task 5. ✅

**Coverage gap acknowledged:** memory/allocation of DOM ops is intentionally deferred to Plan 10
(dhat), not duplicated here.
