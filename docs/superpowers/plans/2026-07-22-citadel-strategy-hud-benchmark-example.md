# Citadel Strategy-HUD Benchmark Example — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build `examples/citadel` — a dense, static-heavy grand-strategy empire HUD (supersolid TSX) plus a copied horde-style headless benchmark harness — to demonstrate that the `reconcile.rs` class/attr equality guard removes a large per-frame flair re-cascade when most nodes are static.

**Architecture:** A deterministic economy/production simulator (no real game) assembles a per-frame `UiSnapshot`; a JSON bridge pushes it to a fine-grained TSX UI (`<Keyed>` per-field reactivity so JS/Boa stays cheap while the reconciler still walks the whole tree). A headless bench binary (`null` + `supersolid` backends, `--profile` per-stage split, `--sweep`) measures it. Mirrors `examples/horde` structure throughout.

**Tech Stack:** Rust, Bevy 0.17, superui/superui_css/superui_bridge, supersolid (host-only transpile in `build.rs`), serde.

## Global Constraints

- **NO GIT COMMITS. NO `git add`.** Leave all changes in the worktree. A parallel process is editing `examples/game_menu` with uncommitted files — never run `git stash`, `git add`, `git commit`, `git checkout`, or `git reset`. Revert files by file-copy backup only.
- **Do not touch `examples/game_menu`** or any file outside `examples/citadel/`, except reading `examples/horde/*` as reference.
- CSS: flair-0.6 subset only. Supported: flex/grid, `position`+top/right/bottom/left, sizes, margin/padding, `border`/`border-radius`/`border-color`, `background-color`, gradients (radial needs a size before `at`, e.g. `ellipse 70% 70% at 50% 42%`), `box-shadow`, `text-shadow` (offset+color only, no blur), `transform`, `z-index`, `color`, `font-size`/`font-family`/`line-height`, `text-align`, `:hover`/`:checked`/descendant/compound selectors, CSS vars, `overflow`/`gap`, `vh/vw/%`. NOT supported (silently dropped): `opacity` (bake into rgba), `inset` (write 4 sides), `font-weight`, `letter-spacing`, `cursor`, `text-transform`, `white-space`, transition/animation longhands, custom fonts. `animation` shorthand duration-first only; only color/background-color animatable.
- TSX authoring: control-flow (`<For>`/`<Show>`/`<Keyed>`) inside a plain element must be `{...}`-wrapped or it renders nothing. Every vertical container needs explicit `flex-direction: column`. No JSX HTML entities; never a bare `<` in JSX text.
- Determinism: manual `DT` clock, seeded RNG, no wall-clock/no ambient randomness. Same seed ⇒ byte-identical runs.
- Windows main-thread stack: `.cargo/config.toml` already sets `/STACK:8MB` workspace-wide; no change needed.
- Crate name: `citadel`; bins `citadel` and `citadel-bench`; asset dir `assets/ui/citadel/`.

**Reference source of truth for copy-heavy code:** `examples/horde/` — specifically `Cargo.toml`, `build.rs`, `src/bin/bench.rs`, `src/bench/mod.rs`, `src/bench/profile.rs`, `src/ui/supersolid/{mod,bridge}.rs`, `assets/ui/horde/{index.html,theme.css,app.tsx}`. When a step says "adapt horde's X", open that file and reproduce it with the named substitutions.

---

### Task 1: Crate scaffold that builds

**Files:**
- Create: `examples/citadel/Cargo.toml`
- Create: `examples/citadel/build.rs`
- Create: `examples/citadel/src/lib.rs`
- Create: `examples/citadel/src/main.rs`
- Create: `examples/citadel/assets/ui/citadel/index.html`
- Create: `examples/citadel/assets/ui/citadel/theme.css` (minimal placeholder)
- Create: `examples/citadel/assets/ui/citadel/app.tsx` (minimal placeholder)

**Interfaces:**
- Produces: crate `citadel` with lib target `citadel`, bins `citadel` (`src/main.rs`) and `citadel-bench` (`src/bin/bench.rs`, `required-features=["bench"]`), features `bench`/`dhat-prof`/`hmr`/`debug-ui`, build-dep `supersolid`. `src/lib.rs` exposes `pub mod sim; pub mod ui; pub mod bench;` (modules added in later tasks — for Task 1 leave `lib.rs` with only what compiles).

- [ ] **Step 1: Write `Cargo.toml`** — copy `examples/horde/Cargo.toml`, then: set `name="citadel"`, `default-run="citadel"`, `[lib] name="citadel"`, bins `citadel`→`src/main.rs` and `citadel-bench`→`src/bin/bench.rs`. Remove features `ui-native`, `mcp_debug`, dep `bevy_brp_extras`, and `default=["debug-ui"]` → keep `default=["debug-ui"]`. Keep `bench`, `dhat-prof`, `hmr`, `debug-ui`, the `[build-dependencies] supersolid`, dhat optional dep, dev-deps, and the wasm/native target blocks. Bin `citadel-bench` needs `required-features=["bench"]`. Since Task 1 has no `src/bin/bench.rs` yet, temporarily comment out the `[[bin]] citadel-bench` block (re-enabled in Task 8).

- [ ] **Step 2: Write `build.rs`** — copy `examples/horde/build.rs`, change the dir to `assets/ui/citadel`.

- [ ] **Step 3: Write `index.html`** — exactly `<div id="root"></div>`.

- [ ] **Step 4: Write placeholder `app.tsx`:**

```tsx
import { render } from "supersolid";
function App() {
  return (<div id="hud"><span class="label">citadel</span></div>);
}
render(() => <App />, document.getElementById("root"));
```

- [ ] **Step 5: Write placeholder `theme.css`:**

```css
#hud { display: flex; flex-direction: column; width: 100%; height: 100%; background-color: rgb(12, 16, 24); color: rgb(220, 230, 245); }
.label { font-size: 18px; }
```

- [ ] **Step 6: Write minimal `src/lib.rs`:**

```rust
// Modules are added by later tasks. Kept minimal so Task 1 compiles standalone.
```

- [ ] **Step 7: Write minimal `src/main.rs`** (windowed app filled in Task 10; stub for now):

```rust
fn main() {
    println!("citadel: run the benchmark with `cargo run -p citadel --features bench --bin citadel-bench`");
}
```

- [ ] **Step 8: Build and verify the transpile ran.**

Run: `cargo build -p citadel`
Expected: PASS. Then confirm `examples/citadel/assets/ui/citadel/app.generated.js` now exists (build.rs produced it).

- [ ] **Step 9: Leave changes in worktree. DO NOT commit or `git add`.**

---

### Task 2: Sim data model

**Files:**
- Create: `examples/citadel/src/sim/model.rs`
- Create: `examples/citadel/src/sim/mod.rs` (module wiring only this task; systems in Task 3)
- Modify: `examples/citadel/src/lib.rs` (add `pub mod sim;`)

**Interfaces:**
- Produces:
  - `pub const DT: f64 = 1.0 / 60.0;`
  - `pub enum ResourceKind { Minerals, Energy, Alloys, Food, Research, Influence, Unity, Population }` with `pub fn all() -> [ResourceKind; 8]`, `pub fn name(self) -> &'static str`, `pub fn icon(self) -> &'static str` (icon = a short unicode glyph or 1–3 ASCII chars).
  - `pub enum Category { Economy, Military, Science, Civic }` with `pub fn class(self) -> &'static str` (`"economy"|"military"|"science"|"civic"`).
  - `pub enum BuildState { Locked, Available, Building, Done }` with `pub fn class(self)->&'static str`.
  - `pub enum TechState { Locked, Researching, Done }` with `pub fn class(self)->&'static str`.
  - `pub enum UnitStatus { Idle, Moving, Combat }` with `pub fn class(self)->&'static str`.
  - structs `Resource{kind,current:f64,rate:f64,cap:f64}`, `Building{id:usize,name:String,category:Category,tier:u8,state:BuildState,progress:f32,level:u32,cost:Vec<(ResourceKind,f64)>,affordable:bool}`, `Unit{id:usize,name:String,count:u32,status:UnitStatus}`, `Tech{id:usize,name:String,state:TechState,progress:f32}`.
  - `#[derive(Resource, Clone)] pub struct CitadelConfig { pub building_count: usize, pub unit_count: usize, pub tech_count: usize, pub seed: u64 }` + `Default` (building_count 120, unit_count 40, tech_count 60, seed 1).
  - `pub struct Rng(u64)` deterministic xorshift: `pub fn new(seed:u64)->Self`, `pub fn next_u32(&mut self)->u32`, `pub fn next_f32(&mut self)->f32` (0..1), `pub fn range(&mut self, lo:usize, hi:usize)->usize`.
  - `pub fn building_name(i: usize) -> String`, `pub fn unit_name(i: usize) -> String`, `pub fn tech_name(i: usize) -> String` — deterministic from index (combine word tables + index; never random).

- [ ] **Step 1: Write the failing test** in `model.rs` `#[cfg(test)] mod tests`:

```rust
use super::*;
#[test]
fn rng_is_deterministic_and_names_are_stable() {
    let mut a = Rng::new(42); let mut b = Rng::new(42);
    for _ in 0..1000 { assert_eq!(a.next_u32(), b.next_u32()); }
    assert_eq!(building_name(7), building_name(7));
    assert_ne!(building_name(7), building_name(8));
    assert_eq!(ResourceKind::all().len(), 8);
    assert_eq!(Category::Military.class(), "military");
}
```

- [ ] **Step 2: Run to verify it fails.** Run: `cargo test -p citadel --lib sim::model`. Expected: FAIL (items undefined).

- [ ] **Step 3: Implement `model.rs`** — the enums (with `class`/`name`/`icon`/`all`), structs, `CitadelConfig`+`Default`, `Rng` (xorshift64: `x ^= x<<13; x^=x>>7; x^=x<<17;`), and the three name functions (e.g. pick from small static `&[&str]` word tables indexed by `i % table.len()` combined with a suffix number `i`). No randomness in name functions.

- [ ] **Step 4: Write `sim/mod.rs`:** `pub mod model; pub use model::*;` (systems added Task 3). Add `pub mod sim;` to `src/lib.rs`.

- [ ] **Step 5: Run tests to verify pass.** Run: `cargo test -p citadel --lib sim::model`. Expected: PASS.

- [ ] **Step 6: Leave changes in worktree. DO NOT commit or `git add`.**

---

### Task 3: Sim plugin + deterministic tick systems

**Files:**
- Modify: `examples/citadel/src/sim/mod.rs`
- Create: `examples/citadel/src/sim/economy.rs`

**Interfaces:**
- Consumes: everything from `model.rs`.
- Produces:
  - `#[derive(Resource)] pub struct Economy { pub resources: Vec<Resource>, pub buildings: Vec<Building>, pub units: Vec<Unit>, pub techs: Vec<Tech>, pub queue: Vec<usize>, pub clock: f64, pub tick: u64, pub events: std::collections::VecDeque<EventLine>, pub rng: Rng }`.
  - `pub struct EventLine { pub text: String, pub age: f32 }`.
  - `pub fn build_economy(cfg: &CitadelConfig) -> Economy` — deterministic initial steady-state: all resources with sane `current`/`rate`/`cap`; `building_count` buildings across the 4 categories & tiers 1..5, states seeded as a mix (~30% Done, ~15% Building w/ random progress and pushed to `queue` capped at 8, ~40% Available, ~15% Locked); `unit_count` units; `tech_count` techs (mix Locked/Researching/Done). Everything present from tick 0.
  - `pub fn tick_economy(econ: &mut Economy, cfg: &CitadelConfig)` — one deterministic step: `clock += DT`; each resource `current = (current + rate).min(cap)`; each queued building `progress += rate_per_building`, on `>=1.0` → `Done`, pop from queue, push an `EventLine`, unlock ~1 `Locked→Available`, bump a `Unit.count`; every K ticks start one `Available`+affordable building (→`Building`, push queue if `queue.len()<8`); recompute `affordable` with hysteresis so only a few flip; every M ticks advance one `Researching` tech / finish it. Age + expire `events` (keep last ~12).
  - `pub struct SimPlugin;` `impl Plugin` — inserts `CitadelConfig::default()` if absent, builds `Economy` at `Startup`, and runs `advance_sim` in `Update` (a system: `tick_economy(&mut econ, &cfg)`).
- Later tasks rely on: `Economy`, `build_economy`, `tick_economy`, `SimPlugin`.

- [ ] **Step 1: Write the failing test** in `economy.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::model::*;
    #[test]
    fn economy_is_steady_and_deterministic() {
        let cfg = CitadelConfig::default();
        let mut a = build_economy(&cfg);
        let mut b = build_economy(&cfg);
        assert_eq!(a.buildings.len(), cfg.building_count);
        assert!(a.buildings.iter().any(|x| x.state == BuildState::Building));
        assert!(a.buildings.iter().any(|x| x.state == BuildState::Done));
        for _ in 0..600 { tick_economy(&mut a, &cfg); tick_economy(&mut b, &cfg); }
        // Determinism: identical after 600 ticks.
        assert_eq!(a.clock.to_bits(), b.clock.to_bits());
        assert_eq!(a.buildings.iter().filter(|x| x.state==BuildState::Done).count(),
                   b.buildings.iter().filter(|x| x.state==BuildState::Done).count());
        // Steady-state: still fully populated, still has activity.
        assert_eq!(a.buildings.len(), cfg.building_count);
        assert!(a.clock > 0.0);
    }
}
```

- [ ] **Step 2: Run to verify it fails.** Run: `cargo test -p citadel --lib sim::economy`. Expected: FAIL.

- [ ] **Step 3: Implement `economy.rs`** (`build_economy`, `tick_economy`, `EventLine`, `Economy`) and wire `SimPlugin` + `advance_sim` in `sim/mod.rs` (add `pub mod economy; pub use economy::*;`). Keep tick logic pure/deterministic (all randomness via `econ.rng`).

- [ ] **Step 4: Run tests.** Run: `cargo test -p citadel --lib sim::economy`. Expected: PASS.

- [ ] **Step 5: Leave changes in worktree. DO NOT commit or `git add`.**

---

### Task 4: UiSnapshot + assemble system

**Files:**
- Create: `examples/citadel/src/sim/snapshot.rs`
- Modify: `examples/citadel/src/sim/mod.rs`

**Interfaces:**
- Consumes: `Economy`, model enums.
- Produces:
  - `#[derive(Resource, Default, Clone)] pub struct UiSnapshot { pub clock: f64, pub tick: u64, pub resources: Vec<ResSnap>, pub buildings: Vec<BldSnap>, pub units: Vec<UnitSnap>, pub techs: Vec<TechSnap>, pub events: Vec<EvtSnap> }` with plain `#[derive(Clone)]` snap structs holding UI-ready fields (ids as `usize`, states as `&'static str` class strings via the `class()` fns, `progress: f32`, resource `current/rate/cap: f64`, unit `count`, etc.).
  - `pub fn assemble_snapshot(econ: Res<Economy>, mut snap: ResMut<UiSnapshot>)` — rebuild `*snap` from `econ` each frame.
  - `SimPlugin` (Task 3) also `init_resource::<UiSnapshot>()` and adds `assemble_snapshot` in `Update` after `advance_sim`.
- Later tasks rely on: `UiSnapshot` and its snap structs.

- [ ] **Step 1: Write the failing test** in `snapshot.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::model::*;
    use crate::sim::economy::*;
    #[test]
    fn snapshot_mirrors_economy() {
        let cfg = CitadelConfig::default();
        let econ = build_economy(&cfg);
        let mut snap = UiSnapshot::default();
        assemble_from(&econ, &mut snap); // pure helper the system calls
        assert_eq!(snap.buildings.len(), econ.buildings.len());
        assert_eq!(snap.resources.len(), 8);
        assert!(snap.buildings.iter().any(|b| b.state == "building"));
    }
}
```

- [ ] **Step 2: Run to verify it fails.** Run: `cargo test -p citadel --lib sim::snapshot`. Expected: FAIL.

- [ ] **Step 3: Implement `snapshot.rs`** with a pure `pub fn assemble_from(econ: &Economy, snap: &mut UiSnapshot)` and the Bevy system `assemble_snapshot` that calls it. Wire into `SimPlugin` and `sim/mod.rs` (`pub mod snapshot; pub use snapshot::*;`).

- [ ] **Step 4: Run tests.** Run: `cargo test -p citadel --lib sim::snapshot`. Expected: PASS.

- [ ] **Step 5: Leave changes in worktree. DO NOT commit or `git add`.**

---

### Task 5: Rust→JS bridge (FrameDto + build_frame + register_bridge)

**Files:**
- Create: `examples/citadel/src/ui/mod.rs` (`pub mod supersolid;`)
- Create: `examples/citadel/src/ui/supersolid/mod.rs` (module decl + plugin stub; plugin body finished Task 6)
- Create: `examples/citadel/src/ui/supersolid/bridge.rs`
- Modify: `examples/citadel/src/lib.rs` (`pub mod ui;`)

**Interfaces:**
- Consumes: `UiSnapshot` and snap structs.
- Produces:
  - serde `#[derive(Serialize)]` DTOs mirroring the snap structs: `ResourceDto`, `BuildingDto`, `UnitDto`, `TechDto`, `EventDto`.
  - `#[derive(Event, Serialize)] pub struct FrameDto { pub clock: f64, pub tick: u64, pub resources: Vec<ResourceDto>, pub buildings: Vec<BuildingDto>, pub units: Vec<UnitDto>, pub techs: Vec<TechDto>, pub events: Vec<EventDto> }`.
  - `pub fn build_frame(snap: &UiSnapshot) -> FrameDto`.
  - `pub fn register_bridge(app: &mut App)` — `add_superui_event::<FrameDto>("frame")` (plus, to match horde's surface, one no-op command `#[derive(Event, Deserialize)] pub struct CitadelIntent { pub kind: String }` registered via `add_superui_command` + an observer that just `warn!`s unknown kinds).
- Later tasks rely on: `build_frame`, `register_bridge`, `FrameDto`.

- [ ] **Step 1: Write the failing test** in `bridge.rs` (mirror horde's bridge test):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::model::CitadelConfig;
    use crate::sim::economy::build_economy;
    use crate::sim::snapshot::{UiSnapshot, assemble_from};
    #[test]
    fn build_frame_maps_snapshot() {
        let cfg = CitadelConfig::default();
        let econ = build_economy(&cfg);
        let mut snap = UiSnapshot::default();
        assemble_from(&econ, &mut snap);
        let f = build_frame(&snap);
        assert_eq!(f.buildings.len(), snap.buildings.len());
        assert_eq!(f.resources.len(), 8);
    }
}
```

- [ ] **Step 2: Run to verify it fails.** Run: `cargo test -p citadel --lib bridge`. Expected: FAIL.

- [ ] **Step 3: Implement `bridge.rs`** (adapt `examples/horde/src/ui/supersolid/bridge.rs`), create `ui/mod.rs`, `ui/supersolid/mod.rs` (module decl + `pub struct SupersolidUiPlugin;` with an empty `impl Plugin { fn build(&self,_:&mut App){} }` for now), add `pub mod ui;` to lib.

- [ ] **Step 4: Run tests.** Run: `cargo test -p citadel --lib bridge`. Expected: PASS.

- [ ] **Step 5: Leave changes in worktree. DO NOT commit or `git add`.**

---

### Task 6: Supersolid plugin (mount + push frame)

**Files:**
- Modify: `examples/citadel/src/ui/supersolid/mod.rs`

**Interfaces:**
- Consumes: `register_bridge`, `build_frame`, `UiSnapshot`.
- Produces: `SupersolidUiPlugin` that: `add_plugins(SuperUiPlugin)`, `register_bridge(app)`, `Startup` `mount_ui` (spawn `SuperUiRoot` filling the window, loading `ui/citadel/index.html`, `theme.css`, and `app.generated.js` — or `app.tsx` under `USE_LIVE_TSX = cfg!(all(not(wasm), feature="hmr"))`), `Update` `push_ui_frame` (`build_frame(&snap)` → `commands.trigger(frame)`).

- [ ] **Step 1: Implement** by adapting `examples/horde/src/ui/supersolid/mod.rs`: rename paths `ui/horde/*`→`ui/citadel/*`, drop the `forward_toggle_inventory`/`GameState` bits, `push_ui_frame` reads only `Res<UiSnapshot>`.

- [ ] **Step 2: Build.** Run: `cargo build -p citadel`. Expected: PASS.

- [ ] **Step 3: Leave changes in worktree. DO NOT commit or `git add`.**

---

### Task 7: The TSX UI + theme.css (the dense, styled screen)

**Files:**
- Rewrite: `examples/citadel/assets/ui/citadel/app.tsx`
- Rewrite: `examples/citadel/assets/ui/citadel/theme.css`

**Interfaces:**
- Consumes: the `frame` event shape from `FrameDto` (resources/buildings/units/techs/events).
- Produces: `app.generated.js` (via build.rs) that mounts a dense HUD.

**Layout requirement (realistic empire screen):** a top **resource ledger** bar (the ~8 resource chips: icon + name + value + rate), then a 3-column body: left **tech rail** (list of tech items, class by state), center **production grid** (all buildings as tier/category-colored cards: name, category tag, level, cost line, a progress track+fill, a status badge), right column split into **unit roster** (rows: name, count, status) over a **build queue** + **event log** (fading lines). Everything laid out with flex/grid + classes; **no per-frame inline style except the in-progress build bars' `width`**.

**Reactivity requirement (keeps Boa cheap — critical):**
- `bevy.on("frame", f => setFrame(f))`; `frame()` signal.
- Resource chip values, the mission clock, and the ≤8 building progress-bar `width`s read `frame()` directly (few nodes).
- The **building grid, unit roster, tech rail render via `<Keyed each={…} by="id">`** with per-field reads (`b.state`, `b.affordable`, `u.count`, `t.state`) so unchanged cards' bindings don't re-run. Follow horde's `Nameplates`/`DamageNumbers` `<Keyed>` pattern in `examples/horde/assets/ui/horde/app.tsx`.

- [ ] **Step 1: Write `app.tsx`** implementing the layout + reactivity above. Respect authoring gotchas (wrap `<Keyed>` in `{…}`; `flex-direction: column` on vertical containers; no bare `<`). Compute `mm:ss` from `clock` in JS.

- [ ] **Step 2: Write `theme.css`** — a polished dark strategy-HUD theme using only supported properties (see Global Constraints). Tier colors (`.tier-1..5`), category accents (`.economy/.military/.science/.civic`), state classes (`.locked/.available/.building/.done`, `.researching`), chips, cards, bars, rails. Grid for the production panel. Bake all alpha into `rgba()`.

- [ ] **Step 3: Transpile + build.** Run: `cargo build -p citadel`. Expected: PASS; `app.generated.js` regenerated with no `cargo:warning=supersolid:` errors (CSS "unsupported property" warnings from flair at *runtime* are acceptable, but avoid them where possible).

- [ ] **Step 4: Headless mount smoke-test.** Add a test at `examples/citadel/tests/mount.rs` that builds the supersolid app headlessly (mirror horde's bench `build_bench_app` mount, or `examples/horde/tests` if present) and asserts the reconciled entity tree has a large node count (e.g. `> 300` entities with `TypeName`) after a few `app.update()`s. Run: `cargo test -p citadel --test mount`. Expected: PASS. (If a full mount test is impractical, defer this assertion to Task 8's bench workload sampler and note it.)

- [ ] **Step 5: Leave changes in worktree. DO NOT commit or `git add`.**

---

### Task 8: Benchmark harness (null + supersolid)

**Files:**
- Create: `examples/citadel/src/bench/mod.rs`
- Create: `examples/citadel/src/bin/bench.rs`
- Modify: `examples/citadel/src/lib.rs` (`pub mod bench;`)
- Modify: `examples/citadel/Cargo.toml` (re-enable the `[[bin]] citadel-bench` block)

**Interfaces:**
- Consumes: `SimPlugin`, `UiSnapshot`, `SupersolidUiPlugin`, `build_frame`.
- Produces (adapt `examples/horde/src/bench/mod.rs`, dropping Native):
  - `pub enum Backend { Null, Supersolid }`, `parse_args`, `pub fn build_bench_app(backend: Backend, cfg: CitadelConfig) -> App` (base plugins like horde; insert `CitadelConfig`; add `SimPlugin`; add `SupersolidUiPlugin` for `Supersolid`; manual `DT` clock).
  - `time_backend`, `stats_from`, `Stats`, `Report { backend, cap, frames, total, shared_ms, ui_ms, marshal_ms }` (`cap` = `building_count`), `run_report` (supersolid: `shared` = null mean, `marshal_ms` via `probe_marshal` calling `build_frame`), `report_table`, `report_json` (fields: `backend, enemy_cap`→rename `building_count`, `frames, total_mean_ms, p50/95/99, fps, shared_ms, ui_ms, marshal_ms`), `sweep_table`, `sample_workload`/`workload_line` (live element counts), `sim_for(cap,seed)`.
  - `--sweep` maps to `building_count`. No `--preset` (single steady-state preset); keep flag parsing tolerant (accept & ignore `--preset` if passed).
- `src/bin/bench.rs`: adapt `examples/horde/src/bin/bench.rs` (drop native; keep `--profile`, `--dhat`, json/table, sweep).

- [ ] **Step 1: Write failing tests** in `bench/mod.rs` (adapt horde's bench tests): `report_computes_ui_cost_as_total_minus_shared` (assert `ui_ms == total_mean - shared_ms`, `ui_ms >= 0`, supersolid has `marshal_ms.is_some()`), `reaches_populated_ui` (after N updates on Supersolid, workload live-node count `> 300`), `json_has_expected_fields`.

- [ ] **Step 2: Run to verify fail.** Run: `cargo test -p citadel --features bench --lib bench`. Expected: FAIL.

- [ ] **Step 3: Implement `bench/mod.rs`** and `src/bin/bench.rs`; re-enable the bin in `Cargo.toml`; add `pub mod bench;` to lib (feature-gate with `#[cfg(feature="bench")]` if horde does, else plain).

- [ ] **Step 4: Run tests.** Run: `cargo test -p citadel --features bench --lib bench`. Expected: PASS.

- [ ] **Step 5: Smoke-run the bench.** Run: `cargo run --release -p citadel --features bench --bin citadel-bench -- --backend supersolid --frames 60 --warmup 120 --format json`. Expected: valid JSON with non-trivial `ui_ms` (tens of ms, since the whole static tree reconciles).

- [ ] **Step 6: Leave changes in worktree. DO NOT commit or `git add`.**

---

### Task 9: Per-stage profile mode

**Files:**
- Create: `examples/citadel/src/bench/profile.rs`
- Modify: `examples/citadel/src/bench/mod.rs` (`pub mod profile;`) and `src/bin/bench.rs` (`--profile` dispatch)

**Interfaces:**
- Consumes: `build_bench_app`, `Backend`, `CitadelConfig`.
- Produces: `pub fn run_profile(cfg: CitadelConfig, frames: usize, warmup: usize)` — copy `examples/horde/src/bench/profile.rs` almost verbatim (the `SystemTimingLayer`, `Bucket`, `bucket_for`, `print_report`). **Remove horde's `keep_player_alive` god-mode** (citadel is steady-state; no god-mode needed). `run_profile` builds `build_bench_app(Backend::Supersolid, cfg)`, warms up, records `frames`, prints the per-stage table + one-liner (Boa/reconcile/flair/taffy/marshal).

- [ ] **Step 1: Implement `profile.rs`** per above; wire `--profile` in `bin/bench.rs` (as horde does: parse `--profile`, call `run_profile`).

- [ ] **Step 2: Build with trace.** Run: `cargo build --release -p citadel --features bench,bevy/trace --bin citadel-bench`. Expected: PASS.

- [ ] **Step 3: Smoke-run profile.** Run: `cargo run --release -p citadel --features bench,bevy/trace --bin citadel-bench -- --profile --frames 120 --warmup 200`. Expected: per-stage table prints; **flair cascade %** is a visible, non-trivial share (the thing we're about to optimize). Note the Boa% — it should be modest (fine-grained), NOT ~74%; if Boa dominates, flag it (risk from spec §11) before proceeding.

- [ ] **Step 4: Leave changes in worktree. DO NOT commit or `git add`.**

---

### Task 10: Windowed app + benchmark.md doc

**Files:**
- Rewrite: `examples/citadel/src/main.rs`
- Create: `examples/citadel/benchmark.md`

**Interfaces:**
- Consumes: `SimPlugin`, `SupersolidUiPlugin`.
- Produces: `cargo run -p citadel` opens a window running the sim + UI (adapt a minimal `DefaultPlugins` + `SimPlugin` + `SupersolidUiPlugin` app; no menu, no input needed). `benchmark.md` mirrors horde's: describes the intended load profile, backends (null+supersolid), `--profile`, `--sweep`, and the before/after method.

- [ ] **Step 1: Write `main.rs`** — `App::new().add_plugins(DefaultPlugins).add_plugins((SimPlugin, SupersolidUiPlugin)).run()`. Insert a fixed manual `DT`? For the windowed app real-time is fine; keep it simple (default time). Ensure `CitadelConfig` is inserted (SimPlugin does it).

- [ ] **Step 2: Build.** Run: `cargo build -p citadel`. Expected: PASS.

- [ ] **Step 3: Write `benchmark.md`** describing the profile and run commands (adapt horde's, null+supersolid only).

- [ ] **Step 4: Leave changes in worktree. DO NOT commit or `git add`.**

---

### Task 11: Drift-controlled before/after measurement (no code change to keep)

**Files:**
- None persisted (measurement only). Temp binaries go in `target/`; scratch notes may go in `target/bench/`.

**Interfaces:**
- Consumes: the guarded `crates/superui_bridge/src/reconcile.rs` (already in worktree) and a baseline variant.

- [ ] **Step 1: Confirm the guard is present** in `crates/superui_bridge/src/reconcile.rs` (the `ec.get::<ClassList>() != Some(&new_classes)` / `AttributeList` guards). Build the **guarded** non-trace + trace bench binaries and copy them aside: `cp target/release/citadel-bench.exe target/release/cit-after.exe` (and a `cit-after-trace.exe` from the `bevy/trace` build).

- [ ] **Step 2: Build the baseline WITHOUT git.** Copy `reconcile.rs` to `reconcile.rs.bak`. Edit `reconcile.rs` to restore the unconditional inserts (`ec.insert(ClassList…)` / `ec.insert(attrs)` with no equality check). Build non-trace + trace bench binaries → `cit-before.exe`, `cit-before-trace.exe`. Restore: copy `reconcile.rs.bak` back over `reconcile.rs`, delete the `.bak`. **Never use git for this.**

- [ ] **Step 3: Pick a fixed loaded config** (e.g. `--building_count`/default 120, `--frames 120 --warmup 200`) and run the non-trace binaries **alternately** for ≥8 pairs; capture `ui_ms` each. Then run the trace binaries alternately (≥5 pairs) capturing the `--profile` flair-cascade + reconcile + Boa + total lines.

- [ ] **Step 4: Compute paired deltas** (mean/median, per-pair sign, simple paired-t) for total `ui_ms` and for the flair-cascade + reconcile stages, exactly as the prior session did. Also run a `--sweep` on both binaries to show the win scaling with node count.

- [ ] **Step 5: Report the numbers** to the user: before/after flair cascade, reconcile, total; whether the win is large (expected) or not, and the honest interpretation. **Do not commit. Leave `reconcile.rs` with the guard in the worktree.** Clean up temp `cit-*.exe`.

---

## Self-Review

**Spec coverage:** §1 purpose → Tasks 7/11; §3 architecture/crate layout → Tasks 1–10; §4 sim → Tasks 2–4; §5 bridge → Task 5; §6 fine-grained TSX → Task 7 (reactivity requirement); §7 styling → Task 7; §8 harness → Tasks 8–9; §9 before/after → Task 11; §10 success criteria → Tasks 7/10/11; §11 risks → called out in Task 9 Step 3 (Boa%) and Task 11 interpretation. Covered.

**Placeholder scan:** No "TBD/TODO/handle edge cases". Copy-heavy code points at exact horde files with exact substitutions (actionable, DRY) rather than transcribing hundreds of lines. Test code is concrete.

**Type consistency:** `Economy`/`build_economy`/`tick_economy` (Task 3) used by Tasks 4/8; `UiSnapshot`/`assemble_from` (Task 4) used by Tasks 5/8; `build_frame`/`register_bridge`/`FrameDto` (Task 5) used by Tasks 6/8; `SupersolidUiPlugin` (Task 6) used by Tasks 8/10; `Backend`/`build_bench_app`/`run_report` (Task 8) used by Task 9. `class()`/`name()`/`icon()` fns defined in Task 2, consumed in Task 4. Consistent.

**Note on no-commit:** every task ends "leave in worktree; do not commit or `git add`" — overrides the skill's default commit steps per explicit user instruction.
