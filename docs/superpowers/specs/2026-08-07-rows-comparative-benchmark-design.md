# Rows — a comparative benchmark citable against bevy-react

**Date:** 2026-08-07
**Status:** design (approved in brainstorm; not yet planned)
**Author:** pairing session
**Prior art:** [bevy-react BENCHMARKS.md](https://github.com/tulustul/bevy-react/blob/main/docs/BENCHMARKS.md) ·
[horde harness](2026-07-21-horde-benchmark-harness-design.md) ·
[citadel](2026-07-22-citadel-strategy-hud-benchmark-example-design.md) ·
[benchmarking roadmap](../plans/2026-07-18-bevy-superui-benchmarking-index.md)

## 1. Purpose

Build `examples/rows` — the js-framework-benchmark ("krausest") rows workload —
and publish its numbers at `docs/BENCHMARKS.md`.

This is a **comparative** benchmark, and that is the whole point of it. It is not
another internal optimization tool. horde and citadel already fill that role: both
measure *steady-state per-frame* cost on a bespoke workload, tuned to make one
specific reconciler change legible. Their numbers are meaningful only against
themselves.

Rows is different. Its job is to produce a number that a reader can put directly
next to somebody else's number — specifically bevy-react's, which runs the same
workload on the same underlying `bevy_ui`. That forces three properties horde and
citadel deliberately do not have:

- **A standard workload.** The op set, row counts, and row markup are not ours to
  choose. They come from js-framework-benchmark via bevy-react.
- **Per-operation latency**, not per-frame steady state. The unit is "how long did
  `swap1` take", measured trigger→settled.
- **A defensible headline number.** One column, `Total`, measured with no
  instrumentation overhead in the build at all.

### Non-goals

- Beating bevy-react. If we lose a row, the row gets published anyway.
- Comparing against real browsers. Different renderer, different question.
- Replacing horde/citadel. They keep their jobs.
- A CI gate. This is a published-numbers benchmark, run deliberately.

## 2. The comparability contract

Everything in this section exists so the numbers stay citable. Changing any of it
invalidates comparison against previously published tables, ours or theirs.

**Ops** — bevy-react's list, exact names, exact order:

`create` · `append1` · `append1k` · `insert1` · `insertEvery2nd` · `updateText1` ·
`updateTextEvery2nd` · `updateColor1` · `updateColorEvery2nd` · `swap1` ·
`swapEvery2nd` · `remove1` · `removeEvery2nd` · `clear`

**Scales** — 1k and 10k, one table each, matching theirs.

**The `Every2nd` rule** — every `*Every2nd` op performs exactly **N/2 operations** at
N rows. `updateTextEvery2nd`/`updateColorEvery2nd` touch N/2 rows; `removeEvery2nd`
removes N/2, leaving N/2; `insertEvery2nd` inserts N/2, leaving 3N/2 (not 2N);
`swapEvery2nd` performs N/2 swaps of adjacent disjoint pairs (0,1),(2,3),…  bevy-react's
`Ops Emitted` column counts React batch entries rather than DOM moves, so it cannot
settle the stride question by itself — the rule is fixed here by internal consistency
instead, and restated in the published doc so a reader can check what was measured
rather than infer it.

**Keying** — keyed only. js-framework-benchmark maintains separate keyed and
non-keyed leaderboards precisely because the two are not comparable; React's entry
there uses `key={item.id}`, so bevy-react is a keyed implementation and ours must
be too. See §3.2.

**Determinism** — fixed seed, seeded PRNG in JS for row labels, no `Math.random`,
no wall-clock in app code.

**Provenance** — every published table states the commit it was run against and
the machine spec, as bevy-react's does.

## 3. Workload

### 3.1 Markup

The js-framework-benchmark row, with `<div>` substituted wherever
`<table>`/`<tr>`/`<td>` would appear. Neither engine has table semantics — superui
and bevy-react both lower to `bevy_ui` flexbox — so a real `<table>` would be a
flex box in both cases and the substitution costs nothing but honesty.

**Node count per row is held identical to the reference row**, and the doc states
it explicitly, so a reader comparing against any other implementation can
normalize. This replaces bevy-react's `Ops Emitted` column, which counts entries
in their op batch — an artifact of their cross-thread protocol with no superui
equivalent. Ours is a `Nodes` column and the doc says exactly what it counts.

`updateColor*` toggles a **CSS class**, not an inline style. This is deliberate:
it is the op that drives the flair cascade, which is the stage bevy-react's
pipeline does not have at all, and inline styles would route around it.

### 3.2 Two backends, one workload

| backend | what it measures |
|---|---|
| `vanilla` | superui as a browser: `document.createElement`/`appendChild`/`insertBefore`/`textContent`/`classList` → `superui_dom` → reconcile → `bevy_ui`. No reactive framework in the path. |
| `supersolid` | the framework: same app in TSX, rows rendered by `<For>`. The direct analogue of bevy-react's React reconciler. |

Both drive byte-identical markup, so the delta between them **is** supersolid's
overhead over raw DOM. That internal comparison is free and lands in the same
tables.

For that delta to mean anything, the two backends must differ *only* in what
drives the DOM. The two asset dirs therefore share one stylesheet verbatim (same
file content, same selector set) and produce the same element tree — if the CSS
differed, the `Flair cascade` column would differ for reasons that have nothing to
do with the framework. The per-op DOM-state tests (§8) run against both backends
and are what actually enforce this.

`superui_api` already exposes everything the vanilla app needs (verified:
`createElement`, `appendChild`, `insertBefore`, `removeChild`, `replaceChild`,
`textContent`, `classList`, `setAttribute`, `getElementById`, `querySelector`,
`addEventListener`).

**`<For>`, not `<Index>` or `<Keyed>`.** `<Keyed>` is excluded on correctness, not
preference — `crates/supersolid_runtime/src/render.js:308` states it plainly:

> DOM order is append/remove (survivors keep their node, new rows append). That is
> correct for absolutely-positioned overlays; use `<For>` when order matters.

`swap1` and `insertEvery2nd` would render in the wrong order under `<Keyed>`; it
exists for horde's absolutely-positioned overlays, where order is meaningless.
`<For>` keys rows by item identity (`render.js:488`), which is both correct here
and the category bevy-react is in. `<Index>` is the non-keyed analogue and is out
of scope. The published doc carries a one-line note on why `<Keyed>` is absent, so
this does not get re-litigated later.

## 4. Measurement protocol

### 4.1 Per-op cycle

Per rep:

1. Drive the table to the op's precondition — the pre-op row count bevy-react
   reports in its `Rows` column (0 for `create`, 1000/10000 otherwise) — and run
   to quiescence. **Untimed.**
2. Trigger via `superui_bridge::events::click_effect(rt, button_node, &mut pending)`.
   This is a real click through the event system, so `Total` covers the same span
   of work as bevy-react's "event trigger → change detected". `click_effect` is
   public and documented for exactly this ("test/automation drivers that can't
   synthesize a real click", `events.rs:129`).
3. `app.update()` in a loop, timing each frame. **`Total` = sum of the frames that
   did work.**

### 4.2 Quiescence is a predicate, not a frame count

Step 3 keeps stepping until a frame applies **zero DOM mutations**. A fixed frame
count would silently absorb any op that spills across frames.

The harness reports a **`Frames`** column beside `Total`. It should read 1 for
every op; if an op ever spills, that becomes visible in the published table
instead of quietly inflating a number. bevy-react has no such column — it is cheap
insurance that our totals are auditable, and it is the kind of thing a skeptical
reader of a comparative benchmark will want.

### 4.3 Statistics

`p50` is the headline, matching bevy-react. `p95`/`p99` always present in JSON
output — horde and citadel both report tails and this repo's convention is that
latency-style scenarios report p50/p95/p99 rather than means
([roadmap](../plans/2026-07-18-bevy-superui-benchmarking-index.md), "Reported
statistics").

Defaults: 20 reps at 1k, 10 at 10k, after `--warmup` discarded reps.

## 5. Reporting — the untraced/traced split

Instrumentation overhead must never contaminate the headline number, and the doc
must never leave a reader guessing which regime a number came from. Two separate
runs, never blended:

| | Build | Reports | Purpose |
|---|---|---|---|
| **Pass A** | no `bevy/trace` | `Total` p50, `Rows`, `Nodes`, `Frames` | The citable number. Zero tracing overhead. |
| **Pass B** | `--profile` + `bevy/trace` | `Total (traced)` + stage breakdown | Where the time goes. |

`docs/BENCHMARKS.md` publishes **four tables** — 1k untraced, 10k untraced, 1k
traced, 10k traced — under a header stating in plain words that:

- Pass A tables carry no tracing overhead and are the numbers to cite;
- stage columns sum to **`Total (traced)`**, not to `Total`;
- the per-op overhead delta between the two is shown, so the cost of Pass B is
  visible rather than implied.

### 5.1 Stage columns

`JS (Boa)` · `Reconcile` · `Flair cascade` · `Taffy` · `Marshal` · `bevy_ui other` · `Other`

Derived from the per-system tracing busy-time attributor (§6). The published doc
carries the system→stage mapping table. **Any unattributed busy time lands in
`Other` and is printed**, never dropped — a breakdown that silently fails to sum
is worse than no breakdown.

**Post-implementation note (final whole-branch review, 2026-08-08):** the column
list above updates this section's original draft, which listed a `Command`
bucket between `Reconcile` and `Flair cascade`. No such bucket exists in
`bucket_for` (`crates/superui_bench_support/src/profile.rs`) — it was never
implemented, because `reconcile_system` is an exclusive system (`world: &mut
World`) that mutates entities directly as it walks the change set rather than
issuing `Commands`, so there is nothing for a separate command-application span
to measure; bevy-react's `Command` cost has no counterpart here and is folded
into `Reconcile`. The list also adds `Marshal` and `bevy_ui other`, two buckets
the implementation needed once the JS-bridge and text/layout-prep systems
turned out not to fit cleanly into the other five (see the published doc's
stage-bucket mapping table for what each one catches).

### 5.2 Crosswalk to bevy-react

The doc carries an explicit column mapping, because the pipelines genuinely
differ and hand-waving it would be the easiest way to publish a misleading table:

| bevy-react | superui | note |
|---|---|---|
| `Pre-apply` | — | Their JS runs on another thread; the column is the round-trip + scheduling. Boa is in-process here. |
| `JS` | `JS (Boa)` | Reconcile/render + DOM calls. |
| `Flush` | — | `serde_v8` decode of the op batch. No serialization hop here. |
| `Translate` | `Reconcile` | Walk the change set → queue ECS commands. |
| `Command` | `Command` | Execute queued ECS commands. |
| `Layout` | `Taffy` | Both `bevy_ui` taffy solve. |
| — | `Flair cascade` | CSS selector matching + property application. bevy-react has no equivalent stage. |
| `Total` | `Total` | **The one strictly comparable number.** |

## 6. Harness extraction

Rows must not become a third copy of the bench harness.

**Citadel's `src/bench/mod.rs` has zero unique items** — every struct and function
in its 617 lines also exists in horde's 870. It was copy-pasted from horde with the
game-specific parts deleted.

But the duplication is **structural, not textual**, and that distinction decides
what can actually move. Measured function-by-function:

| | horde vs citadel |
|---|---|
| `Stats`, `stats_from`, `alloc_table` | byte-identical |
| `time_backend` (4 differing lines), `run_alloc` (2), `probe_marshal` (9) | differ **only** in the config type and the `build_bench_app` call |
| `memory_asset_dir` (8), `parse_args` (19) | genuinely divergent — asset paths; backend variants and `--preset` handling |
| `report_table`, `report_json`, `sweep_table` | genuinely divergent — horde emits `enemy_cap`, `native_total_ms` and a "vs native floor" block; citadel emits `building_count` and different hint text |
| `profile.rs` | ~95% identical — differs in a doc-comment command string, the config type, and horde's god-mode system |

Genuinely horde-specific: `auto_player`, `synthetic_project`,
`trajectory_signature`, `count_ui_nodes`, `BenchFrame`, `VIEWPORT`.

So almost nothing is byte-identical. What was copy-pasted is one *design*, re-typed
against a different config type — which is why a naive "move the duplicated
functions" extraction would fail. The middle row only becomes shareable once the
generic half stops needing the example's config type at all, by taking an
**`impl FnOnce() -> App`** instead. That closure is the whole trick, and it is what
lets `profile` and `alloc` move without dragging `CitadelConfig`/`SimConfig` behind
them.

The bottom two rows do not move. Unifying the report formatters needs either a
config knob per divergence — worse than two copies — or a changed output, which
would defeat the §6.1 gate outright.

| moves | stays |
|---|---|
| `Stats`/`stats_from` (byte-identical) | `report_table`/`report_json`/`sweep_table` — divergent output, and the gate depends on it |
| all of `profile.rs` — tracing layer, buckets, stage table (~350 lines × 2, the bulk of it) | `build_bench_app`, `memory_asset_dir` — per-example plugins and asset paths |
| `AllocReport`/`alloc_table`/`run_alloc`, re-parameterized on the closure | `time_backend`, `probe_marshal`, `sim_for`, workload samplers — bound to per-example config/snapshot types |
| `BenchArgs`/`parse_args`, with `backend` left a `String` for the example to map | `Backend` — the variant sets genuinely differ (horde has `Native`) |

The [benchmarking roadmap](../plans/2026-07-18-bevy-superui-benchmarking-index.md)
already planned this as Plan 2 (`superui_bench_support`); it was never built, which
is why the copy-paste happened. This change builds it.

**`crates/superui_bench_support`** (`publish = false` — it is a workspace member
and must not ship to crates.io), containing the generic core above. Horde,
citadel, and rows all consume it.

### 6.1 Extraction must be behaviour-preserving

horde's and citadel's `benchmark.md` carry published numbers and a documented A/B
methodology. The extraction is a refactor, and "it compiles" is not evidence.

Gate: **run horde and citadel before and after the extraction and confirm output
is unchanged within noise, before rows is touched at all.** If a number moves, the
extraction is wrong and gets fixed before proceeding.

Naming note: the roadmap scoped `superui_bench_support` as fixtures + generator +
fixed clock for Tier-1 criterion micro-benches. This crate is the *macro*-bench
harness — a related but distinct purpose. Reusing the name is intentional (one
bench-support crate, two module groups) but worth confirming at plan time rather
than discovering later.

## 7. Deliverables

```
crates/
  superui_bench_support/       # NEW — stats, report table/json, dhat, tracing profiler
examples/
  rows/
    assets/ui/rows_vanilla/    # index.html, app.js, style.css
    assets/ui/rows_solid/      # index.html, app.tsx, style.css  (<For>)
    build.rs                   # supersolid::build::transpile_dir("assets/ui/rows_solid")
    src/
      bin/bench.rs             # rows-bench
      bench/                   # op driver, precondition setup, quiescence loop
    tests/                     # per-op DOM-state assertions
    benchmark.md               # how to run
  horde/, citadel/             # migrated onto superui_bench_support
docs/
  BENCHMARKS.md                # NEW — published results (same path as bevy-react)
```

`rows-bench` flags: `--backend vanilla|supersolid`, `--rows`, `--reps`,
`--warmup`, `--seed`, `--profile`, `--format table|json`.

## 8. Testing

A benchmark has no red-green cycle, so per the roadmap's bench test-cycle
convention each op carries a cheap correctness assertion guarding the *harness*:

- `examples/rows/tests/` asserts each op leaves the DOM in the expected state —
  `swap1` swaps exactly two rows and preserves order; `removeEvery2nd` leaves the
  right ids in the right sequence; `clear` empties. A harness that measures the
  wrong mutation fast is worse than no harness.
- Both backends run the same assertions, which also proves vanilla and supersolid
  really are doing identical work — the precondition for §3.2's internal
  comparison meaning anything.
- The `Frames` column (§4.2) is asserted to be 1 in tests, so a spill is caught in
  CI rather than in a published table.

## 9. Open questions for plan time

These are empirical and get resolved by measurement during implementation, not by
argument now. Each has a stated fallback so none of them can block.

1. **Does an op settle in one `app.update()`?** §4.2 is designed to be correct
   either way, and the `Frames` column makes the answer public. If ops routinely
   spill, that is itself a finding worth writing up.
2. **Does the vanilla backend have a distinct system bucket for Boa?** In
   supersolid the JS re-render lives in `emit_bevy_inbox_system`; for vanilla the
   handler runs inside event dispatch. If the bucket is not separable by system
   name, `JS (Boa)` merges into its host bucket for that backend and the doc says
   so rather than inventing a split.
3. **Do 10k rows survive headless `bevy_ui`?** ~10k rows × per-row nodes is a
   large tree. If it does not hold, the 10k table is published for whatever
   backends do complete, with the failure documented — not quietly dropped.
4. **`superui_bench_support` naming** vs the roadmap's Tier-1 scope (§6.1).

## 10. Verification

- horde + citadel numbers unchanged across the extraction (§6.1) — the gate.
- `examples/rows/tests/` green on both backends.
- Pass A and Pass B both produce complete tables at 1k and 10k.
- Stage columns sum to `Total (traced)` with the residual visible in `Other`.
- `docs/BENCHMARKS.md` states commit, machine spec, and which tables are traced.
