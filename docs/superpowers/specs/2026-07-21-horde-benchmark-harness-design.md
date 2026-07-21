# Horde — Macro-Benchmark Harness (design)

Date: 2026-07-21
Status: Design agreed.
Related:
- Strategy: [`2026-07-18-bevy-superui-performance-strategy.md`](./2026-07-18-bevy-superui-performance-strategy.md) — this is the horde-specific, Tier-2/Tier-3 realization of that strategy.
- Game design: [`2026-07-20-horde-native-ui-design.md`](./2026-07-20-horde-native-ui-design.md) — the `UiSnapshot`/`Intent` seams and 1:1 native/supersolid panel names this harness exploits.
- Port: `docs/superpowers/plans/2026-07-21-horde-supersolid-ui-port.md` — established the default supersolid backend and the ~5 FPS-under-swarm baseline this harness makes provable.

## 0. What this document is

The design for a **macro-benchmark of the horde game** — a headless, deterministic harness that
turns "I can see ~5 FPS while the window is open" into repeatable, machine-comparable numbers, and
attributes each frame's cost to a named pipeline stage so optimizations can be *predicted* (theoretical
ceiling) before they are attempted and *proven* (before/after delta) after.

It is the horde-specific realization of the performance strategy's **Tier 2** (end-to-end headless
frame time) and **Tier 3** (span attribution / Tracy), scoped to one real-world workload: the horde
game. Per-crate Criterion micro-benches (Tier 1) are out of scope here — this is the macro benchmark
that tells us *whether* an optimization helped a real game and *where* to look next.

## 1. Goals & non-goals

**Goals**
- A single headless command that reports, for a given backend + workload, the **total per-frame cost**
  (mean + p50/p95/p99 + FPS-equivalent) and a **per-stage attribution table** (where the frame goes).
- A **backend differential**: run the identical sim + snapshot + auto-player workload against both the
  **native** `bevy_ui` backend and the **supersolid** backend. Native is the empirical floor (shared
  sim + effectively-free UI); the native-vs-supersolid gap is the optimization target; per-stage
  attribution splits that gap into marshal / reconcile / layout.
- **Determinism**: seeded RNG + scripted auto-player + fixed `dt` → identical sim trajectory across
  backends and across git branches, so numbers are comparable.
- **Before/after workflow**: machine-readable JSON output + a small diff, so a branch's result can be
  compared to a saved baseline to *prove* an improvement.
- **Profiling drill-down**: the same span seams stream to Tracy under `bevy/trace_tracy` for
  flamegraph attribution when the table says "this stage is slow, but why."
- **Scaling insight**: an element-count sweep (enemy cap 60/200/400/800) reporting per-stage cost vs N
  to expose O(n) vs O(n²) traps.

**Non-goals (this stage)**
- No Tier-1 per-crate Criterion benches (separate, existing plans).
- No CI gating / stored-baseline dashboards (the strategy's deferred "phase 2"); local
  save-baseline/diff only.
- No windowed/GPU frame measurement as the primary number (see §7 risk 1 for the offscreen fallback);
  the provable number is headless CPU-side cost.
- No new gameplay, no optimization work — this builds the *measuring instrument*, not the fixes.

## 2. Harness shape & invocation

A dedicated headless binary: **`examples/horde/src/bin/bench.rs`**, so `cargo run -p horde` stays the
playable game untouched.

**Why a bin, not a `--bench` subcommand of the game:** the game's `main.rs` builds a full windowed app
via `DefaultPlugins`; the bench needs a different plugin set (headless, both UI backends compiled in,
scripted input instead of `bevy_input`). A separate entry keeps each assembly clean.

**Backend selection at runtime.** The game selects its UI backend at compile time via the `cfg`-gated
`ui::add_ui` (native under `ui-native`, else the supersolid panic seam). The bench is built with
`--features ui-native` so **both** `ui::native` and `ui::supersolid` modules compile, and it **bypasses
`ui::add_ui`**, adding the plugin named by `--backend` directly. This is the one place the `cfg` seam is
deliberately sidestepped, and only in the bench binary.

**App assembly (headless).** `MinimalPlugins` + `AssetPlugin` (no `watch_for_changes`) + `StatesPlugin`
+ `SimPlugin` + `project_snapshot` (given a fixed synthetic viewport, not a real `Camera`) + the chosen
UI plugin + bevy_ui's `taffy` layout systems. No `WinitPlugin`, no render backend, no window. Drive
`app.update()` in a fixed-count loop.

**CLI / env surface:**
- `--backend native|supersolid` (required)
- `--preset play|stress` (default `play`)
- `--enemy-cap N` (overrides preset; the sweep runs a list)
- `--sweep 60,200,400,800` (runs the scenario at each cap, one table row per cap)
- `--frames N` (measured frames; default e.g. 1000)
- `--warmup N` (excluded from stats; default e.g. 100)
- `--seed N` (default the game's fixed seed)
- `--format table|json` (default `table`)
- `--dhat` (run the allocation pass instead of the timing pass — see §5)

Env fallbacks reuse the existing `HORDE_*` vars where they overlap (`HORDE_SEED`, `HORDE_ENEMY_CAP`,
`HORDE_PRESET`).

## 3. Deterministic workload — the scripted auto-player

The sim already runs on a seeded, deterministic `FixedUpdate` step with no wall-clock. The missing
piece for a benchmark is **input**: real play needs a player who moves and shoots. The bench supplies a
**scripted auto-player** — a deterministic system that fills `IntentQueue` from a fixed program keyed on
the frame counter (not wall-clock):

- **Move**: orbit / figure-eight pattern around the arena center (keeps the player alive and mobile so
  enemies chase, spawn, and die — full churn).
- **Aim + shoot**: aim at the nearest enemy (or a rotating heading) and hold-fire continuously → steady
  projectile spawn/despawn, damage numbers, kills, combat-log events.
- **Weapon switch** every K frames → exercises the weapon-bar / inventory diff.
- **Inventory toggle** every M frames → exercises the modal open/close reconcile.

`StartGame` is issued on frame 0 so the sim enters `Playing`. `dt` is a fixed constant per `update()`
so the sim advances identically regardless of how fast the host runs. Because the RNG is seeded and the
script is frame-indexed, the **entire sim trajectory is identical** across backends and branches — the
only thing that differs between two runs is the UI backend and/or the code under optimization.

The auto-player lives in the bench binary (it is test/bench scaffolding, not game logic) and pushes the
same `Intent`s the game's `input.rs` would, so the sim consumes them through the unchanged path.

## 4. Stage attribution — the ceiling engine

The frame is instrumented at the boundaries the game design already named
(`advance sim → assemble snapshot → project snapshot → UI build/reconcile`), refined to the stages that
matter for the native-vs-supersolid gap:

| Span | Covers | Shared by both backends? |
|---|---|---|
| `advance_sim` | `SimPlugin` FixedUpdate chain | yes (the shared floor) |
| `assemble_snapshot` | `assemble_world_snapshot` | yes |
| `project_snapshot` | world→screen projection | yes |
| `build_frame` | `FrameDto` construction + serde marshal | supersolid only |
| `js_reconcile` | trigger → JS `bevy.on("frame")` → supersolid render/reconcile → ECS commands | supersolid only |
| `taffy_layout` | bevy_ui layout pass | both (native builds nodes directly) |

Each seam is annotated with `tracing::info_span!` (cheap-when-off; costs nothing in normal game builds).
A **lightweight accumulating `tracing` subscriber** installed by the bench sums each span's wall-time
across the measured frames and divides by frame count → a per-stage mean and its **share of the frame**.
That share *is* the Amdahl ceiling: a stage at 60% of the frame caps its own optimization payoff at 60%.

Because native shares `advance_sim/assemble_snapshot/project_snapshot/taffy_layout` and pays ~nothing
for `build_frame/js_reconcile`, the table reads directly as: *"of the total native-vs-supersolid gap,
this much lives in marshal, this much in reconcile, this much in extra layout churn."*

**Attribution mechanism — decided with a fallback.** Primary: reuse the `tracing` spans (single seam
set, also feeds Tracy). If per-span subscriber overhead proves too noisy for the sub-millisecond micro
stages, fall back to explicit `Instant` deltas recorded by thin wrapper systems around the same seam
list into an accumulator resource. The seam list and the report shape are identical either way; only the
timing source changes.

## 5. Metrics & report

Every timing run reports, per (backend, cap):
- **Total frame cost**: mean, p50, p95, p99, and FPS-equivalent (`1000 / mean_ms`). Percentiles because
  hitches hide in the tail.
- **Per-stage attribution table** (§4): each stage's mean ms and % share.
- **The native floor alongside supersolid** and the explicit gap (×) whenever both are run.

**Element-count scaling sweep** (`--sweep`): the same scenario at 60/200/400/800 enemy caps, reporting
per-stage cost as a function of N. A stage whose cost grows super-linearly in N is the O(n²) suspect;
this is where "which stage dominates as the swarm grows" gets answered.

**Allocation churn** (`--dhat`): a **separate pass** (dhat perturbs timing, so never mixed into the
timing numbers). Reports bytes + allocation count for representative frames — idle (pre-swarm), steady
(mid-run), peak-swarm — with the steady-state expectation of ~near-zero per-frame allocation. The
`FrameDto` JSON marshal is the prime churn suspect and this pass is what confirms or clears it.

**Output formats:**
- `table` — human-readable, aligned; the at-a-glance "is it faster / where did it go" view.
- `json` — machine-readable record `{ backend, preset, cap, frames, seed, total: {mean,p50,p95,p99,fps},
  stages: [{name, mean_ms, share}], }`. This is the before/after substrate.

**Before/after workflow:** save a JSON run as a baseline (e.g. `bench-before.json`), make the change,
run again (`bench-after.json`), and a tiny diff (a `critcmp`-style helper, or `jq`) shows the per-stage
and total deltas. Manual, trustworthy, zero infra — matching the strategy's "Now" CI posture.

## 6. Tracy drill-down

The §4 spans are ordinary `tracing` spans, so building the bench (or the real game) with
`--features bevy/trace_tracy` streams them to the Tracy profiler, composed with Bevy's own spans, for
per-frame flamegraphs. One seam set, three consumers: the in-process attribution table, the JSON record,
and Tracy. Native-only dev tooling; never on the wasm path.

The loop the strategy prescribes applies directly here: the bench table flags *which stage*, Tracy pins
*where inside it*, an optimization is attempted, and the bench proves the delta.

## 7. Risks & open technical questions

1. **Headless reconcile is the load-bearing assumption.** The whole harness depends on `SuperUiPlugin`
   + bevy_ui `taffy` layout running without a window or render backend. This needs an **early spike**:
   it likely requires a stub camera / synthetic viewport size so `100%`-sized nodes resolve (the game's
   `SuperUiRoot` fills the window; headless has no window). **Fallback if fully-headless proves
   impractical:** a hidden / offscreen `winit` window at a fixed resolution — still scripted, still
   deterministic, still no vsync-capped presentation — accepting a small render cost that is identical
   across backends and cancels out of the gap.
2. **Attribution overhead** (§4) — resolved with the Instant-wrapper fallback; flagged so the spike also
   sanity-checks span cost against a known-idle frame.
3. **Determinism of `js_reconcile` across branches** — Boa must not pull wall-clock/random; the game
   design already mandates a seeded JS clock. The bench asserts a fixed JS clock so reconcile work is
   reproducible.
4. **Native "floor" is a floor for *this* harness, not an absolute** — it measures native's CPU cost
   under the same headless loop, not the windowed game's GPU-bound reality. Stated so the gap is read as
   "UI-backend CPU overhead," which is exactly the quantity being optimized.

## 8. Where it lives

```
examples/horde/
  src/bin/bench.rs         # headless bench binary: assembly, auto-player, CLI, report
  benches/README.md        # how to run each mode, read a diff, attach Tracy; native-vs-supersolid
                           #   gap as the standing optimization target
  src/ (game)              # tracing::info_span! seam annotations added at the §4 boundaries,
                           #   behind cheap-when-off spans (no cost in normal builds)
```

The seam annotations are the only change to shipped game code; everything else (auto-player, loop,
subscriber, report, dhat pass) is confined to the bench binary and its README.

## 9. Definition of done

- `cargo run -p horde --features ui-native --bin bench -- --backend supersolid` prints a total-frame
  stats block + per-stage attribution table headlessly.
- The same command with `--backend native` prints the floor; running both shows the gap.
- `--sweep 60,200,400,800` prints the per-stage scaling curve.
- `--dhat` prints allocation bytes + counts for idle / steady / peak-swarm without perturbing timing.
- `--format json` emits a machine-readable record, and a documented diff shows before/after deltas.
- The §4 spans exist in game code and stream to Tracy under `bevy/trace_tracy`.
- `benches/README.md` documents the run modes, the before/after loop, and the native-vs-supersolid gap
  as the standing target.
- The headless-reconcile spike (§7.1) has either confirmed fully-headless operation or landed the
  offscreen-window fallback.
