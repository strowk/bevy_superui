# bevy_superui Benchmarking — Plan Index & Roadmap

Date: 2026-07-18
Status: In progress (index + Plan 1 written; remaining plans pending review)
Specs: [design](../specs/2026-07-18-bevy-superui-design.md) ·
[performance strategy](../specs/2026-07-18-bevy-superui-performance-strategy.md)

## What this is

The performance strategy defines three benchmark tiers and a per-crate Tier-1 layout. This
index breaks that into a sequence of **individual implementation plans** — one per crate for
Tier-1 micro-benches, then cross-cutting plans for the remaining tiers and infrastructure.

Each plan is self-contained and executable on its own (subject to the prerequisite below) via
`superpowers:subagent-driven-development` or `superpowers:executing-plans`.

## Two constraints that shape every plan

**1. Benches only, forward-referenced.** These plans write *benchmark harnesses*, not crate
functionality. Each per-crate plan references the API the main design intends for that crate
(listed verbatim in the plan's `Interfaces → Consumes` block). The crate's actual Phase-1
implementation is **out of scope** here (that was the declined "build crate" option).

> **Execution prerequisite:** a per-crate bench plan compiles and runs only once that crate
> exposes the assumed Phase-1 API. Until then the plan is "ready to execute when the crate
> lands." The `Consumes` block is the contract the crate must satisfy; if the real API differs,
> update the bench's call sites (the *scenarios* don't change). This is the accepted cost of
> writing benches ahead of the code.

**2. Bench "test cycle" replaces red-green TDD.** A benchmark has no failing-then-passing
assertion. Every bench task instead follows:

1. Write the bench function(s).
2. Run `cargo bench --bench <name> -- --quick` → confirm it **compiles and emits a timing** (a
   number, not a panic). `--quick` keeps iteration fast.
3. (Wrap-up task only) `cargo bench --bench <name> -- --save-baseline main` to record the first
   baseline, then `git commit`.

Where a bench *can* carry a correctness assertion cheaply (e.g. "dispatch returns N listeners in
capture→bubble order"), the plan adds a tiny `#[test]` alongside it so the harness itself is
guarded. Benches measure; the optional test guards the bench's own setup.

## Global constraints (apply to all plans)

- **Bevy 0.17**; fork base **bevy_flair 0.6.0** (the 0.8/Bevy-0.19 vendored copy is reference
  only). Benches are **native-only dev tooling** — never on the wasm runtime path.
- **Criterion** as the harness, declared `[[bench]]` with `harness = false` in each crate's
  `Cargo.toml`, criterion as a `[dev-dependencies]`.
- **Determinism (non-negotiable):** no wall-clock, no unseeded randomness in bench bodies. Fixed
  seeds and a fixed injected JS clock where Boa is involved (Plan 2 provides the helpers).
- **Layer discipline:** a bench for crate X depends only on X (+ `superui_bench_support` from
  Plan 2 where fixtures/clock are needed). `superui_dom`/`superui_html`/`superui_js`/
  `superui_api` benches pull in **no Bevy**. Only Tier-2/Tier-3 plans touch Bevy.
- **Reported statistics:** latency-style scenarios report **p50/p95/p99**, not mean. Steady-state
  scenarios report mean ± stddev. Cold (parse/first-cascade) and warm (steady-state) are
  **separate benches**, never averaged together.

## The plans

Ordering is by dependency. `superui_dom` (Plan 1) is genuinely standalone — it builds trees
programmatically and needs no shared fixtures, which is why it's the foundational first plan.
Plan 2 (`superui_bench_support`) provides the shared fixtures/determinism that Plans 3–10 consume.

| # | Plan | Tier | Crate(s) | Depends on | File |
|---|------|------|----------|-----------|------|
| 1 | superui_dom benches | T1 | `superui_dom` | — | `2026-07-18-superui-dom-benches.md` ✅ |
| 2 | bench-support foundation | infra | `superui_bench_support` (new dev crate) | — | `2026-07-18-superui-bench-support.md` |
| 3 | superui_html benches | T1 | `superui_html` | 2 | `2026-07-18-superui-html-benches.md` |
| 4 | superui_js benches | T1 | `superui_js` | 2 | `2026-07-18-superui-js-benches.md` |
| 5 | superui_api benches | T1 | `superui_api` | 2, 4 | `2026-07-18-superui-api-benches.md` |
| 6 | superui_css benches | T1 | `superui_css` (forked flair) + taffy | 2 | `2026-07-18-superui-css-benches.md` |
| 7 | superui_bridge benches | T1 | `superui_bridge` (reconciler diff) | 2 | `2026-07-18-superui-bridge-benches.md` |
| 8 | Tier-2 end-to-end frame bench | T2 | `superui` umbrella, `examples/bench_scene` | 2–7 | `2026-07-18-tier2-frame-bench.md` |
| 9 | Tier-3 profiling (Tracy + spans) | T3 | all (span annotations) + `superui` | 8 | `2026-07-18-tier3-tracy-profiling.md` |
| 10 | Memory / allocation benches (dhat) | mem | dom, bridge, `superui` | 2, 7, 8 | `2026-07-18-memory-dhat-benches.md` |
| 11 | CI posture + benches/README | infra | workspace | 1–10 | `2026-07-18-bench-ci-and-docs.md` |

### Per-crate Tier-1 scope (Plans 1, 3–7)

Each writes the scenarios named in strategy §3–§4 for that crate:

- **1 · superui_dom** — tree construction (100/1k/5k), mutation (append/insert/remove),
  `querySelector`/`All`, event-dispatch path build (capture→bubble) at depth 10/50/200. *No
  fixtures — programmatic trees.*
- **3 · superui_html** — parse `small/` (TodoMVC) and generated `large/` HTML → DOM; cold parse
  cost at 100/1k/5k nodes.
- **4 · superui_js** — raw `JsEngine` snippet execution; **DOM↔JS marshalling cost per boundary
  crossing** (handle wrap/unwrap, value marshalling); context reuse vs re-create.
- **5 · superui_api** — web-API operations through JS: `getElementById`, `querySelector`,
  `classList.add/remove/toggle`, `.style.*`, `createElement`+`appendChild`,
  `addEventListener`+dispatch round-trip.
- **6 · superui_css** — cascade + selector matching and taffy layout **in isolation**, as tree
  size and selector count grow; single-class-toggle re-cascade cost (guards the incremental path).
- **7 · superui_bridge** — reconciler diff cost: unchanged tree (must be ~0), single-node
  mutation, subtree replace, large-list rebuild → measured as *cost*, not just command count.

### Remaining-work scope (Plans 8–11)

- **8 · Tier-2** — headless `SuperUiPlugin` app in `examples/bench_scene`; scripted canonical
  scenarios (idle / single-mutation / structural-churn / large-rebuild / interaction-latency);
  per-frame cost + latency percentiles; dual-mode runner (plain table / Tracy). Covers the
  `superui` umbrella crate (which has no Tier-1 plan of its own).
- **9 · Tier-3** — `tracing`/`info_span!` at the seams (`reconcile`, `cascade`, `layout`,
  `js_dispatch`, `marshal`), behind cheap-when-off spans; wire `bevy/trace_tracy`; document
  attaching Tracy. Enables the found-slow → Tier-1-graduate loop.
- **10 · Memory** — `dhat`-based allocation benches (bytes + alloc count) for idle /
  single-mutation / large-rebuild; assert steady-state per-frame churn ≈ 0 and arena/Boa-heap
  stability across a mutation cycle.
- **11 · CI + docs** — staged CI posture (Tier-1 directional gate on a pinned runner; Tier-2
  informational), `critcmp` before/after workflow, and `benches/README.md` documenting how to
  run each tier, read a baseline diff, attach Tracy, and follow the found-slow loop.

## Repo layout these plans build toward

```
bevy_superui/
├─ crates/
│  ├─ superui_bench_support/   # Plan 2 — dev crate: fixtures, large-tree/HTML generator, fixed clock/seed
│  │  └─ fixtures/
│  │     ├─ small/             # TodoMVC index.html/style.css/app.js (shared with the example)
│  │     └─ large/             # generator output is produced in-code, not checked in
│  ├─ superui_dom/benches/dom_ops.rs          # Plan 1
│  ├─ superui_html/benches/html_parse.rs      # Plan 3
│  ├─ superui_js/benches/js_exec.rs           # Plan 4
│  ├─ superui_api/benches/dom_api.rs          # Plan 5
│  ├─ superui_css/benches/cascade_layout.rs   # Plan 6
│  └─ superui_bridge/benches/reconcile.rs     # Plan 7
├─ examples/
│  └─ bench_scene/             # Plan 8 (Tier 2) + Plan 9 (Tracy) + Plan 10 (dhat)
└─ benches/
   └─ README.md                # Plan 11
```

## Next step

Plan 1 (`superui_dom`) is written in full as the format reference. On your approval of its depth
and shape, I'll generate Plans 2–11 in dependency order.
