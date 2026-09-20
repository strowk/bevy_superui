# superui benchmarks

## JS engine

superui selects its JS engine by build target — no runtime choice, no bundled
interpreter:

| Target | Engine |
|---|---|
| native | V8, via [`deno_core`](https://crates.io/crates/deno_core) |
| `wasm32` (web) | the host browser's own JS engine |

## Results (V8, release)

One release run on the development machine, `--seed 1 --frames 120 --warmup 30`.
`ui_ms` is superui's per-frame cost (its systems' share of the frame); `fps` is
the full-frame rate the harness reports. Raw captures:
`docs/superpowers/bench/raw/{horde,citadel}-v8-release.json`, `rows-v8-release.json`.

### horde (`enemy_cap` sweep, `--preset stress`)

| enemy_cap | `ui_ms` | `fps` |
|---:|---:|---:|
| 60  | 2.61 | 354.8 |
| 200 | 3.51 | 268.1 |
| 400 | 3.43 | 275.3 |

### citadel (`building_count` sweep)

| building_count | `ui_ms` | `fps` |
|---:|---:|---:|
| 60  | 6.64  | 147.1 |
| 120 | 16.30 | 60.5 |
| 240 | 32.43 | 30.6 |

### rows (1,000 rows, per-op `p50_ms`)

| op | `p50_ms` |
|---|---:|
| `create` | 237.63 |
| `append1` | 53.70 |
| `append1k` | 279.04 |
| `insert1` | 52.59 |
| `insertEvery2nd` | 166.16 |
| `updateText1` | 17.75 |
| `updateTextEvery2nd` | 33.69 |
| `updateColor1` | 18.48 |
| `updateColorEvery2nd` | 34.10 |
| `swap1` | 38.54 |
| `swapEvery2nd` | 40.38 |
| `remove1` | 39.12 |
| `removeEvery2nd` | 48.38 |
| `clear` | 49.19 |

`rows-bench` rebuilds a fresh `App` and JS engine per rep, so each number is
dominated by isolate-bootstrap and full-bundle-eval, not steady-state
render/reconcile cost.

## Running

```bash
cargo run -q -p horde   --bin horde-bench   --release --features bench -- \
    --backend supersolid --preset stress --sweep 60,200,400 --frames 120 --warmup 30 --seed 1 --format json
cargo run -q -p citadel --bin citadel-bench --release --features bench -- \
    --backend supersolid --sweep 60,120,240 --frames 120 --warmup 30 --seed 1 --format json
cargo run -q -p rows    --bin rows-bench    --release --features bench -- \
    --backend supersolid --rows 1000 --reps 10 --warmup 3 --format json
```
