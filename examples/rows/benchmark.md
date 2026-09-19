# Rows macro-benchmark

The [js-framework-benchmark](https://github.com/krausest/js-framework-benchmark)
("krausest") **rows** workload, run on superui in two backends. Unlike horde and
citadel — bespoke workloads tuned to make one specific reconciler change legible,
meaningful only against their own history — rows runs a standard workload whose op
list and row structure follow a public spec rather than superui's internals.

Published results and methodology live in
[`docs/BENCHMARKS.md`](../../docs/BENCHMARKS.md). This file covers how to run it.

## Backends

Both backends drive byte-identical markup and share one stylesheet, so the delta
between them **is** the framework's overhead over raw DOM manipulation.

| backend | what it measures |
|---|---|
| `vanilla` | superui as a browser: `document.createElement`/`appendChild`/`insertBefore`/`textContent`/`classList` → `superui_dom` → reconcile → `bevy_ui`. No reactive framework in the path. |
| `supersolid` | the framework: same app in TSX, rows rendered by `<For>`. |

## Row markup

The js-framework-benchmark row, with `<div>` substituted wherever
`<table>`/`<tr>`/`<td>` would appear — superui has no table semantics, `<div>` and a
real `<table>` both lower to `bevy_ui` flexbox, so the substitution costs nothing
but honesty.

**8 elements + 2 text nodes per row**, matching the reference implementation
exactly: the row `<div>`, four column `<div>`s, the label `<a>`, the remove `<a>`,
and a nested glyph `<span>` inside it (carries no text but is still the 8th
element — dropping it would make the `Nodes` column incomparable with every other
published number on this workload); text nodes are the id cell and the label. The
published `Nodes` column counts these row-attributable elements only — each
backend's own boot chrome (the static DOM present at 0 rows: buttons, containers,
and for supersolid the `#root` mount wrapper vanilla has no equivalent of) is
measured once and subtracted, so the column reflects only what the workload itself
produced.

## Scale-dependent ops

`create` is the only op whose size follows the `--rows` scale — it builds 1,000
rows in the 1k table and 10,000 in the 10k table, so the two scales exercise
genuinely different amounts of work. Every other op is fixed-size by definition (`append1k` appends
1,000 rows at *either* scale) or derives its size from the current table's row
count (the `*Every2nd` family). The fixture exposes the 10k build as a separate
`create10k` button; `examples/rows/src/bench/mod.rs::button_for(op, rows)` is
the single place that maps the published op name `create` to the right button
for the scale in play, and every caller that fires an op by name goes through
it — a regression test there (`button_for_tests`) pins that only `create` at
10k remaps and every other op/scale combination is the identity, specifically
so a future scale-dependent op can't reintroduce a mismatch by adding a second
call site that forgets to route through it (which is exactly how one of the
tables in `docs/BENCHMARKS.md` momentarily went wrong — see that document's
provenance note).

## `<For>`, not `<Keyed>`

superui's `<Keyed>` control flow builds each row once and never reorders its DOM
node — survivors keep their existing node, new rows append. That is correct for
horde's absolutely-positioned overlays, where order is meaningless, but **wrong**
here: `swap1` and `insertEvery2nd` would render rows in the wrong order under
`<Keyed>` (`crates/supersolid_runtime/src/render.js:308`). `<For>` is used instead
— it keys rows by item identity and reorders DOM nodes to match, which is correct
for this workload. `<Index>`, the non-keyed analogue, is out of scope —
js-framework-benchmark maintains separate keyed/non-keyed leaderboards precisely
because the two are not comparable.

## Run

```bash
cargo run --release -p rows --features bench --bin rows-bench -- \
    --backend vanilla --rows 1000 --reps 10 --warmup 3 --format json
```

Flags: `--backend vanilla|supersolid` (required), `--rows 1000|10000`, `--reps N`,
`--warmup N` (discarded before timing), `--format table|json`, `--profile` (see
below). The published tables in `docs/BENCHMARKS.md` used `--reps 10 --warmup 3`
at both scales (10k rows is slow enough — several seconds per rep once
`create`/`append1k`/`insertEvery2nd` are counted — that this trades a wider
confidence interval for a session that finishes in one sitting; the design
spec's default of 20 reps at 1k is fine too if you have the time).

**The workload is fixed-seed and therefore deterministic by construction.** Both
JS fixtures hardcode `srand(1)` / `_seed = 1` for row-label generation; there is
no `--seed` flag on `rows-bench` (unlike horde/citadel) because the row data
never varies between runs. Do not expect — and do not need — a different seed
to reproduce a result.

**Never compare a debug build's numbers against a release build's, and never
compare either against the traced pass below** — an unoptimized or instrumented
binary is not the same measurement as the citable release/no-tracing numbers.

## Windowed example

    ROWS_BACKEND=vanilla cargo run -p rows        # or ROWS_BACKEND=supersolid

No load crank (unlike citadel) — the workload is discrete per-operation clicks
(`create`, `swap1`, …), not a steady-state frame cost to dial.

## Reading the report

Each op (`create`, `append1`, …, `clear` — the full krausest op list, in order) is
measured trigger→settled: precondition the table to the op's expected pre-row
count (untimed), fire a real click through the event system
(`superui_bridge::click_effect`), then step `app.update()` until a frame applies
zero DOM mutations. `Total` is the sum of the frames that did work; `Frames` is how
many frames that took (expected 1 — published so a spill is visible rather than
silently absorbed into the total). `p50` is the headline; `p95`/`p99` are always in
the JSON output.

## Two passes, never blended

Instrumentation overhead must never contaminate the headline number, so rows is
measured in two separate runs that are never merged into one table:

- **untraced pass** (this section's command, no `bevy/trace`) — `Total` p50,
  `Rows`, `Nodes`, `Frames`. Zero tracing overhead. **These are the numbers to
  cite.**
- **traced pass** (`--profile`, requires `bevy/trace` *and* `bevy/debug`) — splits
  `Total` into `JS (Boa)` / `Reconcile` / `Flair cascade` / `Taffy` / `Marshal` /
  `bevy_ui other` / `Other`, so it is visible where an op's time actually goes. Its
  `Total (traced)` includes the cost of tracing itself, so it reads higher than the
  untraced pass's `Total` for the same op — the stage columns sum to *that* traced
  total, not to the untraced number.

```bash
cargo run --release -p rows --features bench,bevy/trace,bevy/debug --bin rows-bench -- \
    --backend vanilla --rows 1000 --reps 10 --warmup 2 --profile
```

(the published traced tables used exactly this — `--reps 10 --warmup 2` at both
scales.)

`bevy/trace` alone is not enough: it creates the per-system spans, but without
`bevy/debug` their names are unresolvable and every one collapses into the `Other`
bucket — a report that looks complete but attributes nothing. Both features are
required together.

See `docs/BENCHMARKS.md` for the published tables, the per-op overhead delta
between the two passes, and the full list of caveats (the `removeChild`
workaround, the `Every2nd` N/2 rule, why `JS (Boa)` reads ~0% on **both**
backends, and more).
