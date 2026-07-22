# Citadel — a loaded strategy-HUD example that proves the reconciler class/attr guard

**Date:** 2026-07-22
**Status:** design (approved to draft; not yet planned)
**Author:** pairing session

## 1. Purpose

Build a new, keepable example app — `examples/citadel` — whose UI is a dense
grand-strategy **empire command screen** (Stellaris / EU4 flavor: resource
ledger, production grid, tech tree, fleet/unit roster, build queue, event log).
It exists to demonstrate a load profile that horde structurally **cannot** show:
a large, mostly-**static** UI where a *tiny* live region forces a full-tree
reconcile every frame.

It is the counter-example to horde for the reconciler optimization
"skip re-inserting unchanged `ClassList`/`AttributeList`" (`superui_bridge`
`reconcile.rs` `sync_identity`). In horde every node *moves* every frame, so an
inline-style change re-marks it for flair recalculation regardless of the guard
(`bevy_flair_style` `mark_changed_nodes_for_recalculation`), and the guard buys
almost nothing (~3%). Citadel is the opposite: hundreds–thousands of nodes with
**stable class + attr + inline style**, so the baseline's unconditional
`ec.insert` re-cascades the whole tree every frame purely as waste, and the guard
should remove nearly all of it.

### Why this actually works (verified mechanism)

`UiRuntime::reconcile` (`reconcile.rs:32`) is a **whole-tree walk**: whenever the
runtime is `dirty`, `sync_children` recurses over every node and calls
`sync_identity` on every element. `dirty` is set by *any* DOM mutation. So a
single per-frame change (a ticking clock) triggers a full re-sync of the entire
tree. In the baseline, `sync_identity` unconditionally re-inserts
`ClassList`/`AttributeList` on every node → Bevy marks them `Changed` →
flair's `Changed<ClassList>`/`Changed<AttributeList>` systems fire → the whole
tree re-cascades. The guard skips the insert when the value is unchanged, so only
genuinely-changed nodes re-cascade.

Because a competently fine-grained UI keeps the JS re-render (Boa) cheap, the
flair cascade becomes the dominant per-frame cost in the baseline — exactly the
cost the guard removes. That makes the before/after delta large and legible.

## 2. Non-goals

- No actual game: no map, no AI, no combat, no pathing, no win/lose, no menus.
- No native `bevy_ui` backend (per decision): backends are `null` + `supersolid`
  only. The before/after and `--profile` per-stage split are supersolid-only
  anyway.
- No input handling beyond what's trivially needed to mount (no clicks required
  for the benchmark). Buttons may exist visually but need not be wired.
- Not chasing a specific node count. Build a *realistic* dense strategy screen;
  make the scale **configurable/sweepable** so the benchmark can push it. If a
  reasonable layout lands near ~2k nodes, good; ~800–1200 is already ample.

## 3. Architecture (mirrors horde so the harness copies cleanly)

New crate `examples/citadel` (workspace `members = ["crates/*", "examples/*"]`
picks it up automatically — no root `Cargo.toml` edit).

```
examples/citadel/
  Cargo.toml                # mirror horde: lib + `citadel` bin + `citadel-bench` bin
  build.rs                  # pre-transpile app.tsx -> app.generated.js (host-only)
  assets/ui/citadel/
    index.html              # <div id="root"></div>
    theme.css               # the dark strategy-HUD theme (flair-0.6 subset only)
    app.tsx                 # the TSX UI
    app.generated.js        # committed transpile output (bench loads this)
  src/
    main.rs                 # windowed app (mount UI + run sim)
    lib.rs
    sim/                    # the "game side" economy/production simulator
      mod.rs                # SimPlugin, CitadelConfig, tick systems, RNG
      model.rs              # Resource, Building, Unit, Tech, states
      snapshot.rs           # UiSnapshot resource assembled each frame
    ui/
      mod.rs
      supersolid/
        mod.rs              # SupersolidUiPlugin (copy of horde's, renamed)
        bridge.rs           # FrameDto + build_frame + register_bridge
    bench/
      mod.rs                # Backend{Null,Supersolid}, build_bench_app, run_report,
                            # report_json/table, sweep, marshal probe, workload
      profile.rs            # per-stage span profiler (copy of horde's)
    bin/
      bench.rs              # citadel-bench (required-features = ["bench"])
```

Cargo features copied from horde minus `ui-native`/`mcp_debug`: `bench`,
`dhat-prof`, `hmr`, `debug-ui`. `build-dependencies` = `supersolid` (host
transpile). `.cargo/config.toml` `/STACK:8MB` already applies workspace-wide
(Windows main-thread stack / Boa parse).

## 4. The simulator (game side)

A small, deterministic economy/production sim. Manual `DT` clock, seeded RNG for
events (no wall-clock, no randomness of its own) so runs are byte-identical —
same rigor as horde bench.

**Model (`sim/model.rs`):**
- `Resource` (fixed set, ~8: Minerals, Energy, Alloys, Food, Research, Influence,
  Unity, Population): `current: f64`, `rate: f64`, `cap: f64`.
- `Building` (a `Vec`, count = `CitadelConfig::building_count`, default ~120,
  sweepable): `id`, `name` (deterministic name table), `category`, `tier` (1..5,
  drives color class), `state` (`Locked|Available|Building|Done`),
  `progress: f32`, `level: u32`, `cost: [(ResourceKind, f64); k]`,
  `affordable: bool` (derived).
- `Unit` roster (`Vec`, ~40): `id`, `name`, `count`, `status`.
- `Tech` rail (`Vec`, ~60): `id`, `name`, `state` (`Locked|Researching|Done`).
- `clock` (elapsed secs), `tick` counter.

**Per-tick systems (`sim/mod.rs`):**
- Advance `clock += DT`.
- `resource.current = min(cap, current + rate)` — smooth per-frame change on ~8
  resource value texts (content only; no class/inline change).
- For each `Building` in the build queue (≤ ~8): `progress += speed`; on
  completion → `state = Done`, drop from queue, maybe unlock some
  `Locked→Available` (class change), bump a `Unit.count` (content change).
- Throttled: pick an `Available` + affordable building → `state = Building`, push
  to queue (a class change + one queue row appears).
- Recompute `affordable` against a hysteresis band so only a *trickle* flip per
  second (class change on a few cards).
- Throttled: a `Tech` finishes → `Done` (class change on one rail icon).

**Steady-state by construction:** all buildings/units/techs exist from frame ~0
(a mix of Locked/Available/Building/Done). There is no death/GameOver cliff, so
warmup is short and no god-mode hack is needed (unlike horde). The workload is
deterministic and never collapses.

**Snapshot (`sim/snapshot.rs`):** a `UiSnapshot` resource rebuilt each frame from
the model (mirrors horde's assemble step). The `null` backend runs the sim +
snapshot and nothing else — the shared floor.

## 5. Rust → JS bridge (`ui/supersolid/bridge.rs`)

Copy horde's pattern. A `#[derive(Event, Serialize)] FrameDto` with:
- scalars: `clock` (mm:ss computed JS-side), `tick`, per-resource `current`/`rate`.
- `resources: Vec<ResourceDto>` (kind, value, rate, cap).
- `buildings: Vec<BuildingDto>` (`id`, `name`, `category`, `tier`, `state`,
  `progress`, `level`, `affordable`, cost summary).
- `units: Vec<UnitDto>` (`id`, `name`, `count`, `status`).
- `techs: Vec<TechDto>` (`id`, `name`, `state`).
- `events: Vec<EventDto>` (recent event-log lines, fading).

`build_frame(&UiSnapshot) -> FrameDto`; triggered every `Update` via
`push_ui_frame` → JS `bevy.on("frame")`. `register_bridge` wires the event
(and a couple of no-op commands so the surface matches horde's shape). `sim/`
stays serde-free; DTOs live in the bridge.

## 6. The TSX UI (`app.tsx`) — fine-grained on purpose

Structure so the **static bulk does not re-run in JS every frame**, while the
reconciler still walks the whole tree (that's what exposes the guard):

- Top: `bevy.on("frame", f => setFrame(f))`; a `frame()` signal.
- **Live, top-level reads** (few nodes, re-run every frame): the mission clock,
  the ~8 resource value chips, and the ≤8 in-progress build bars.
- **Static bulk via `<Keyed each=… by="id">`** (per-field reactive reads, the
  same machinery horde's overlays use): the building grid, the unit roster, and
  the tech rail. A per-frame snapshot diff touches only the fields that changed
  (a card's `state`/`affordable`, a unit's `count`), so only those bindings
  re-run — everything else's DOM stays put and its class/attr are unchanged.
- **Positioning is CSS-class/grid/flex only** for the static bulk → inline style
  is stable → the guard applies to them. The ≤8 progress bars use per-frame
  inline `width` (realistically few things build at once) and are the *only*
  inline-animated nodes; they honestly will **not** benefit from the guard — a
  built-in contrast with the static majority.

Authoring gotchas to respect (from prior memory):
- Control-flow (`<For>`/`<Show>`/`<Keyed>`) inside a plain element must be
  `{...}`-wrapped or it silently renders nothing.
- Every vertical container needs explicit `flex-direction: column`.
- No JSX HTML entities; use a bare `>` or a JS string expr, never a bare `<`.

## 7. Styling (`theme.css`) — flair-0.6 subset only

A polished dark strategy-HUD look, using only supported properties (see the
flair-0.6 subset memo): flex/grid, `position`+sides, sizes, marg/pad, borders +
`border-radius`, `background-color`, gradients (radial needs a size before `at`),
`box-shadow`, `text-shadow` (offset+color only), `transform`, `z-index`, `color`,
`font-size`/`family`/`line-height`, `text-align`, `:hover`/compound/descendant
selectors, CSS vars, `overflow`/`gap`, `vh/vw/%`. Bake alpha into `rgba()` (no
`opacity`); write four sides (no `inset`); no `font-weight`/`letter-spacing`/
`cursor`. Tier/category colors via classes; a subtle animated accent only via the
`animation` shorthand (duration-first) if used at all. Goal: reads as a real,
attractive empire-management screen we'd keep as a showcase.

## 8. Benchmark / profiling harness (copied from horde)

`src/bench/mod.rs`, `src/bench/profile.rs`, `src/bin/bench.rs`, adapted:
- Backends: `Null` (sim+snapshot floor), `Supersolid` (TSX). Native removed.
- `citadel-bench --backend null|supersolid --format json|table`,
  `--frames N --warmup N --seed N`, `--sweep <building_counts>` (the sweep param
  maps to `building_count`, the dominant node driver), `--dhat`.
- `--profile` per-stage span split (Boa render / reconcile / **flair cascade** /
  taffy / marshal) via `bevy/trace`, identical to horde's. No god-mode needed
  (steady-state sim), so `--profile` just runs warmup then records.
- JSON report fields: `total_mean_ms`, `p50/95/99`, `fps`, `shared_ms`,
  `ui_ms` (= total − shared), `marshal_ms`. (`native_total_ms` dropped.)
- `benchmark.md` doc in `examples/citadel/` mirroring horde's, describing the
  intended load profile and the before/after method.

## 9. Before / after methodology (drift-controlled, git-safe)

The proven method from the prior session, adjusted to avoid touching git while a
parallel process edits the worktree:
1. Build the **guarded** binary (current `reconcile.rs`).
2. Back up `reconcile.rs` by **file copy**, hand-revert the guard, build the
   **baseline** binary, then restore `reconcile.rs` from the backup. **No
   `git stash`** (the parallel `game_menu` process has uncommitted files).
3. Run the two binaries **alternately** at a fixed loaded config (drift cancels
   pairwise); report paired deltas for total frame and the `--profile` flair
   cascade + reconcile stages, plus the full per-stage split.
4. Also report a `--sweep` so the win is shown scaling with static node count.

Expectation (to be validated, may be wrong): a **large** flair-cascade reduction
and a clear total-frame win — unlike horde's ~3%. If it doesn't materialize, the
example still documents the honest result.

## 10. Success criteria

- `cargo run -p citadel` shows a dense, good-looking strategy HUD, updating
  smoothly (clock, resources, build bars, occasional events).
- `citadel-bench` runs headless and deterministic on `null` + `supersolid`,
  including `--profile` and `--sweep`.
- A drift-controlled before/after at a fixed loaded config produces a clear,
  reproducible flair-cascade + total-frame improvement attributable to the guard,
  with the per-stage table showing the cascade collapse. Numbers reported; no
  code committed.

## 11. Open risks

- If the reconcile **walk** itself (building `ClassList`/`AttributeList` +
  compare for every node every frame) dominates, the total-frame win may be
  smaller than the cascade-stage win. That's still a valid, informative result
  (and points at whole-tree-dirty tracking as a separate future optimization).
- `<Keyed>` per-field reactivity must actually keep Boa cheap; if the whole tree
  re-renders in JS anyway, Boa will dominate like horde. Verify early with
  `--profile` (Boa% should be modest, not ~74%).
