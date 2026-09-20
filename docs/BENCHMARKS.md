# superui benchmarks

## JS engine

superui selects its JS engine by build target; there is no runtime choice and
no bundled interpreter:

| Target | Engine |
|---|---|
| native | V8, via [`deno_core`](https://crates.io/crates/deno_core) |
| `wasm32` (web) | the host browser's own JS engine |

Boa (the pure-Rust interpreter superui used on native before 2026-09-20) has
been removed from the workspace; `crates/superui_boa_engine` and
`crates/superui_boa_parser` no longer exist. Tables further down this document
that show a `JS (Boa)` column were captured before the removal and are kept as
a historical record — see the provenance note on each.

## Engine swap: Boa → V8

Native `ui_ms`/`p50_ms`, same harness and sweep on both engines, captured in
the same session on the same machine (`--seed 1 --frames 120 --warmup 30`,
debug build). Raw captures: `docs/superpowers/bench/raw/{horde,citadel,rows}-boa-baseline*.json`
(Boa) and `{horde,citadel,rows}-v8.json` (V8).

### horde (`enemy_cap` sweep, `--preset stress`)

| enemy_cap | Boa `ui_ms` | V8 `ui_ms` | speedup |
|---:|---:|---:|---:|
| 60  | 59.09  | 11.48 | 5.1× |
| 200 | 80.48  | 15.94 | 5.0× |
| 400 | 81.73  | 15.57 | 5.3× |

### citadel (`building_count` sweep)

| building_count | Boa `ui_ms` | V8 `ui_ms` | speedup |
|---:|---:|---:|---:|
| 60  | 51.34  | 22.57 | 2.3× |
| 120 | 81.38  | 40.80 | 2.0× |
| 240 | 135.39 | 76.27 | 1.8× |

### rows (1,000 rows, per-op `p50_ms`)

| op | Boa | V8 | speedup |
|---|---:|---:|---:|
| `create` | 2680.81 | 867.69 | 3.09× |
| `append1` | 424.29 | 348.09 | 1.22× |
| `append1k` | 3053.30 | 1201.58 | 2.54× |
| `insert1` | 448.30 | 345.77 | 1.30× |
| `insertEvery2nd` | 1756.40 | 770.52 | 2.28× |
| `updateText1` | 68.66 | 74.10 | 0.93× |
| `updateTextEvery2nd` | 223.34 | 88.61 | 2.52× |
| `updateColor1` | 68.10 | 74.43 | 0.91× |
| `updateColorEvery2nd` | 201.38 | 115.77 | 1.74× |
| `swap1` | 267.66 | 208.56 | 1.28× |
| `swapEvery2nd` | 301.61 | 159.06 | 1.90× |
| `remove1` | 277.49 | 217.69 | 1.27× |
| `removeEvery2nd` | 281.52 | 181.13 | 1.55× |
| `clear` | 260.67 | 102.96 | 2.53× |

`rows-bench` rebuilds a fresh `App` (and JS engine) per rep, so every number
above is dominated by isolate-bootstrap + full-bundle-eval cost, not
steady-state render/reconcile cost. The Boa-vs-V8 ratios are still fair —
both engines pay their own per-rep bootstrap — but do not read them as
steady-state reconcile speedups.

Most ops are faster on V8; `updateText1`/`updateColor1` (single-value touches,
mostly `bevy_ui`/layout cost rather than JS) are ~8% slower — not every op
improved, and this is reported as measured.

## V8, release build (current numbers)

The numbers a real (release) build ships at. No Boa release baseline exists
for horde/citadel, so this table is V8-only, not a before/after. Raw:
`{horde,citadel}-v8-release.json`, `rows-v8-release.json`.

| example | sweep value | `ui_ms` | fps |
|---|---:|---:|---:|
| horde   | enemy_cap 60  | 2.52  | 370.1 |
| horde   | enemy_cap 200 | 3.22  | 292.9 |
| horde   | enemy_cap 400 | 3.17  | 296.7 |
| citadel | building_count 60  | 6.01  | 162.4 |
| citadel | building_count 120 | 13.58 | 73.0 |
| citadel | building_count 240 | 26.51 | 37.5 |

rows (1,000 rows, per-op `p50_ms`, release):

| `create` | `append1` | `append1k` | `insert1` | `insertEvery2nd` | `updateText1` | `updateTextEvery2nd` | `updateColor1` | `updateColorEvery2nd` | `swap1` | `swapEvery2nd` | `remove1` | `removeEvery2nd` | `clear` |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 232.97 | 50.75 | 270.08 | 50.80 | 157.38 | 18.85 | 35.90 | 19.06 | 35.58 | 42.66 | 51.31 | 43.94 | 53.21 | 54.54 |

No bevy_react comparison is measured in this repo; none is claimed here.

## Rows — the krausest rows benchmark on superui

**Historical, pre-swap capture.** Everything below this point (Provenance
through Findings) was captured before the V8 swap, when Boa was superui's
only native JS engine — the `JS (Boa)` column in the traced tables names a
profiling stage bucket (`bucket_for` in `superui_bench_support`), not a claim
that Boa is still in use. It measures the `vanilla`-vs-`supersolid` framework
overhead question below, which is orthogonal to and unaffected by which JS
engine executes the `supersolid` side — see "Engine swap" above for the
Boa/V8 comparison.

The [js-framework-benchmark](https://github.com/krausest/js-framework-benchmark)
("krausest") **rows** workload, run on superui in two backends (`examples/rows`).
Unlike horde and citadel — bespoke workloads tuned to make one specific reconciler
change legible, meaningful only against their own history — this one runs a
standard workload whose op list and row structure follow a public spec rather than
superui's internals. The two backends are `vanilla` (superui driven as a browser:
raw DOM API calls, no reactive framework) and `supersolid` (the same app in TSX,
rows rendered by `<For>`); the delta between them is the framework's overhead over
raw DOM manipulation. See `examples/rows/benchmark.md` for how to run this
yourself.

### Provenance

| | |
|---|---|
| Commit | `47a1576c8ae256469a808575c8f28ca563f4fe1f` (traced 10k pair: `e502035`, see note below) |
| Capture window | 2026-08-08, 19:40–20:49 (local, CEST) — one continuous session, back to back |
| Load average during capture | 3.2–4.4 (16 logical CPUs), logged per capture, no spikes |
| CPU | AMD Ryzen 9 7940HS w/ Radeon 780M Graphics — 8 cores / 16 threads, up to 5.26 GHz |
| RAM | 60 GiB |
| GPU | AMD Radeon 780M (integrated) — not exercised; the harness is headless (`bevy_ui` layout only, no render backend) |
| OS | Linux |

**Read this before citing any number below.** The **untraced pass** is what gets
cited (`Total` p50, measured in a build with zero tracing instrumentation at
all). The **traced pass** exists to show where the time goes — but its
`Total (traced)` brackets a *wider window* than the untraced `Total`, not the
same window measured with tracing turned on: the op's reconciling frame (what
`Total` measures) plus the uncounted settle frame, plus a full-tree node count,
plus tracing's own recording cost, all bracketed together. Its stage columns sum
to their own `Total (traced)`, never to the untraced pass's `Total`. The two are
never blended, and this document never mixes a number from one table into the
other's row. **How much bigger that traced window reads than the untraced
`Total`** — most of which turns out to be the extra frame and the node-count
walk, not tracing itself — is tabulated explicitly in
["Traced-pass window vs. untraced `Total`"](#traced-pass-window-vs-untraced-total)
so that cost is visible rather than implied.

**Provenance note — two commits, one comparison.** Six of the eight raw files
(both untraced tables, both traced 1k tables) were captured at `47a1576`. The two
traced **10k** files were re-captured at `e502035`, a same-day follow-up commit
that fixes a driver bug where the traced pass's `create` op didn't scale with
`--rows` (see the fix's own commit message for detail; the two-line diff changed
only which button a `create` op click targets — nothing that touches `swap1`,
`clear`, or any other op's code path). The two 10k traced files were captured as
a matched pair, back to back, under load 3.37–4.03 — consistent with the rest of
the session's 3.2–4.4 — so the vanilla/supersolid comparison *within* the 10k
traced tables is sound even though their commit differs from the other six files.
The old (pre-fix) 10k traced captures are kept for reference under
`rows-results/pre-e502035-traced10k/` and are not cited anywhere in this document.

### How to reproduce

Untraced pass (the citable numbers), one invocation per backend × scale:

```bash
cargo run --release -p rows --features bench --bin rows-bench -- \
    --backend vanilla    --rows 1000  --reps 10 --warmup 3 --format json
cargo run --release -p rows --features bench --bin rows-bench -- \
    --backend vanilla    --rows 10000 --reps 10 --warmup 3 --format json
cargo run --release -p rows --features bench --bin rows-bench -- \
    --backend supersolid --rows 1000  --reps 10 --warmup 3 --format json
cargo run --release -p rows --features bench --bin rows-bench -- \
    --backend supersolid --rows 10000 --reps 10 --warmup 3 --format json
```

Traced pass (the per-stage breakdown) — **requires both `bevy/trace` AND
`bevy/debug`**. `bevy/trace` is what creates the per-system spans in the first
place; without `bevy/debug` their names are unresolvable, every span reads
`<Enable the debug feature to see the name>`, and the whole frame collapses into
the `Other` bucket — a report that prints cleanly but attributes nothing. All four
traced captures below contain zero placeholder names; that is what the two
features together look like when correct.

```bash
cargo run --release -p rows --features bench,bevy/trace,bevy/debug --bin rows-bench -- \
    --backend vanilla    --rows 1000  --reps 10 --warmup 2 --profile
cargo run --release -p rows --features bench,bevy/trace,bevy/debug --bin rows-bench -- \
    --backend vanilla    --rows 10000 --reps 10 --warmup 2 --profile
cargo run --release -p rows --features bench,bevy/trace,bevy/debug --bin rows-bench -- \
    --backend supersolid --rows 1000  --reps 10 --warmup 2 --profile
cargo run --release -p rows --features bench,bevy/trace,bevy/debug --bin rows-bench -- \
    --backend supersolid --rows 10000 --reps 10 --warmup 2 --profile
```

Raw output for every command above: `.superpowers/sdd/2026-08-07-rows-comparative-benchmark/rows-results/{untraced,traced}-{vanilla,supersolid}-{1000,10000}.{json,txt}`.
Every number in this document appears there.

### Untraced tables (the citable numbers)

`Rows` is the pre-op row count: 0 before `create`, 1000/10000 otherwise. `Nodes`
counts row-attributable DOM elements only — each
backend's boot chrome (buttons/containers, plus supersolid's `#root` mount
wrapper) is measured once at 0 rows and subtracted, so the column is comparable
across backends. `Frames` counts the frames that *reconciled* (applied at least
one DOM mutation) while settling the op — **not** the total number of
`app.update()` calls it took to reach quiescence, which is always one more: the
final frame that reconciles nothing is what quiescence detection watches for,
and it is deliberately excluded from both `Frames` and `Total` (see
`step_to_quiescence`, `examples/rows/src/bench/mod.rs`). `Frames` **reads 1 for
every op, on both backends, at both scales (56/56 cells)**, which is what makes
summing "the frames that did work" into a single `Total` legitimate rather than
silently spreading the true cost across an unstated number of frames.

#### 1,000 rows

| Op | Rows | Nodes | Frames | vanilla p50 (ms) | supersolid p50 (ms) |
|---|---:|---:|---:|---:|---:|
| `create` | 0 | 8000 | 1 | 287.81 | 423.66 |
| `append1` | 1000 | 8008 | 1 | 38.41 | 45.00 |
| `append1k` | 1000 | 16000 | 1 | 318.24 | 460.13 |
| `insert1` | 1000 | 8008 | 1 | 37.46 | 46.73 |
| `insertEvery2nd` | 1000 | 12000 | 1 | 163.82 | 210.42 |
| `updateText1` | 1000 | 8000 | 1 | 14.66 | 15.50 |
| `updateTextEvery2nd` | 1000 | 8000 | 1 | 57.18 | 53.57 |
| `updateColor1` | 1000 | 8000 | 1 | 14.16 | 15.79 |
| `updateColorEvery2nd` | 1000 | 8000 | 1 | 23.00 | 31.35 |
| `swap1` | 1000 | 8000 | 1 | 27.67 | 34.71 |
| `swapEvery2nd` | 1000 | 8000 | 1 | 29.69 | 36.16 |
| `remove1` | 1000 | 7992 | 1 | 29.30 | 36.00 |
| `removeEvery2nd` | 1000 | 4000 | 1 | 38.26 | 46.82 |
| `clear` | 1000 | 0 | 1 | 38.26 | 52.20 |

#### 10,000 rows

| Op | Rows | Nodes | Frames | vanilla p50 (ms) | supersolid p50 (ms) |
|---|---:|---:|---:|---:|---:|
| `create` | 0 | 80000 | 1 | 2809.18 | 4505.53 |
| `append1` | 10000 | 80008 | 1 | 385.25 | 485.80 |
| `append1k` | 10000 | 88000 | 1 | 655.76 | 841.79 |
| `insert1` | 10000 | 80008 | 1 | 394.60 | 487.83 |
| `insertEvery2nd` | 10000 | 120000 | 1 | 1768.97 | 2925.01 |
| `updateText1` | 10000 | 80000 | 1 | 173.63 | 175.42 |
| `updateTextEvery2nd` | 10000 | 80000 | 1 | 618.56 | 578.74 |
| `updateColor1` | 10000 | 80000 | 1 | 171.76 | 175.26 |
| `updateColorEvery2nd` | 10000 | 80000 | 1 | 297.60 | 355.93 |
| `swap1` | 10000 | 80000 | 1 | 300.28 | 356.85 |
| `swapEvery2nd` | 10000 | 80000 | 1 | 387.89 | 389.37 |
| `remove1` | 10000 | 79992 | 1 | 308.42 | 360.49 |
| `removeEvery2nd` | 10000 | 40000 | 1 | 462.28 | 543.07 |
| `clear` | 10000 | 0 | 1 | 550.41 | 647.90 |

`create` at 10,000 rows genuinely builds 10,000 rows (80,000 nodes) on both
backends — confirmed by the `Nodes` column matching every neighbouring row
(`80000`, `80008`, `88000`, …), not the 8,000 a 1,000-row `create` would produce.

### Stage-bucket mapping

The traced pass sums per-system busy time (`bevy/trace` root spans) into six
stages plus a residual, via `crates/superui_bench_support/src/profile.rs::bucket_for`:

| stage | system(s) matched |
|---|---|
| `JS (Boa)` | `emit_bevy_inbox_system` |
| `Reconcile` | any system whose path contains `reconcile` (`reconcile_system`) |
| `Flair cascade` | any system whose path contains `flair` (`bevy_flair_style::systems::*`) |
| `Taffy` | `bevy_ui::layout::ui_layout_system` |
| `Marshal` | `push_ui_frame`, `forward_event_observer`, `forward_toggle`, `drain_bevy_outbox`, `drain_dom_events_system`, keyboard-event systems |
| `bevy_ui other` | any system whose path contains `bevy_ui`, `bevy_text`, or `ui_stack` (text measurement/layout prep not captured above) |
| `Other` | everything else (never dropped — see spec §5.1) |

### Traced tables (the per-stage breakdown)

Every op below settled in the same 1 frame per rep as the untraced pass (per-rep
frame counts are not separately printed by `--profile`, but the driver code path
— `measure_op` → `step_to_quiescence` — is identical to the untraced pass's; see
`examples/rows/src/bench/profile.rs`). `Total (traced)` is **not** a per-frame
figure and **not** the untraced `Total`: it is a per-*operation* figure spanning
the click and both `app.update()` calls `step_to_quiescence` needs to reach
quiescence — the frame that reconciles (counted by `Frames` and by the untraced
`Total`) *and* the settle frame that reconciles nothing (counted by neither) —
plus `dom_node_count`'s full-tree `query_selector_all(document, "*")` walk and
the tracing layer's own recording cost. See
["Traced-pass window vs. untraced `Total`"](#traced-pass-window-vs-untraced-total)
for what that extra window actually costs, op by op. Stage columns are ms/op and sum to
`Total (traced)` (modulo ~1–2 ms of
schedule-runner/registry overhead outside the leaf systems). `JS (Boa)` is ~0.00
ms for every op on **both** backends — see the caveat below explaining where
that cost actually went before concluding either backend's JS is free.

#### 1,000 rows — vanilla

| Op | Total (traced) ms | JS (Boa) | Reconcile | Flair cascade | Taffy | Marshal | bevy_ui other | Other |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `create` | 350.7 | 0.001 | 32.19 | 80.32 | 45.41 | 21.95 | 172.70 | 0.18 |
| `append1` | 49.7 | 0.001 | 4.80 | 26.77 | 13.34 | 0.12 | 2.57 | 0.23 |
| `append1k` | 395.2 | 0.001 | 39.06 | 93.05 | 68.24 | 23.57 | 175.20 | 0.28 |
| `insert1` | 51.1 | 0.001 | 5.34 | 26.98 | 13.28 | 0.39 | 2.98 | 0.23 |
| `insertEvery2nd` | 219.7 | 0.001 | 22.78 | 58.52 | 38.95 | 10.45 | 89.82 | 0.28 |
| `updateText1` | 26.6 | 0.002 | 5.94 | 1.89 | 13.82 | 0.08 | 2.65 | 0.23 |
| `updateTextEvery2nd` | 99.1 | 0.001 | 11.41 | 6.15 | 23.94 | 2.46 | 53.82 | 0.26 |
| `updateColor1` | 25.4 | 0.001 | 5.26 | 1.81 | 13.29 | 0.05 | 2.58 | 0.23 |
| `updateColorEvery2nd` | 65.5 | 0.002 | 5.20 | 6.83 | 23.43 | 0.51 | 27.30 | 0.26 |
| `swap1` | 36.7 | 0.001 | 4.97 | 17.07 | 10.19 | 0.05 | 1.99 | 0.25 |
| `swapEvery2nd` | 38.1 | 0.001 | 4.84 | 16.53 | 10.17 | 1.82 | 2.16 | 0.25 |
| `remove1` | 37.0 | 0.001 | 4.71 | 16.40 | 10.86 | 0.43 | 2.05 | 0.23 |
| `removeEvery2nd` | 45.3 | 0.001 | 23.45 | 10.40 | 6.19 | 0.83 | 2.21 | 0.25 |
| `clear` | 44.4 | 0.002 | 37.38 | 0.64 | 2.80 | 0.94 | 1.83 | 0.21 |

#### 1,000 rows — supersolid

| Op | Total (traced) ms | JS (Boa) | Reconcile | Flair cascade | Taffy | Marshal | bevy_ui other | Other |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `create` | 509.2 | 0.001 | 33.33 | 75.30 | 43.10 | 188.13 | 170.93 | 0.17 |
| `append1` | 71.8 | 0.001 | 7.05 | 27.79 | 13.20 | 18.55 | 2.85 | 0.23 |
| `append1k` | 597.2 | 0.001 | 42.75 | 95.42 | 68.57 | 217.42 | 176.61 | 0.29 |
| `insert1` | 60.5 | 0.001 | 5.94 | 29.27 | 13.91 | 6.05 | 2.90 | 0.24 |
| `insertEvery2nd` | 302.4 | 0.004 | 25.32 | 61.36 | 38.70 | 88.45 | 88.84 | 0.27 |
| `updateText1` | 22.8 | 0.001 | 5.76 | 1.68 | 10.50 | 0.16 | 2.29 | 0.23 |
| `updateTextEvery2nd` | 60.9 | 0.001 | 6.11 | 1.74 | 14.54 | 9.08 | 27.30 | 0.25 |
| `updateColor1` | 25.8 | 0.001 | 5.81 | 1.82 | 13.00 | 0.12 | 2.64 | 0.23 |
| `updateColorEvery2nd` | 75.1 | 0.001 | 6.43 | 6.85 | 24.83 | 6.42 | 28.04 | 0.24 |
| `swap1` | 44.3 | 0.001 | 6.17 | 17.32 | 10.32 | 5.64 | 2.21 | 0.24 |
| `swapEvery2nd` | 45.1 | 0.001 | 5.94 | 16.20 | 10.24 | 7.26 | 2.58 | 0.24 |
| `remove1` | 44.3 | 0.001 | 5.84 | 17.28 | 10.27 | 5.96 | 2.31 | 0.23 |
| `removeEvery2nd` | 53.4 | 0.001 | 22.88 | 9.86 | 6.52 | 9.70 | 2.17 | 0.23 |
| `clear` | 85.4 | 0.001 | 38.52 | 0.70 | 2.71 | 40.48 | 1.77 | 0.20 |

#### 10,000 rows — vanilla

Captured at `e502035` (the `button_for` fix), not `47a1576` — see the provenance
note above.

| Op | Total (traced) ms | JS (Boa) | Reconcile | Flair cascade | Taffy | Marshal | bevy_ui other | Other |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `create` | 3680.5 | 0.001 | 334.19 | 768.76 | 652.13 | 253.03 | 1726.37 | 0.35 |
| `append1` | 589.3 | 0.001 | 64.87 | 257.46 | 216.80 | 0.12 | 40.71 | 0.69 |
| `append1k` | 918.9 | 0.001 | 102.58 | 313.31 | 292.99 | 17.05 | 213.17 | 0.79 |
| `insert1` | 602.7 | 0.001 | 71.86 | 257.04 | 218.04 | 2.77 | 42.62 | 0.73 |
| `insertEvery2nd` | 2403.8 | 0.001 | 253.38 | 563.53 | 602.16 | 114.58 | 923.59 | 0.84 |
| `updateText1` | 345.1 | 0.001 | 64.24 | 16.89 | 214.00 | 0.08 | 40.52 | 0.70 |
| `updateTextEvery2nd` | 1116.3 | 0.001 | 137.45 | 57.49 | 384.39 | 24.87 | 529.45 | 0.73 |
| `updateColor1` | 350.3 | 0.001 | 68.47 | 16.73 | 214.30 | 0.05 | 40.90 | 0.72 |
| `updateColorEvery2nd` | 795.1 | 0.001 | 69.54 | 60.73 | 379.65 | 3.79 | 281.45 | 0.72 |
| `swap1` | 433.9 | 0.001 | 67.39 | 147.62 | 167.93 | 0.07 | 40.57 | 0.78 |
| `swapEvery2nd` | 512.7 | 0.001 | 69.46 | 148.33 | 169.59 | 73.20 | 41.18 | 0.74 |
| `remove1` | 439.0 | 0.001 | 67.04 | 150.80 | 168.13 | 2.60 | 40.21 | 0.75 |
| `removeEvery2nd` | 560.7 | 0.001 | 289.70 | 102.98 | 110.28 | 18.94 | 29.62 | 0.59 |
| `clear` | 579.5 | 0.001 | 498.73 | 3.51 | 43.88 | 20.25 | 15.89 | 0.46 |

#### 10,000 rows — supersolid

Also captured at `e502035`, as a matched pair with the vanilla table above.

| Op | Total (traced) ms | JS (Boa) | Reconcile | Flair cascade | Taffy | Marshal | bevy_ui other | Other |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `create` | 5675.4 | 0.001 | 349.54 | 734.30 | 650.68 | 2285.88 | 1707.48 | 0.41 |
| `append1` | 716.8 | 0.001 | 98.20 | 272.54 | 225.48 | 59.91 | 47.13 | 0.73 |
| `append1k` | 1152.4 | 0.002 | 135.81 | 336.55 | 301.67 | 179.06 | 219.55 | 0.75 |
| `insert1` | 711.2 | 0.001 | 89.61 | 275.25 | 225.49 | 60.65 | 46.28 | 0.73 |
| `insertEvery2nd` | 3756.3 | 0.001 | 295.10 | 598.68 | 613.99 | 1370.70 | 932.76 | 0.88 |
| `updateText1` | 343.0 | 0.001 | 93.19 | 15.62 | 174.51 | 0.30 | 45.16 | 0.72 |
| `updateTextEvery2nd` | 754.2 | 0.001 | 93.33 | 15.84 | 258.67 | 96.96 | 287.94 | 0.74 |
| `updateColor1` | 395.6 | 0.001 | 94.81 | 16.76 | 223.95 | 0.25 | 45.52 | 0.75 |
| `updateColorEvery2nd` | 910.3 | 0.001 | 98.27 | 61.52 | 386.75 | 72.33 | 288.47 | 0.73 |
| `swap1` | 546.7 | 0.001 | 93.22 | 157.08 | 175.77 | 60.66 | 45.25 | 0.74 |
| `swapEvery2nd` | 571.7 | 0.001 | 92.31 | 152.43 | 176.69 | 89.74 | 45.01 | 0.73 |
| `remove1` | 536.9 | 0.001 | 90.88 | 152.61 | 175.50 | 61.30 | 43.04 | 0.72 |
| `removeEvery2nd` | 691.2 | 0.001 | 307.33 | 105.19 | 111.92 | 119.75 | 32.70 | 0.58 |
| `clear` | 726.5 | 0.001 | 513.56 | 3.04 | 43.67 | 150.69 | 16.01 | 0.45 |

`create` now scales as it should: 3,680.5 ms at 10k vs 350.7 ms at 1k on vanilla
(10.5×), 5,675.4 ms vs 509.2 ms on supersolid (11.1×) — in line with the untraced
pass's own 1k→10k `create` ratios (9.8× vanilla, 10.6× supersolid) rather than the
pre-fix traced capture's 1.007× (353.3 ms vs 350.7 ms, i.e. no scaling at all).

### Traced-pass window vs. untraced `Total`

How much bigger `Total (traced)` reads than the untraced pass's `Total`, for the
identical op, same backend, same scale. **This table does not isolate tracing
overhead** — the two passes bracket different windows, not the same window
measured with and without instrumentation. The untraced `Total` sums only the
frame(s) that reconciled (see `Frames` above). `Total (traced)` brackets the
whole `measure_op` call: the click, every `app.update()` needed to reach
quiescence — including the final settle frame that reconciles nothing and so is
excluded from `Total`/`Frames` — and `dom_node_count`'s full-tree
`query_selector_all(document, "*")` walk (a scan over every node currently in
the DOM: 80,000+ at the 10k scale). Tracing's own recording cost is only one of
those three components. Measured directly with tracing removed from the build,
roughly 80% of the delta below persists (e.g. `updateColor1` at 10k reads 1.69×
with tracing off vs. 2.04× in the table below) — so most of what this table
shows is the settle frame and the node-count walk, not tracing. Never use a
traced `Total (traced)` figure where the untraced `Total` belongs, and never cite
this Δ column as "tracing overhead."

#### 1,000 rows

| Op | vanilla `Total` | vanilla `Total (traced)` | Δ | supersolid `Total` | supersolid `Total (traced)` | Δ |
|---|---:|---:|---:|---:|---:|---:|
| `create` | 287.81 | 350.7 | +62.9 (1.22×) | 423.66 | 509.2 | +85.6 (1.20×) |
| `append1` | 38.41 | 49.7 | +11.3 (1.29×) | 45.00 | 71.8 | +26.8 (1.60×) |
| `append1k` | 318.24 | 395.2 | +77.0 (1.24×) | 460.13 | 597.2 | +137.1 (1.30×) |
| `insert1` | 37.46 | 51.1 | +13.7 (1.36×) | 46.73 | 60.5 | +13.8 (1.29×) |
| `insertEvery2nd` | 163.82 | 219.7 | +55.9 (1.34×) | 210.42 | 302.4 | +91.9 (1.44×) |
| `updateText1` | 14.66 | 26.6 | +11.9 (1.81×) | 15.50 | 22.8 | +7.3 (1.47×) |
| `updateTextEvery2nd` | 57.18 | 99.1 | +41.9 (1.73×) | 53.57 | 60.9 | +7.3 (1.14×) |
| `updateColor1` | 14.16 | 25.4 | +11.2 (1.79×) | 15.79 | 25.8 | +10.0 (1.63×) |
| `updateColorEvery2nd` | 23.00 | 65.5 | +42.5 (2.85×) | 31.35 | 75.1 | +43.7 (2.39×) |
| `swap1` | 27.67 | 36.7 | +9.0 (1.33×) | 34.71 | 44.3 | +9.6 (1.28×) |
| `swapEvery2nd` | 29.69 | 38.1 | +8.4 (1.28×) | 36.16 | 45.1 | +8.9 (1.25×) |
| `remove1` | 29.30 | 37.0 | +7.7 (1.26×) | 36.00 | 44.3 | +8.3 (1.23×) |
| `removeEvery2nd` | 38.26 | 45.3 | +7.0 (1.18×) | 46.82 | 53.4 | +6.6 (1.14×) |
| `clear` | 38.26 | 44.4 | +6.2 (1.16×) | 52.20 | 85.4 | +33.2 (1.64×) |

#### 10,000 rows

Every `Total (traced)` column in this table is the `e502035` capture (see the
provenance note); every `Total` column is still the `47a1576` untraced capture.
For `create` this is not just comparable but necessary — the fix changed only
which button the traced pass's `create` clicks, so its untraced figure was
already correct and unaffected. For the other 13 ops, both captures measure the
same unchanged code path, so the small session-to-session shifts visible here
(e.g. `append1` traced 611.8 ms in the pre-fix capture vs. 589.3 ms here) are
ordinary rep-to-rep/load variance, not a code change — well within the ~10%
drift this machine has already been shown to produce across sessions.

| Op | vanilla `Total` | vanilla `Total (traced)` | Δ | supersolid `Total` | supersolid `Total (traced)` | Δ |
|---|---:|---:|---:|---:|---:|---:|
| `create` | 2809.18 | 3680.5 | +871.4 (1.31×) | 4505.53 | 5675.4 | +1169.8 (1.26×) |
| `append1` | 385.25 | 589.3 | +204.1 (1.53×) | 485.80 | 716.8 | +231.0 (1.48×) |
| `append1k` | 655.76 | 918.9 | +263.1 (1.40×) | 841.79 | 1152.4 | +310.6 (1.37×) |
| `insert1` | 394.60 | 602.7 | +208.1 (1.53×) | 487.83 | 711.2 | +223.4 (1.46×) |
| `insertEvery2nd` | 1768.97 | 2403.8 | +634.8 (1.36×) | 2925.01 | 3756.3 | +831.3 (1.28×) |
| `updateText1` | 173.63 | 345.1 | +171.4 (1.99×) | 175.42 | 343.0 | +167.6 (1.96×) |
| `updateTextEvery2nd` | 618.56 | 1116.3 | +497.7 (1.80×) | 578.74 | 754.2 | +175.4 (1.30×) |
| `updateColor1` | 171.76 | 350.3 | +178.5 (2.04×) | 175.26 | 395.6 | +220.3 (2.26×) |
| `updateColorEvery2nd` | 297.60 | 795.1 | +497.5 (2.67×) | 355.93 | 910.3 | +554.4 (2.56×) |
| `swap1` | 300.28 | 433.9 | +133.7 (1.45×) | 356.85 | 546.7 | +189.9 (1.53×) |
| `swapEvery2nd` | 387.89 | 512.7 | +124.8 (1.32×) | 389.37 | 571.7 | +182.3 (1.47×) |
| `remove1` | 308.42 | 439.0 | +130.6 (1.42×) | 360.49 | 536.9 | +176.4 (1.49×) |
| `removeEvery2nd` | 462.28 | 560.7 | +98.4 (1.21×) | 543.07 | 691.2 | +148.1 (1.27×) |
| `clear` | 550.41 | 579.5 | +29.0 (1.05×) | 647.90 | 726.5 | +78.6 (1.12×) |

The Δ ratio is **~1.1×–2.0×** for most ops; `updateColorEvery2nd` is consistently
the highest outlier (2.4×–2.85× across both scales and backends), with
`updateColor1` also crossing 2× at 10k (2.04×/2.26×). This does **not** track
tracing cost — it tracks how expensive the op's uncounted settle frame plus the
full-tree `dom_node_count` walk are *relative to the op's own untraced cost*.
`clear` sits near 1.0 (1.05×–1.16×) because after it runs the tree is empty: the
settle frame has nothing to reconcile and the node-count walk has nothing to
scan. `updateColor1`/`updateColorEvery2nd` leave the full 8,000–80,000-node tree
in place, so the same fixed settle-frame-plus-node-count cost lands on top of a
comparatively cheap single/half-table value touch and dominates the ratio. This
window difference (not tracing) is also why the untraced pass, not the traced
one, is what gets cited.

### Caveats

- **`<Keyed>` is deliberately absent.** superui's `<Keyed>` control flow (used by
  horde's overlays) builds each row once and never reorders its DOM node —
  survivors keep their existing node, new rows append
  (`crates/supersolid_runtime/src/render.js:308`). That is correct when order is
  meaningless but would render `swap1` and `insertEvery2nd` in the wrong DOM order
  here. `<For>` is used instead: it keys rows by item identity and reorders DOM
  nodes to match, which is correct for this workload.
- **`<div>` substitutes for `<table>`/`<tr>`/`<td>`.** superui has no table
  semantics — `<div>` and a real `<table>` both lower to `bevy_ui` flexbox — so the
  substitution costs nothing but honesty.
- **8 elements + 2 text nodes per row**, matching the reference implementation:
  the row `<div>`, four column `<div>`s, the label `<a>`, the remove `<a>`, and a
  nested glyph `<span>` inside it (no text, but still the 8th element the workload
  spec fixes); text nodes are the id cell and the label. `Nodes` counts these
  row-attributable elements only — each backend's own boot chrome is measured
  once at 0 rows and subtracted, since supersolid mounts inside a `#root` wrapper
  that vanilla has no equivalent of. The arithmetic is directly checkable in the
  untraced tables above: every `Nodes` value is `(row count) × 8`, e.g. 10,000
  rows → 80,000, 1,500 rows (after `insertEvery2nd`) → 12,000.
- **The `Every2nd` rule: every `*Every2nd` op performs exactly N/2 operations at N
  rows.** `updateTextEvery2nd`/`updateColorEvery2nd` touch N/2 rows;
  `removeEvery2nd` removes N/2, leaving N/2; `insertEvery2nd` inserts N/2, leaving
  3N/2 (not 2N); `swapEvery2nd` performs N/2 disjoint adjacent-pair swaps. This is
  stated explicitly because a reader cannot recover the stride from the op names
  alone. It is directly verifiable in the
  `Nodes` column above: `removeEvery2nd` at 10,000 rows leaves `40000` nodes
  (5,000 rows × 8), `insertEvery2nd` leaves `120000` (15,000 rows × 8).
- **The vanilla fixture uses `tbody.removeChild(node)`, not the natural
  `node.remove()`.** `Element.prototype.remove()` is not bound in
  `superui_api` — only `appendChild`, `removeChild`, `insertBefore`, and
  `replaceChild` are. Calling the unbound method would throw, and the
  event-dispatch path currently discards listener exceptions, so the op would
  silently do nothing. `removeChild` performs the same single detachment (every
  row is a direct child of `tbody`, so no parent lookup is skipped) — this is a
  divergence from how the app would naturally be written, not a difference in
  what gets measured.
- **`JS (Boa)` reads ~0.00 ms for every op, on *both* backends** — this is not
  "the JS did no work" on either side, and it is not only a vanilla artifact.
  Both backends' click-triggered JS runs synchronously inside
  `drain_dom_events_system`, which the stage mapping assigns to `Marshal`, not to
  `JS (Boa)` (`emit_bevy_inbox_system`, the system the stage mapping was designed
  around, is never entered on this code path in any of the 56 measured
  op×backend×scale cells). Concretely, on vanilla's 1k `create`,
  `drain_dom_events_system` costs 21.945 ms/frame — visible as `Marshal`'s 21.948
  ms in that row. On supersolid's 1k `create` the *same* system costs 188.131
  ms/frame (`Marshal`'s 188.13 ms) — ~8.6× vanilla's, because supersolid's
  click handler does substantially more synchronous work in that call: building
  1,000 signal-backed row objects and `<For>`'s initial list, not just issuing
  DOM-API calls. So `Marshal` is where **both** backends' JS execution cost
  actually lives for this workload, and its size differs by backend and op
  depending on how much synchronous JS the click handler does — a reader looking
  only at `JS (Boa)` would wrongly conclude either backend's JS is free.
- **Traced pass measures only the operation.** Each rep needs an untimed
  precondition reset first (clear + rebuild back to the op's starting row count).
  That reset runs with tracing recording switched off; only the click and its
  settle-to-quiescence frame are recorded. Folding the reset into the recording
  window was tried and rejected during development — it buried small ops
  (`swap1`, 2 rows touched) under the reset's cost until they read identically to
  `create` (up to 10,000 rows built).
- **`p95` equals `p99` in every cell of every untraced table.** Both are
  nearest-rank statistics over 10 reps: nearest-rank maps rank ⌈0.95×10⌉=10 and
  rank ⌈0.99×10⌉=10 to the same sample — the maximum of the 10. So the two
  columns carry only one sample's worth of tail information here, not two
  independent tail estimates; do not read a gap or an agreement between them as
  meaningful at this rep count.
- **The untraced pass rebuilds the app fresh for every rep; the traced pass
  reuses one app across all reps for an op.** `run_ops` (`report.rs:42`) builds
  a new `App` inside its per-rep loop; `run_profile_driven` (`profile.rs:52`)
  builds the app once, before the loop, and resets it between reps via
  `precondition_reset` (clear + rebuild). Build and reset are both untimed in
  either pass, so this does not directly add wall time to either `Total` — but
  it means the untraced pass's timed click-and-settle window always runs
  against a freshly constructed `App` (containers, allocations, and asset state
  starting cold), while the traced pass's timed window runs against an `App`
  whose containers have already grown to size across warmup and every prior
  rep of that op. That is a second, independent difference between the two
  passes' windows, on top of the settle-frame/node-count difference described
  in ["Traced-pass window vs. untraced `Total`"](#traced-pass-window-vs-untraced-total)
  above; this document has not measured how large its effect is.

### Findings

**`updateTextEvery2nd` is supersolid's one clean, reproducible win** — faster
than vanilla at both scales: 53.57 ms vs. 57.18 ms at 1k (0.937×) and 578.74 ms
vs. 618.56 ms at 10k (0.936×). The ratio holding almost exactly across a 10×
scale change is what makes this a real result rather than noise.

**`updateText1` and `updateColor1` are within 1–2% at 10,000 rows** —
effectively parity: `updateText1` 175.42 vs. 173.63 ms (1.010×), `updateColor1`
175.26 vs. 171.76 ms (1.020×). `swapEvery2nd` at 10k is also parity, 389.37 vs.
387.89 ms (1.004×). All three are single-value-touch ops where the cost is
dominated by `Taffy`/`Reconcile` walking the same 80,000-node tree on both
backends (see the traced tables — `updateText1`/`updateColor1` are >50% `Taffy`
at 1k) — supersolid's fixed per-op reactive overhead becomes a rounding error
against that shared cost at 10k.

**`create` is supersolid's largest overhead in absolute milliseconds, and it
*grows* with scale**: 1.47× at 1k (423.66 vs. 287.81 ms), 1.60× at 10k (4505.53
vs. 2809.18 ms) — a 1,696.35 ms gap at 10k, the largest of any op in raw
milliseconds. As a *ratio* the largest is `insertEvery2nd` at 10k, 1.65×
(2925.01 vs. 1768.97 ms), edging out `create`'s 1.60× — `create` is larger in ms
only because it moves far more absolute data at the same relative overhead. Most
other ops' relative overhead *shrinks* toward parity as scale grows (`clear`
1.36× → 1.18×, `updateColorEvery2nd` 1.36× → 1.20×, `remove1` 1.23× → 1.17×) —
`create` and `insertEvery2nd` (1.28× → 1.65×) are the two exceptions, both ops where
supersolid must construct a proportionally large number of *new* signal-backed
row objects rather than patch existing ones. The traced tables corroborate this
directly: `create`'s `Marshal` bucket (the JS execution itself, per the caveat
above) is 188.13 ms for supersolid vs. 21.95 ms for vanilla at 1k (8.6×), and
*widens* to 2285.88 ms vs. 253.03 ms at 10k (9.0×) — even as the untraced total
itself grows ~10× (9.8× vanilla, 10.6× supersolid) between the two scales. So the
*reactive construction* cost is not merely failing to amortize as the table
grows — it is getting relatively more expensive against vanilla's equivalent
DOM-building work, not less.

**supersolid loses every row published here** — vanilla is faster than
supersolid on every op at every scale except `updateTextEvery2nd`, ranging from
near-parity (`swapEvery2nd`/`updateText1`/`updateColor1` at 10k, ~1.0×) up to
supersolid taking ~1.65× as long. The largest gap of any op is
`insertEvery2nd` at 10k (1.65×) — see above for why that, not `create`, is the
largest as a ratio. (Figures in the 2–3× range appear elsewhere in this
document, but only in the ["Traced-pass window vs. untraced `Total`"](#traced-pass-window-vs-untraced-total)
table — that Δ compares the traced pass's wider window against the untraced
`Total` for the *same* backend, and must never be read as a vanilla-vs-supersolid
ratio.) This is published as measured: a benchmark that only publishes the rows
it wins is not a benchmark.

**Stage dominance varies sharply by op, which is the traced pass's actual
value** (all figures 1k, vanilla — see the tables above for supersolid and 10k):
`clear` is 84% `Reconcile` (tearing down 1,000/8,000 nodes in one pass);
`updateText1`/`updateColor1` are ~52% `Taffy` (a single value change still walks
layout for the whole table); `swap1`/`remove1` are `Flair`-dominated (44–47%,
a style recompute across the table for one moved/removed row); `create`/`append1k`
are `bevy_ui other`-dominated (~44–49%, mostly `text_system` building
thousands of text nodes) with `Flair` a distant second (~23%). No single stage
explains the whole workload — which is exactly why a single `Total` column hides
more than it shows, and why the traced pass exists at all.

**Machine drift caveat.** This project has already measured ~10% timing drift on
this same machine across a 5-hour gap with *unchanged* code (see Task 12's
drift experiment). Six of the eight files here were captured in one unbroken
69-minute session; the traced 10k pair was captured separately (see the
provenance note) but, critically, **as a matched pair against each other** under
equivalent load (3.37–4.03 vs. the main session's 3.2–4.4) — which is what makes
the vanilla/supersolid comparison *within* that 10k traced table sound, even
though it is not drift-controlled against the other six files.
