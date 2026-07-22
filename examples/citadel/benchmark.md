# Citadel macro-benchmark

Headless, deterministic benchmark of the citadel strategy-HUD example. Measures
per-frame cost and attributes it across UI backends so the impact of reconcile
optimizations can be predicted and proven.

## Windowed example + load crank

    cargo run -p citadel                # add --features debug-ui for an FPS overlay

The windowed app starts at a comfortable **40 buildings** (~900 nodes) so it renders
smoothly, and you can dial the load live to find the frame-rate breaking point:

- `[` or `-` — fewer buildings (lighter, higher FPS)
- `]` or `=` — more buildings (heavier, stress)

Range is 8..400 in steps of 16; the current count is logged on each change. This is
the same knob the headless benchmark drives with `--building-count`/`--sweep` — the
windowed default is lighter only so the example is pleasant out of the box; the
benchmark's own default stays at 120.

## Load profile

Citadel is a **dense, mostly-static grand-strategy empire HUD** — the opposite of
horde's swarm overlay. Most of the tree (~2 600+ nodes at 120 buildings) never
changes between frames: building cards, tech items, and unit roster rows hold the
same text, class, and state as the previous frame. Only a tiny live region updates:
the mission clock (every frame), resource values + rates (every frame), and ≤ 8
in-progress build bars whose `width` binding updates as construction advances.

This makes it a direct stress-test of the **reconciler's per-frame write
discipline** on static UIs. It turns out the entire flair cascade *and* taffy
layout re-run every frame even though nothing changed — but (see "Findings" below)
the reconciler's class/attribute equality guard alone does **not** stop them. The
real culprit is `replace_children` being called on every parent every frame;
skipping it when the child list is unchanged cuts the frame ~1.5× (taffy 24 → 9 ms)
with byte-identical layout and styling.

Fine-grained `<Keyed>` reactivity (per-field signals per entity row) keeps Boa's
re-render cheap — only the bound values that changed re-run their closures. The
bottleneck shifts from JS re-render to the **reconcile walk + flair cascade + taffy
layout** that the guard eliminates.

## Backends

Two backends only (no native UI in citadel):

- `null` — sim + snapshot + JSON marshal, no UI. This is the shared floor every
  backend pays. Subtract it from any backend's total to get the pure UI cost.
- `supersolid` — the TSX HUD via `SupersolidUiPlugin`. Total cost includes Boa
  reactive re-render, DOM reconcile, flair cascade, and taffy layout.

## Run

    cargo run --release -p citadel --features bench --bin citadel-bench -- --backend supersolid

Common flags:

- `--building-count N` — number of buildings in the HUD grid (default 120)
- `--frames N` (measured) `--warmup N` (excluded, default 300)
- `--seed N` — RNG seed for deterministic runs (default 1)
- `--format table|json` — output format
- `--sweep N1,N2,...` — run multiple building counts in sequence (e.g. `30,60,120,240`)

## Reproducing the dense-HUD bottleneck

The default config (120 buildings) already produces a full dense screen. A single
null vs. supersolid pair reveals the guard's impact:

    cargo run --release -p citadel --features bench --bin citadel-bench -- \
        --backend null --frames 300 --warmup 200

    cargo run --release -p citadel --features bench --bin citadel-bench -- \
        --backend supersolid --frames 300 --warmup 200

For a scaling sweep across building counts:

    cargo run --release -p citadel --features bench --bin citadel-bench -- \
        --backend supersolid --sweep 30,60,120,240 --frames 300 --warmup 200

## Reading the report

- `total` — mean + p50/p95/p99 + FPS-equivalent of one `app.update()`.
- `shared` — the null-backend floor (sim + snapshot + JSON marshal); the same for
  every backend. Computed once, then subtracted.
- `ui_backend` = `total − shared` — the backend's pure UI cost and the **ceiling** of
  any UI-only optimization.
- `marshal` (supersolid) — isolated `build_frame` cost (JSON bridge only). At 120
  buildings this is ~1% of total and irrelevant; optimize the reconcile, not the bridge.

## Findings — redundant reconciler writes (the payoff)

Citadel was built to prove that a dense static UI benefits a lot from the
`reconcile.rs` class/attribute equality guard. It proved the *spirit* right and the
*specifics* wrong: the class/attr guard barely helps (same ~2–5% as horde) even on
this ideal static UI, because *other* unconditional per-frame reconciler writes
re-mark the whole static tree every frame. The example surfaced two:

1. **`replace_children` on every parent, every pass** — re-sets each child's
   `ChildOf`/the parent's `Children`, marking them `Changed`, which makes **taffy
   re-lay-out the whole tree** every frame (and flair's `calculate_is_root` touch
   every `NodeStyleData`). Fix: skip the call when the child list is unchanged.
   Verified to produce **byte-identical layout + styling** (see `layout_is_non_degenerate`
   and `applies_styles_to_many_nodes` in `tests/mount.rs`).
2. **Class/attr re-insert** — the original equality guard; removes `apply_classes`/
   `apply_attributes` re-fires (tiny in this workload, ~1 ms of flair).

Drift-controlled A/B at **120 buildings (~2 673 nodes), release**, `--frames 120
--warmup 200`, alternating binaries to cancel machine drift, means of 6 pairs
(`ui_ms` = pure UI cost):

| variant | flair cascade | taffy | **ui_ms** | vs baseline |
|---------|--------------:|------:|----------:|------------:|
| baseline (no guards) | 13.4 ms | 23.9 ms | **49.0 ms** | — |
| + class/attr guard (original) | ~12.7 ms | 23.7 ms | **~47.9 ms** | ~1.02× |
| + `replace_children` skip (**shipped**) | 12.4 ms | **9.4 ms** | **32.1 ms** | **1.53×** |

**The class/attr guard alone buys ~2%** — because `replace_children` still forces a
full taffy relayout. Adding the `replace_children` skip cuts the pure UI cost
**1.53× (−34%)**, entirely by dropping taffy **24 → 9 ms**; styling and layout are
unchanged (byte-identical fingerprint).

> ⚠️ **Rejected optimization — do not re-add.** An earlier attempt also guarded the
> **root `NodeStyleSheet` re-insert** (insert-once instead of every frame). It made
> the flair cascade "drop" 13 → 0.4 ms and looked like a 2.9× win — but it was
> **bogus**: it *stripped all styles* (descendants never received an effective
> stylesheet, so 0 nodes were cascaded — the "win" was the cascade not running).
> `tests/mount.rs::applies_styles_to_many_nodes` is the regression guard that
> catches this (asserts >100 non-transparent `BackgroundColor`s). The root stylesheet
> must keep being re-inserted every frame.

### Re-running the A/B

    cargo run --release -p citadel --features bench --bin citadel-bench -- \
        --backend supersolid --format json > before.json
    # ...apply / revert a reconcile.rs guard...
    cargo run --release -p citadel --features bench --bin citadel-bench -- \
        --backend supersolid --format json > after.json
    # compare total_mean_ms, ui_ms between the two JSON objects.
    # For the per-stage split (flair/taffy), use --profile with bevy/trace (below).

## Allocation churn

    cargo run --release -p citadel --features bench,dhat-prof --bin citadel-bench -- \
        --backend supersolid --dhat

Reports bytes + allocations per frame during the measurement window. With the
equality guard, allocation churn in the reconcile path should be near zero for
static nodes.

## Profiling — per-stage attribution (`--profile`)

Splits the opaque `ui_backend` bucket into the five reconcile stages and prints a
per-stage ms + %-of-frame table plus a one-line summary. Requires `bevy/trace`:

    cargo run --release -p citadel --features bench,bevy/trace --bin citadel-bench -- \
        --profile --frames 120 --warmup 200

How it works: with `bevy/trace` every system is wrapped in a root
`info_span!("system", name=…)`, and each stage lives in a different system, so a
tracing layer that sums per-system busy-time attributes the whole frame. The **same
spans feed a Tracy flamegraph** — swap `bevy/trace` for `bevy/trace_tracy` and
attach Tracy for the visual timeline; `--profile` is the headless equivalent.

### Profile finding (Task 9 baseline)

At **120 buildings (~2 673 nodes), release, ~53 ms/frame**:

| stage | ms | % of frame |
|-------|----|----------:|
| Boa reactive re-render | ~6.9 | ~13% |
| DOM reconcile (walk + diff) | ~4.8 | ~9% |
| flair cascade | ~13.3 | ~25% |
| taffy layout | ~27.0 | ~51% |
| marshal (JSON bridge) | ~0.5 | ~1% |

The **flair cascade (25%) and taffy layout (51%)** together take ~76% of the frame
even though almost nothing changed — the redundant work the reconciler forces. The
class/attr equality guard does **not** remove these; the `replace_children` skip
(see "Findings") removes the taffy half (24 → 9 ms). The remaining flair cascade
(~12 ms) is *legitimate* work triggered by the root stylesheet being re-propagated
every frame — which must stay, because removing it strips all styles.

Boa at ~13% is already modest (fine-grained `<Keyed>` keeps JS re-runs proportional
to *changed* bindings only, not the full tree). The bottleneck is **not JS** — it
is the downstream taffy relayout that the `replace_children` skip eliminates.

So: the shipped optimization is the **`replace_children` skip in `reconcile.rs`**
(plus the small class/attr guard), not the JSON bridge or the JS render.
