# Horde Supersolid UI Port — Implementation Plan

**Status: COMPLETE (2026-07-21).** Windowed launch verified, all panels render, viewport-sizing bug fixed (commit e6e7d97). Tests: 32 passing (supersolid default), 20 passing (native). Perf baseline documented: ~5 FPS under stress swarm (reactive-store follow-up is the named gap).

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port the entire `examples/horde` UI from native `bevy_ui` to superui TSX (supersolid), keeping the native UI behind an opt-in `ui-native` feature that is removed from `default`, with both backends reading the identical `UiSnapshot` and raising the identical intents.

**Architecture:** A backend-owned system serializes `UiSnapshot` + `GameState` to JSON each frame and pushes it via the existing `window.bevy` event bridge (`bevy.on("frame", cb)`); a single-file `app.tsx` sets one `snapshot` signal and derives all panels with memos + keyed `<For>`. TSX buttons call `bevy.send("HordeIntent", …)`, mapped back onto the existing `IntentQueue`. `sim/` is untouched; the world→screen projection is lifted to a shared module both backends use.

**Tech Stack:** Rust + Bevy 0.17, `superui` / `superui_css` / `superui_bridge` crates, `supersolid` transpiler (host-only build-dep), Boa JS engine, flair CSS.

**Design spec:** `docs/superpowers/specs/2026-07-21-horde-supersolid-ui-port-design.md`.
**Reference example:** `examples/todomvc_supersolid/` (asset scaffold, build.rs, HMR gate, headless test harness).

## Global Constraints

- **Single-file TSX only.** The supersolid transpiler warns-and-strips any non-runtime import (`crates/supersolid/src/imports.rs`). All panels/screens are component functions inside one `assets/ui/horde/app.tsx`. No local `.tsx`/`.ts` module imports.
- **`sim/` stays pure.** Do not add `serde`, `ui`, `bevy_ui`, or Boa to `sim/`. The JSON DTO lives in the supersolid backend and is built *from* `UiSnapshot`; do not derive `Serialize` on sim types.
- **`ui-native` is removed from `default`.** Default build = supersolid. Native = `--no-default-features --features ui-native`. The two backends are mutually exclusive via `cfg`.
- **Both backends read the same `UiSnapshot` and raise the same `Intent`s** (differential-oracle parity).
- **Control-flow gotcha:** `<For>`/`<Show>`/`<Switch>` used as an element child MUST be wrapped in `{…}` (e.g. `{<For …>…</For>}`) or the transpiler routes them through `$ss.child` and they render nothing. Always `{…}`-wrap control-flow inside plain elements.
- **Dynamic layout via bound `style` string.** Set dynamic `left`/`top`/`width`/`background-color`/`opacity` by binding the whole `style` attribute string (e.g. `style={`width: ${w()}%`}`). This is the confirmed code path (`crates/superui_bridge/src/reconcile.rs:381` → `InlineStyle`). Keep static layout (e.g. `position: absolute`) in CSS classes; put only the dynamic props in the inline `style`.
- **Windows main-thread stack** is already `/STACK:8MB` in `.cargo/config.toml` (Boa parses render.js at mount). Green headless tests run on big-stack worker threads and do NOT prove a windowed launch — a windowed smoke check is required (Task C3).
- **Colors:** flair parses `#rrggbb`, `#rrggbbaa`, `rgb()/rgba()`, named. Semi-transparent panels/backdrops may use `rgba(...)`.
- Mirror `todomvc_supersolid` for build.rs, the `USE_LIVE_TSX` gate, and feature shape.

---

## File Structure

**Rust (backend):**
- `examples/horde/src/ui/mod.rs` — MODIFY: `add_ui()` adds shared projection, then `cfg`-selects `native` | `supersolid`. Panic arm removed.
- `examples/horde/src/ui/project.rs` — CREATE (moved from `native/project.rs`): shared `project_snapshot`.
- `examples/horde/src/ui/native/mod.rs` — MODIFY: drop `pub mod project;` and its system (now shared).
- `examples/horde/src/ui/native/project.rs` — DELETE (moved).
- `examples/horde/src/ui/supersolid/mod.rs` — CREATE: `SupersolidUiPlugin` (mount root, register bridge, `push_ui_frame`).
- `examples/horde/src/ui/supersolid/bridge.rs` — CREATE: JSON DTOs, `HordeIntent`/`AdjustEnemyCap` commands + observers, `ToggleInventoryFwd` event, DTO builder.
- `examples/horde/Cargo.toml` — MODIFY: deps + feature rearrangement.
- `examples/horde/build.rs` — CREATE: pre-transpile `app.tsx` → `app.generated.js`.

**Assets (TSX/CSS):**
- `examples/horde/assets/ui/horde/index.html` — CREATE: `<div id="root"></div>`.
- `examples/horde/assets/ui/horde/supersolid-shim.d.ts` — CREATE: IDE ambient types.
- `examples/horde/assets/ui/horde/theme.css` — CREATE: palette/spacing custom properties + base.
- `examples/horde/assets/ui/horde/components.css` — CREATE: panel/bar/button/screen styling.
- `examples/horde/assets/ui/horde/app.tsx` — CREATE + grow: root App + all panels/screens.
- `examples/horde/assets/ui/horde/app.generated.js` — build.rs output (git-ignored or committed like todomvc).

**Tests:**
- `examples/horde/tests/support/mod.rs` — CREATE: headless harness (mount, tick, snapshot injection, DOM readback, click).
- `examples/horde/tests/supersolid_ui.rs` — CREATE + grow: per-panel integration tests.

---

## DTO / bridge contract (single source of truth for all tasks)

The JSON object delivered to JS via `bevy.on("frame", f => …)` has exactly these fields (built in `bridge.rs`):

```
f.state            : "MainMenu" | "Playing" | "Paused" | "GameOver"
f.player_hp        : number      f.player_max_hp : number
f.xp : number   f.level : number   f.wave : number   f.kills : number   f.pickups : number
f.active_weapon    : string | null      // weapon name, e.g. "Pistol"
f.ammo : number   f.ammo_size : number   f.reloading : bool   f.cooldown_frac : number
f.dps : number    f.elapsed : number     // elapsed seconds
f.inventory : [ { index:number, name:string, active:bool,
                  dmg:number, rof:number, spread:number, projectiles:number,
                  mag:number, reload:number } ]
f.enemies   : [ { id:string, sx:number, sy:number, frac:number } ]   // sx/sy = viewport px
f.damage_numbers : [ { id:string, sx:number, sy:number, text:string, crit:bool, alpha:number } ]
f.blips     : [ { id:string, mx:number, my:number, kind:"player"|"enemy"|"pickup" } ] // mx/my in 0..1
f.log       : [ { text:string, alpha:number } ]
```

Intents (JS → Rust): `bevy.send("HordeIntent", { kind, index })` where `kind ∈
{"StartGame","Pause","Resume","Restart","Quit","SwitchWeapon"}` and `index` used only for
`SwitchWeapon`. Settings: `bevy.send("AdjustEnemyCap", { delta })`. Inventory keyboard parity:
`bevy.on("toggleInventory", () => …)`.

`id` values are strings (`u64` rendered as a JSON string is safe; build them with `.to_string()`).

---

## Phase A — Rust backend scaffolding

### Task A1: Lift `project_snapshot` to a shared module

**Files:**
- Create: `examples/horde/src/ui/project.rs`
- Delete: `examples/horde/src/ui/native/project.rs`
- Modify: `examples/horde/src/ui/native/mod.rs`, `examples/horde/src/ui/mod.rs`

**Interfaces:**
- Produces: `crate::ui::project::project_snapshot` (Bevy system) usable by both backends.

- [ ] **Step 1: Create the shared projection module** — copy the existing file verbatim.

`examples/horde/src/ui/project.rs`:
```rust
use bevy::prelude::*;
use crate::sim::UiSnapshot;

/// Fills `screen_pos` for every world-positioned snapshot item using the 2D camera.
/// Skipped cleanly when there is no camera/window (headless). Shared by both UI backends.
pub fn project_snapshot(
    mut snap: ResMut<UiSnapshot>,
    cameras: Query<(&Camera, &GlobalTransform)>,
) {
    let Ok((camera, cam_t)) = cameras.single() else { return };
    let project = |world: Vec2| -> Vec2 {
        camera.world_to_viewport(cam_t, world.extend(0.0)).unwrap_or(Vec2::new(-1000.0, -1000.0))
    };
    for n in snap.enemies.iter_mut() { n.screen_pos = project(n.world_pos); }
    for d in snap.damage_numbers.iter_mut() { d.screen_pos = project(d.world_pos); }
    for b in snap.blips.iter_mut() { b.screen_pos = project(b.world_pos); }
}
```

- [ ] **Step 2: Delete the native copy**

Run: `git rm examples/horde/src/ui/native/project.rs`

- [ ] **Step 3: Remove projection from `NativeUiPlugin`**

In `examples/horde/src/ui/native/mod.rs`: delete the line `pub mod project;` and change the plugin body so it no longer adds `project::project_snapshot`. The result:
```rust
use bevy::prelude::*;

pub mod theme;
pub mod widgets;
pub mod interaction;
pub mod hud;
pub mod screens;

pub struct NativeUiPlugin;

impl Plugin for NativeUiPlugin {
    fn build(&self, app: &mut App) {
        app
            // Generic button hover/press feedback (runs in every state).
            .add_systems(Update, interaction::button_feedback)
            .add_plugins((hud::HudPlugin, screens::ScreensPlugin));
    }
}
```

- [ ] **Step 4: Add shared projection in `ui/mod.rs`**

Replace `examples/horde/src/ui/mod.rs` with:
```rust
use bevy::prelude::*;

pub mod project;

#[cfg(feature = "ui-native")]
pub mod native;

pub fn add_ui(app: &mut App) {
    // Shared boundary: fills screen_pos from world_pos, for BOTH backends.
    app.add_systems(
        Update,
        project::project_snapshot.after(crate::sim::snapshot::assemble_world_snapshot),
    );

    #[cfg(feature = "ui-native")]
    app.add_plugins(native::NativeUiPlugin);

    #[cfg(not(feature = "ui-native"))]
    panic!(
        "Supersolid UI backend not yet wired — see Task A4. TODO(supersolid-runtime)."
    );
}
```
(The panic arm is temporary; Task A4 replaces it with `SupersolidUiPlugin`.)

- [ ] **Step 5: Verify native still builds and tests pass**

Run: `cargo build -p horde` (default still includes `ui-native` at this point)
Expected: builds clean.
Run: `cargo test -p horde`
Expected: existing sim tests pass.

- [ ] **Step 6: Commit**
```bash
git add examples/horde/src/ui
git commit -m "refactor(horde): lift project_snapshot to shared ui::project for both backends"
```

---

### Task A2: Add supersolid dependencies + feature scaffolding to Cargo.toml

**Files:**
- Modify: `examples/horde/Cargo.toml`

- [ ] **Step 1: Rewrite `Cargo.toml`** (keeps `ui-native` in default for now; Task A4 removes it)
```toml
[package]
name = "horde"
version = "0.1.0"
edition = "2021"
publish = false

[features]
# NOTE (Task A4 flips this): ui-native is temporarily still in default so the repo
# stays runnable until the supersolid backend is wired.
default = ["ui-native", "debug-ui"]
ui-native = []
# Native state-preserving hot reload: live app.tsx via TsxLoader + the asset watcher.
hmr = ["superui/hmr", "bevy/file_watcher"]
debug-ui = ["bevy/bevy_dev_tools"]
mcp_debug = ["dep:bevy_brp_extras", "bevy/bevy_remote"]

[dependencies]
bevy = { version = "0.17" }
superui = { path = "../../crates/superui" }
superui_css = { path = "../../crates/superui_css" }
superui_bridge = { path = "../../crates/superui_bridge" }
serde = { version = "1", features = ["derive"] }

[dependencies.bevy_brp_extras]
optional = true
version = "0.17.3"

# Host-only: pre-transpile app.tsx -> app.generated.js for wasm / no-HMR native.
[build-dependencies]
supersolid = { path = "../../crates/supersolid" }

[dev-dependencies]
superui_dom = { path = "../../crates/superui_dom" }
superui_html = { path = "../../crates/superui_html" }

# file_watcher (hot-reload) is native-only; wasm build drops it.
[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
bevy = { version = "0.17", features = ["file_watcher"] }

[target.'cfg(target_arch = "wasm32")'.dependencies]
getrandom = { version = "0.3", features = ["wasm_js"] }
```

- [ ] **Step 2: Verify it still builds**

Run: `cargo build -p horde`
Expected: builds (new deps compile; nothing uses them yet).

- [ ] **Step 3: Commit**
```bash
git add examples/horde/Cargo.toml
git commit -m "build(horde): add superui/supersolid deps + hmr feature scaffolding"
```

---

### Task A3: Asset scaffolding + build.rs + minimal app.tsx

**Files:**
- Create: `examples/horde/assets/ui/horde/index.html`, `supersolid-shim.d.ts`, `theme.css`, `components.css`, `app.tsx`
- Create: `examples/horde/build.rs`

**Interfaces:**
- Produces: `assets/ui/horde/app.tsx` (root `App` reading a `frame` signal) and `app.generated.js` (build.rs output). The `bevy.on("frame")` contract per the DTO section.

- [ ] **Step 1: index.html**

`examples/horde/assets/ui/horde/index.html`:
```html
<div id="root"></div>
```

- [ ] **Step 2: supersolid-shim.d.ts** — copy `examples/todomvc_supersolid/assets/ui/todomvc_supersolid/supersolid-shim.d.ts` verbatim, then add `Match`, `onMount`, `onCleanup` to the `declare module "supersolid"` block:
```ts
  export const Switch: (props: { children: unknown }) => unknown;
  export const Match: (props: { when: unknown; children: unknown }) => unknown;
  export function onMount(fn: () => void): void;
  export function onCleanup(fn: () => void): void;
```

- [ ] **Step 3: theme.css** (palette from `ui/native/theme.rs`, converted to hex)

`examples/horde/assets/ui/horde/theme.css`:
```css
:root {
  --bg: #090b13;
  --panel: #171c2b;
  --panel-top: #262e45;
  --panel-border: #3d527a;
  --text: #edf5ff;
  --text-dim: #8ca1c7;
  --accent: #40d9ff;
  --danger: #ff5461;
  --good: #59eb8c;
  --warn: #ffc747;
  --bar-track: #1a1f2e;
  --slot-active: #384d6b;
  --slot-idle: #262938;
  --btn-idle: #212940;
  --btn-hover: #36456b;
  --btn-pressed: #1a2133;
  --space: 8px;
  --radius: 8px;
}

#root {
  position: absolute;
  width: 100%;
  height: 100%;
}
```

- [ ] **Step 4: components.css** — start minimal; Task C1 fills it out. Create the file with just:
```css
/* Horde component styles. Expanded in Task C1. */
#hud { position: absolute; width: 100%; height: 100%; }
```

- [ ] **Step 5: Minimal app.tsx** (root reads the frame signal; proves mount + bridge)

`examples/horde/assets/ui/horde/app.tsx`:
```tsx
import { createSignal, render } from "supersolid";

// Default/empty frame until the first bevy.on("frame") arrives.
const EMPTY = {
  state: "MainMenu",
  player_hp: 0, player_max_hp: 1, xp: 0, level: 0, wave: 0, kills: 0, pickups: 0,
  active_weapon: null, ammo: 0, ammo_size: 0, reloading: false, cooldown_frac: 0,
  dps: 0, elapsed: 0,
  inventory: [], enemies: [], damage_numbers: [], blips: [], log: [],
};

function App() {
  const [frame, setFrame] = createSignal(EMPTY);
  // Rust pushes the whole UiSnapshot+state here every frame (design §2).
  bevy.on("frame", (f) => setFrame(f));

  return (
    <div id="hud">
      <h1 id="title">HORDE</h1>
      <span id="state">{frame().state}</span>
    </div>
  );
}

render(() => <App />, document.getElementById("root"));
```
(`bevy` is a global installed by the bridge; no import needed. The shim doesn't declare it — that's fine, it's a runtime global.)

- [ ] **Step 6: build.rs** — copy the todomvc pattern with horde paths.

`examples/horde/build.rs`:
```rust
//! Pre-transpile the Supersolid app so wasm and no-HMR native builds have plain
//! `.js` to load. Build scripts compile for the HOST, so `supersolid` (oxc) never
//! enters the wasm binary.

use std::path::PathBuf;

fn main() {
    let dir = PathBuf::from("assets/ui/horde");
    let input = dir.join("app.tsx");
    let output = dir.join("app.generated.js");
    println!("cargo:rerun-if-changed={}", input.display());
    match supersolid::transpile_file(&input, &output) {
        Ok(result) => {
            for d in &result.diagnostics {
                println!("cargo:warning=supersolid: {}", d.message);
            }
        }
        Err(e) => {
            println!("cargo:warning=supersolid: could not transpile {}: {e}", input.display());
        }
    }
}
```

- [ ] **Step 7: Build and confirm generated JS appears**

Run: `cargo build -p horde`
Expected: builds; `examples/horde/assets/ui/horde/app.generated.js` now exists.
Run: `git status --porcelain examples/horde/assets/ui/horde/app.generated.js`
Expected: shows the generated file.

- [ ] **Step 8: Commit** (commit the generated JS too, matching todomvc)
```bash
git add examples/horde/assets/ui/horde examples/horde/build.rs
git commit -m "feat(horde): supersolid asset scaffold (index/theme/app.tsx) + build.rs pre-transpile"
```

---

### Task A4: Wire the supersolid backend + flip the default feature

**Files:**
- Create: `examples/horde/src/ui/supersolid/bridge.rs`
- Create: `examples/horde/src/ui/supersolid/mod.rs`
- Modify: `examples/horde/src/ui/mod.rs`, `examples/horde/Cargo.toml`

**Interfaces:**
- Consumes: `crate::sim::{UiSnapshot, SimConfig, IntentQueue, Intent, WeaponKind, weapon_stats, BlipKind}`, `crate::game_state::GameState`, `superui::prelude::{SuperUiPlugin, SuperUiRoot, SuperUiApp}`, `superui::JsSource`, `superui_css::style::StyleSheet`.
- Produces: `SupersolidUiPlugin`, `bridge::{FrameDto, build_frame, register_bridge, HordeIntent, AdjustEnemyCap, ToggleInventoryFwd}`.

- [ ] **Step 1: bridge.rs — DTOs, commands, event, builder, unit tests**

`examples/horde/src/ui/supersolid/bridge.rs`:
```rust
//! JSON marshalling between the Rust sim and the TSX UI (design §2). The DTO is
//! built FROM `UiSnapshot` + `GameState` here so `sim/` stays serde-free.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::game_state::GameState;
use crate::sim::snapshot::BlipKind;
use crate::sim::{weapon_stats, Intent, IntentQueue, SimConfig, UiSnapshot};

// ── Rust → JS: the per-frame payload ─────────────────────────────────────────

#[derive(Serialize)]
pub struct SlotDto {
    pub index: usize, pub name: &'static str, pub active: bool,
    pub dmg: f32, pub rof: f32, pub spread: f32, pub projectiles: u32,
    pub mag: u32, pub reload: f32,
}
#[derive(Serialize)]
pub struct EnemyDto { pub id: String, pub sx: f32, pub sy: f32, pub frac: f32 }
#[derive(Serialize)]
pub struct DmgDto { pub id: String, pub sx: f32, pub sy: f32, pub text: String, pub crit: bool, pub alpha: f32 }
#[derive(Serialize)]
pub struct BlipDto { pub id: String, pub mx: f32, pub my: f32, pub kind: &'static str }
#[derive(Serialize)]
pub struct LogDto { pub text: String, pub alpha: f32 }

/// Whole-frame payload. Triggered every frame; forwarded to JS `bevy.on("frame")`.
#[derive(Event, Serialize)]
pub struct FrameDto {
    pub state: &'static str,
    pub player_hp: f32, pub player_max_hp: f32,
    pub xp: u32, pub level: u32, pub wave: u32, pub kills: u32, pub pickups: u32,
    pub active_weapon: Option<&'static str>,
    pub ammo: u32, pub ammo_size: u32, pub reloading: bool, pub cooldown_frac: f32,
    pub dps: f32, pub elapsed: f32,
    pub inventory: Vec<SlotDto>,
    pub enemies: Vec<EnemyDto>,
    pub damage_numbers: Vec<DmgDto>,
    pub blips: Vec<BlipDto>,
    pub log: Vec<LogDto>,
}

fn state_name(s: &GameState) -> &'static str {
    match s {
        GameState::MainMenu => "MainMenu",
        GameState::Playing => "Playing",
        GameState::Paused => "Paused",
        GameState::GameOver => "GameOver",
    }
}

/// Build the JSON DTO from the current snapshot + state. `arena_half` normalizes
/// minimap blip positions to 0..1 (matching native minimap projection).
pub fn build_frame(snap: &UiSnapshot, state: &GameState, arena_half: f32) -> FrameDto {
    let inventory = snap.inventory.iter().map(|s| {
        let st = weapon_stats(s.kind);
        SlotDto {
            index: s.index, name: s.kind.name(), active: s.active,
            dmg: st.damage, rof: st.fire_interval, spread: st.spread,
            projectiles: st.projectiles, mag: st.mag_size, reload: st.reload_time,
        }
    }).collect();

    let enemies = snap.enemies.iter().map(|n| EnemyDto {
        id: n.id.to_string(), sx: n.screen_pos.x, sy: n.screen_pos.y,
        frac: (n.hp / n.max_hp.max(0.0001)).clamp(0.0, 1.0),
    }).collect();

    let damage_numbers = snap.damage_numbers.iter().map(|d| DmgDto {
        id: d.id.to_string(), sx: d.screen_pos.x, sy: d.screen_pos.y,
        text: format!("{}", d.amount.round() as i32),
        crit: d.crit,
        alpha: (1.0 - d.age / d.ttl.max(0.0001)).clamp(0.0, 1.0),
    }).collect();

    let half = arena_half.max(0.0001);
    let blips = snap.blips.iter().map(|b| {
        let nx = (b.world_pos.x / half).clamp(-1.0, 1.0);
        let ny = (b.world_pos.y / half).clamp(-1.0, 1.0);
        BlipDto {
            id: b.id.to_string(),
            mx: nx * 0.5 + 0.5,
            my: (-ny) * 0.5 + 0.5,
            kind: match b.kind { BlipKind::Player => "player", BlipKind::Enemy => "enemy", BlipKind::Pickup => "pickup" },
        }
    }).collect();

    let log = snap.log.iter().map(|l| LogDto {
        text: l.text.clone(),
        alpha: (1.0 - l.age / 6.0).clamp(0.25, 1.0),
    }).collect();

    FrameDto {
        state: state_name(state),
        player_hp: snap.player_hp, player_max_hp: snap.player_max_hp,
        xp: snap.xp, level: snap.level, wave: snap.wave, kills: snap.kills, pickups: snap.pickups,
        active_weapon: snap.active_weapon.map(|w| w.name()),
        ammo: snap.ammo, ammo_size: snap.ammo_size, reloading: snap.reloading, cooldown_frac: snap.cooldown_frac,
        dps: snap.dps, elapsed: snap.elapsed,
        inventory, enemies, damage_numbers, blips, log,
    }
}

// ── JS → Rust: intents & config ──────────────────────────────────────────────

/// `bevy.send("HordeIntent", { kind, index })` → onto the existing IntentQueue.
#[derive(Event, Deserialize)]
pub struct HordeIntent {
    pub kind: String,
    #[serde(default)]
    pub index: i64,
}

/// `bevy.send("AdjustEnemyCap", { delta })` → mutate SimConfig.enemy_cap (settings knob).
#[derive(Event, Deserialize)]
pub struct AdjustEnemyCap {
    pub delta: i64,
}

/// ECS → JS: forwards the keyboard `ToggleInventory` intent to the TSX inventory modal.
#[derive(Event, Serialize)]
pub struct ToggleInventoryFwd;

fn on_horde_intent(ev: On<HordeIntent>, mut intents: ResMut<IntentQueue>, mut exit: MessageWriter<AppExit>) {
    let e = ev.event();
    match e.kind.as_str() {
        "StartGame" => intents.push(Intent::StartGame),
        "Pause" => intents.push(Intent::Pause),
        "Resume" => intents.push(Intent::Resume),
        "Restart" => intents.push(Intent::Restart),
        "SwitchWeapon" => intents.push(Intent::SwitchWeapon(e.index.max(0) as usize)),
        "Quit" => { exit.write(AppExit::Success); }
        other => warn!("horde: unknown HordeIntent kind '{other}'"),
    }
}

fn on_adjust_enemy_cap(ev: On<AdjustEnemyCap>, mut cfg: ResMut<SimConfig>) {
    let next = (cfg.enemy_cap as i64 + ev.event().delta).clamp(0, 800);
    cfg.enemy_cap = next as u32;
}

/// Register the JS-visible command/event surface. Called by SupersolidUiPlugin
/// and by the test harness.
pub fn register_bridge(app: &mut App) {
    use superui::prelude::SuperUiApp;
    app.add_superui_command::<HordeIntent>("HordeIntent")
        .add_superui_command::<AdjustEnemyCap>("AdjustEnemyCap")
        .add_superui_event::<FrameDto>("frame")
        .add_superui_event::<ToggleInventoryFwd>("toggleInventory")
        .add_observer(on_horde_intent)
        .add_observer(on_adjust_enemy_cap);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn horde_intent_maps_switch_weapon() {
        let mut app = App::new();
        app.init_resource::<IntentQueue>();
        app.add_message::<AppExit>();
        app.add_observer(on_horde_intent);
        app.world_mut().trigger(HordeIntent { kind: "SwitchWeapon".into(), index: 2 });
        let q = app.world().resource::<IntentQueue>();
        assert!(matches!(q.0.as_slice(), [Intent::SwitchWeapon(2)]));
    }
}
```

Note: confirm `SimConfig` has a public `enemy_cap: u32` and `arena_half: f32` (see `sim/config.rs`); if a name differs, adjust here and in Task B6/A4. `BlipKind` is re-exported from `crate::sim::snapshot`.

- [ ] **Step 2: mod.rs — the plugin**

`examples/horde/src/ui/supersolid/mod.rs`:
```rust
use bevy::prelude::*;
use superui::prelude::{SuperUiPlugin, SuperUiRoot};
use superui::JsSource;
use superui_css::style::StyleSheet;

use crate::game_state::GameState;
use crate::sim::{IntentQueue, Intent, SimConfig, UiSnapshot};

pub mod bridge;
use bridge::{build_frame, register_bridge, ToggleInventoryFwd};

/// Live `.tsx` (transpiled at load, hot-reloadable) only on native + `hmr`;
/// every other build loads the pre-transpiled `.js`.
const USE_LIVE_TSX: bool = cfg!(all(not(target_arch = "wasm32"), feature = "hmr"));

pub struct SupersolidUiPlugin;

impl Plugin for SupersolidUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(SuperUiPlugin);
        register_bridge(app);
        app.add_systems(Startup, mount_ui);
        // Push the whole frame every update; forwarded to JS bevy.on("frame").
        app.add_systems(Update, (push_ui_frame, forward_toggle_inventory));
    }
}

fn mount_ui(mut commands: Commands, assets: Res<AssetServer>) {
    let js: Handle<JsSource> = if USE_LIVE_TSX {
        assets.load("ui/horde/app.tsx")
    } else {
        assets.load("ui/horde/app.generated.js")
    };
    commands.spawn((
        Node::default(),
        SuperUiRoot {
            html: assets.load("ui/horde/index.html"),
            css: assets.load::<StyleSheet>("ui/horde/theme.css"),
            js,
        },
    ));
}

fn push_ui_frame(
    snap: Res<UiSnapshot>,
    state: Res<State<GameState>>,
    cfg: Res<SimConfig>,
    mut commands: Commands,
) {
    let frame = build_frame(&snap, state.get(), cfg.arena_half);
    commands.trigger(frame);
}

/// Forward keyboard `ToggleInventory` to the TSX inventory modal (keyboard parity).
fn forward_toggle_inventory(intents: Res<IntentQueue>, mut commands: Commands) {
    if intents.0.iter().any(|i| matches!(i, Intent::ToggleInventory)) {
        commands.trigger(ToggleInventoryFwd);
    }
}
```

Note: `SuperUiRoot.css` loads `theme.css`; the cascade also needs `components.css`. Confirm how `todomvc_supersolid` links a second stylesheet — if `SuperUiRoot` takes a single `css` handle, `@import "components.css";` at the top of `theme.css` OR merge both into one `style.css`. **Decision:** merge — put all CSS in a single `theme.css` for now (Task C1 keeps the palette `:root` block at the top and appends component rules), and delete the separate `components.css` reference. Update Task A3/C1 file list accordingly: single `assets/ui/horde/theme.css`.

- [ ] **Step 3: Replace the panic arm in `ui/mod.rs`**

In `examples/horde/src/ui/mod.rs`, add the module and swap the panic for the plugin:
```rust
#[cfg(not(feature = "ui-native"))]
pub mod supersolid;
```
and in `add_ui`:
```rust
    #[cfg(not(feature = "ui-native"))]
    app.add_plugins(supersolid::SupersolidUiPlugin);
```
(remove the `panic!`).

- [ ] **Step 4: Flip the default feature** — in `examples/horde/Cargo.toml`, change:
```toml
default = ["debug-ui"]
```
(remove `ui-native` from `default`; it remains opt-in).

- [ ] **Step 5: Build both backends**

Run: `cargo build -p horde`
Expected: builds (supersolid backend, default).
Run: `cargo build -p horde --no-default-features --features ui-native`
Expected: builds (native backend).

- [ ] **Step 6: Run bridge unit test**

Run: `cargo test -p horde --lib` (or `cargo test -p horde`)
Expected: `horde_intent_maps_switch_weapon` passes.

- [ ] **Step 7: Commit**
```bash
git add examples/horde/src/ui examples/horde/Cargo.toml
git commit -m "feat(horde): wire SupersolidUiPlugin + bridge; default backend = supersolid"
```

---

## Phase B — TSX authoring (headless-tested)

### Task B0: Headless test harness + dynamic-`style` spike

**Files:**
- Create: `examples/horde/tests/support/mod.rs`
- Create: `examples/horde/tests/supersolid_ui.rs`
- Modify: `examples/horde/assets/ui/horde/app.tsx` (add a bound-style bar to prove the path)

**Interfaces:**
- Produces: harness fns `app()`, `mount()`, `tick()`, `set_state()`, `edit_snapshot()`, `node_by_selector()`, `nodes_by_selector()`, `text_content()`, `attr()`, `classes()`, `click()`.

- [ ] **Step 1: Harness** — `examples/horde/tests/support/mod.rs`:
```rust
//! Headless harness: mount the REAL authored Supersolid assets through the real
//! `superui` runtime, inject a UiSnapshot + GameState, tick, and read the DOM.
#![allow(dead_code)]

use bevy::asset::io::memory::{Dir, MemoryAssetReader};
use bevy::asset::io::{AssetSource, AssetSourceId};
use bevy::asset::AssetPlugin;
use bevy::image::TextureAtlasPlugin;
use bevy::input::InputPlugin;
use bevy::input_focus::{InputFocus, InputFocusVisible};
use bevy::picking::{InteractionPlugin, PickingPlugin};
use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use bevy::text::TextPlugin;
use bevy::ui::UiPlugin;
use superui::prelude::{SuperUiPlugin, SuperUiRoot};
use superui::JsSource;
use superui_bridge::{PendingDomEvent, PendingDomEvents, UiRuntime};
use superui_css::style::StyleSheet;
use superui_dom::NodeId;

use horde::game_state::GameState;
use horde::sim::{IntentQueue, SimConfig, UiSnapshot};
use horde::ui::supersolid::bridge::register_bridge;

pub const HTML: &str = include_str!("../../assets/ui/horde/index.html");
pub const CSS: &str = include_str!("../../assets/ui/horde/theme.css");
pub const TSX: &str = include_str!("../../assets/ui/horde/app.tsx");

pub fn app() -> App {
    let dir = Dir::new("assets".into());
    dir.insert_asset("ui/horde/index.html".as_ref(), HTML.as_bytes());
    dir.insert_asset("ui/horde/theme.css".as_ref(), CSS.as_bytes());
    dir.insert_asset("ui/horde/app.tsx".as_ref(), TSX.as_bytes());

    let mut app = App::new();
    app.register_asset_source(
        AssetSourceId::Default,
        AssetSource::build().with_reader(move || Box::new(MemoryAssetReader { root: dir.clone() })),
    );
    app.add_plugins((
        bevy::time::TimePlugin,
        bevy::app::TaskPoolPlugin::default(),
        AssetPlugin::default(),
        WindowPlugin::default(),
        bevy::image::ImagePlugin::default(),
        TextureAtlasPlugin,
        TextPlugin,
        (InputPlugin, PickingPlugin, InteractionPlugin, UiPlugin),
        StatesPlugin,
    ));
    app.init_resource::<InputFocus>().init_resource::<InputFocusVisible>();
    app.init_state::<GameState>();
    app.init_resource::<UiSnapshot>();
    app.init_resource::<IntentQueue>();
    app.insert_resource(SimConfig::play());
    app.add_plugins(SuperUiPlugin);
    register_bridge(&mut app);
    // Emit the frame each tick from the injected snapshot (mirrors push_ui_frame,
    // but the test controls the snapshot).
    app.add_systems(Update, emit_frame);
    app.finish();
    app
}

fn emit_frame(
    snap: Res<UiSnapshot>,
    state: Res<State<GameState>>,
    cfg: Res<SimConfig>,
    mut commands: Commands,
) {
    let f = horde::ui::supersolid::bridge::build_frame(&snap, state.get(), cfg.arena_half);
    commands.trigger(f);
}

pub fn mount(app: &mut App) -> Entity {
    let (html, css, js) = {
        let s = app.world().resource::<AssetServer>().clone();
        (s.load("ui/horde/index.html"),
         s.load::<StyleSheet>("ui/horde/theme.css"),
         s.load::<JsSource>("ui/horde/app.tsx"))
    };
    let root = app.world_mut().spawn((Node::default(), SuperUiRoot { html, css, js })).id();
    for _ in 0..256 {
        app.update();
        if app.world().contains_non_send::<UiRuntime>() { break; }
    }
    root
}

pub fn tick(app: &mut App, n: usize) { for _ in 0..n { app.update(); } }

pub fn set_state(app: &mut App, s: GameState) {
    app.world_mut().resource_mut::<NextState<GameState>>().set(s);
    tick(app, 2);
}

pub fn edit_snapshot(app: &mut App, f: impl FnOnce(&mut UiSnapshot)) {
    f(&mut app.world_mut().resource_mut::<UiSnapshot>());
    tick(app, 2);
}

pub fn node_by_selector(app: &App, sel: &str) -> NodeId {
    let rt = app.world().non_send_resource::<UiRuntime>();
    let d = rt.dom.borrow();
    d.query_selector(d.document(), sel).unwrap_or_else(|| panic!("selector matched nothing: {sel}"))
}
pub fn maybe_node(app: &App, sel: &str) -> Option<NodeId> {
    let rt = app.world().non_send_resource::<UiRuntime>();
    let d = rt.dom.borrow();
    d.query_selector(d.document(), sel)
}
pub fn nodes_by_selector(app: &App, sel: &str) -> Vec<NodeId> {
    let rt = app.world().non_send_resource::<UiRuntime>();
    let d = rt.dom.borrow();
    d.query_selector_all(d.document(), sel)
}
pub fn text_content(app: &App, node: NodeId) -> String {
    app.world().non_send_resource::<UiRuntime>().dom.borrow().text_content(node)
}
pub fn attr(app: &App, node: NodeId, name: &str) -> String {
    app.world().non_send_resource::<UiRuntime>().dom.borrow().get_attribute(node, name).unwrap_or_default()
}
pub fn classes(app: &App, node: NodeId) -> Vec<String> {
    app.world().non_send_resource::<UiRuntime>().dom.borrow().classes(node)
}
pub fn click(app: &mut App, node: NodeId) {
    app.world_mut().resource_mut::<PendingDomEvents>().0.push(PendingDomEvent::new(node, "click"));
    tick(app, 2);
}
```

Notes for the implementer:
- This requires `horde` to be usable as a library. Add `examples/horde/src/lib.rs` re-exporting the modules (`pub mod sim; pub mod game_state; pub mod ui;`) OR mark modules `pub` and confirm the crate exposes them. Simplest: add `src/lib.rs` with `pub use` of `sim`, `game_state`, `ui`; keep `main.rs` using the crate. If adding a lib target, set `[lib]`/`[[bin]]` in Cargo.toml. **Do this in Step 2 before writing tests.**
- Confirm `UiRuntime` exposes `get_attribute`, `classes` on the DOM (used by todomvc harness `classes`; `get_attribute` per reconcile.rs). If a helper is missing, use the closest DOM accessor.

- [ ] **Step 2: Make horde a lib+bin so tests can import it**

Create `examples/horde/src/lib.rs`:
```rust
#![allow(dead_code)]
pub mod game_state;
pub mod input;
pub mod sim;
pub mod ui;
pub mod world_render;
```
In `examples/horde/src/main.rs`, replace the `mod …;` declarations with `use horde::{game_state, input, sim, ui, world_render};` (and `use horde::game_state::GameState;`). In `Cargo.toml` add:
```toml
[lib]
name = "horde"
path = "src/lib.rs"

[[bin]]
name = "horde"
path = "src/main.rs"
```
Run: `cargo build -p horde` and `cargo build -p horde --no-default-features --features ui-native`.
Expected: both build. Commit is at the end of the task.

- [ ] **Step 3: Add a dynamic-`style` bar to `app.tsx` (the spike)**

In `app.tsx`, replace the `App` return with a version that renders an HP fill whose width is bound from the frame:
```tsx
  return (
    <div id="hud">
      <h1 id="title">HORDE</h1>
      <span id="state">{frame().state}</span>
      <div id="spike-track">
        <div id="spike-fill"
             style={`width: ${Math.round(100 * frame().player_hp / frame().player_max_hp)}%`}></div>
      </div>
    </div>
  );
```

- [ ] **Step 4: Spike test** — `examples/horde/tests/supersolid_ui.rs`:
```rust
mod support;
use support::*;
use horde::game_state::GameState;

#[test]
fn mounts_and_shows_title() {
    let mut app = app();
    let _root = mount(&mut app);
    let title = node_by_selector(&app, "#title");
    assert_eq!(text_content(&app, title), "HORDE");
}

#[test]
fn dynamic_style_width_binds_from_snapshot() {
    let mut app = app();
    let _root = mount(&mut app);
    set_state(&mut app, GameState::Playing);
    edit_snapshot(&mut app, |s| { s.player_hp = 50.0; s.player_max_hp = 100.0; });
    let fill = node_by_selector(&app, "#spike-fill");
    let style = attr(&app, fill, "style");
    assert!(style.contains("width: 50%"), "got style: {style:?}");
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p horde --test supersolid_ui`
Expected: both tests PASS. **If `dynamic_style_width_binds_from_snapshot` fails, STOP** — the dynamic-style path is the foundation for every panel; resolve before continuing (check `$ss.bind` on `style`, reconcile re-reading the attribute, and that the effect re-runs on signal change).

- [ ] **Step 6: Commit**
```bash
git add examples/horde
git commit -m "test(horde): headless supersolid harness + dynamic-style bar spike"
```

---

### Task B1: Root `App` + `main_menu` screen

**Files:**
- Modify: `examples/horde/assets/ui/horde/app.tsx`
- Modify: `examples/horde/tests/supersolid_ui.rs`

**Interfaces:**
- Produces: `<Switch>`-on-state root; `MainMenu` component; `intent(kind, index?)` helper calling `bevy.send("HordeIntent", …)`.

- [ ] **Step 1: Rewrite `app.tsx` with the state-routed skeleton + MainMenu**

Replace the whole file (keeps `EMPTY`, `frame` signal; removes the spike nodes):
```tsx
import { createSignal, createMemo, For, Show, Switch, Match, render } from "supersolid";

const EMPTY = {
  state: "MainMenu",
  player_hp: 0, player_max_hp: 1, xp: 0, level: 0, wave: 0, kills: 0, pickups: 0,
  active_weapon: null, ammo: 0, ammo_size: 0, reloading: false, cooldown_frac: 0,
  dps: 0, elapsed: 0,
  inventory: [], enemies: [], damage_numbers: [], blips: [], log: [],
};

function intent(kind, index) {
  bevy.send("HordeIntent", { kind, index: index || 0 });
}

function MainMenu() {
  const [settingsOpen, setSettingsOpen] = createSignal(false);
  return (
    <div class="screen" id="main-menu">
      <h1 class="title" id="title">HORDE</h1>
      <span class="subtitle">survive the swarm</span>
      <button class="menu-btn" id="start" onClick={() => intent("StartGame")}>Start  (Enter)</button>
      <button class="menu-btn" id="open-settings" onClick={() => setSettingsOpen(true)}>Settings</button>
      <button class="menu-btn" id="quit" onClick={() => intent("Quit")}>Quit</button>
      {<Show when={settingsOpen()}>
        <Settings onClose={() => setSettingsOpen(false)} />
      </Show>}
    </div>
  );
}

// Placeholder; real body added in Task B6.
function Settings(props) {
  return (
    <div class="modal" id="settings">
      <button id="settings-close" onClick={() => props.onClose()}>Close</button>
    </div>
  );
}

function App() {
  const [frame, setFrame] = createSignal(EMPTY);
  bevy.on("frame", (f) => setFrame(f));
  const state = createMemo(() => frame().state);

  return (
    <div id="hud">
      {<Switch>
        <Match when={state() === "MainMenu"}><MainMenu /></Match>
        <Match when={state() === "Playing"}><div id="playing" /></Match>
        <Match when={state() === "Paused"}><div id="paused" /></Match>
        <Match when={state() === "GameOver"}><div id="game-over" /></Match>
      </Switch>}
    </div>
  );
}

render(() => <App />, document.getElementById("root"));
```
(Later tasks flesh out the `Playing`/`Paused`/`GameOver` arms and `Settings`.)

- [ ] **Step 2: Tests for menu render + Start intent**

Append to `supersolid_ui.rs`:
```rust
#[test]
fn main_menu_shows_and_start_raises_intent() {
    use horde::sim::Intent;
    let mut app = app();
    let _root = mount(&mut app);
    // Default state is MainMenu.
    assert_eq!(text_content(&app, node_by_selector(&app, "#title")), "HORDE");
    // Click Start → HordeIntent("StartGame") → IntentQueue.
    let start = node_by_selector(&app, "#start");
    click(&mut app, start);
    let q = app.world().resource::<horde::sim::IntentQueue>();
    assert!(q.0.iter().any(|i| matches!(i, Intent::StartGame)), "queue: {:?}", q.0);
}
```
Update the earlier `mounts_and_shows_title` test if needed (title now has `class="title" id="title"`). Remove the obsolete `dynamic_style_width_binds_from_snapshot` spike test (the spike nodes are gone) — the dynamic-style path is re-covered by the player_status test in B2.

- [ ] **Step 3: Run**

Run: `cargo test -p horde --test supersolid_ui`
Expected: PASS.
Run: `cargo build -p horde` (regenerate app.generated.js) and confirm no transpile warnings about control-flow.

- [ ] **Step 4: Commit**
```bash
git add examples/horde
git commit -m "feat(horde): supersolid root state router + main menu (Start/Quit intents)"
```

---

### Task B2: `player_status`, `meters`, `combat_log` (Playing HUD readouts)

**Files:**
- Modify: `examples/horde/assets/ui/horde/app.tsx`
- Modify: `examples/horde/tests/supersolid_ui.rs`

**Interfaces:**
- Consumes: `frame()` fields (`player_hp`, `player_max_hp`, `xp`, `active_weapon`, `ammo`, `ammo_size`, `reloading`, `wave`, `kills`, `dps`, `elapsed`, `log`).
- Produces: `hpColor(f)` helper; `PlayerStatus`, `Meters`, `CombatLog`, and a `Hud` wrapper mounted in the `Playing` arm.

- [ ] **Step 1: Add helpers + components + mount them in the Playing arm.**

In `app.tsx`, add near `intent`:
```tsx
function hpColor(f) {
  f = Math.max(0, Math.min(1, f));
  const r = Math.round((0.95 * (1 - f * f) + 0.10) * 255);
  const g = Math.round((0.30 + 0.62 * f) * 255);
  const b = Math.round(0.30 * 255);
  return `rgb(${r}, ${g}, ${b})`;
}
function mmss(sec) {
  const m = Math.floor(sec / 60), s = Math.floor(sec % 60);
  return `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
}
```

Add components:
```tsx
function PlayerStatus(props) {
  const f = props.f;
  const hpFrac = () => f().player_hp / f().player_max_hp;
  const xpFrac = () => (f().xp % 100) / 100;
  return (
    <div class="panel" id="player-status">
      <span class="label">HP</span>
      <div class="bar-track">
        <div class="bar-fill" id="hp-fill"
             style={`width: ${Math.round(100 * hpFrac())}%; background-color: ${hpColor(hpFrac())}`}></div>
      </div>
      <span class="label">XP</span>
      <div class="bar-track">
        <div class="bar-fill xp" id="xp-fill" style={`width: ${Math.round(100 * xpFrac())}%`}></div>
      </div>
      <span class="badge" id="weapon-badge">{f().active_weapon || "—"}</span>
      <span class="ammo" id="ammo">{f().reloading ? "reloading…" : `${f().ammo} / ${f().ammo_size}`}</span>
    </div>
  );
}

function Meters(props) {
  const f = props.f;
  return (
    <div class="panel" id="meters">
      <span>{`Wave ${f().wave}   Kills ${f().kills}   DPS ${Math.round(f().dps)}   ${mmss(f().elapsed)}`}</span>
    </div>
  );
}

function CombatLog(props) {
  return (
    <div class="panel" id="combat-log">
      {<For each={props.f().log}>
        {(line) => <span class="log-line" style={`opacity: ${line.alpha}`}>{line.text}</span>}
      </For>}
    </div>
  );
}

function Hud(props) {
  return (
    <div id="playing">
      <PlayerStatus f={props.f} />
      <Meters f={props.f} />
      <CombatLog f={props.f} />
    </div>
  );
}
```

Change the Playing arm from `<div id="playing" />` to `<Hud f={frame} />`. (Pass the `frame` accessor down so each panel reads reactively.)

- [ ] **Step 2: Tests**

Append:
```rust
#[test]
fn player_status_reflects_snapshot() {
    let mut app = app();
    let _root = mount(&mut app);
    set_state(&mut app, GameState::Playing);
    edit_snapshot(&mut app, |s| {
        s.player_hp = 30.0; s.player_max_hp = 120.0;
        s.active_weapon = Some(horde::sim::WeaponKind::Shotgun);
        s.ammo = 4; s.ammo_size = 6; s.reloading = false;
    });
    let style = attr(&app, node_by_selector(&app, "#hp-fill"), "style");
    assert!(style.contains("width: 25%"), "hp style: {style:?}");
    assert_eq!(text_content(&app, node_by_selector(&app, "#weapon-badge")), "Shotgun");
    assert_eq!(text_content(&app, node_by_selector(&app, "#ammo")), "4 / 6");
}

#[test]
fn meters_and_log_render() {
    use horde::sim::snapshot::LogLine;
    let mut app = app();
    let _root = mount(&mut app);
    set_state(&mut app, GameState::Playing);
    edit_snapshot(&mut app, |s| {
        s.wave = 3; s.kills = 12; s.dps = 47.6; s.elapsed = 75.0;
        s.log = vec![LogLine { text: "Wave 3".into(), age: 0.0 }];
    });
    assert_eq!(text_content(&app, node_by_selector(&app, "#meters")), "Wave 3   Kills 12   DPS 48   01:15");
    let lines = nodes_by_selector(&app, ".log-line");
    assert_eq!(lines.len(), 1);
    assert_eq!(text_content(&app, lines[0]), "Wave 3");
}
```
(Confirm `LogLine` path: `horde::sim::snapshot::LogLine`. Confirm `WeaponKind` is re-exported at `horde::sim::WeaponKind`.)

- [ ] **Step 3: Run**

Run: `cargo build -p horde && cargo test -p horde --test supersolid_ui`
Expected: PASS.

- [ ] **Step 4: Commit**
```bash
git add examples/horde
git commit -m "feat(horde): player-status/meters/combat-log HUD panels in supersolid"
```

---

### Task B3: `weapon_bar` + `minimap`

**Files:** Modify `app.tsx`, `supersolid_ui.rs`.

**Interfaces:**
- Consumes: `frame().inventory` (slots), `frame().blips`.
- Produces: `WeaponBar` (SwitchWeapon intent), `Minimap` components mounted inside `Hud`.

- [ ] **Step 1: Components**
```tsx
function WeaponBar(props) {
  return (
    <div id="weapon-bar">
      {<For each={props.f().inventory}>
        {(slot) => (
          <button class={slot.active ? "slot active" : "slot"} data-index={slot.index}
                  onClick={() => intent("SwitchWeapon", slot.index)}>
            {`${slot.index + 1}. ${slot.name}`}
          </button>
        )}
      </For>}
    </div>
  );
}

function Minimap(props) {
  return (
    <div class="panel" id="minimap">
      {<For each={props.f().blips}>
        {(b) => (
          <div class={"blip " + b.kind}
               style={`left: ${Math.round(b.mx * 100)}%; top: ${Math.round(b.my * 100)}%`}></div>
        )}
      </For>}
    </div>
  );
}
```
Add `<WeaponBar f={props.f} />` and `<Minimap f={props.f} />` inside `Hud`'s `#playing` div.

- [ ] **Step 2: Tests**
```rust
#[test]
fn weapon_bar_lists_slots_and_switch_raises_intent() {
    use horde::sim::{Intent, WeaponKind};
    use horde::sim::snapshot::WeaponSlot;
    let mut app = app();
    let _root = mount(&mut app);
    set_state(&mut app, GameState::Playing);
    edit_snapshot(&mut app, |s| {
        s.inventory = vec![
            WeaponSlot { index: 0, kind: WeaponKind::Pistol, active: true },
            WeaponSlot { index: 1, kind: WeaponKind::Smg, active: false },
        ];
    });
    let slots = nodes_by_selector(&app, "#weapon-bar .slot");
    assert_eq!(slots.len(), 2);
    assert_eq!(text_content(&app, slots[0]), "1. Pistol");
    assert!(classes(&app, slots[0]).iter().any(|c| c == "active"));
    click(&mut app, slots[1]);
    let q = app.world().resource::<horde::sim::IntentQueue>();
    assert!(q.0.iter().any(|i| matches!(i, Intent::SwitchWeapon(1))), "queue: {:?}", q.0);
}

#[test]
fn minimap_renders_blips_positioned() {
    use horde::sim::snapshot::{Blip, BlipKind};
    use bevy::prelude::Vec2;
    let mut app = app();
    let _root = mount(&mut app);
    set_state(&mut app, GameState::Playing);
    edit_snapshot(&mut app, |s| {
        s.blips = vec![Blip { id: u64::MAX, world_pos: Vec2::ZERO, screen_pos: Vec2::ZERO, kind: BlipKind::Player }];
    });
    let blips = nodes_by_selector(&app, "#minimap .blip");
    assert_eq!(blips.len(), 1);
    // world (0,0) → center (mx=my=0.5) → 50%.
    let style = attr(&app, blips[0], "style");
    assert!(style.contains("left: 50%") && style.contains("top: 50%"), "style: {style:?}");
    assert!(classes(&app, blips[0]).iter().any(|c| c == "player"));
}
```

- [ ] **Step 3: Run** — `cargo build -p horde && cargo test -p horde --test supersolid_ui` → PASS.
- [ ] **Step 4: Commit** — `git commit -am "feat(horde): weapon-bar (switch intent) + minimap blips in supersolid"`

---

### Task B4: `enemy_nameplates` + `damage_numbers` (keyed, positioned overlays)

**Files:** Modify `app.tsx`, `supersolid_ui.rs`.

**Interfaces:** Consumes `frame().enemies`, `frame().damage_numbers`. Produces `Nameplates`, `DamageNumbers` overlays inside `Hud`.

- [ ] **Step 1: Components** (keyed `<For>` by `id`; absolute position from `sx/sy`)
```tsx
function Nameplates(props) {
  return (
    <div class="overlay" id="nameplates">
      {<For each={props.f().enemies}>
        {(e) => (
          <div class="nameplate" data-id={e.id}
               style={`left: ${Math.round(e.sx - 22)}px; top: ${Math.round(e.sy - 30)}px`}>
            <div class="np-fill"
                 style={`width: ${Math.round(e.frac * 100)}%; background-color: ${hpColor(e.frac)}`}></div>
          </div>
        )}
      </For>}
    </div>
  );
}

function DamageNumbers(props) {
  return (
    <div class="overlay" id="damage-numbers">
      {<For each={props.f().damage_numbers}>
        {(d) => (
          <span class={d.crit ? "dmg crit" : "dmg"} data-id={d.id}
                style={`left: ${Math.round(d.sx)}px; top: ${Math.round(d.sy)}px; opacity: ${d.alpha}`}>
            {d.text}
          </span>
        )}
      </For>}
    </div>
  );
}
```
Add `<Nameplates f={props.f} />` and `<DamageNumbers f={props.f} />` inside `Hud`.

- [ ] **Step 2: Tests**
```rust
#[test]
fn nameplates_and_damage_numbers_render_positioned() {
    use horde::sim::snapshot::{Nameplate, FloatingNumber};
    use horde::sim::EnemyKind;
    use bevy::prelude::Vec2;
    let mut app = app();
    let _root = mount(&mut app);
    set_state(&mut app, GameState::Playing);
    edit_snapshot(&mut app, |s| {
        s.enemies = vec![Nameplate {
            id: 7, world_pos: Vec2::ZERO, screen_pos: Vec2::new(100.0, 200.0),
            hp: 15.0, max_hp: 30.0, kind: EnemyKind::Grunt,
        }];
        s.damage_numbers = vec![FloatingNumber {
            id: 9, world_pos: Vec2::ZERO, screen_pos: Vec2::new(50.0, 60.0),
            amount: 42.0, crit: true, age: 0.0, ttl: 1.0,
        }];
    });
    let np = node_by_selector(&app, "#nameplates .nameplate");
    let s = attr(&app, np, "style");
    assert!(s.contains("left: 78px") && s.contains("top: 170px"), "np style: {s:?}");
    let fill = node_by_selector(&app, "#nameplates .np-fill");
    assert!(attr(&app, fill, "style").contains("width: 50%"));
    let dmg = node_by_selector(&app, "#damage-numbers .dmg");
    assert_eq!(text_content(&app, dmg), "42");
    assert!(classes(&app, dmg).iter().any(|c| c == "crit"));
}
```
(Note screen_pos is set directly here; the shared `project_snapshot` doesn't run headlessly without a camera, which is fine — we inject `screen_pos`.)

- [ ] **Step 3: Run** → PASS. **Step 4: Commit** — `git commit -am "feat(horde): enemy nameplates + damage numbers overlays in supersolid"`

---

### Task B5: `pause` + `game_over` screens

**Files:** Modify `app.tsx`, `supersolid_ui.rs`.

**Interfaces:** Consumes `frame()` (state + game_over stats). Produces `Pause`, `GameOver` components in the `Paused`/`GameOver` arms.

- [ ] **Step 1: Components**
```tsx
function Pause() {
  return (
    <div class="screen dim" id="paused">
      <h2 class="screen-title">Paused</h2>
      <button class="menu-btn" id="resume" onClick={() => intent("Resume")}>Resume  (Esc)</button>
      <button class="menu-btn" id="restart" onClick={() => intent("Restart")}>Restart</button>
      <button class="menu-btn" id="pause-quit" onClick={() => intent("Quit")}>Quit</button>
    </div>
  );
}

function GameOver(props) {
  const f = props.f;
  return (
    <div class="screen dim" id="game-over">
      <h2 class="screen-title danger">You Died</h2>
      <div class="panel stats">
        <span>{`Kills: ${f().kills}`}</span>
        <span>{`Wave reached: ${f().wave}`}</span>
        <span>{`Pickups: ${f().pickups}`}</span>
        <span>{`Time survived: ${mmss(f().elapsed)}`}</span>
      </div>
      <button class="menu-btn" id="go-restart" onClick={() => intent("Restart")}>Restart  (Enter)</button>
      <button class="menu-btn" id="go-quit" onClick={() => intent("Quit")}>Quit</button>
    </div>
  );
}
```
Change the `Paused` arm to `<Pause />` and the `GameOver` arm to `<GameOver f={frame} />`.

- [ ] **Step 2: Tests**
```rust
#[test]
fn pause_resume_intent() {
    use horde::sim::Intent;
    let mut app = app();
    let _root = mount(&mut app);
    set_state(&mut app, GameState::Paused);
    click(&mut app, node_by_selector(&app, "#resume"));
    let q = app.world().resource::<horde::sim::IntentQueue>();
    assert!(q.0.iter().any(|i| matches!(i, Intent::Resume)));
}

#[test]
fn game_over_shows_stats() {
    let mut app = app();
    let _root = mount(&mut app);
    edit_snapshot(&mut app, |s| { s.kills = 9; s.wave = 4; s.pickups = 2; s.elapsed = 130.0; });
    set_state(&mut app, GameState::GameOver);
    let panel = node_by_selector(&app, "#game-over .stats");
    let txt = text_content(&app, panel);
    assert!(txt.contains("Kills: 9") && txt.contains("Wave reached: 4") && txt.contains("02:10"), "txt: {txt:?}");
}
```

- [ ] **Step 3: Run** → PASS. **Step 4: Commit** — `git commit -am "feat(horde): pause + game-over screens in supersolid"`

---

### Task B6: `inventory` modal + `settings` modal

**Files:** Modify `app.tsx`, `supersolid_ui.rs`.

**Interfaces:** Consumes `frame().inventory` (with weapon stats), `bevy.on("toggleInventory")`. Produces `Inventory` component (Playing overlay, local open signal) and real `Settings` body (`AdjustEnemyCap` command).

- [ ] **Step 1: Inventory modal + toggle wiring.**

Add an inventory-open signal at `App` scope and subscribe to the forwarded event. In `App`, after the `frame` signal:
```tsx
  const [invOpen, setInvOpen] = createSignal(false);
  bevy.on("toggleInventory", () => setInvOpen((o) => !o));
```
Add the component:
```tsx
function Inventory(props) {
  return (
    <div class="modal dim" id="inventory">
      <h2 class="screen-title">Inventory (I to close)</h2>
      <div class="inv-grid">
        {<For each={props.f().inventory}>
          {(w) => (
            <div class={w.active ? "inv-card active" : "inv-card"}>
              <span class="inv-name">{w.name}</span>
              <span class="inv-stat">{`DMG ${Math.round(w.dmg)}   RoF ${w.rof.toFixed(2)}s`}</span>
              <span class="inv-stat">{`Spread ${w.spread.toFixed(2)}   x${w.projectiles}`}</span>
              <span class="inv-stat">{`Mag ${w.mag}   Reload ${w.reload.toFixed(1)}s`}</span>
            </div>
          )}
        </For>}
      </div>
      <button class="menu-btn" id="inv-close" onClick={() => props.onClose()}>Close</button>
    </div>
  );
}
```
In the `Playing` arm, render the modal conditionally alongside `<Hud>`:
```tsx
        <Match when={state() === "Playing"}>
          <div id="playing-root">
            <Hud f={frame} />
            {<Show when={invOpen()}>
              <Inventory f={frame} onClose={() => setInvOpen(false)} />
            </Show>}
          </div>
        </Match>
```
(`w.rof.toFixed` etc. are numbers from JSON — fine.)

- [ ] **Step 2: Real Settings body.**

Replace the placeholder `Settings`:
```tsx
function Settings(props) {
  const [cap, setCap] = createSignal(0);
  // Read the live enemy cap off the frame if present; else start from the last known.
  return (
    <div class="modal dim" id="settings">
      <h2 class="screen-title">Settings</h2>
      <div class="settings-row">
        <button id="cap-dec" onClick={() => bevy.send("AdjustEnemyCap", { delta: -20 })}>−</button>
        <span id="cap-label">Enemy cap ±20</span>
        <button id="cap-inc" onClick={() => bevy.send("AdjustEnemyCap", { delta: 20 })}>+</button>
      </div>
      <span class="inv-stat">UI backend: supersolid (TSX)</span>
      <button class="menu-btn" id="settings-close" onClick={() => props.onClose()}>Close</button>
    </div>
  );
}
```
(The native settings shows the numeric cap; exposing the live number would require adding `enemy_cap` to the DTO. Keep the label static here — YAGNI — and verify the `AdjustEnemyCap` command actually mutates `SimConfig`. If desired later, add `enemy_cap` to `FrameDto`.)

- [ ] **Step 3: Tests**
```rust
#[test]
fn toggle_inventory_event_opens_modal() {
    use horde::ui::supersolid::bridge::ToggleInventoryFwd;
    use horde::sim::{WeaponKind};
    use horde::sim::snapshot::WeaponSlot;
    let mut app = app();
    let _root = mount(&mut app);
    set_state(&mut app, GameState::Playing);
    edit_snapshot(&mut app, |s| {
        s.inventory = vec![WeaponSlot { index: 0, kind: WeaponKind::Rocket, active: true }];
    });
    assert!(maybe_node(&app, "#inventory").is_none());
    app.world_mut().trigger(ToggleInventoryFwd);
    tick(&mut app, 2);
    assert!(maybe_node(&app, "#inventory").is_some());
    let card = node_by_selector(&app, "#inventory .inv-card");
    let txt = text_content(&app, card);
    assert!(txt.contains("Rocket") && txt.contains("DMG 60"), "card: {txt:?}");
}

#[test]
fn settings_adjust_enemy_cap_mutates_config() {
    let mut app = app();
    let _root = mount(&mut app);
    let before = app.world().resource::<horde::sim::SimConfig>().enemy_cap;
    // Open settings, click "+".
    click(&mut app, node_by_selector(&app, "#open-settings"));
    click(&mut app, node_by_selector(&app, "#cap-inc"));
    let after = app.world().resource::<horde::sim::SimConfig>().enemy_cap;
    assert_eq!(after, (before + 20).min(800));
}
```

- [ ] **Step 4: Run** → PASS. **Step 5: Commit** — `git commit -am "feat(horde): inventory + settings modals (toggle event, enemy-cap command)"`

---

## Phase C — Styling, cross-target, verification

### Task C1: Full `theme.css` component styling (visual parity)

**Files:** Modify `examples/horde/assets/ui/horde/theme.css`.

**Interfaces:** none (styling only). Uses the class/id names authored in B1–B6.

- [ ] **Step 1: Append component rules** to `theme.css` (after the `:root`/`#root` block). Style to match `ui/native/theme.rs` + panel layout positions from the native panels. Include:
  - `#hud`, `.overlay` — absolute full-size, non-interacting where appropriate.
  - `.panel` — `background-color: var(--panel); border: 1px var(--panel-border); border-radius: var(--radius); padding: var(--space);` flex column.
  - `#player-status` top-right (`position:absolute; top:12px; right:12px; width:240px`), `#meters` top-center, `#minimap` bottom-right 160×160, `#weapon-bar` bottom-center, `#combat-log` bottom-left, per the native positions in the inventory report.
  - `.bar-track` (`height:14px; background-color: var(--bar-track); border-radius:4px`) and `.bar-fill` (`height:100%; border-radius:4px`); `.bar-fill.xp { background-color: var(--accent) }`.
  - `.slot` (70×48, idle bg/border), `.slot.active { background-color: var(--slot-active); border-color: var(--accent) }`, `.slot:hover`.
  - `.nameplate` (`position:absolute; width:44px; height:5px; background-color:#262633; overflow:hidden`), `.np-fill { height:100% }`.
  - `.dmg` (`position:absolute; color: var(--text)`), `.dmg.crit { color: var(--warn) }`.
  - `.blip` (`position:absolute; width:4px; height:4px; border-radius:2px`), `.blip.player { background-color: var(--accent); width:6px; height:6px }`, `.blip.enemy { background-color: var(--danger) }`, `.blip.pickup { background-color: var(--good) }`.
  - `.screen` (fullscreen centered flex column), `.screen.dim { background-color: rgba(0,0,0,0.6) }`, `#main-menu { background-color: var(--bg) }`.
  - `.title` (`font-size:72px; color: var(--accent)`), `.subtitle`, `.screen-title` (`font-size:30px`), `.screen-title.danger { color: var(--danger) }`.
  - `.menu-btn` (`background-color: var(--btn-idle); border:1px var(--panel-border); border-radius: var(--radius); padding: 11px 20px`), `.menu-btn:hover { background-color: var(--btn-hover); border-color: var(--accent) }`, `.menu-btn:active { background-color: var(--btn-pressed) }`.
  - `.modal`/`.modal.dim` (fullscreen dim overlay, centered), `.inv-grid` (row wrap / 2-col), `.inv-card` (`background-color:#292b3d`), `.inv-card.active { border-color: var(--accent) }`.
  - Keep the **top-left corner empty** (FPS overlay) — no panel there.

  Write concrete rules (no placeholders); reference `ui/native/*.rs` for exact px positions/sizes.

- [ ] **Step 2: Build + tests still pass** (CSS changes must not break DOM structure/IDs).

Run: `cargo build -p horde && cargo test -p horde --test supersolid_ui`
Expected: PASS.

- [ ] **Step 3: Commit** — `git commit -am "style(horde): supersolid design system matching native theme"`

---

### Task C2: Cross-target builds (wasm, HMR, native regression)

**Files:** none (verification); may touch `.gitignore` for `app.generated.js` if the repo ignores it.

- [ ] **Step 1: Default (supersolid) build + generated JS fresh**

Run: `cargo build -p horde`
Expected: builds; `app.generated.js` regenerated with the full app.

- [ ] **Step 2: HMR feature builds**

Run: `cargo build -p horde --features hmr`
Expected: builds (pulls `superui/hmr` + `bevy/file_watcher`).

- [ ] **Step 3: wasm build**

Run: `cargo build -p horde --target wasm32-unknown-unknown`
Expected: builds (loads `app.generated.js`; transpiler stays host-side via build.rs).

- [ ] **Step 4: native backend build + its tests**

Run: `cargo build -p horde --no-default-features --features ui-native`
Expected: builds.
Run: `cargo test -p horde --no-default-features --features ui-native`
Expected: sim + native tests pass (supersolid integration tests are excluded — they require the default backend; gate the test file with a `#![cfg(not(feature = "ui-native"))]` at the top of `tests/supersolid_ui.rs` and `tests/support/mod.rs` so `--features ui-native` test runs skip them cleanly).

- [ ] **Step 5: Commit** (if any files changed, e.g. the test `cfg` guards)
```bash
git add examples/horde
git commit -m "build(horde): verify wasm + hmr + native backends; gate supersolid tests"
```

---

### Task C3: Windowed verification, native parity screenshot, docs & memory

**Files:** Modify `examples/horde/README.md` (create if absent), `docs/superpowers/specs/2026-07-20-horde-native-ui-design.md` (status note), `docs/superpowers/plans/2026-07-21-horde-supersolid-ui-port.md` (mark done), `C:\Users\strow\.claude\projects\C--work-bevy-superui\memory\` (new memory + MEMORY.md line).

- [ ] **Step 1: Launch the supersolid app in a real window** (green tests do NOT prove a windowed launch — Boa parses render.js on the main thread; `/STACK:8MB` is set).

Run: `cargo run -p horde` (optionally with `--features mcp_debug` to allow BRP screenshots)
Expected: a window opens on the Main Menu; clicking **Start** enters gameplay with the full HUD (player status, meters, minimap, weapon bar, combat log, nameplates, damage numbers); `I` opens the inventory; `Esc` pauses; death shows game-over. If it panics at mount with a stack overflow, confirm `.cargo/config.toml` still sets `/STACK:8MB`.

Capture a screenshot (BRP `brp_extras_screenshot`, or manual) for the record.

- [ ] **Step 2: Launch the native backend for side-by-side comparison**

Run: `cargo run -p horde --no-default-features --features ui-native`
Expected: identical gameplay + panels on native `bevy_ui`. Screenshot and compare against Step 1 — layouts and palette should read as the same design (design goal: meaningful screenshot comparison).

- [ ] **Step 2b:** If the comparison reveals notable layout/color drift, adjust `theme.css` (Task C1 rules), rebuild, re-verify. Small differences are acceptable; gross mismatches are not.

- [ ] **Step 3: README** — write `examples/horde/README.md` documenting: default = supersolid (`cargo run -p horde`), native via `--no-default-features --features ui-native`, `--features hmr` for live `.tsx`, wasm build line, and the DTO/intent bridge summary. Note the "reactive store gap" (design §9) as the known follow-up.

- [ ] **Step 4: Status notes** — in the plan doc, check off completion; add a one-line "Superseded by supersolid port (2026-07-21)" note to the native design spec's status.

- [ ] **Step 5: Memory** — write `C:\Users\strow\.claude\projects\C--work-bevy-superui\memory\horde-supersolid-ui-port.md` (type: project) capturing: horde default backend is now supersolid; native behind `ui-native` (out of default); data path = per-frame full-`UiSnapshot` JSON over `bevy.on("frame")` + `HordeIntent`; single-file `app.tsx`; the documented reactive-store gap + named follow-up; dynamic layout via bound `style` string; link `[[supersolid-todomvc-plan6]]`, `[[windows-main-thread-stack-boa]]`. Add a one-line pointer to `MEMORY.md`.

- [ ] **Step 6: Final full test sweep**

Run: `cargo test -p horde` and `cargo test -p horde --no-default-features --features ui-native`
Expected: both green.

- [ ] **Step 7: Commit**
```bash
git add examples/horde/README.md docs/superpowers
git commit -m "docs(horde): README + status notes for supersolid UI port"
```

---

## Self-review — spec coverage

- **§1 default=supersolid, native opt-in, ui-native out of default** → Tasks A2/A4 (feature flip), A1/A4 (`cfg` select). ✓
- **§1 sim/input/world/state untouched** → only `ui/` + `Cargo.toml` + `build.rs` + assets + `lib.rs` split touched; `sim/` unchanged. ✓
- **§1 both backends read same UiSnapshot + intents** → shared `UiSnapshot`; `HordeIntent`→`IntentQueue`. ✓
- **§2.1 Rust→TSX per-frame push** → `push_ui_frame` + `FrameDto` + `bevy.on("frame")` (A4). ✓
- **§2.1 memos + keyed For** → `<For each>` keyed by `id`; `createMemo` for `state`; panels read `frame()` (B1–B6). ✓
- **§2.2 intents small surface; inventory/settings local; keyboard I forwarded** → `HordeIntent` (A4/B1/B3/B5), local `invOpen`/settings signals (B6), `ToggleInventoryFwd` (A4/B6). ✓
- **§2.3 gameplay input unchanged** → `input.rs` untouched. ✓
- **§3 module structure + projection lift** → A1 (lift), A4 (supersolid module). ✓
- **§3 single-file app.tsx** → enforced (Global Constraints). ✓
- **§4 build/HMR/wasm** → A3 (build.rs), A4 (`USE_LIVE_TSX`), C2 (targets). ✓
- **§5 feature arrangement** → A2/A4. ✓
- **§6 all panels/screens 1:1** → B2 (player_status/meters/combat_log), B3 (weapon_bar/minimap), B4 (nameplates/damage_numbers), B1 (main_menu), B5 (pause/game_over), B6 (inventory/settings). ✓
- **§6 CSS ported from theme.rs, hover/active** → theme.css (A3 palette, C1 components). ✓
- **§7 acceptance** → C2 (builds), C3 (windowed + native parity + tests). ✓
- **§9 documented gap** → README + memory (C3), spec already records it. ✓

**Placeholder scan:** the only intentional deferrals are the `Settings`/`Playing`-arm bodies filled in later tasks (B6/B2) and `components.css` merged into `theme.css` (noted in A4 Step 2). No "TODO/handle edge cases" left in code.

**Type consistency:** DTO field names in `bridge.rs` (`build_frame`) match every `frame().<field>` read in `app.tsx` and every test. `HordeIntent{kind,index}`, `AdjustEnemyCap{delta}`, `ToggleInventoryFwd` names match across `bridge.rs`, `app.tsx` (`bevy.send`/`bevy.on`), and tests. `register_bridge`/`build_frame` signatures match the harness call sites.

**Known verification points for the implementer** (confirm against source, adjust if a name differs — do not assume): `SimConfig::{enemy_cap: u32, arena_half: f32}` and `SimConfig::play()`; `crate::sim::snapshot::{BlipKind, WeaponSlot, Nameplate, FloatingNumber, Blip, LogLine}` paths and `WeaponKind`/`EnemyKind`/`weapon_stats` re-exports; that `UiRuntime.dom` exposes `get_attribute`/`classes`/`text_content`/`query_selector*`; whether `SuperUiRoot` accepts one stylesheet (drove the single-`theme.css` decision); and that horde can be split into `lib.rs` + `bin` without disturbing existing features.
