# bevy_superui — Performance Benchmarking & Optimization Strategy

Date: 2026-07-18
Status: Approved for planning (strategy only — no implementation yet)
Related: [`2026-07-18-bevy-superui-design.md`](./2026-07-18-bevy-superui-design.md)

## 1. Purpose & scope

This document defines *how* we will benchmark `bevy_superui` and *how* we will drive
optimization from evidence. It is a strategy, not an implementation plan.

**Near-term goal is regression tracking, not premature tuning.** The primary value is a
trustworthy, repeatable answer to *"is it faster now than before?"* — so any performance
spike is powered by real numbers rather than intuition. Optimizations are pulled from a menu
(§5) *only after* measurement earns them.

### Priorities (what this strategy protects)

Ranked, from the design conversation:

1. **Steady-state frame cost** — per-frame UI cost during gameplay (reconcile diff, event
   dispatch, JS handler calls, cascade re-eval). Idle UI must be near-free; active UI must
   stay well under frame budget so it never causes hitches.
2. **Interaction latency** — input (click/keypress) → JS handler → DOM mutation → reconcile →
   visible pixel change. The felt responsiveness.
3. **Memory** — heap footprint (arena DOM, Boa heap, ECS mirror) and, most importantly,
   *per-frame allocation churn* (steady-state should allocate ~nothing).
4. **wasm binary size** — *explicitly deferred* for now. Not a near-term concern.

### Workloads

- **TodoMVC-class (Phase 1)** — tens to low-hundreds of nodes; represents realistic game
  HUDs/menus. Optimize per-frame fixed overhead so idle UI is nearly free.
- **Stress / large trees** — thousands of nodes, deep nesting, large CSS, rapid mutation
  bursts. Flushes out O(n²) traps in diff / cascade / layout early.

Framework-output (Phase 3) and pathological-micro workloads are out of near-term scope but
the scenario design (§4) leaves room for them.

## 2. Approach

Chosen combination (from three candidates weighed during brainstorming):

- **A — Criterion-first, layered (backbone).** Criterion for headless micro-benches (one target
  per crate) plus a headless-Bevy-app integration bench. Local
  `--save-baseline`/`--baseline` (and `critcmp`) provide the "faster than before?" loop with
  near-zero infrastructure. Plays directly to the design's headless-testable crate boundaries.
- **B — App-instrumentation (complement).** A dedicated bench example/scene captures real
  per-frame timings and interaction-latency percentiles from a running app — the numbers
  Criterion can't easily get through Bevy's schedule.

Rejected for now: **C — full CI-tracked continuous benchmarking upfront** (stored-baseline
dashboards, automated PR gating). Best long-term protection but too much infra before the code
exists, and noisy runners make micro-bench gating flaky. Deferred to a "phase 2 of
benchmarking" once benches are stable (see §6).

## 3. Harness architecture — three tiers + the loop

Three tiers mapped onto the crate boundaries so each isolates one thing:

**Tier 1 — Headless micro-benches (Criterion, per crate).** No Bevy app, no window.
- `superui_dom` — tree ops (`appendChild`/`insertBefore`/`removeChild`), `querySelector`,
  event dispatch (capture/bubble) through a chain of depth N.
- `superui_html` — parse fixture → DOM, at small and large sizes.
- `superui_js` / `superui_api` — execute representative JS snippets against a DOM; isolate
  **DOM↔JS marshalling cost per boundary crossing** (the Boa risk surfaces here).
- `superui_bridge` — reconciler diff: unchanged tree (near-zero), single-node mutation,
  subtree replace, large-list rebuild — asserting *cost*, not just the command set.
- `superui_css` (forked flair) + taffy — cascade / selector-matching and layout as tree size
  and sheet complexity grow, **benched in isolation** so cost attributes to "our code" vs
  "reused upstream."

**Tier 2 — End-to-end frame time (headless Bevy app).** Full `SuperUiPlugin`, drive synthetic
input, `app.update()` in a loop, measure real per-frame cost and interaction latency. This is
the B-style bench scene, runnable headlessly so it can also live in CI later.

**Tier 3 — Profiling / attribution (Tracy + `tracing` spans).** Annotate our seams —
`reconcile`, `cascade`, `layout`, `js_dispatch`, `marshal` — with `tracing`/`info_span!`.
Running Tier 2 (or the real example) under `bevy/trace_tracy` streams those spans to the Tracy
profiler for per-frame flamegraphs, composed with Bevy's own spans. Tracy is the instrument
that turns "a frame is slow" into "*this span* is slow." Native-only, dev-profiling — no wasm
concern.

### What Tier answers what

- **Tier 1:** did *this crate* regress?
- **Tier 2:** did the *whole thing* regress?
- **Tier 3:** *where* did the frame go?

### The optimization loop (the actual deliverable)

```
Tier 2 flags it    →  "end-to-end frame cost regressed / spikes on large-list rebuild"
      │
Tier 3 pins it     →  Tracy flamegraph: cost is in cascade recompute, not the diff
      │
Tier 1 captures it →  a focused Criterion bench reproducing *just* that
                      (e.g. "re-cascade after one class toggle on a 2k-node tree")
      │
Optimize           →  change the code
      │
Tier 1 proves it   →  criterion --baseline shows the isolated win
      │
Tier 2 confirms    →  the end-to-end number moved, nothing else regressed
      │
Bench stays        →  the new Tier-1 bench is permanent regression protection
```

A Tier-2/Tier-3 finding **graduates into a permanent Tier-1 bench**. Over time the micro-bench
suite becomes a growing library of "things we once found slow and now guard." Every
optimization spike ends by leaving a bench behind.

## 4. Scenarios, fixtures, determinism

**Shared fixtures** (checked in, so every run measures the same thing):
- `small/` — the TodoMVC assets (index.html, style.css, app.js): realistic HUD/menu scale.
- `large/` — a generated stress tree, parameterized (N nodes, depth D, S selectors), benched at
  e.g. 100 / 1k / 5k nodes to observe scaling curves, not a single point.

**Canonical scenarios** (each a named bench, run at both scales):
- **Idle frame** — no DOM change. *The most important steady-state guard:* the dirty-flag gate
  should make this near-zero; a regression here means idle UI started doing work.
- **Single mutation** — toggle one todo's `checked` / one class. Tests the incremental path
  (diff + scoped re-cascade + layout).
- **Structural churn** — add / delete todos; filter all↔active↔completed (subtree add/remove).
  The framework-churn precursor.
- **Large-list rebuild** — replace a big subtree at once. Worst-case diff + cascade + layout.
- **Interaction latency** — synthetic click/keypress → input→visible-change across the full
  pipeline, reported as **percentiles (p50/p95/p99), not mean** (latency cliffs hide in the
  tail).

**Determinism rules (non-negotiable for regression tracking):**
- **Seed Boa / avoid wall-clock** — inject a fixed JS clock and fixed seed (the design already
  notes `Date.now()`/random need the JS clock) so runs are reproducible.
- **Fixed synthetic input** — scripted event sequences, no real window/timing.
- **Warm vs cold separated** — parse / first-cascade (cold) benched apart from steady-state
  (warm); never mixed into one number.
- **Pin & label the environment** — record toolchain, `--release`, CPU-governor caveats.
  Tier-1 micro-benches are the CI-gating numbers (stable); Tier-2 frame numbers are directional
  (noisier runners) — stated explicitly rather than pretending CI wall-clock is precise.

**Memory** (priority 3): a `dhat`-based allocation bench on the same scenarios — track bytes +
allocation count for idle, single-mutation, and large-rebuild. The signal we care about is
**per-frame allocation churn** (steady-state should allocate ~nothing) and **arena / Boa-heap
growth** across a mutation cycle.

## 5. Optimization levers

The menu we pull from *only after* a Tier-2/Tier-3 finding justifies it. Grouped by where the
architecture concentrates cost. Every lever is **paired with the bench that would prove it**.

**Reconciler (single coupling point — highest leverage):**
- **Dirty-flag granularity** — evolve the design's global dirty flag toward per-node/subtree
  dirty marking so a single mutation doesn't walk the whole tree. Guarded by the "idle frame"
  and "single mutation" benches.
- **Diff avoidance** — skip provably-unchanged subtrees via version counters / structural
  hashing, so large trees with local edits stay cheap.
- **ECS command batching** — coalesce spawn/despawn/reparent/text/style into batched commands.

**DOM↔JS boundary (Boa risk):**
- **Minimize boundary crossings** — batch marshalling; cache JS handle→NodeId wrappers rather
  than rebuilding per access.
- **Precompile / reuse** — reuse the Boa context and pre-parse event-handler sources rather
  than re-parsing per dispatch.
- **The trait swap as a lever** — because `JsEngine` is a trait, a native `rquickjs` backend can
  be measured against Boa *through the same harness*. Worth a bench-time feature flag so the
  comparison is one command, even though rquickjs is later.

**Cascade / layout (reused flair + taffy):**
- **Incremental / scoped cascade** — restyle only the affected subtree on a class/attr toggle
  rather than re-cascading globally (a fork-of-flair extension the design already contemplates).
- **Attribute cost to upstream** — the isolated Tier-1 flair/taffy benches tell us whether to
  optimize *our* reconciler or push a fix into forked flair; we don't guess.

**Allocation / memory (steady-state churn):**
- **Arena free-list reuse** — recycle `NodeData` slots (generational ids already support this)
  so mutation cycles don't grow the heap.
- **String interning** — intern class / attribute / selector names so comparisons are
  id-equality and repeated names don't reallocate.

**Cross-cutting principle:** never ship an optimization without an isolated Tier-1 number
showing the win and a Tier-2 check showing nothing else regressed. Levers stay *unpulled* until
measurement earns them (YAGNI on optimization).

## 6. Where it lives & how it plugs in

**Repo layout** (extends the workspace in the design's §4, doesn't disturb it):
- `benches/` per crate — Criterion targets colocated with the crate they measure
  (`crates/superui_dom/benches/`, etc.), declared `[[bench]]` with `harness = false`. Tier-1
  benches sit next to the headless code they exercise.
- `benches/fixtures/` (shared) — the `small/` (TodoMVC) and generated `large/` assets from §4.
- `examples/bench_scene/` — the Tier-2 headless-app bench: full `SuperUiPlugin`, scripted
  scenarios, frame-time + latency-percentile capture. Runnable two ways: plain (prints a
  numbers table) and under `--features bevy/trace_tracy` (Tier-3 Tracy profiling).
- `crates/*/src` — the `tracing`/`info_span!` seam annotations, behind cheap-when-off spans so
  they cost nothing in normal builds.

**Plugs into the design's §11 testing strategy as a sibling, not a replacement:**
- Existing headless unit/reconciler tests already build fixtures and drivers — benches **reuse
  those same drivers** (same synthetic-event harness, same fixture loader). A bench is "the
  test, timed." No parallel infrastructure.
- The "example-as-integration-test" (§11) and `bench_scene` are the same scripted TodoMVC flow
  at two throttles: asserting *correctness* vs. measuring *cost*.

**CI posture (staged, per the A+B choice):**
- **Now:** benches exist and run locally; workflow is `criterion --save-baseline before` →
  change → `--baseline before` (or `critcmp before after`). Manual, trustworthy, zero infra.
- **CI gate (Tier-1 only, when stable):** run deterministic micro-benches on a pinned runner;
  treat as directional regression signals, not hard fails initially.
- **Deferred (the "C" we set aside):** stored-baseline trend dashboards / automated PR gating —
  added only once the bench suite has proven stable enough to not be noisy.

**Documentation seam:** a short `benches/README.md` — how to run each tier, how to read a
baseline diff, how to attach Tracy, and the **"found-slow → graduate-to-Tier-1-bench" loop**
written down as the standing process, so every future optimization spike follows it.

## 7. Risks & caveats

- **Tier-2 CI noise** — wall-clock on shared runners is imprecise; we gate on Tier-1 and treat
  Tier-2 as directional. Stated up front to avoid false regression alarms.
- **Tracy is native-only** — fine; it's a dev-profiling tool, never on the wasm runtime path.
- **Bench drift from real usage** — generated `large/` fixtures may not match real framework
  churn; revisit fixtures when Phase 3 (framework output) lands.
- **Over-benching** — resist adding benches with no found-slow origin; the suite grows from the
  loop (§3), not speculatively.

## 8. Definition of done (for the benchmarking capability)

- Tier-1 Criterion benches exist for each crate's core operations, using shared fixtures.
- A Tier-2 `bench_scene` runs the canonical scenarios headlessly and reports frame cost +
  latency percentiles.
- Tier-3 spans exist at the named seams and stream to Tracy under `bevy/trace_tracy`.
- The `criterion --save-baseline` / `critcmp` "faster than before?" workflow is documented in
  `benches/README.md`, including the found-slow → Tier-1-bench loop.
- A `dhat` allocation bench covers idle / single-mutation / large-rebuild.
