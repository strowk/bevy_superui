# Horde macro-benchmark

Headless, deterministic benchmark of the horde game. Measures per-frame cost and
attributes it across UI backends so optimizations can be predicted and proven.

Design: `docs/superpowers/specs/2026-07-21-horde-benchmark-harness-design.md`.

> **Both profiles matter — measure in the one you care about, and stay consistent.**
> Release is what ships; debug is what you and users build day-to-day (faster
> builds), and it's where the swarm hitch bites hardest — debug is roughly 5–6×
> slower than release. Supersolid is slower than native in *both* profiles, so
> either is valid for the native-vs-supersolid gap; just never compare a debug
> "before" against a release "after". Examples below use `--release`; drop it to
> measure debug. Optimizations should ideally help both.

## Current finding (what to optimize)

- Supersolid's cost is **almost entirely the per-frame reconcile** — `ui_backend`
  is ~98–99.7% of the frame; `marshal` (the JSON bridge) is ~0.1% and irrelevant.
- It scales ~**linearly with live enemy count**: ~0.32 ms of reconcile *per enemy
  per frame*. ~25 enemies → 9.6 ms (102 fps); ~392 enemies → 125 ms (8 fps).
- **Native does the identical tree ~19–85× faster** (1.5 ms at 400 enemies). The
  gap is reconcile *overhead* (Boa re-running the reactive render for every `<For>`
  row + DOM diff + flair re-cascade + taffy every frame), not the underlying work.
- The **p99 tail** (e.g. p50 90 / p99 451 ms at 400) is the visible hitch: a wave
  spawns → the `<For>` reconcile spikes.

So: optimize the **reconcile path / update granularity**, not the JSON bridge.

## Run

    cargo run --release -p horde --features bench --bin horde-bench -- --backend supersolid

Backends: `null` (sim+snapshot floor), `native` (bevy_ui), `supersolid` (TSX).

Common flags:

- `--preset play|stress`
- `--enemy-cap N` or `--sweep 60,200,400,800`
- `--frames N` (measured) `--warmup N` (excluded)
- `--seed N` `--format table|json`

## Reproducing the swarm bottleneck

The pathological ~5 FPS the game shows under a large swarm needs `--preset stress`
(or high `--sweep` caps) **and** enough `--frames` for waves to accumulate.
`enemy_cap` is a *cap* that the spawn system fills over time — short runs at cap 60
stay pre-swarm and look fast.  To let the swarm build and see real stress numbers:

    cargo run --release -p horde --features bench --bin horde-bench -- \
        --backend supersolid --preset stress --frames 3000 --warmup 300

For a scaling sweep across enemy counts:

    cargo run --release -p horde --features bench --bin horde-bench -- \
        --backend supersolid --sweep 60,200,400,800 --frames 1000 --warmup 100

## Reading the report

- `total` — mean + p50/p95/p99 + FPS-equivalent of one `app.update()`.
- `shared` — the null-backend floor (sim + snapshot); the same for every backend.
- `ui_backend` = `total − shared` — the backend's UI cost, and the **ceiling** of
  any UI-only optimization (you can't save more than this).
- `marshal` (supersolid) — isolated `build_frame` (JSON bridge) cost; the rest of
  `ui_backend` is reconcile + layout.
- `vs native floor` — supersolid's gap to native; closing it is the standing target.

## Before / after

    cargo run --release -p horde --features bench --bin horde-bench -- \
        --backend supersolid --format json > before.json
    # ...optimize...
    cargo run --release -p horde --features bench --bin horde-bench -- \
        --backend supersolid --format json > after.json
    # compare total_mean_ms, ui_ms, marshal_ms between the two JSON objects.

## Allocation churn

    cargo run --release -p horde --features bench,dhat-prof --bin horde-bench -- \
        --backend supersolid --dhat

Reports bytes + allocations per frame (`bytes_per_frame` is total allocated during
the measurement window — heap churn — not net-live or peak heap).
Steady-state should trend toward ~zero as the allocations amortize.

## Profiling — per-stage attribution (`--profile`)

Splits the opaque `ui_backend` bucket into the five reconcile stages and prints a
per-stage ms + %-of-frame table plus a one-line summary. Headless, deterministic,
no GUI. Opt-in and zero-cost when off: it needs both the `--profile` flag **and**
Bevy's per-system spans (`bevy/trace`); without the feature the report is empty and
says so, and a normal run is byte-for-byte unaffected.

    cargo run --release -p horde --features bench,bevy/trace --bin horde-bench -- \
        --profile --preset stress --enemy-cap 400 --frames 200 --warmup 1500

How it works: with `bevy/trace` every system is wrapped in a root
`info_span!("system", name=…)`, and each stage lives in a different system, so a
tracing layer that sums per-system busy-time attributes the whole frame (schedule-
runner wrappers like `run_main` are excluded so leaves stay disjoint). The **same
spans feed a Tracy flamegraph** — swap `bevy/trace` for `bevy/trace_tracy` and
attach Tracy for the visual timeline; `--profile` is the headless equivalent.

Note: `--profile` god-modes the player (refills `Health` each frame) so the game
holds a steady full-swarm `Playing` frame. Without it, a long warmup under a stress
swarm ends in the cheap `GameOver` screen and the profile measures an empty UI.

Finding (400-enemy swarm, release, ~115 ms/frame): **~77% Boa render** (JS reactive
re-run in `emit_bevy_inbox_system`), ~9% reconcile (DOM diff + bevy_ui apply), ~8%
flair cascade, ~1% taffy, ~0.6% marshal. The bottleneck is the JS-side full
re-render every frame, **not** the Rust reconcile or the JSON bridge — so optimize
update granularity (fine-grained/keyed reactivity), not the diff or the marshal.
