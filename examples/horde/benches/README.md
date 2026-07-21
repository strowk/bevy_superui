# Horde macro-benchmark

Headless, deterministic benchmark of the horde game. Measures per-frame cost and
attributes it across UI backends so optimizations can be predicted and proven.

Design: `docs/superpowers/specs/2026-07-21-horde-benchmark-harness-design.md`.

> **Always use `--release` for cross-backend comparison.**
> In debug builds the backends' relative costs can invert — native has been
> measured *slower* than supersolid in debug mode because Boa's interpreter
> overhead is dwarfed by unoptimized Bevy-UI widget traversal. Debug numbers
> are misleading for any before/after or native-gap work. Use
> `cargo run --release` (shown in every example below) for meaningful results.

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

## Profiling (Tracy)

Build the bin with `--features bench,bevy/trace_tracy` and attach Tracy to drill
into which system inside `ui_backend` dominates (reconcile vs layout vs cascade).
