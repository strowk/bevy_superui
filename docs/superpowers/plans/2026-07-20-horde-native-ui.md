# Horde Native-UI Game — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a complete, playable, styled top-down horde-survival game whose UI is written entirely in native `bevy_ui`, with the game simulation fully decoupled from the UI behind a plain-data `UiSnapshot` seam.

**Architecture:** Three independent layers wired in `main.rs`: `SimPlugin` (deterministic `FixedUpdate` game logic, zero UI/`bevy_ui`/Boa deps) produces a `UiSnapshot` resource; a boundary `project_snapshot` system fills screen positions using the camera; `NativeUiPlugin` reads the snapshot and raises `Intent`s. One feature flag `ui-native` (in `default`) selects the backend; the absent arm panics as a Supersolid seam.

**Tech Stack:** Rust, Bevy 0.17 (`bevy_ui`, 2D sprites, `States`, `FixedUpdate`), a hand-rolled seeded xorshift RNG. No external RNG/asset crates for the sim.

## Global Constraints

- Bevy version: **0.17** (`bevy = { version = "0.17", features = ["file_watcher"] }`). Copy exact dep shape from `examples/todomvc/Cargo.toml`.
- Crate name / package: `horde`, `publish = false`, inherit `edition`/`version`/`license` from workspace where todomvc does.
- Feature flag: `default = ["ui-native"]`. `ui-native` present → native UI. Absent → `panic!` Supersolid seam. Also mirror `debug-ui = []` and `mcp_debug = ["dep:bevy_brp_extras", "bevy/bevy_remote"]` from todomvc.
- **`src/sim/` must never import `crate::ui`, `bevy::ui` / `bevy_ui`, or any Boa/superui crate.** Enforced by review of `use` statements in every sim task.
- **No panel or screen system may run an ECS query over sim entities.** UI systems read `Res<UiSnapshot>` and write `ResMut<IntentQueue>` only. The single exception is `project_snapshot`, which may read sim `Transform`s + `Camera`.
- Sim is deterministic: all randomness flows through the `Rng` resource seeded from `SimConfig::seed`. No `Time`-of-wall-clock or `rand::thread_rng` in `sim/`.
- Stable ids in the snapshot are `Entity::to_bits()` (a `u64`).
- Every commit message ends with the co-author trailer:
  `Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>`
- Run all commands from repo root `C:\work\bevy_superui`. Build/test with `cargo … -p horde`.

---

## File Structure

```
examples/horde/
  Cargo.toml
  assets/ui/horde/.gitkeep         # placeholder (Supersolid TSX/CSS deferred)
  src/
    main.rs                        # app assembly; camera; states; adds SimPlugin + project_snapshot + ui::add_ui
    game_state.rs                  # GameState (Bevy States) + transition system
    input.rs                       # raw input -> Intent (shared, uses bevy_input only)
    sim/
      mod.rs                       # SimPlugin, shared components, EnemyKind/WeaponKind, weapon_stats()
      config.rs                    # SimConfig resource + play()/stress()/from_env()
      rng.rs                       # Rng resource (xorshift64*)
      intent.rs                    # Intent enum + IntentQueue resource
      player.rs                    # player spawn + movement + shooting + death
      enemy.rs                     # enemy movement (seek + separation) + melee
      spawn.rs                     # wave spawner
      projectile.rs                # projectile motion + collision -> damage
      pickup.rs                    # pickup spawn + grab + weapon switch
      damage.rs                    # DamageNumber lifecycle + DamageHistory + CombatLog + Progression
      snapshot.rs                  # UiSnapshot + snapshot sub-types + assemble_world_snapshot
    ui/
      mod.rs                       # cfg backend select: add_ui() = native | panic seam
      native/
        mod.rs                     # NativeUiPlugin
        theme.rs                   # palette/spacing/font constants
        widgets.rs                 # small DRY spawn helpers (bar, panel, text, button)
        project.rs                 # project_snapshot boundary system (world->screen)
        hud/
          mod.rs
          player_status.rs enemy_nameplates.rs damage_numbers.rs
          minimap.rs weapon_bar.rs meters.rs combat_log.rs
        screens/
          mod.rs
          main_menu.rs pause.rs game_over.rs inventory.rs settings.rs
```

---

## Phase A — Sim foundation

### Task 1: Crate scaffold that compiles and opens a window

**Files:**
- Create: `examples/horde/Cargo.toml`
- Create: `examples/horde/assets/ui/horde/.gitkeep`
- Create: `examples/horde/src/main.rs`
- Create: `examples/horde/src/ui/mod.rs`

**Interfaces:**
- Produces: `horde::ui::add_ui(app: &mut App)` — backend selector.

- [ ] **Step 1: Write `Cargo.toml`**

```toml
[package]
name = "horde"
version = "0.1.0"
edition = "2021"
publish = false

[features]
default = ["ui-native"]
ui-native = []
debug-ui = []
mcp_debug = ["dep:bevy_brp_extras", "bevy/bevy_remote"]

[dependencies]
bevy = { version = "0.17", features = ["file_watcher"] }

[dependencies.bevy_brp_extras]
optional = true
version = "0.17.3"

[target.'cfg(target_arch = "wasm32")'.dependencies]
getrandom = { version = "0.3", features = ["wasm_js"] }
```

- [ ] **Step 2: Add the example to the workspace if needed**

Check `C:\work\bevy_superui\Cargo.toml`. If `examples/*` is not already a glob workspace member, add `"examples/horde"` to `[workspace] members`. (todomvc is already a member; mirror exactly how it is listed.)

- [ ] **Step 3: Write the placeholder asset file**

Create `examples/horde/assets/ui/horde/.gitkeep` with a single line:

```
Supersolid TSX/CSS assets go here (deferred). See docs/superpowers/specs/2026-07-20-horde-native-ui-design.md
```

- [ ] **Step 4: Write `src/ui/mod.rs` (backend selector)**

```rust
use bevy::prelude::*;

#[cfg(feature = "ui-native")]
pub mod native;

/// Adds the selected UI backend. Native today; Supersolid later (see design §2).
#[cfg(feature = "ui-native")]
pub fn add_ui(app: &mut App) {
    app.add_plugins(native::NativeUiPlugin);
}

#[cfg(not(feature = "ui-native"))]
pub fn add_ui(_app: &mut App) {
    panic!(
        "Supersolid UI backend not yet implemented — build with the default \
         `ui-native` feature. TODO(supersolid-runtime)."
    );
}
```

- [ ] **Step 5: Write a minimal `src/main.rs`**

```rust
use bevy::prelude::*;

mod ui;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(AssetPlugin {
            watch_for_changes_override: Some(true),
            ..default()
        }))
        .add_systems(Startup, setup_camera)
        .run();
}

fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}
```

(Note: `ui::add_ui` is wired in Task 5 once there is a plugin behind it. For now `mod ui;` must compile; the native submodule is added in Task 18, so temporarily guard the `pub mod native;` — implement Task 18's `native/mod.rs` stub now as an empty plugin if the compiler complains, or defer wiring `add_ui` until Task 18. Simplest: leave `add_ui` unused for Task 1.)

To keep Task 1 compiling without the native module yet, make `src/ui/mod.rs` temporarily:

```rust
use bevy::prelude::*;

#[cfg(feature = "ui-native")]
pub fn add_ui(_app: &mut App) { /* NativeUiPlugin wired in Task 18 */ }

#[cfg(not(feature = "ui-native"))]
pub fn add_ui(_app: &mut App) {
    panic!("Supersolid UI backend not yet implemented — build with the default \
            `ui-native` feature. TODO(supersolid-runtime).");
}
```

- [ ] **Step 6: Build and run**

Run: `cargo run -p horde`
Expected: a window opens with an empty dark viewport, no panics. Close it.

Run: `cargo build -p horde --no-default-features`
Expected: compiles (the panic is runtime, not compile-time).

- [ ] **Step 7: Commit**

```bash
git add examples/horde Cargo.toml
git commit -m "feat(horde): crate scaffold, window, backend-select seam"
```

---

### Task 2: `SimConfig` resource with presets and env override

**Files:**
- Create: `examples/horde/src/sim/mod.rs`
- Create: `examples/horde/src/sim/config.rs`
- Modify: `examples/horde/src/main.rs` (add `mod sim;`)

**Interfaces:**
- Produces: `SimConfig { enemy_cap: usize, spawn_interval: f32, damage_number_ttl: f32, blip_cap: usize, inventory_size: usize, arena_half: f32, seed: u64 }`, `SimConfig::play()`, `SimConfig::stress()`, `SimConfig::from_env()`.

- [ ] **Step 1: Create `src/sim/mod.rs` with the module tree**

```rust
use bevy::prelude::*;

pub mod config;

pub use config::SimConfig;
```

- [ ] **Step 2: Write the failing test in `src/sim/config.rs`**

```rust
use bevy::prelude::*;

#[derive(Resource, Clone, Debug)]
pub struct SimConfig {
    pub enemy_cap: usize,
    pub spawn_interval: f32,
    pub damage_number_ttl: f32,
    pub blip_cap: usize,
    pub inventory_size: usize,
    pub arena_half: f32,
    pub seed: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stress_has_more_enemies_than_play() {
        assert!(SimConfig::stress().enemy_cap > SimConfig::play().enemy_cap);
    }

    #[test]
    fn seed_is_nonzero() {
        assert_ne!(SimConfig::play().seed, 0, "xorshift seed must be nonzero");
    }
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test -p horde config::tests`
Expected: FAIL — `SimConfig::play`/`stress` not found.

- [ ] **Step 4: Implement presets and env override**

Add to `src/sim/config.rs`:

```rust
impl SimConfig {
    pub fn play() -> Self {
        SimConfig {
            enemy_cap: 60,
            spawn_interval: 0.8,
            damage_number_ttl: 0.9,
            blip_cap: 80,
            inventory_size: 4,
            arena_half: 600.0,
            seed: 0x00C0FFEE_D00Du64,
        }
    }

    pub fn stress() -> Self {
        SimConfig {
            enemy_cap: 400,
            spawn_interval: 0.15,
            blip_cap: 400,
            ..Self::play()
        }
    }

    /// Preset from `HORDE_PRESET` (`play`|`stress`, default `play`), then per-field
    /// overrides from `HORDE_SEED`, `HORDE_ENEMY_CAP`, `HORDE_ARENA_HALF`.
    pub fn from_env() -> Self {
        let mut cfg = match std::env::var("HORDE_PRESET").as_deref() {
            Ok("stress") => Self::stress(),
            _ => Self::play(),
        };
        if let Ok(v) = std::env::var("HORDE_SEED") {
            if let Ok(n) = v.parse::<u64>() {
                if n != 0 {
                    cfg.seed = n;
                }
            }
        }
        if let Ok(v) = std::env::var("HORDE_ENEMY_CAP") {
            if let Ok(n) = v.parse::<usize>() {
                cfg.enemy_cap = n;
            }
        }
        if let Ok(v) = std::env::var("HORDE_ARENA_HALF") {
            if let Ok(n) = v.parse::<f32>() {
                cfg.arena_half = n;
            }
        }
        cfg
    }
}
```

- [ ] **Step 5: Add `mod sim;` to `main.rs`** (below `mod ui;`).

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test -p horde config::tests`
Expected: PASS (2 tests).

- [ ] **Step 7: Commit**

```bash
git add examples/horde/src
git commit -m "feat(horde): SimConfig resource with play/stress presets + env override"
```

---

### Task 3: Seeded RNG resource

**Files:**
- Create: `examples/horde/src/sim/rng.rs`
- Modify: `examples/horde/src/sim/mod.rs` (add `pub mod rng;`)

**Interfaces:**
- Produces: `Rng { state: u64 }`, `Rng::new(seed: u64)`, `Rng::next_u64() -> u64`, `Rng::next_f32() -> f32` (0.0..1.0), `Rng::range(lo: f32, hi: f32) -> f32`, `Rng::unit_vec() -> Vec2`.

- [ ] **Step 1: Write the failing test in `src/sim/rng.rs`**

```rust
use bevy::prelude::*;

#[derive(Resource, Clone)]
pub struct Rng {
    pub state: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_sequence() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..1000 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn f32_in_unit_range() {
        let mut r = Rng::new(7);
        for _ in 0..10_000 {
            let v = r.next_f32();
            assert!((0.0..1.0).contains(&v), "out of range: {v}");
        }
    }

    #[test]
    fn range_respects_bounds() {
        let mut r = Rng::new(9);
        for _ in 0..10_000 {
            let v = r.range(-3.0, 5.0);
            assert!((-3.0..=5.0).contains(&v));
        }
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p horde rng::tests`
Expected: FAIL — `Rng::new` not found.

- [ ] **Step 3: Implement the xorshift64\* RNG**

Add to `src/sim/rng.rs`:

```rust
impl Rng {
    pub fn new(seed: u64) -> Self {
        // xorshift requires nonzero state.
        Rng { state: if seed == 0 { 0x9E3779B97F4A7C15 } else { seed } }
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(0x2545F491_4F6CDD1D)
    }

    /// Uniform in [0.0, 1.0). Uses the top 24 bits for an exact f32 mantissa.
    pub fn next_f32(&mut self) -> f32 {
        ((self.next_u64() >> 40) as f32) / ((1u32 << 24) as f32)
    }

    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.next_f32()
    }

    /// Uniformly-distributed unit vector.
    pub fn unit_vec(&mut self) -> Vec2 {
        let a = self.range(0.0, std::f32::consts::TAU);
        Vec2::new(a.cos(), a.sin())
    }
}
```

- [ ] **Step 4: Register the module** — in `src/sim/mod.rs` add `pub mod rng;` and `pub use rng::Rng;`.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p horde rng::tests`
Expected: PASS (3 tests).

- [ ] **Step 6: Commit**

```bash
git add examples/horde/src/sim
git commit -m "feat(horde): deterministic seeded xorshift RNG resource"
```

---

### Task 4: `Intent` enum and `IntentQueue` resource

**Files:**
- Create: `examples/horde/src/sim/intent.rs`
- Modify: `examples/horde/src/sim/mod.rs` (add `pub mod intent;`)

**Interfaces:**
- Produces: `enum Intent`, `IntentQueue(Vec<Intent>)` with `push`, `drain() -> std::vec::Drain<Intent>`; `WeaponKind` (defined here for `SwitchWeapon` clarity is deferred — keep `SwitchWeapon(usize)` index-based).

- [ ] **Step 1: Write the failing test in `src/sim/intent.rs`**

```rust
use bevy::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Intent {
    Move(Vec2),
    Aim(Vec2),
    Shoot(bool),
    SwitchWeapon(usize),
    CycleWeapon(i32),
    ToggleInventory,
    Pause,
    Resume,
    Restart,
    StartGame,
    Quit,
}

#[derive(Resource, Default)]
pub struct IntentQueue(pub Vec<Intent>);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drain_empties_queue() {
        let mut q = IntentQueue::default();
        q.push(Intent::Pause);
        q.push(Intent::Resume);
        let drained: Vec<_> = q.drain().collect();
        assert_eq!(drained, vec![Intent::Pause, Intent::Resume]);
        assert!(q.0.is_empty());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p horde intent::tests`
Expected: FAIL — `push`/`drain` not found.

- [ ] **Step 3: Implement the queue methods**

```rust
impl IntentQueue {
    pub fn push(&mut self, i: Intent) {
        self.0.push(i);
    }
    pub fn drain(&mut self) -> std::vec::Drain<'_, Intent> {
        self.0.drain(..)
    }
}
```

- [ ] **Step 4: Register the module** — `pub mod intent;` + `pub use intent::{Intent, IntentQueue};` in `sim/mod.rs`.

- [ ] **Step 5: Run test / commit**

Run: `cargo test -p horde intent::tests` → PASS.

```bash
git add examples/horde/src/sim
git commit -m "feat(horde): Intent enum + IntentQueue resource"
```

---

### Task 5: `GameState` states and `SimPlugin` skeleton

**Files:**
- Create: `examples/horde/src/game_state.rs`
- Modify: `examples/horde/src/sim/mod.rs` (add `SimPlugin`)
- Modify: `examples/horde/src/main.rs` (init state, add plugins, wire `ui::add_ui`)

**Interfaces:**
- Produces: `enum GameState { MainMenu, Playing, Paused, GameOver }` (derives `States`), `SimPlugin`.
- Consumes: `SimConfig::from_env()`, `Rng::new`, `IntentQueue`.

- [ ] **Step 1: Write `src/game_state.rs`**

```rust
use bevy::prelude::*;

#[derive(States, Default, Debug, Clone, PartialEq, Eq, Hash)]
pub enum GameState {
    #[default]
    MainMenu,
    Playing,
    Paused,
    GameOver,
}
```

- [ ] **Step 2: Write `SimPlugin` in `src/sim/mod.rs`**

Replace the file contents with:

```rust
use bevy::prelude::*;

pub mod config;
pub mod rng;
pub mod intent;

pub use config::SimConfig;
pub use rng::Rng;
pub use intent::{Intent, IntentQueue};

/// The game simulation. No dependency on `crate::ui`, `bevy_ui`, or Boa.
pub struct SimPlugin;

impl Plugin for SimPlugin {
    fn build(&self, app: &mut App) {
        let cfg = SimConfig::from_env();
        let rng = Rng::new(cfg.seed);
        app.insert_resource(cfg)
            .insert_resource(rng)
            .init_resource::<IntentQueue>();
        // Systems added in later tasks run in FixedUpdate, gated on GameState::Playing.
    }
}
```

- [ ] **Step 3: Wire `main.rs`**

```rust
use bevy::prelude::*;

mod game_state;
mod sim;
mod ui;

use game_state::GameState;

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(AssetPlugin {
        watch_for_changes_override: Some(true),
        ..default()
    }))
    .init_state::<GameState>()
    .add_plugins(sim::SimPlugin)
    .add_systems(Startup, setup_camera);

    ui::add_ui(&mut app);

    app.run();
}

fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}
```

- [ ] **Step 4: Build and run**

Run: `cargo run -p horde`
Expected: window opens, no panic. (Still empty — sim entities/UI come next.)

- [ ] **Step 5: Commit**

```bash
git add examples/horde/src
git commit -m "feat(horde): GameState states + SimPlugin skeleton, wired into app"
```

---

### Task 6: Shared sim components + player spawn + movement

**Files:**
- Modify: `examples/horde/src/sim/mod.rs` (add components + `EnemyKind`/`WeaponKind` + `weapon_stats`)
- Create: `examples/horde/src/sim/player.rs`

**Interfaces:**
- Produces components: `Player`, `Health { current: f32, max: f32 }`, `Velocity(Vec2)`, `Facing(Vec2)`, `Inventory { slots: Vec<WeaponKind>, active: usize }`, `FireCooldown(f32)`, `Ammo { current: u32, size: u32, reload: f32 }`.
- Produces enums: `EnemyKind { Grunt, Fast, Brute }`, `WeaponKind { Pistol, Shotgun, Smg, Rocket }`, `WeaponStats`, `weapon_stats(WeaponKind) -> WeaponStats`.
- Produces: `spawn_player(commands, cfg)`, `player_movement` system.

- [ ] **Step 1: Add components + enums + weapon table to `sim/mod.rs`**

Append to `src/sim/mod.rs`:

```rust
pub mod player;

#[derive(Component)]
pub struct Player;

#[derive(Component, Clone, Copy)]
pub struct Health {
    pub current: f32,
    pub max: f32,
}

#[derive(Component, Clone, Copy, Default)]
pub struct Velocity(pub Vec2);

#[derive(Component, Clone, Copy)]
pub struct Facing(pub Vec2);

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnemyKind {
    Grunt,
    Fast,
    Brute,
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeaponKind {
    Pistol,
    Shotgun,
    Smg,
    Rocket,
}

impl WeaponKind {
    pub const ALL: [WeaponKind; 4] =
        [WeaponKind::Pistol, WeaponKind::Shotgun, WeaponKind::Smg, WeaponKind::Rocket];
    pub fn name(self) -> &'static str {
        match self {
            WeaponKind::Pistol => "Pistol",
            WeaponKind::Shotgun => "Shotgun",
            WeaponKind::Smg => "SMG",
            WeaponKind::Rocket => "Rocket",
        }
    }
}

#[derive(Clone, Copy)]
pub struct WeaponStats {
    pub fire_interval: f32, // seconds between shots
    pub damage: f32,
    pub spread: f32,        // radians, total cone
    pub projectiles: u32,
    pub speed: f32,         // projectile units/sec
    pub mag_size: u32,
    pub reload_time: f32,
}

pub fn weapon_stats(kind: WeaponKind) -> WeaponStats {
    match kind {
        WeaponKind::Pistol => WeaponStats { fire_interval: 0.35, damage: 12.0, spread: 0.02, projectiles: 1, speed: 620.0, mag_size: 12, reload_time: 0.9 },
        WeaponKind::Shotgun => WeaponStats { fire_interval: 0.75, damage: 7.0, spread: 0.5, projectiles: 7, speed: 560.0, mag_size: 6, reload_time: 1.4 },
        WeaponKind::Smg => WeaponStats { fire_interval: 0.09, damage: 5.0, spread: 0.12, projectiles: 1, speed: 700.0, mag_size: 30, reload_time: 1.2 },
        WeaponKind::Rocket => WeaponStats { fire_interval: 1.1, damage: 60.0, spread: 0.0, projectiles: 1, speed: 420.0, mag_size: 3, reload_time: 1.8 },
    }
}

#[derive(Component)]
pub struct Inventory {
    pub slots: Vec<WeaponKind>,
    pub active: usize,
}

impl Inventory {
    pub fn active_kind(&self) -> WeaponKind {
        self.slots[self.active]
    }
}

#[derive(Component, Clone, Copy)]
pub struct FireCooldown(pub f32);

#[derive(Component, Clone, Copy)]
pub struct Ammo {
    pub current: u32,
    pub size: u32,
    pub reload: f32, // seconds remaining; 0.0 = ready
}
```

- [ ] **Step 2: Write the failing test in `src/sim/player.rs`**

```rust
use bevy::prelude::*;
use crate::sim::*;

/// Integrates velocity from a Move intent and clamps to the arena.
pub fn player_movement(
    time: Res<Time>,
    cfg: Res<SimConfig>,
    mut q: Query<&mut Transform, With<Player>>,
    mut intents: ResMut<IntentQueue>,
) {
    let dt = time.delta_secs();
    let speed = 260.0_f32;
    let mut dir = Vec2::ZERO;
    let mut aim: Option<Vec2> = None;
    for i in intents.0.iter() {
        match i {
            Intent::Move(v) => dir = *v,
            Intent::Aim(v) => aim = Some(*v),
            _ => {}
        }
    }
    let _ = aim; // facing handled in a later task; kept for interface stability
    for mut t in q.iter_mut() {
        let delta = dir.clamp_length_max(1.0) * speed * dt;
        let half = cfg.arena_half;
        t.translation.x = (t.translation.x + delta.x).clamp(-half, half);
        t.translation.y = (t.translation.y + delta.y).clamp(-half, half);
    }
    let _ = &mut intents; // intents drained by the sim entry system in Task 15
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixed_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.insert_resource(SimConfig::play());
        app.init_resource::<IntentQueue>();
        app
    }

    #[test]
    fn move_intent_moves_player_and_clamps() {
        let mut app = fixed_app();
        let id = app
            .world_mut()
            .spawn((Player, Transform::from_xyz(595.0, 0.0, 0.0)))
            .id();
        app.world_mut().resource_mut::<IntentQueue>().push(Intent::Move(Vec2::X));
        app.add_systems(Update, player_movement);
        // Force a known dt by advancing time twice.
        app.update();
        app.update();
        let t = app.world().get::<Transform>(id).unwrap();
        assert!(t.translation.x <= app.world().resource::<SimConfig>().arena_half);
        assert!(t.translation.x >= 595.0, "should not move backwards");
    }
}
```

- [ ] **Step 3: Run test to verify it fails, then passes**

Run: `cargo test -p horde player::tests`
Expected: first FAIL if `player` module not registered; add `// player module already declared in mod.rs Step 1`. Then PASS.

- [ ] **Step 4: Add `spawn_player`**

Append to `src/sim/player.rs`:

```rust
pub fn spawn_player(commands: &mut Commands, cfg: &SimConfig) {
    let slots: Vec<WeaponKind> =
        WeaponKind::ALL.iter().copied().take(cfg.inventory_size.max(1)).collect();
    let start = WeaponKind::Pistol;
    let stats = weapon_stats(start);
    commands.spawn((
        Player,
        Transform::from_xyz(0.0, 0.0, 1.0),
        Health { current: 100.0, max: 100.0 },
        Velocity::default(),
        Facing(Vec2::Y),
        Inventory { slots: vec![start], active: 0 },
        FireCooldown(0.0),
        Ammo { current: stats.mag_size, size: stats.mag_size, reload: 0.0 },
    ));
    let _ = slots; // full inventory is granted via pickups (Task 12)
}
```

- [ ] **Step 5: Run tests / commit**

Run: `cargo test -p horde player::tests` → PASS.

```bash
git add examples/horde/src/sim
git commit -m "feat(horde): sim components, weapon table, player spawn + movement"
```

---

### Task 7: Wave spawner — enemies at arena edges

**Files:**
- Create: `examples/horde/src/sim/enemy.rs`
- Create: `examples/horde/src/sim/spawn.rs`
- Modify: `examples/horde/src/sim/mod.rs` (register modules)

**Interfaces:**
- Produces: component `Enemy { kind: EnemyKind }`; `enemy_stats(EnemyKind) -> EnemyStats { hp, speed, damage, radius }`; resource `SpawnState { timer: f32, wave: u32 }`; `edge_spawn_pos(rng, arena_half) -> Vec2`; system `spawn_waves`.

- [ ] **Step 1: Write `src/sim/enemy.rs` base types**

```rust
use bevy::prelude::*;
use crate::sim::*;

#[derive(Component, Clone, Copy)]
pub struct Enemy {
    pub kind: EnemyKind,
}

#[derive(Clone, Copy)]
pub struct EnemyStats {
    pub hp: f32,
    pub speed: f32,
    pub damage: f32,
    pub radius: f32,
}

pub fn enemy_stats(kind: EnemyKind) -> EnemyStats {
    match kind {
        EnemyKind::Grunt => EnemyStats { hp: 30.0, speed: 70.0, damage: 8.0, radius: 14.0 },
        EnemyKind::Fast => EnemyStats { hp: 16.0, speed: 130.0, damage: 5.0, radius: 11.0 },
        EnemyKind::Brute => EnemyStats { hp: 90.0, speed: 45.0, damage: 18.0, radius: 22.0 },
    }
}
```

- [ ] **Step 2: Write the failing test in `src/sim/spawn.rs`**

```rust
use bevy::prelude::*;
use crate::sim::*;
use crate::sim::enemy::{enemy_stats, Enemy, EnemyStats};

#[derive(Resource, Default)]
pub struct SpawnState {
    pub timer: f32,
    pub wave: u32,
}

/// A point on the square arena boundary of half-extent `arena_half`.
pub fn edge_spawn_pos(rng: &mut Rng, arena_half: f32) -> Vec2 {
    let t = rng.range(-arena_half, arena_half);
    match (rng.next_u64() % 4) as u8 {
        0 => Vec2::new(t, arena_half),
        1 => Vec2::new(t, -arena_half),
        2 => Vec2::new(arena_half, t),
        _ => Vec2::new(-arena_half, t),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edge_positions_are_on_the_boundary() {
        let mut rng = Rng::new(5);
        for _ in 0..1000 {
            let p = edge_spawn_pos(&mut rng, 600.0);
            let on_edge = (p.x.abs() - 600.0).abs() < 0.001 || (p.y.abs() - 600.0).abs() < 0.001;
            assert!(on_edge, "not on edge: {p:?}");
        }
    }
}
```

- [ ] **Step 3: Run test to verify it fails, then passes**

Run: `cargo test -p horde spawn::tests`
Expected: FAIL then, after registering modules (Step 5), PASS.

- [ ] **Step 4: Implement `spawn_waves`**

Append to `src/sim/spawn.rs`:

```rust
pub fn spawn_waves(
    time: Res<Time>,
    cfg: Res<SimConfig>,
    mut rng: ResMut<Rng>,
    mut state: ResMut<SpawnState>,
    enemies: Query<(), With<Enemy>>,
    mut commands: Commands,
) {
    state.timer -= time.delta_secs();
    if state.timer > 0.0 {
        return;
    }
    state.timer = cfg.spawn_interval;
    state.wave += 1;

    let live = enemies.iter().count();
    let room = cfg.enemy_cap.saturating_sub(live);
    let batch = room.min(6 + (state.wave as usize / 2));
    for _ in 0..batch {
        let kind = match rng.next_u64() % 10 {
            0..=5 => EnemyKind::Grunt,
            6..=8 => EnemyKind::Fast,
            _ => EnemyKind::Brute,
        };
        let stats: EnemyStats = enemy_stats(kind);
        let pos = edge_spawn_pos(&mut rng, cfg.arena_half);
        commands.spawn((
            Enemy { kind },
            Transform::from_xyz(pos.x, pos.y, 0.0),
            Health { current: stats.hp, max: stats.hp },
            Velocity::default(),
        ));
    }
}
```

- [ ] **Step 5: Register modules** — in `sim/mod.rs` add `pub mod enemy;` and `pub mod spawn;`.

- [ ] **Step 6: Run tests / commit**

Run: `cargo test -p horde spawn::tests` → PASS.

```bash
git add examples/horde/src/sim
git commit -m "feat(horde): wave spawner + enemy stats, edge spawning"
```

---

### Task 8: Enemy seek + separation + melee contact damage

**Files:**
- Modify: `examples/horde/src/sim/enemy.rs`

**Interfaces:**
- Produces: `separation(pos, neighbors, radius) -> Vec2` (pure), systems `enemy_movement`, `enemy_melee`.
- Consumes: `Player` transform, `Health`, `enemy_stats`.

- [ ] **Step 1: Write the failing test (pure separation) in `enemy.rs`**

```rust
/// Sum of push-away vectors from neighbors closer than `min_dist`.
pub fn separation(pos: Vec2, neighbors: &[Vec2], min_dist: f32) -> Vec2 {
    let mut push = Vec2::ZERO;
    for &n in neighbors {
        let d = pos - n;
        let len = d.length();
        if len > 0.0001 && len < min_dist {
            push += d / len * (min_dist - len);
        }
    }
    push
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn separation_pushes_away_from_close_neighbor() {
        let p = Vec2::new(0.0, 0.0);
        let f = separation(p, &[Vec2::new(1.0, 0.0)], 10.0);
        assert!(f.x < 0.0, "should push in -x away from neighbor at +x");
    }

    #[test]
    fn separation_ignores_far_neighbors() {
        let p = Vec2::ZERO;
        let f = separation(p, &[Vec2::new(100.0, 0.0)], 10.0);
        assert_eq!(f, Vec2::ZERO);
    }
}
```

- [ ] **Step 2: Run test → FAIL → implement → PASS**

Run: `cargo test -p horde enemy::tests` (FAIL: `separation` missing until Step 1 saved; then PASS).

- [ ] **Step 3: Implement the movement + melee systems**

Append to `enemy.rs`:

```rust
pub fn enemy_movement(
    time: Res<Time>,
    player: Query<&Transform, (With<Player>, Without<Enemy>)>,
    mut enemies: Query<(&Enemy, &mut Transform), Without<Player>>,
) {
    let Ok(player_t) = player.single() else { return };
    let target = player_t.translation.truncate();
    let dt = time.delta_secs();

    let positions: Vec<Vec2> =
        enemies.iter().map(|(_, t)| t.translation.truncate()).collect();

    for (enemy, mut t) in enemies.iter_mut() {
        let pos = t.translation.truncate();
        let seek = (target - pos).normalize_or_zero();
        let sep = separation(pos, &positions, 26.0) * 0.02;
        let stats = enemy_stats(enemy.kind);
        let dir = (seek + sep).normalize_or_zero();
        let step = dir * stats.speed * dt;
        t.translation.x += step.x;
        t.translation.y += step.y;
    }
}

pub fn enemy_melee(
    time: Res<Time>,
    mut player: Query<(&Transform, &mut Health), With<Player>>,
    enemies: Query<(&Enemy, &Transform), Without<Player>>,
) {
    let Ok((player_t, mut hp)) = player.single_mut() else { return };
    let ppos = player_t.translation.truncate();
    let dt = time.delta_secs();
    for (enemy, t) in enemies.iter() {
        let stats = enemy_stats(enemy.kind);
        if ppos.distance(t.translation.truncate()) < stats.radius + 16.0 {
            // Damage-per-second on contact (scaled by dt for frame independence).
            hp.current = (hp.current - stats.damage * dt).max(0.0);
        }
    }
}
```

- [ ] **Step 4: Run tests / commit**

Run: `cargo test -p horde enemy::tests` → PASS (2 tests).

```bash
git add examples/horde/src/sim/enemy.rs
git commit -m "feat(horde): enemy seek + separation movement and melee contact damage"
```

---

### Task 9: Shooting → projectiles

**Files:**
- Create: `examples/horde/src/sim/projectile.rs`
- Modify: `examples/horde/src/sim/mod.rs` (register)
- Modify: `examples/horde/src/sim/player.rs` (`player_shoot` system + facing from Aim)

**Interfaces:**
- Produces: component `Projectile { damage: f32, ttl: f32 }`; systems `player_shoot`, `projectile_motion` (Task 10 adds collision).
- Consumes: `Facing`, `FireCooldown`, `Ammo`, `Inventory`, `weapon_stats`, `Rng`, `IntentQueue` (Aim/Shoot).

- [ ] **Step 1: Update `player.rs` to apply Aim to Facing**

In `player_movement`, replace the `let _ = aim;` block with:

```rust
    if let Some(a) = aim {
        for mut _t in q.iter_mut() {}
        // facing is stored on the player entity; update via a dedicated query below
    }
```

Then add a separate small system (cleaner than mixing):

```rust
pub fn player_aim(mut intents_dir: Local<Vec2>, intents: Res<IntentQueue>, mut q: Query<&mut Facing, With<Player>>) {
    for i in intents.0.iter() {
        if let Intent::Aim(v) = i {
            if *v != Vec2::ZERO {
                *intents_dir = v.normalize_or_zero();
            }
        }
    }
    for mut f in q.iter_mut() {
        if *intents_dir != Vec2::ZERO {
            f.0 = *intents_dir;
        }
    }
}
```

Revert `player_movement`'s aim handling back to `let _ = aim;` (aim now owned by `player_aim`).

- [ ] **Step 2: Write `projectile.rs` with a failing cadence test**

```rust
use bevy::prelude::*;
use crate::sim::*;

#[derive(Component, Clone, Copy)]
pub struct Projectile {
    pub damage: f32,
    pub ttl: f32,
}

pub fn projectile_motion(
    time: Res<Time>,
    mut q: Query<(Entity, &mut Transform, &mut Projectile, &Velocity)>,
    mut commands: Commands,
) {
    let dt = time.delta_secs();
    for (e, mut t, mut p, v) in q.iter_mut() {
        t.translation.x += v.0.x * dt;
        t.translation.y += v.0.y * dt;
        p.ttl -= dt;
        if p.ttl <= 0.0 {
            commands.entity(e).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projectile_despawns_after_ttl() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        let e = app
            .world_mut()
            .spawn((
                Projectile { damage: 1.0, ttl: 0.01 },
                Transform::default(),
                Velocity(Vec2::X),
            ))
            .id();
        app.add_systems(Update, projectile_motion);
        for _ in 0..5 {
            app.update();
        }
        assert!(app.world().get_entity(e).is_err(), "expired projectile should despawn");
    }
}
```

- [ ] **Step 3: Run → FAIL → register `pub mod projectile;` in `sim/mod.rs` → PASS**

Run: `cargo test -p horde projectile::tests` → PASS.

- [ ] **Step 4: Implement `player_shoot` in `player.rs`**

```rust
use crate::sim::projectile::Projectile;

pub fn player_shoot(
    time: Res<Time>,
    intents: Res<IntentQueue>,
    mut rng: ResMut<Rng>,
    mut q: Query<(&Transform, &Facing, &Inventory, &mut FireCooldown, &mut Ammo), With<Player>>,
    mut commands: Commands,
) {
    let dt = time.delta_secs();
    let shooting = intents.0.iter().any(|i| matches!(i, Intent::Shoot(true)));
    for (t, facing, inv, mut cd, mut ammo) in q.iter_mut() {
        // Reload handling.
        if ammo.reload > 0.0 {
            ammo.reload = (ammo.reload - dt).max(0.0);
            if ammo.reload == 0.0 {
                ammo.current = ammo.size;
            }
            continue;
        }
        cd.0 = (cd.0 - dt).max(0.0);
        if !shooting || cd.0 > 0.0 || ammo.current == 0 {
            if ammo.current == 0 {
                let stats = weapon_stats(inv.active_kind());
                ammo.reload = stats.reload_time;
            }
            continue;
        }
        let stats = weapon_stats(inv.active_kind());
        cd.0 = stats.fire_interval;
        ammo.current = ammo.current.saturating_sub(1);
        let base = facing.0.normalize_or_zero();
        for k in 0..stats.projectiles {
            let frac = if stats.projectiles > 1 {
                (k as f32 / (stats.projectiles - 1) as f32) - 0.5
            } else {
                0.0
            };
            let jitter = rng.range(-0.02, 0.02);
            let angle = frac * stats.spread + jitter;
            let (s, c) = angle.sin_cos();
            let dir = Vec2::new(base.x * c - base.y * s, base.x * s + base.y * c);
            commands.spawn((
                Projectile { damage: stats.damage, ttl: 1.2 },
                Transform::from_xyz(t.translation.x, t.translation.y, 0.5),
                Velocity(dir * stats.speed),
            ));
        }
    }
}
```

- [ ] **Step 5: Run tests / commit**

Run: `cargo test -p horde` → all PASS.

```bash
git add examples/horde/src/sim
git commit -m "feat(horde): shooting spawns projectiles (spread/mag/reload) + projectile motion"
```

---

### Task 10: Projectile ↔ enemy collision, damage, death, kills

**Files:**
- Modify: `examples/horde/src/sim/projectile.rs`
- Create: `examples/horde/src/sim/damage.rs` (Progression + events; full body in Task 13/14 — introduce minimal here)
- Modify: `examples/horde/src/sim/mod.rs` (register `damage`)

**Interfaces:**
- Produces: `DamageEvent { pos: Vec2, amount: f32, crit: bool }` (event), `Progression { kills, xp, level, wave, pickups, elapsed }` resource; system `projectile_collision`.
- Consumes: `Projectile`, `Enemy`, `Health`, `enemy_stats`.

- [ ] **Step 1: Introduce `damage.rs` minimal types**

```rust
use bevy::prelude::*;

#[derive(Event, Clone, Copy)]
pub struct DamageEvent {
    pub pos: Vec2,
    pub amount: f32,
    pub crit: bool,
}

#[derive(Resource, Default, Clone)]
pub struct Progression {
    pub kills: u32,
    pub xp: u32,
    pub level: u32,
    pub wave: u32,
    pub pickups: u32,
    pub elapsed: f32,
}
```

Register in `sim/mod.rs`: `pub mod damage;` and in `SimPlugin::build` add `.init_resource::<Progression>().add_event::<damage::DamageEvent>()`.

- [ ] **Step 2: Write the failing collision test in `projectile.rs`**

```rust
#[cfg(test)]
mod collision_tests {
    use super::*;
    use crate::sim::enemy::Enemy;
    use crate::sim::damage::Progression;

    #[test]
    fn projectile_hits_enemy_deals_damage_and_despawns() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<Progression>();
        app.add_event::<crate::sim::damage::DamageEvent>();
        let enemy = app.world_mut().spawn((
            Enemy { kind: EnemyKind::Grunt },
            Transform::from_xyz(0.0, 0.0, 0.0),
            Health { current: 30.0, max: 30.0 },
        )).id();
        let proj = app.world_mut().spawn((
            Projectile { damage: 12.0, ttl: 1.0 },
            Transform::from_xyz(0.0, 0.0, 0.5),
            Velocity(Vec2::ZERO),
        )).id();
        app.add_systems(Update, projectile_collision);
        app.update();
        assert!(app.world().get_entity(proj).is_err(), "projectile consumed on hit");
        let hp = app.world().get::<Health>(enemy).unwrap();
        assert_eq!(hp.current, 18.0);
    }
}
```

- [ ] **Step 3: Run → FAIL → implement `projectile_collision` → PASS**

Append to `projectile.rs`:

```rust
use crate::sim::enemy::{enemy_stats, Enemy};
use crate::sim::damage::{DamageEvent, Progression};

pub fn projectile_collision(
    projectiles: Query<(Entity, &Transform, &Projectile)>,
    mut enemies: Query<(Entity, &Enemy, &Transform, &mut Health)>,
    mut progression: ResMut<Progression>,
    mut dmg_events: EventWriter<DamageEvent>,
    mut commands: Commands,
) {
    for (pe, pt, proj) in projectiles.iter() {
        let ppos = pt.translation.truncate();
        for (ee, enemy, et, mut hp) in enemies.iter_mut() {
            let r = enemy_stats(enemy.kind).radius + 4.0;
            if ppos.distance(et.translation.truncate()) <= r {
                hp.current -= proj.damage;
                dmg_events.write(DamageEvent {
                    pos: et.translation.truncate(),
                    amount: proj.damage,
                    crit: false,
                });
                if hp.current <= 0.0 {
                    commands.entity(ee).despawn();
                    progression.kills += 1;
                    progression.xp += 5;
                    progression.level = 1 + progression.xp / 100;
                }
                commands.entity(pe).despawn();
                break;
            }
        }
    }
}
```

Run: `cargo test -p horde projectile` → PASS.

- [ ] **Step 4: Commit**

```bash
git add examples/horde/src/sim
git commit -m "feat(horde): projectile-enemy collision, damage events, kills/xp progression"
```

---

### Task 11: Player death → GameOver transition

**Files:**
- Modify: `examples/horde/src/sim/player.rs` (`player_death` system)
- Modify: `examples/horde/src/sim/mod.rs` (register system wiring happens in Task 15)

**Interfaces:**
- Produces: `player_death(query, next_state)` — sets `NextState<GameState>` to `GameOver` when player HP ≤ 0.

- [ ] **Step 1: Write the failing test in `player.rs`**

```rust
#[cfg(test)]
mod death_tests {
    use super::*;
    use crate::game_state::GameState;

    #[test]
    fn zero_hp_triggers_game_over() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_state::<GameState>();
        app.world_mut().spawn((Player, Health { current: 0.0, max: 100.0 }));
        app.add_systems(Update, player_death);
        app.update();
        // Applying state transition requires an extra update.
        app.update();
        assert_eq!(*app.world().resource::<State<GameState>>().get(), GameState::GameOver);
    }
}
```

- [ ] **Step 2: Run → FAIL → implement → PASS**

```rust
use crate::game_state::GameState;

pub fn player_death(
    q: Query<&Health, With<Player>>,
    mut next: ResMut<NextState<GameState>>,
) {
    if let Ok(hp) = q.single() {
        if hp.current <= 0.0 {
            next.set(GameState::GameOver);
        }
    }
}
```

Run: `cargo test -p horde player::death_tests` → PASS.

- [ ] **Step 3: Commit**

```bash
git add examples/horde/src/sim/player.rs
git commit -m "feat(horde): player death transitions to GameOver"
```

---

### Task 12: Pickups — spawn, grab, weapon switch

**Files:**
- Create: `examples/horde/src/sim/pickup.rs`
- Modify: `examples/horde/src/sim/mod.rs` (register)

**Interfaces:**
- Produces: component `Pickup { kind: WeaponKind }`; systems `spawn_pickups`, `grab_pickups`, `switch_weapon`.
- Consumes: `Rng`, `SimConfig`, `Player`/`Inventory`/`Ammo`, `Progression`, `IntentQueue` (SwitchWeapon/CycleWeapon).

- [ ] **Step 1: Write `pickup.rs` with a grab test**

```rust
use bevy::prelude::*;
use crate::sim::*;
use crate::sim::damage::Progression;

#[derive(Component, Clone, Copy)]
pub struct Pickup {
    pub kind: WeaponKind,
}

pub fn grab_pickups(
    mut commands: Commands,
    mut player: Query<(&Transform, &mut Inventory), With<Player>>,
    pickups: Query<(Entity, &Transform, &Pickup)>,
    mut progression: ResMut<Progression>,
) {
    let Ok((pt, mut inv)) = player.single_mut() else { return };
    let ppos = pt.translation.truncate();
    for (e, t, pk) in pickups.iter() {
        if ppos.distance(t.translation.truncate()) < 26.0 {
            if !inv.slots.contains(&pk.kind) {
                inv.slots.push(pk.kind);
            }
            progression.pickups += 1;
            commands.entity(e).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walking_over_pickup_adds_weapon() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<Progression>();
        app.world_mut().spawn((
            Player,
            Transform::from_xyz(0.0, 0.0, 0.0),
            Inventory { slots: vec![WeaponKind::Pistol], active: 0 },
        ));
        app.world_mut().spawn((
            Pickup { kind: WeaponKind::Shotgun },
            Transform::from_xyz(5.0, 0.0, 0.0),
        ));
        app.add_systems(Update, grab_pickups);
        app.update();
        let inv = app.world_mut().query::<&Inventory>().single(app.world()).unwrap();
        assert!(inv.slots.contains(&WeaponKind::Shotgun));
    }
}
```

- [ ] **Step 2: Run → FAIL → register `pub mod pickup;` → PASS**

Run: `cargo test -p horde pickup::tests` → PASS.

- [ ] **Step 3: Implement `spawn_pickups` and `switch_weapon`**

```rust
#[derive(Resource, Default)]
pub struct PickupTimer(pub f32);

pub fn spawn_pickups(
    time: Res<Time>,
    cfg: Res<SimConfig>,
    mut rng: ResMut<Rng>,
    mut timer: ResMut<PickupTimer>,
    existing: Query<(), With<Pickup>>,
    mut commands: Commands,
) {
    timer.0 -= time.delta_secs();
    if timer.0 > 0.0 || existing.iter().count() >= 5 {
        return;
    }
    timer.0 = 6.0;
    let kind = WeaponKind::ALL[(rng.next_u64() % 4) as usize];
    let half = cfg.arena_half * 0.8;
    let pos = Vec2::new(rng.range(-half, half), rng.range(-half, half));
    commands.spawn((Pickup { kind }, Transform::from_xyz(pos.x, pos.y, 0.0)));
}

pub fn switch_weapon(
    intents: Res<IntentQueue>,
    mut q: Query<(&mut Inventory, &mut Ammo), With<Player>>,
) {
    let Ok((mut inv, mut ammo)) = q.single_mut() else { return };
    let n = inv.slots.len();
    if n == 0 {
        return;
    }
    let mut changed = false;
    for i in intents.0.iter() {
        match i {
            Intent::SwitchWeapon(idx) if *idx < n => {
                inv.active = *idx;
                changed = true;
            }
            Intent::CycleWeapon(d) => {
                let cur = inv.active as i32;
                inv.active = (cur + d).rem_euclid(n as i32) as usize;
                changed = true;
            }
            _ => {}
        }
    }
    if changed {
        let stats = weapon_stats(inv.active_kind());
        ammo.size = stats.mag_size;
        ammo.current = stats.mag_size;
        ammo.reload = 0.0;
    }
}
```

Register `PickupTimer` in `SimPlugin` via `.init_resource::<pickup::PickupTimer>()` (Task 15 wiring).

- [ ] **Step 4: Run tests / commit**

Run: `cargo test -p horde pickup` → PASS.

```bash
git add examples/horde/src/sim
git commit -m "feat(horde): pickups spawn/grab + weapon switch (keys/scroll)"
```

---

### Task 13: Damage-number entities lifecycle

**Files:**
- Modify: `examples/horde/src/sim/damage.rs`

**Interfaces:**
- Produces: component `DamageNumber { amount: f32, crit: bool, age: f32, ttl: f32 }`; systems `spawn_damage_numbers` (from `DamageEvent`), `tick_damage_numbers`.
- Consumes: `DamageEvent`, `SimConfig::damage_number_ttl`.

- [ ] **Step 1: Write the failing test in `damage.rs`**

```rust
#[derive(Component, Clone, Copy)]
pub struct DamageNumber {
    pub amount: f32,
    pub crit: bool,
    pub age: f32,
    pub ttl: f32,
}

#[cfg(test)]
mod dn_tests {
    use super::*;
    use crate::sim::SimConfig;

    #[test]
    fn damage_number_despawns_after_ttl() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.insert_resource(SimConfig::play());
        let e = app.world_mut().spawn((
            DamageNumber { amount: 10.0, crit: false, age: 0.0, ttl: 0.02 },
            Transform::default(),
        )).id();
        app.add_systems(Update, tick_damage_numbers);
        for _ in 0..10 { app.update(); }
        assert!(app.world().get_entity(e).is_err());
    }
}
```

- [ ] **Step 2: Run → FAIL → implement → PASS**

```rust
pub fn spawn_damage_numbers(
    mut events: EventReader<DamageEvent>,
    cfg: Res<SimConfig>,
    mut commands: Commands,
) {
    for ev in events.read() {
        commands.spawn((
            DamageNumber { amount: ev.amount, crit: ev.crit, age: 0.0, ttl: cfg.damage_number_ttl },
            Transform::from_xyz(ev.pos.x, ev.pos.y, 2.0),
        ));
    }
}

pub fn tick_damage_numbers(
    time: Res<Time>,
    mut q: Query<(Entity, &mut Transform, &mut DamageNumber)>,
    mut commands: Commands,
) {
    let dt = time.delta_secs();
    for (e, mut t, mut dn) in q.iter_mut() {
        dn.age += dt;
        t.translation.y += 40.0 * dt; // drift up
        if dn.age >= dn.ttl {
            commands.entity(e).despawn();
        }
    }
}
```

Add `use crate::sim::SimConfig;` at the top of `damage.rs`.

Run: `cargo test -p horde damage::dn_tests` → PASS.

- [ ] **Step 3: Commit**

```bash
git add examples/horde/src/sim/damage.rs
git commit -m "feat(horde): floating damage-number entities with drift + ttl"
```

---

### Task 14: DPS window + combat log + wave/elapsed progression

**Files:**
- Modify: `examples/horde/src/sim/damage.rs`

**Interfaces:**
- Produces: resources `DamageHistory(VecDeque<(f32, f32)>)`, `CombatLog(VecDeque<LogEvent>)`; `LogEvent { text: String, age: f32 }`; systems `record_damage_history`, `dps_over_window(history, now, window) -> f32` (pure), `tick_progression`, `push_log(log, text)`.

- [ ] **Step 1: Write the failing pure test in `damage.rs`**

```rust
use std::collections::VecDeque;

#[derive(Resource, Default)]
pub struct DamageHistory(pub VecDeque<(f32, f32)>); // (timestamp, amount)

#[derive(Clone)]
pub struct LogEvent {
    pub text: String,
    pub age: f32,
}

#[derive(Resource, Default)]
pub struct CombatLog(pub VecDeque<LogEvent>);

/// Sum of damage within `window` seconds of `now`, divided by window = DPS.
pub fn dps_over_window(history: &VecDeque<(f32, f32)>, now: f32, window: f32) -> f32 {
    let cutoff = now - window;
    let sum: f32 = history.iter().filter(|(t, _)| *t >= cutoff).map(|(_, a)| *a).sum();
    sum / window
}

#[cfg(test)]
mod dps_tests {
    use super::*;

    #[test]
    fn dps_sums_recent_window_only() {
        let mut h = VecDeque::new();
        h.push_back((0.0, 100.0)); // old, excluded
        h.push_back((9.5, 20.0));
        h.push_back((10.0, 30.0));
        let dps = dps_over_window(&h, 10.0, 1.0); // window [9.0, 10.0]
        assert_eq!(dps, 50.0);
    }
}
```

- [ ] **Step 2: Run → FAIL → implement supporting systems → PASS**

```rust
pub fn push_log(log: &mut CombatLog, text: impl Into<String>) {
    log.0.push_front(LogEvent { text: text.into(), age: 0.0 });
    while log.0.len() > 8 {
        log.0.pop_back();
    }
}

pub fn record_damage_history(
    mut events: EventReader<DamageEvent>,
    mut history: ResMut<DamageHistory>,
    prog: Res<Progression>,
) {
    for ev in events.read() {
        history.0.push_back((prog.elapsed, ev.amount));
    }
    let cutoff = prog.elapsed - 3.0;
    while let Some((t, _)) = history.0.front() {
        if *t < cutoff {
            history.0.pop_front();
        } else {
            break;
        }
    }
}

pub fn tick_progression(
    time: Res<Time>,
    mut prog: ResMut<Progression>,
    mut log: ResMut<CombatLog>,
) {
    let prev_wave = prog.wave;
    prog.elapsed += time.delta_secs();
    for e in log.0.iter_mut() {
        e.age += time.delta_secs();
    }
    if prog.wave != prev_wave {
        push_log(&mut log, format!("Wave {}", prog.wave));
    }
}
```

Register in `SimPlugin`: `.init_resource::<DamageHistory>().init_resource::<CombatLog>()` (Task 15 wiring).
Run: `cargo test -p horde damage::dps_tests` → PASS.

- [ ] **Step 3: Commit**

```bash
git add examples/horde/src/sim/damage.rs
git commit -m "feat(horde): DPS sliding window, combat log, elapsed/wave progression"
```

---

### Task 15: `UiSnapshot` + assembly + full sim system wiring

**Files:**
- Create: `examples/horde/src/sim/snapshot.rs`
- Modify: `examples/horde/src/sim/mod.rs` (register snapshot; wire ALL sim systems into `FixedUpdate` gated on `Playing`; add the drain-intents entry system; init all resources)

**Interfaces:**
- Produces: `UiSnapshot` (Resource) + sub-types `Nameplate`, `FloatingNumber`, `Blip`, `BlipKind`, `WeaponSlot`, `LogLine`; system `assemble_world_snapshot`; entry system `drain_intents_into_state` for menu intents.
- Consumes: all sim components/resources above.

- [ ] **Step 1: Write `snapshot.rs` with an assembly test**

```rust
use bevy::prelude::*;
use std::collections::VecDeque;
use crate::sim::*;
use crate::sim::enemy::Enemy;
use crate::sim::damage::{DamageNumber, DamageHistory, CombatLog, Progression, dps_over_window};

#[derive(Clone, Copy)]
pub enum BlipKind { Player, Enemy, Pickup }

#[derive(Clone, Copy)]
pub struct Nameplate {
    pub id: u64,
    pub world_pos: Vec2,
    pub screen_pos: Vec2,
    pub hp: f32,
    pub max_hp: f32,
    pub kind: EnemyKind,
}

#[derive(Clone, Copy)]
pub struct FloatingNumber {
    pub id: u64,
    pub world_pos: Vec2,
    pub screen_pos: Vec2,
    pub amount: f32,
    pub crit: bool,
    pub age: f32,
    pub ttl: f32,
}

#[derive(Clone, Copy)]
pub struct Blip {
    pub id: u64,
    pub world_pos: Vec2,
    pub screen_pos: Vec2,
    pub kind: BlipKind,
}

#[derive(Clone, Copy)]
pub struct WeaponSlot {
    pub index: usize,
    pub kind: WeaponKind,
    pub active: bool,
}

#[derive(Clone)]
pub struct LogLine {
    pub text: String,
    pub age: f32,
}

#[derive(Resource, Default, Clone)]
pub struct UiSnapshot {
    pub player_hp: f32,
    pub player_max_hp: f32,
    pub xp: u32,
    pub level: u32,
    pub wave: u32,
    pub kills: u32,
    pub pickups: u32,
    pub active_weapon: Option<WeaponKind>,
    pub ammo: u32,
    pub ammo_size: u32,
    pub reloading: bool,
    pub cooldown_frac: f32,
    pub inventory: Vec<WeaponSlot>,
    pub enemies: Vec<Nameplate>,
    pub damage_numbers: Vec<FloatingNumber>,
    pub blips: Vec<Blip>,
    pub dps: f32,
    pub log: Vec<LogLine>,
    pub elapsed: f32,
}

impl Default for UiSnapshot {
    // derive can't handle Option/Vec defaults cleanly with our fields; provide explicitly.
    // (If derive(Default) compiles for your field set, delete this impl.)
    fn default() -> Self {
        UiSnapshot {
            player_hp: 0.0, player_max_hp: 1.0, xp: 0, level: 0, wave: 0, kills: 0, pickups: 0,
            active_weapon: None, ammo: 0, ammo_size: 0, reloading: false, cooldown_frac: 0.0,
            inventory: Vec::new(), enemies: Vec::new(), damage_numbers: Vec::new(),
            blips: Vec::new(), dps: 0.0, log: Vec::new(), elapsed: 0.0,
        }
    }
}
```

Note: remove the manual `Default` impl if `#[derive(Default)]` already compiles; keep only one.

Add the assembly system + test:

```rust
pub fn assemble_world_snapshot(
    mut snap: ResMut<UiSnapshot>,
    cfg: Res<SimConfig>,
    prog: Res<Progression>,
    history: Res<DamageHistory>,
    log: Res<CombatLog>,
    player: Query<(&Health, &Inventory, &Ammo, &FireCooldown, &Transform), With<Player>>,
    enemies: Query<(Entity, &Enemy, &Transform, &Health)>,
    dmg: Query<(Entity, &DamageNumber, &Transform)>,
    pickups: Query<(Entity, &Transform), With<crate::sim::pickup::Pickup>>,
) {
    let mut s = UiSnapshot::default();
    s.elapsed = prog.elapsed;
    s.xp = prog.xp; s.level = prog.level; s.wave = prog.wave;
    s.kills = prog.kills; s.pickups = prog.pickups;
    s.dps = dps_over_window(&history.0, prog.elapsed, 1.0);

    if let Ok((hp, inv, ammo, cd, ptrans)) = player.single() {
        s.player_hp = hp.current;
        s.player_max_hp = hp.max;
        s.active_weapon = Some(inv.active_kind());
        s.ammo = ammo.current;
        s.ammo_size = ammo.size;
        s.reloading = ammo.reload > 0.0;
        let interval = weapon_stats(inv.active_kind()).fire_interval.max(0.0001);
        s.cooldown_frac = (cd.0 / interval).clamp(0.0, 1.0);
        s.inventory = inv.slots.iter().enumerate().map(|(i, k)| WeaponSlot {
            index: i, kind: *k, active: i == inv.active,
        }).collect();
        s.blips.push(Blip { id: 0, world_pos: ptrans.translation.truncate(), screen_pos: Vec2::ZERO, kind: BlipKind::Player });
    }

    for (e, enemy, t, hp) in enemies.iter() {
        let wp = t.translation.truncate();
        s.enemies.push(Nameplate { id: e.to_bits(), world_pos: wp, screen_pos: Vec2::ZERO, hp: hp.current, max_hp: hp.max, kind: enemy.kind });
        if s.blips.len() < cfg.blip_cap {
            s.blips.push(Blip { id: e.to_bits(), world_pos: wp, screen_pos: Vec2::ZERO, kind: BlipKind::Enemy });
        }
    }
    for (e, t) in pickups.iter() {
        s.blips.push(Blip { id: e.to_bits(), world_pos: t.translation.truncate(), screen_pos: Vec2::ZERO, kind: BlipKind::Pickup });
    }
    for (e, dn, t) in dmg.iter() {
        s.damage_numbers.push(FloatingNumber { id: e.to_bits(), world_pos: t.translation.truncate(), screen_pos: Vec2::ZERO, amount: dn.amount, crit: dn.crit, age: dn.age, ttl: dn.ttl });
    }
    s.log = log.0.iter().map(|l| LogLine { text: l.text.clone(), age: l.age }).collect();

    *snap = s;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_reflects_player_and_enemies() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.insert_resource(SimConfig::play());
        app.init_resource::<UiSnapshot>();
        app.init_resource::<Progression>();
        app.init_resource::<DamageHistory>();
        app.init_resource::<CombatLog>();
        app.world_mut().spawn((
            Player, Transform::default(),
            Health { current: 80.0, max: 100.0 },
            Inventory { slots: vec![WeaponKind::Pistol], active: 0 },
            Ammo { current: 12, size: 12, reload: 0.0 },
            FireCooldown(0.0),
        ));
        app.world_mut().spawn((
            Enemy { kind: EnemyKind::Grunt }, Transform::from_xyz(50.0, 0.0, 0.0),
            Health { current: 30.0, max: 30.0 },
        ));
        app.add_systems(Update, assemble_world_snapshot);
        app.update();
        let s = app.world().resource::<UiSnapshot>();
        assert_eq!(s.player_hp, 80.0);
        assert_eq!(s.enemies.len(), 1);
        assert_eq!(s.enemies[0].kind, EnemyKind::Grunt);
    }
}
```

- [ ] **Step 2: Run → FAIL → register + wire → PASS**

In `sim/mod.rs`: add `pub mod snapshot;` and `pub use snapshot::UiSnapshot;`. Ensure `pub mod pickup;`, `pub mod projectile;` are declared.

- [ ] **Step 3: Wire the full `SimPlugin`**

Replace `SimPlugin::build` with the complete wiring:

```rust
impl Plugin for SimPlugin {
    fn build(&self, app: &mut App) {
        let cfg = SimConfig::from_env();
        let rng = Rng::new(cfg.seed);
        app.insert_resource(cfg)
            .insert_resource(rng)
            .init_resource::<IntentQueue>()
            .init_resource::<UiSnapshot>()
            .init_resource::<damage::Progression>()
            .init_resource::<damage::DamageHistory>()
            .init_resource::<damage::CombatLog>()
            .init_resource::<spawn::SpawnState>()
            .init_resource::<pickup::PickupTimer>()
            .add_event::<damage::DamageEvent>();

        app.add_systems(OnEnter(crate::game_state::GameState::Playing), setup_playing);

        app.add_systems(
            FixedUpdate,
            (
                player::player_aim,
                player::player_movement,
                player::player_shoot,
                projectile::projectile_motion,
                projectile::projectile_collision,
                enemy::enemy_movement,
                enemy::enemy_melee,
                spawn::spawn_waves,
                pickup::spawn_pickups,
                pickup::grab_pickups,
                pickup::switch_weapon,
                damage::spawn_damage_numbers,
                damage::tick_damage_numbers,
                damage::record_damage_history,
                damage::tick_progression,
                player::player_death,
            )
                .chain()
                .run_if(in_state(crate::game_state::GameState::Playing)),
        );

        // Snapshot assembly runs every frame regardless of pause so the UI can render.
        app.add_systems(Update, snapshot::assemble_world_snapshot);
        // Intents are cleared at end of frame after all consumers have read them.
        app.add_systems(Last, clear_intents);
    }
}

fn setup_playing(
    mut commands: Commands,
    cfg: Res<SimConfig>,
    existing: Query<Entity, Or<(With<Player>, With<enemy::Enemy>, With<projectile::Projectile>, With<pickup::Pickup>, With<damage::DamageNumber>)>>,
    mut prog: ResMut<damage::Progression>,
) {
    // Fresh start (also handles Restart): clear old sim entities and progression.
    for e in existing.iter() {
        commands.entity(e).despawn();
    }
    *prog = damage::Progression::default();
    player::spawn_player(&mut commands, &cfg);
}

fn clear_intents(mut q: ResMut<IntentQueue>) {
    q.0.clear();
}
```

Note: keep `spawn::SpawnState` and `spawn::SpawnState { wave }` in sync with `Progression.wave` — in `spawn_waves`, after `state.wave += 1;` also set progression wave. Add a `mut prog: ResMut<crate::sim::damage::Progression>` param to `spawn_waves` and `prog.wave = state.wave;`. Update the earlier `spawn_waves` signature accordingly and re-run `cargo test -p horde spawn` (still passes; the test doesn't add Progression, so gate that line — instead set progression wave in `tick_progression`? Simpler: read spawn wave). **Resolution:** add `mut prog` param and in the test add `app.init_resource::<Progression>()`. Update `spawn.rs` test's `fixed_app` accordingly.

- [ ] **Step 4: Run all sim tests**

Run: `cargo test -p horde`
Expected: PASS. Fix any signature drift flagged by the compiler (add `Progression` resource to the `spawn` test app).

- [ ] **Step 5: Commit**

```bash
git add examples/horde/src/sim
git commit -m "feat(horde): UiSnapshot assembly + full SimPlugin FixedUpdate wiring"
```

---

## Phase B — World rendering + input

### Task 16: Input mapping — raw input → `Intent`

**Files:**
- Create: `examples/horde/src/input.rs`
- Modify: `examples/horde/src/main.rs` (add `mod input;`, add system)

**Interfaces:**
- Produces: `gather_input` system writing to `IntentQueue`.
- Consumes: `ButtonInput<KeyCode>`, `ButtonInput<MouseButton>`, `MouseWheel`, `Window`, `Camera`, `IntentQueue`, `GameState`.

- [ ] **Step 1: Write `src/input.rs`**

```rust
use bevy::prelude::*;
use bevy::input::mouse::MouseWheel;
use crate::sim::{Intent, IntentQueue, Player};
use crate::game_state::GameState;

pub fn gather_input(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut wheel: EventReader<MouseWheel>,
    windows: Query<&Window>,
    cameras: Query<(&Camera, &GlobalTransform)>,
    players: Query<&Transform, With<Player>>,
    state: Res<State<GameState>>,
    mut intents: ResMut<IntentQueue>,
) {
    // Global: Escape toggles pause/resume; Enter starts / restarts.
    if keys.just_pressed(KeyCode::Escape) {
        match state.get() {
            GameState::Playing => intents.push(Intent::Pause),
            GameState::Paused => intents.push(Intent::Resume),
            _ => {}
        }
    }
    if keys.just_pressed(KeyCode::Enter) {
        match state.get() {
            GameState::MainMenu => intents.push(Intent::StartGame),
            GameState::GameOver => intents.push(Intent::Restart),
            _ => {}
        }
    }
    if *state.get() != GameState::Playing {
        return;
    }

    // Movement (WASD / arrows).
    let mut dir = Vec2::ZERO;
    if keys.any_pressed([KeyCode::KeyW, KeyCode::ArrowUp]) { dir.y += 1.0; }
    if keys.any_pressed([KeyCode::KeyS, KeyCode::ArrowDown]) { dir.y -= 1.0; }
    if keys.any_pressed([KeyCode::KeyA, KeyCode::ArrowLeft]) { dir.x -= 1.0; }
    if keys.any_pressed([KeyCode::KeyD, KeyCode::ArrowRight]) { dir.x += 1.0; }
    intents.push(Intent::Move(dir));

    // Aim: mouse world position relative to player.
    if let (Ok(window), Ok((camera, cam_t)), Ok(ptrans)) =
        (windows.single(), cameras.single(), players.single())
    {
        if let Some(cursor) = window.cursor_position() {
            if let Ok(world) = camera.viewport_to_world_2d(cam_t, cursor) {
                let aim = (world - ptrans.translation.truncate()).normalize_or_zero();
                if aim != Vec2::ZERO {
                    intents.push(Intent::Aim(aim));
                }
            }
        }
    }

    intents.push(Intent::Shoot(mouse.pressed(MouseButton::Left)));

    // Weapon switch: number keys 1-4.
    for (i, key) in [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4].iter().enumerate() {
        if keys.just_pressed(*key) {
            intents.push(Intent::SwitchWeapon(i));
        }
    }
    // Scroll cycles.
    let mut scroll = 0.0;
    for ev in wheel.read() { scroll += ev.y; }
    if scroll > 0.0 { intents.push(Intent::CycleWeapon(1)); }
    else if scroll < 0.0 { intents.push(Intent::CycleWeapon(-1)); }

    if keys.just_pressed(KeyCode::KeyI) {
        intents.push(Intent::ToggleInventory);
    }
}
```

- [ ] **Step 2: Wire into `main.rs`**

Add `mod input;` and, in `main()`, `.add_systems(PreUpdate, input::gather_input)`.

- [ ] **Step 3: Build**

Run: `cargo build -p horde`
Expected: compiles. (Intents are consumed by sim in `FixedUpdate`; visual result appears after Task 17.)

- [ ] **Step 4: Commit**

```bash
git add examples/horde/src
git commit -m "feat(horde): input mapping raw input -> Intent (move/aim/shoot/switch/pause)"
```

---

### Task 17: World rendering + menu-intent state machine → playable game (no HUD)

**Files:**
- Create: `examples/horde/src/world_render.rs`
- Modify: `examples/horde/src/main.rs` (add module + systems)
- Modify: `examples/horde/src/game_state.rs` (menu-intent transition system)

**Interfaces:**
- Produces: `sync_sprites` (adds/updates `Sprite` for sim entities), `apply_menu_intents` (drains menu intents → `NextState`).
- Consumes: sim components, `IntentQueue`.

- [ ] **Step 1: Write `apply_menu_intents` in `game_state.rs`**

```rust
use crate::sim::{Intent, IntentQueue};

pub fn apply_menu_intents(
    intents: Res<IntentQueue>,
    state: Res<State<GameState>>,
    mut next: ResMut<NextState<GameState>>,
) {
    for i in intents.0.iter() {
        match (state.get(), i) {
            (GameState::MainMenu, Intent::StartGame) => next.set(GameState::Playing),
            (GameState::Playing, Intent::Pause) => next.set(GameState::Paused),
            (GameState::Paused, Intent::Resume) => next.set(GameState::Playing),
            (GameState::GameOver, Intent::Restart) => next.set(GameState::Playing),
            (GameState::Paused, Intent::Restart) => next.set(GameState::Playing),
            (_, Intent::Quit) => { /* handled by app-exit system if desired */ }
            _ => {}
        }
    }
}
```

- [ ] **Step 2: Write `world_render.rs`**

```rust
use bevy::prelude::*;
use crate::sim::*;
use crate::sim::enemy::{enemy_stats, Enemy};
use crate::sim::projectile::Projectile;
use crate::sim::pickup::Pickup;

/// Ensures every sim entity that should be visible carries a `Sprite`, tinted by
/// type/state. Colored shapes only — no art assets (design §5).
pub fn sync_sprites(
    mut commands: Commands,
    players: Query<Entity, (With<Player>, Without<Sprite>)>,
    new_enemies: Query<(Entity, &Enemy), Without<Sprite>>,
    new_proj: Query<Entity, (With<Projectile>, Without<Sprite>)>,
    new_pick: Query<(Entity, &Pickup), Without<Sprite>>,
    mut enemy_tint: Query<(&Enemy, &Health, &mut Sprite)>,
) {
    for e in players.iter() {
        commands.entity(e).insert(Sprite {
            color: Color::srgb(0.3, 0.8, 1.0),
            custom_size: Some(Vec2::splat(22.0)),
            ..default()
        });
    }
    for (e, enemy) in new_enemies.iter() {
        let size = enemy_stats(enemy.kind).radius * 2.0;
        commands.entity(e).insert(Sprite {
            color: Color::srgb(0.9, 0.3, 0.3),
            custom_size: Some(Vec2::splat(size)),
            ..default()
        });
    }
    for e in new_proj.iter() {
        commands.entity(e).insert(Sprite {
            color: Color::srgb(1.0, 0.95, 0.5),
            custom_size: Some(Vec2::splat(6.0)),
            ..default()
        });
    }
    for (e, pk) in new_pick.iter() {
        let c = weapon_color(pk.kind);
        commands.entity(e).insert(Sprite { color: c, custom_size: Some(Vec2::splat(16.0)), ..default() });
    }
    // Tint enemies by remaining HP (green->red).
    for (_enemy, hp, mut sprite) in enemy_tint.iter_mut() {
        let f = (hp.current / hp.max).clamp(0.0, 1.0);
        sprite.color = Color::srgb(0.9 * (1.0 - f) + 0.2, 0.2 + 0.6 * f, 0.2);
    }
}

pub fn weapon_color(kind: WeaponKind) -> Color {
    match kind {
        WeaponKind::Pistol => Color::srgb(0.7, 0.7, 0.7),
        WeaponKind::Shotgun => Color::srgb(0.9, 0.6, 0.2),
        WeaponKind::Smg => Color::srgb(0.4, 0.7, 0.9),
        WeaponKind::Rocket => Color::srgb(0.9, 0.3, 0.5),
    }
}

/// Despawns sprites' entities is handled by sim; here we only draw an arena border once.
pub fn spawn_arena(mut commands: Commands, cfg: Res<SimConfig>) {
    commands.spawn((
        Sprite { color: Color::srgb(0.10, 0.10, 0.13), custom_size: Some(Vec2::splat(cfg.arena_half * 2.0)), ..default() },
        Transform::from_xyz(0.0, 0.0, -1.0),
    ));
}
```

- [ ] **Step 3: Wire into `main.rs`**

Add `mod world_render;`. In `main()`:

```rust
.add_systems(Startup, world_render::spawn_arena)
.add_systems(Update, (game_state::apply_menu_intents, world_render::sync_sprites))
```

- [ ] **Step 4: Run and play**

Run: `cargo run -p horde`
Then in-window: the app starts in `MainMenu` (no menu UI yet — press **Enter** to enter Playing). Move with WASD, aim with mouse, hold left-mouse to shoot, watch enemies spawn from edges, path in, and die; grab pickups; press **1-4**/scroll to switch weapons; **Esc** to pause. When the player dies, state flips to GameOver (press Enter to restart).
Expected: a fully playable game rendered as colored shapes, no HUD yet.

- [ ] **Step 5: Commit**

```bash
git add examples/horde/src
git commit -m "feat(horde): colored-sprite world rendering + menu-intent state machine (playable)"
```

---

## Phase C — Native UI

### Task 18: `NativeUiPlugin` scaffold + theme + widgets + snapshot projection

**Files:**
- Create: `examples/horde/src/ui/native/mod.rs`
- Create: `examples/horde/src/ui/native/theme.rs`
- Create: `examples/horde/src/ui/native/widgets.rs`
- Create: `examples/horde/src/ui/native/project.rs`
- Modify: `examples/horde/src/ui/mod.rs` (real `add_ui`)

**Interfaces:**
- Produces: `NativeUiPlugin`; `theme` color/space constants; widget helpers `bar`, `panel`, `label`, `menu_button`; `project_snapshot` system.
- Consumes: `UiSnapshot`, `Camera`.

- [ ] **Step 1: Replace `src/ui/mod.rs` with the real selector**

```rust
use bevy::prelude::*;

#[cfg(feature = "ui-native")]
pub mod native;

#[cfg(feature = "ui-native")]
pub fn add_ui(app: &mut App) {
    app.add_plugins(native::NativeUiPlugin);
}

#[cfg(not(feature = "ui-native"))]
pub fn add_ui(_app: &mut App) {
    panic!(
        "Supersolid UI backend not yet implemented — build with the default \
         `ui-native` feature. TODO(supersolid-runtime)."
    );
}
```

- [ ] **Step 2: Write `theme.rs`**

```rust
use bevy::prelude::*;

pub const BG: Color = Color::srgb(0.07, 0.07, 0.10);
pub const PANEL: Color = Color::srgba(0.13, 0.14, 0.20, 0.92);
pub const PANEL_BORDER: Color = Color::srgb(0.28, 0.30, 0.42);
pub const TEXT: Color = Color::srgb(0.90, 0.92, 0.98);
pub const TEXT_DIM: Color = Color::srgb(0.60, 0.63, 0.72);
pub const ACCENT: Color = Color::srgb(0.35, 0.75, 1.0);
pub const DANGER: Color = Color::srgb(0.95, 0.35, 0.38);
pub const GOOD: Color = Color::srgb(0.45, 0.85, 0.45);
pub const WARN: Color = Color::srgb(0.95, 0.75, 0.30);

pub const SPACE: f32 = 8.0;
pub const RADIUS: f32 = 6.0;
pub const FONT: f32 = 15.0;
pub const FONT_SM: f32 = 12.0;
pub const FONT_LG: f32 = 28.0;

/// HP-fraction color ramp used by health bars and nameplates.
pub fn hp_color(frac: f32) -> Color {
    let f = frac.clamp(0.0, 1.0);
    Color::srgb(0.9 * (1.0 - f) + 0.15, 0.25 + 0.6 * f, 0.28)
}
```

- [ ] **Step 3: Write `widgets.rs` (DRY spawn helpers)**

```rust
use bevy::prelude::*;
use super::theme;

/// A styled container node bundle.
pub fn panel(width: Val, padding: f32) -> impl Bundle {
    (
        Node {
            width,
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(padding)),
            row_gap: Val::Px(theme::SPACE),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(theme::PANEL),
        BorderColor::all(theme::PANEL_BORDER),
        BorderRadius::all(Val::Px(theme::RADIUS)),
    )
}

/// A text bundle at a given size/color.
pub fn label(text: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(text),
        TextFont::from_font_size(size),
        TextColor(color),
    )
}

/// A horizontal progress bar: outer track + inner fill sized by `frac`.
/// Returns the outer bundle; caller spawns the fill child via `bar_fill`.
pub fn bar_track(height: f32) -> impl Bundle {
    (
        Node {
            width: Val::Percent(100.0),
            height: Val::Px(height),
            ..default()
        },
        BackgroundColor(Color::srgb(0.18, 0.19, 0.26)),
        BorderRadius::all(Val::Px(3.0)),
    )
}

pub fn bar_fill(frac: f32, color: Color) -> impl Bundle {
    (
        Node {
            width: Val::Percent((frac.clamp(0.0, 1.0)) * 100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        BackgroundColor(color),
        BorderRadius::all(Val::Px(3.0)),
    )
}

/// A menu button with label. Add your own marker component alongside for click routing.
pub fn menu_button() -> impl Bundle {
    (
        Button,
        Node {
            padding: UiRect::axes(Val::Px(18.0), Val::Px(10.0)),
            border: UiRect::all(Val::Px(1.0)),
            justify_content: JustifyContent::Center,
            ..default()
        },
        BackgroundColor(Color::srgb(0.16, 0.18, 0.26)),
        BorderColor::all(theme::PANEL_BORDER),
        BorderRadius::all(Val::Px(theme::RADIUS)),
    )
}
```

Note: verify `BorderColor::all` and `BorderRadius::all` exist in Bevy 0.17; if the API is `BorderColor(color)` (single-field), use that form instead. Check `crates/bevy_flair_css_parser/src/reflect/ui.rs` for the exact constructor this repo relies on and match it.

- [ ] **Step 4: Write `project.rs`**

```rust
use bevy::prelude::*;
use crate::sim::UiSnapshot;

/// Fills `screen_pos` for every world-positioned snapshot item using the 2D camera.
/// Skipped cleanly when there is no camera/window (headless).
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

Note: confirm the Bevy 0.17 name — `Camera::world_to_viewport(&self, &GlobalTransform, Vec3) -> Result<Vec2, _>`. If it returns `Option`, adjust `.unwrap_or`. Verify against Bevy 0.17 docs before implementing.

- [ ] **Step 5: Write `native/mod.rs`**

```rust
use bevy::prelude::*;

pub mod theme;
pub mod widgets;
pub mod project;
pub mod hud;
pub mod screens;

pub struct NativeUiPlugin;

impl Plugin for NativeUiPlugin {
    fn build(&self, app: &mut App) {
        app
            // Projection runs after sim assembly, before UI reads the snapshot.
            .add_systems(Update, project::project_snapshot.after(crate::sim::snapshot::assemble_world_snapshot))
            .add_plugins((hud::HudPlugin, screens::ScreensPlugin));
    }
}
```

Create empty `hud/mod.rs` and `screens/mod.rs` stubs exposing `HudPlugin`/`ScreensPlugin` that do nothing yet (filled in later tasks):

```rust
// hud/mod.rs
use bevy::prelude::*;
pub struct HudPlugin;
impl Plugin for HudPlugin { fn build(&self, _app: &mut App) {} }
```
```rust
// screens/mod.rs
use bevy::prelude::*;
pub struct ScreensPlugin;
impl Plugin for ScreensPlugin { fn build(&self, _app: &mut App) {} }
```

- [ ] **Step 6: Build and run**

Run: `cargo run -p horde`
Expected: same playable game as Task 17, now with the native UI plugin loaded (no visible panels yet).

Run: `cargo run -p horde --no-default-features`
Expected: **panics** at startup with the Supersolid seam message.

- [ ] **Step 7: Commit**

```bash
git add examples/horde/src/ui
git commit -m "feat(horde): NativeUiPlugin scaffold, theme, widgets, snapshot projection"
```

---

### Task 19: HUD root + `player_status` panel

**Files:**
- Modify: `examples/horde/src/ui/native/hud/mod.rs`
- Create: `examples/horde/src/ui/native/hud/player_status.rs`

**Interfaces:**
- Produces: `HudRoot` marker + `spawn_hud_root` (on `OnEnter(Playing)`), `despawn_hud` (on `OnExit(Playing)`), and the `player_status` build+update systems.
- Consumes: `UiSnapshot`, `theme`, `widgets`.

- [ ] **Step 1: Write `hud/mod.rs` with root lifecycle**

```rust
use bevy::prelude::*;
use crate::game_state::GameState;

pub mod player_status;

#[derive(Component)]
pub struct HudRoot;

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Playing), spawn_hud_root)
            .add_systems(OnExit(GameState::Playing), despawn_hud)
            .add_plugins(player_status::PlayerStatusPlugin);
    }
}

fn spawn_hud_root(mut commands: Commands) {
    commands.spawn((
        HudRoot,
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            padding: UiRect::all(Val::Px(12.0)),
            ..default()
        },
        // The HUD overlay must not block gameplay mouse input.
        Pickable::IGNORE,
    ));
}

fn despawn_hud(mut commands: Commands, roots: Query<Entity, With<HudRoot>>) {
    for e in roots.iter() {
        commands.entity(e).despawn();
    }
}
```

Note: `despawn` recursively despawns children in Bevy 0.17 (entities own their children). Confirm; if a separate recursive call is required, use it.

- [ ] **Step 2: Write `player_status.rs`**

```rust
use bevy::prelude::*;
use crate::game_state::GameState;
use crate::sim::UiSnapshot;
use super::super::{theme, widgets};
use super::HudRoot;

#[derive(Component)] struct PlayerStatusPanel;
#[derive(Component)] struct HpFill;
#[derive(Component)] struct XpFill;
#[derive(Component)] struct WeaponBadge;
#[derive(Component)] struct AmmoText;

pub struct PlayerStatusPlugin;
impl Plugin for PlayerStatusPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Playing), build)
            .add_systems(Update, update.run_if(in_state(GameState::Playing)));
    }
}

fn build(mut commands: Commands, roots: Query<Entity, With<HudRoot>>) {
    let Ok(root) = roots.single() else { return };
    commands.entity(root).with_children(|p| {
        p.spawn((
            PlayerStatusPanel,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(12.0),
                top: Val::Px(12.0),
                width: Val::Px(240.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(theme::SPACE),
                padding: UiRect::all(Val::Px(theme::SPACE)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(theme::PANEL),
            BorderColor::all(theme::PANEL_BORDER),
            BorderRadius::all(Val::Px(theme::RADIUS)),
        ))
        .with_children(|c| {
            c.spawn(widgets::label("HP", theme::FONT_SM, theme::TEXT_DIM));
            c.spawn(widgets::bar_track(14.0)).with_children(|b| {
                b.spawn((HpFill, widgets::bar_fill(1.0, theme::GOOD)));
            });
            c.spawn(widgets::label("XP", theme::FONT_SM, theme::TEXT_DIM));
            c.spawn(widgets::bar_track(8.0)).with_children(|b| {
                b.spawn((XpFill, widgets::bar_fill(0.0, theme::ACCENT)));
            });
            c.spawn((WeaponBadge, widgets::label("Pistol", theme::FONT, theme::TEXT)));
            c.spawn((AmmoText, widgets::label("12 / 12", theme::FONT_SM, theme::TEXT_DIM)));
        });
    });
}

fn update(
    snap: Res<UiSnapshot>,
    mut hp: Query<&mut Node, (With<HpFill>, Without<XpFill>)>,
    mut xp: Query<&mut Node, (With<XpFill>, Without<HpFill>)>,
    mut hp_col: Query<&mut BackgroundColor, With<HpFill>>,
    mut badge: Query<&mut Text, (With<WeaponBadge>, Without<AmmoText>)>,
    mut ammo: Query<&mut Text, (With<AmmoText>, Without<WeaponBadge>)>,
) {
    let frac = (snap.player_hp / snap.player_max_hp).clamp(0.0, 1.0);
    if let Ok(mut n) = hp.single_mut() { n.width = Val::Percent(frac * 100.0); }
    if let Ok(mut c) = hp_col.single_mut() { c.0 = theme::hp_color(frac); }
    let xp_frac = (snap.xp % 100) as f32 / 100.0;
    if let Ok(mut n) = xp.single_mut() { n.width = Val::Percent(xp_frac * 100.0); }
    if let Ok(mut t) = badge.single_mut() {
        *t = Text::new(snap.active_weapon.map(|w| w.name()).unwrap_or("—"));
    }
    if let Ok(mut t) = ammo.single_mut() {
        *t = Text::new(if snap.reloading { "reloading…".to_string() }
                       else { format!("{} / {}", snap.ammo, snap.ammo_size) });
    }
}
```

- [ ] **Step 3: Build and visually verify**

Run: `cargo run -p horde` → press Enter → a top-left panel shows an HP bar that drains when enemies hit you, an XP bar that fills on kills, the active weapon name, and ammo counting down / reloading.
Expected: panel updates live; HP bar color shifts green→red as HP drops.

- [ ] **Step 4: Commit**

```bash
git add examples/horde/src/ui/native/hud
git commit -m "feat(horde): native HUD root + player_status panel (HP/XP/weapon/ammo)"
```

---

### Task 20: `enemy_nameplates` panel

**Files:**
- Create: `examples/horde/src/ui/native/hud/enemy_nameplates.rs`
- Modify: `hud/mod.rs` (register)

**Interfaces:**
- Produces: `EnemyNameplatesPlugin`; a pool of nameplate nodes positioned at `screen_pos`, keyed by enemy id.
- Consumes: `UiSnapshot.enemies`.

- [ ] **Step 1: Write `enemy_nameplates.rs`**

```rust
use bevy::prelude::*;
use crate::game_state::GameState;
use crate::sim::UiSnapshot;
use super::super::{theme, widgets};
use super::HudRoot;

#[derive(Component)] struct NameplateLayer;
#[derive(Component)] struct Nameplate { id: u64 }
#[derive(Component)] struct NameplateFill;

pub struct EnemyNameplatesPlugin;
impl Plugin for EnemyNameplatesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Playing), build_layer)
            .add_systems(Update, sync.run_if(in_state(GameState::Playing)));
    }
}

fn build_layer(mut commands: Commands, roots: Query<Entity, With<HudRoot>>) {
    let Ok(root) = roots.single() else { return };
    commands.entity(root).with_children(|p| {
        p.spawn((NameplateLayer, Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0), height: Val::Percent(100.0), ..default()
        }, Pickable::IGNORE));
    });
}

/// Keyed reconcile: reuse nodes by enemy id, spawn missing, despawn stale.
fn sync(
    mut commands: Commands,
    snap: Res<UiSnapshot>,
    layer: Query<Entity, With<NameplateLayer>>,
    mut existing: Query<(Entity, &Nameplate, &mut Node)>,
    mut fills: Query<(&ChildOf, &mut Node), (With<NameplateFill>, Without<Nameplate>)>,
) {
    let Ok(layer) = layer.single() else { return };
    use std::collections::HashMap;
    let mut want: HashMap<u64, &crate::sim::snapshot::Nameplate> =
        snap.enemies.iter().map(|n| (n.id, n)).collect();

    // Update / remove existing.
    for (e, np, mut node) in existing.iter_mut() {
        if let Some(n) = want.remove(&np.id) {
            node.left = Val::Px(n.screen_pos.x - 22.0);
            node.top = Val::Px(n.screen_pos.y - 30.0);
            let frac = (n.hp / n.max_hp).clamp(0.0, 1.0);
            for (parent, mut fnode) in fills.iter_mut() {
                if parent.parent() == e {
                    fnode.width = Val::Percent(frac * 100.0);
                }
            }
        } else {
            commands.entity(e).despawn();
        }
    }
    // Spawn newcomers.
    for (id, n) in want {
        commands.entity(layer).with_children(|p| {
            p.spawn((
                Nameplate { id },
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(n.screen_pos.x - 22.0),
                    top: Val::Px(n.screen_pos.y - 30.0),
                    width: Val::Px(44.0),
                    height: Val::Px(5.0),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.15, 0.15, 0.2)),
                Pickable::IGNORE,
            ))
            .with_children(|b| {
                b.spawn((NameplateFill, widgets::bar_fill((n.hp / n.max_hp).clamp(0.0, 1.0), theme::hp_color((n.hp / n.max_hp).clamp(0.0, 1.0)))));
            });
        });
    }
}
```

Note: `ChildOf` is Bevy 0.17's parent component and `.parent()` returns the parent `Entity`. Verify the exact API name (`ChildOf` vs `Parent`) in Bevy 0.17 and adjust. If iterating fills-by-parent proves awkward, store the fill entity id on the `Nameplate` component instead.

- [ ] **Step 2: Register in `hud/mod.rs`** — add `pub mod enemy_nameplates;` and `.add_plugins(enemy_nameplates::EnemyNameplatesPlugin)`.

- [ ] **Step 3: Build and visually verify**

Run: `cargo run -p horde` → nameplate HP bars float above each on-screen enemy, follow them, shrink+redden as they take damage, and disappear when the enemy dies. This is the spawn/despawn churn panel.

- [ ] **Step 4: Commit**

```bash
git add examples/horde/src/ui/native/hud
git commit -m "feat(horde): enemy_nameplates panel with keyed spawn/despawn reconcile"
```

---

### Task 21: `damage_numbers` panel

**Files:**
- Create: `examples/horde/src/ui/native/hud/damage_numbers.rs`
- Modify: `hud/mod.rs`

**Interfaces:**
- Produces: `DamageNumbersPlugin`; keyed floaters positioned at `screen_pos`, alpha from `age/ttl`.
- Consumes: `UiSnapshot.damage_numbers`.

- [ ] **Step 1: Write `damage_numbers.rs`**

```rust
use bevy::prelude::*;
use crate::game_state::GameState;
use crate::sim::UiSnapshot;
use super::super::theme;
use super::HudRoot;

#[derive(Component)] struct DamageLayer;
#[derive(Component)] struct Floater { id: u64 }

pub struct DamageNumbersPlugin;
impl Plugin for DamageNumbersPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Playing), build_layer)
            .add_systems(Update, sync.run_if(in_state(GameState::Playing)));
    }
}

fn build_layer(mut commands: Commands, roots: Query<Entity, With<HudRoot>>) {
    let Ok(root) = roots.single() else { return };
    commands.entity(root).with_children(|p| {
        p.spawn((DamageLayer, Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0), height: Val::Percent(100.0), ..default()
        }, Pickable::IGNORE));
    });
}

fn sync(
    mut commands: Commands,
    snap: Res<UiSnapshot>,
    layer: Query<Entity, With<DamageLayer>>,
    mut existing: Query<(Entity, &Floater, &mut Node, &mut TextColor)>,
) {
    let Ok(layer) = layer.single() else { return };
    use std::collections::HashMap;
    let mut want: HashMap<u64, &crate::sim::snapshot::FloatingNumber> =
        snap.damage_numbers.iter().map(|d| (d.id, d)).collect();

    for (e, f, mut node, mut color) in existing.iter_mut() {
        if let Some(d) = want.remove(&f.id) {
            node.left = Val::Px(d.screen_pos.x);
            node.top = Val::Px(d.screen_pos.y);
            let alpha = (1.0 - d.age / d.ttl).clamp(0.0, 1.0);
            color.0 = color.0.with_alpha(alpha);
        } else {
            commands.entity(e).despawn();
        }
    }
    for (id, d) in want {
        let col = if d.crit { theme::WARN } else { theme::TEXT };
        commands.entity(layer).with_children(|p| {
            p.spawn((
                Floater { id },
                Node { position_type: PositionType::Absolute, left: Val::Px(d.screen_pos.x), top: Val::Px(d.screen_pos.y), ..default() },
                Text::new(format!("{}", d.amount.round() as i32)),
                TextFont::from_font_size(if d.crit { theme::FONT } else { theme::FONT_SM }),
                TextColor(col),
                Pickable::IGNORE,
            ));
        });
    }
}
```

- [ ] **Step 2: Register + build + verify**

Register in `hud/mod.rs`. Run: `cargo run -p horde` → numbers pop at hit locations, drift up, fade, and disappear.

- [ ] **Step 3: Commit**

```bash
git add examples/horde/src/ui/native/hud
git commit -m "feat(horde): damage_numbers floaters (keyed create/dispose + fade)"
```

---

### Task 22: `minimap` panel

**Files:**
- Create: `examples/horde/src/ui/native/hud/minimap.rs`
- Modify: `hud/mod.rs`

**Interfaces:**
- Produces: `MinimapPlugin`; a bottom-right fixed box with blip dots keyed by id, positioned from `world_pos` scaled into the box.
- Consumes: `UiSnapshot.blips`, `SimConfig.arena_half`.

- [ ] **Step 1: Write `minimap.rs`**

```rust
use bevy::prelude::*;
use crate::game_state::GameState;
use crate::sim::{UiSnapshot, SimConfig};
use crate::sim::snapshot::BlipKind;
use super::super::theme;
use super::HudRoot;

const MAP: f32 = 160.0;

#[derive(Component)] struct MinimapBox;
#[derive(Component)] struct BlipDot { id: u64 }

pub struct MinimapPlugin;
impl Plugin for MinimapPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Playing), build)
            .add_systems(Update, sync.run_if(in_state(GameState::Playing)));
    }
}

fn build(mut commands: Commands, roots: Query<Entity, With<HudRoot>>) {
    let Ok(root) = roots.single() else { return };
    commands.entity(root).with_children(|p| {
        p.spawn((
            MinimapBox,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(12.0),
                bottom: Val::Px(12.0),
                width: Val::Px(MAP),
                height: Val::Px(MAP),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.05, 0.08, 0.85)),
            BorderColor::all(theme::PANEL_BORDER),
            BorderRadius::all(Val::Px(theme::RADIUS)),
            Pickable::IGNORE,
        ));
    });
}

fn blip_color(kind: BlipKind) -> Color {
    match kind {
        BlipKind::Player => theme::ACCENT,
        BlipKind::Enemy => theme::DANGER,
        BlipKind::Pickup => theme::GOOD,
    }
}

fn sync(
    mut commands: Commands,
    snap: Res<UiSnapshot>,
    cfg: Res<SimConfig>,
    map: Query<Entity, With<MinimapBox>>,
    mut existing: Query<(Entity, &BlipDot, &mut Node)>,
) {
    let Ok(map) = map.single() else { return };
    let to_local = |w: Vec2| -> Vec2 {
        let n = (w / cfg.arena_half).clamp(Vec2::splat(-1.0), Vec2::splat(1.0));
        // world +y is up; UI +y is down → flip y.
        Vec2::new((n.x * 0.5 + 0.5) * MAP, ((-n.y) * 0.5 + 0.5) * MAP)
    };
    use std::collections::HashMap;
    let mut want: HashMap<u64, &crate::sim::snapshot::Blip> =
        snap.blips.iter().map(|b| (b.id, b)).collect();
    for (e, dot, mut node) in existing.iter_mut() {
        if let Some(b) = want.remove(&dot.id) {
            let l = to_local(b.world_pos);
            node.left = Val::Px(l.x - 2.0);
            node.top = Val::Px(l.y - 2.0);
        } else {
            commands.entity(e).despawn();
        }
    }
    for (id, b) in want {
        let l = to_local(b.world_pos);
        let size = if matches!(b.kind, BlipKind::Player) { 6.0 } else { 4.0 };
        commands.entity(map).with_children(|p| {
            p.spawn((
                BlipDot { id },
                Node { position_type: PositionType::Absolute, left: Val::Px(l.x - 2.0), top: Val::Px(l.y - 2.0), width: Val::Px(size), height: Val::Px(size), ..default() },
                BackgroundColor(blip_color(b.kind)),
                BorderRadius::all(Val::Px(size / 2.0)),
                Pickable::IGNORE,
            ));
        });
    }
}
```

Note: blip ids collide if an enemy's `to_bits()` equals the player blip id `0`. The player blip uses id `0` (see snapshot). Bevy entity bits are never 0 in practice, but to be safe, offset the player blip id to `u64::MAX` in `snapshot.rs` and update accordingly.

- [ ] **Step 2: Register + build + verify**

Run: `cargo run -p horde` → bottom-right minimap shows player (accent), enemies (red) and pickups (green) moving in real time. With `HORDE_PRESET=stress` the map fills with dots (blip fan-out).

- [ ] **Step 3: Commit**

```bash
git add examples/horde/src/ui/native/hud examples/horde/src/sim/snapshot.rs
git commit -m "feat(horde): minimap panel with keyed blip fan-out"
```

---

### Task 23: `weapon_bar` panel (with click-to-switch)

**Files:**
- Create: `examples/horde/src/ui/native/hud/weapon_bar.rs`
- Modify: `hud/mod.rs`

**Interfaces:**
- Produces: `WeaponBarPlugin`; a bottom-center row of weapon slots, active highlighted; clicking a slot raises `Intent::SwitchWeapon(i)`.
- Consumes: `UiSnapshot.inventory`, `IntentQueue`.

- [ ] **Step 1: Write `weapon_bar.rs`**

```rust
use bevy::prelude::*;
use crate::game_state::GameState;
use crate::sim::{UiSnapshot, IntentQueue, Intent};
use super::super::theme;
use super::HudRoot;

#[derive(Component)] struct WeaponBar;
#[derive(Component)] struct Slot { index: usize }

pub struct WeaponBarPlugin;
impl Plugin for WeaponBarPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Playing), build)
            .add_systems(Update, (rebuild_on_change, handle_clicks).run_if(in_state(GameState::Playing)));
    }
}

fn build(mut commands: Commands, roots: Query<Entity, With<HudRoot>>) {
    let Ok(root) = roots.single() else { return };
    commands.entity(root).with_children(|p| {
        p.spawn((
            WeaponBar,
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(12.0),
                left: Val::Percent(50.0),
                margin: UiRect::left(Val::Px(-160.0)),
                width: Val::Px(320.0),
                column_gap: Val::Px(theme::SPACE),
                justify_content: JustifyContent::Center,
                ..default()
            },
        ));
    });
}

/// Rebuild slots when inventory size/active changes (cheap; inventory is tiny).
fn rebuild_on_change(
    mut commands: Commands,
    snap: Res<UiSnapshot>,
    bar: Query<Entity, With<WeaponBar>>,
    slots: Query<Entity, With<Slot>>,
) {
    let Ok(bar) = bar.single() else { return };
    // Clear and rebuild each frame the inventory changed; guard with a simple length/active check.
    // For simplicity and given ≤4 slots, rebuild every frame.
    for e in slots.iter() { commands.entity(e).despawn(); }
    commands.entity(bar).with_children(|p| {
        for s in snap.inventory.iter() {
            let bg = if s.active { Color::srgb(0.22, 0.30, 0.42) } else { Color::srgb(0.15, 0.16, 0.22) };
            let border = if s.active { theme::ACCENT } else { theme::PANEL_BORDER };
            p.spawn((
                Slot { index: s.index },
                Button,
                Node {
                    width: Val::Px(70.0), height: Val::Px(48.0),
                    justify_content: JustifyContent::Center, align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(2.0)), ..default()
                },
                BackgroundColor(bg),
                BorderColor::all(border),
                BorderRadius::all(Val::Px(theme::RADIUS)),
            ))
            .with_children(|c| {
                c.spawn((Text::new(format!("{}. {}", s.index + 1, s.kind.name())), TextFont::from_font_size(theme::FONT_SM), TextColor(theme::TEXT)));
            });
        }
    });
}

fn handle_clicks(
    slots: Query<(&Slot, &Interaction), Changed<Interaction>>,
    mut intents: ResMut<IntentQueue>,
) {
    for (slot, interaction) in slots.iter() {
        if *interaction == Interaction::Pressed {
            intents.push(Intent::SwitchWeapon(slot.index));
        }
    }
}
```

Note: rebuilding slots every frame is acceptable here (≤4 nodes) and keeps the code simple; the perf-sensitive panels are the high-count ones (nameplates/damage/minimap), which reconcile by key.

- [ ] **Step 2: Register + build + verify**

Run: `cargo run -p horde` → grab pickups to gain weapons; the bottom-center bar shows slots; the active one is highlighted; clicking a slot or pressing 1-4 / scrolling switches weapons and the highlight follows.

- [ ] **Step 3: Commit**

```bash
git add examples/horde/src/ui/native/hud
git commit -m "feat(horde): weapon_bar panel with active highlight + click-to-switch"
```

---

### Task 24: `meters` panel (DPS / kills / wave)

**Files:**
- Create: `examples/horde/src/ui/native/hud/meters.rs`
- Modify: `hud/mod.rs`

**Interfaces:**
- Produces: `MetersPlugin`; a top-center readout of DPS, kills, wave, elapsed.
- Consumes: `UiSnapshot`.

- [ ] **Step 1: Write `meters.rs`**

```rust
use bevy::prelude::*;
use crate::game_state::GameState;
use crate::sim::UiSnapshot;
use super::super::{theme, widgets};
use super::HudRoot;

#[derive(Component)] struct MetersText;

pub struct MetersPlugin;
impl Plugin for MetersPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Playing), build)
            .add_systems(Update, update.run_if(in_state(GameState::Playing)));
    }
}

fn build(mut commands: Commands, roots: Query<Entity, With<HudRoot>>) {
    let Ok(root) = roots.single() else { return };
    commands.entity(root).with_children(|p| {
        p.spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(12.0),
                left: Val::Percent(50.0),
                margin: UiRect::left(Val::Px(-140.0)),
                width: Val::Px(280.0),
                justify_content: JustifyContent::Center,
                padding: UiRect::axes(Val::Px(theme::SPACE), Val::Px(6.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(theme::PANEL),
            BorderColor::all(theme::PANEL_BORDER),
            BorderRadius::all(Val::Px(theme::RADIUS)),
            Pickable::IGNORE,
        ))
        .with_children(|c| {
            c.spawn((MetersText, widgets::label("", theme::FONT, theme::TEXT)));
        });
    });
}

fn update(snap: Res<UiSnapshot>, mut q: Query<&mut Text, With<MetersText>>) {
    if let Ok(mut t) = q.single_mut() {
        *t = Text::new(format!(
            "Wave {}   Kills {}   DPS {:.0}   {:02}:{:02}",
            snap.wave, snap.kills, snap.dps,
            (snap.elapsed as u32) / 60, (snap.elapsed as u32) % 60,
        ));
    }
}
```

- [ ] **Step 2: Register + build + verify**

Run: `cargo run -p horde` → top-center bar shows wave/kills/DPS/timer updating live (DPS rises while firing into a crowd).

- [ ] **Step 3: Commit**

```bash
git add examples/horde/src/ui/native/hud
git commit -m "feat(horde): meters panel (wave/kills/DPS/time)"
```

---

### Task 25: `combat_log` panel

**Files:**
- Create: `examples/horde/src/ui/native/hud/combat_log.rs`
- Modify: `hud/mod.rs`; also emit a pickup log line in `sim/pickup.rs`.

**Interfaces:**
- Produces: `CombatLogPlugin`; a left-bottom stack of recent log lines, older lines dimmer.
- Consumes: `UiSnapshot.log`.

- [ ] **Step 1: Emit a log line on pickup**

In `sim/pickup.rs` `grab_pickups`, add `mut log: ResMut<crate::sim::damage::CombatLog>` param and, when a weapon is grabbed:

```rust
crate::sim::damage::push_log(&mut log, format!("Picked up {}", pk.kind.name()));
```

Update the pickup grab test to `app.init_resource::<crate::sim::damage::CombatLog>();`.

- [ ] **Step 2: Write `combat_log.rs`**

```rust
use bevy::prelude::*;
use crate::game_state::GameState;
use crate::sim::UiSnapshot;
use super::super::theme;
use super::HudRoot;

#[derive(Component)] struct LogPanel;

pub struct CombatLogPlugin;
impl Plugin for CombatLogPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Playing), build)
            .add_systems(Update, update.run_if(in_state(GameState::Playing)));
    }
}

fn build(mut commands: Commands, roots: Query<Entity, With<HudRoot>>) {
    let Ok(root) = roots.single() else { return };
    commands.entity(root).with_children(|p| {
        p.spawn((
            LogPanel,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(12.0),
                bottom: Val::Px(12.0),
                width: Val::Px(240.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                ..default()
            },
            Pickable::IGNORE,
        ));
    });
}

fn update(mut commands: Commands, snap: Res<UiSnapshot>, panel: Query<Entity, With<LogPanel>>, children: Query<&Children, With<LogPanel>>) {
    let Ok(panel) = panel.single() else { return };
    // Rebuild the small log list each frame (≤8 lines).
    if let Ok(kids) = children.single() {
        for &c in kids.iter() { commands.entity(c).despawn(); }
    }
    commands.entity(panel).with_children(|p| {
        for line in snap.log.iter() {
            let alpha = (1.0 - line.age / 6.0).clamp(0.25, 1.0);
            p.spawn((
                Text::new(line.text.clone()),
                TextFont::from_font_size(theme::FONT_SM),
                TextColor(theme::TEXT.with_alpha(alpha)),
                Pickable::IGNORE,
            ));
        }
    });
}
```

- [ ] **Step 3: Register + build + verify**

Run: `cargo run -p horde` → bottom-left log shows "Wave N" and "Picked up X" lines, newest on top, fading as they age.

- [ ] **Step 4: Commit**

```bash
git add examples/horde/src
git commit -m "feat(horde): combat_log panel + pickup log events"
```

---

### Task 26: Register all HUD panels together (integration check)

**Files:**
- Modify: `examples/horde/src/ui/native/hud/mod.rs`

- [ ] **Step 1: Ensure `HudPlugin` adds every panel plugin**

```rust
.add_plugins((
    player_status::PlayerStatusPlugin,
    enemy_nameplates::EnemyNameplatesPlugin,
    damage_numbers::DamageNumbersPlugin,
    minimap::MinimapPlugin,
    weapon_bar::WeaponBarPlugin,
    meters::MetersPlugin,
    combat_log::CombatLogPlugin,
));
```
with matching `pub mod …;` lines.

- [ ] **Step 2: Build, run, and play a full round**

Run: `cargo run -p horde` → all seven HUD panels visible and live simultaneously. Play until death; confirm no panic and stable framerate.

Run: `HORDE_PRESET=stress cargo run -p horde` (PowerShell: `$env:HORDE_PRESET="stress"; cargo run -p horde`) → hundreds of enemies, dense nameplates/blips; confirm the knob changes on-screen element counts.

- [ ] **Step 3: Commit**

```bash
git add examples/horde/src/ui/native/hud/mod.rs
git commit -m "feat(horde): wire all seven HUD panels into HudPlugin"
```

---

### Task 27: Screens root + `main_menu`

**Files:**
- Modify: `examples/horde/src/ui/native/screens/mod.rs`
- Create: `examples/horde/src/ui/native/screens/main_menu.rs`

**Interfaces:**
- Produces: `ScreensPlugin` composing all screen plugins; `main_menu` with Start/Quit buttons raising intents; state-scoped spawn/despawn on `OnEnter`/`OnExit(MainMenu)`.
- Consumes: `IntentQueue`, `GameState`.

- [ ] **Step 1: Write `screens/mod.rs`**

```rust
use bevy::prelude::*;

pub mod main_menu;
pub mod pause;
pub mod game_over;
pub mod inventory;
pub mod settings;

pub struct ScreensPlugin;
impl Plugin for ScreensPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            main_menu::MainMenuPlugin,
            pause::PausePlugin,
            game_over::GameOverPlugin,
            inventory::InventoryPlugin,
            settings::SettingsPlugin,
        ));
    }
}

/// Shared: a fullscreen centered overlay container bundle.
pub fn overlay(dim: bool) -> impl Bundle {
    (
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            row_gap: Val::Px(16.0),
            ..default()
        },
        BackgroundColor(if dim { Color::srgba(0.0, 0.0, 0.0, 0.6) } else { super::theme::BG }),
    )
}
```

- [ ] **Step 2: Write `main_menu.rs`**

```rust
use bevy::prelude::*;
use crate::game_state::GameState;
use crate::sim::{IntentQueue, Intent};
use super::super::{theme, widgets};
use super::overlay;

#[derive(Component)] struct MainMenuUi;
#[derive(Component)] enum MenuAction { Start, Quit }

pub struct MainMenuPlugin;
impl Plugin for MainMenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::MainMenu), build)
            .add_systems(OnExit(GameState::MainMenu), despawn)
            .add_systems(Update, buttons.run_if(in_state(GameState::MainMenu)));
    }
}

fn build(mut commands: Commands) {
    commands.spawn((MainMenuUi, overlay(false))).with_children(|p| {
        p.spawn((Text::new("HORDE"), TextFont::from_font_size(64.0), TextColor(theme::ACCENT)));
        p.spawn((Text::new("survive the swarm"), TextFont::from_font_size(theme::FONT), TextColor(theme::TEXT_DIM)));
        p.spawn((MenuAction::Start, widgets::menu_button())).with_children(|b| {
            b.spawn(widgets::label("Start  (Enter)", theme::FONT, theme::TEXT));
        });
        p.spawn((MenuAction::Quit, widgets::menu_button())).with_children(|b| {
            b.spawn(widgets::label("Quit", theme::FONT, theme::TEXT));
        });
    });
}

fn despawn(mut commands: Commands, q: Query<Entity, With<MainMenuUi>>) {
    for e in q.iter() { commands.entity(e).despawn(); }
}

fn buttons(
    q: Query<(&MenuAction, &Interaction), Changed<Interaction>>,
    mut intents: ResMut<IntentQueue>,
    mut exit: EventWriter<AppExit>,
) {
    for (action, interaction) in q.iter() {
        if *interaction == Interaction::Pressed {
            match action {
                MenuAction::Start => intents.push(Intent::StartGame),
                MenuAction::Quit => { exit.write(AppExit::Success); }
            }
        }
    }
}
```

- [ ] **Step 3: Stub the other four screen plugins** so `ScreensPlugin` compiles now (each filled in Tasks 28-31):

For `pause.rs`, `game_over.rs`, `inventory.rs`, `settings.rs`, create a minimal plugin:

```rust
use bevy::prelude::*;
pub struct PausePlugin; // rename per file
impl Plugin for PausePlugin { fn build(&self, _app: &mut App) {} }
```

- [ ] **Step 4: Build, run, verify**

Run: `cargo run -p horde` → the main menu shows on launch with a big title and Start/Quit buttons; clicking Start (or Enter) begins play; Quit exits.

- [ ] **Step 5: Commit**

```bash
git add examples/horde/src/ui/native/screens
git commit -m "feat(horde): screens root + main_menu (Start/Quit)"
```

---

### Task 28: `pause` overlay

**Files:**
- Modify: `examples/horde/src/ui/native/screens/pause.rs`

**Interfaces:**
- Produces: dimmed overlay on `OnEnter(Paused)`, Resume/Restart/Quit buttons raising intents.

- [ ] **Step 1: Write `pause.rs`**

```rust
use bevy::prelude::*;
use crate::game_state::GameState;
use crate::sim::{IntentQueue, Intent};
use super::super::{theme, widgets};
use super::overlay;

#[derive(Component)] struct PauseUi;
#[derive(Component)] enum PauseAction { Resume, Restart, Quit }

pub struct PausePlugin;
impl Plugin for PausePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Paused), build)
            .add_systems(OnExit(GameState::Paused), despawn)
            .add_systems(Update, buttons.run_if(in_state(GameState::Paused)));
    }
}

fn build(mut commands: Commands) {
    commands.spawn((PauseUi, overlay(true))).with_children(|p| {
        p.spawn((Text::new("Paused"), TextFont::from_font_size(theme::FONT_LG), TextColor(theme::TEXT)));
        for (label, action) in [("Resume  (Esc)", PauseAction::Resume), ("Restart", PauseAction::Restart), ("Quit", PauseAction::Quit)] {
            p.spawn((action, widgets::menu_button())).with_children(|b| {
                b.spawn(widgets::label(label, theme::FONT, theme::TEXT));
            });
        }
    });
}

fn despawn(mut commands: Commands, q: Query<Entity, With<PauseUi>>) {
    for e in q.iter() { commands.entity(e).despawn(); }
}

fn buttons(
    q: Query<(&PauseAction, &Interaction), Changed<Interaction>>,
    mut intents: ResMut<IntentQueue>,
    mut exit: EventWriter<AppExit>,
) {
    for (action, interaction) in q.iter() {
        if *interaction == Interaction::Pressed {
            match action {
                PauseAction::Resume => intents.push(Intent::Resume),
                PauseAction::Restart => intents.push(Intent::Restart),
                PauseAction::Quit => { exit.write(AppExit::Success); }
            }
        }
    }
}
```

- [ ] **Step 2: Build + verify**

Run: `cargo run -p horde` → during play press Esc; a dimmed overlay with Resume/Restart/Quit appears over the frozen game (sim systems are gated on `Playing`, so it truly pauses). Resume continues; Restart starts fresh.

- [ ] **Step 3: Commit**

```bash
git add examples/horde/src/ui/native/screens/pause.rs
git commit -m "feat(horde): pause overlay (resume/restart/quit)"
```

---

### Task 29: `game_over` screen

**Files:**
- Modify: `examples/horde/src/ui/native/screens/game_over.rs`

**Interfaces:**
- Produces: stats summary on `OnEnter(GameOver)` (kills, waves, time survived from `UiSnapshot`), Restart/Quit.

- [ ] **Step 1: Write `game_over.rs`**

```rust
use bevy::prelude::*;
use crate::game_state::GameState;
use crate::sim::{IntentQueue, Intent, UiSnapshot};
use super::super::{theme, widgets};
use super::overlay;

#[derive(Component)] struct GameOverUi;
#[derive(Component)] enum GameOverAction { Restart, Quit }

pub struct GameOverPlugin;
impl Plugin for GameOverPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::GameOver), build)
            .add_systems(OnExit(GameState::GameOver), despawn)
            .add_systems(Update, buttons.run_if(in_state(GameState::GameOver)));
    }
}

fn build(mut commands: Commands, snap: Res<UiSnapshot>) {
    let mins = (snap.elapsed as u32) / 60;
    let secs = (snap.elapsed as u32) % 60;
    commands.spawn((GameOverUi, overlay(true))).with_children(|p| {
        p.spawn((Text::new("You Died"), TextFont::from_font_size(theme::FONT_LG), TextColor(theme::DANGER)));
        p.spawn((GameOverUi, widgets::panel(Val::Px(300.0), 16.0))).with_children(|c| {
            c.spawn(widgets::label(format!("Kills: {}", snap.kills), theme::FONT, theme::TEXT));
            c.spawn(widgets::label(format!("Wave reached: {}", snap.wave), theme::FONT, theme::TEXT));
            c.spawn(widgets::label(format!("Pickups: {}", snap.pickups), theme::FONT, theme::TEXT));
            c.spawn(widgets::label(format!("Time survived: {:02}:{:02}", mins, secs), theme::FONT, theme::TEXT));
        });
        for (label, action) in [("Restart  (Enter)", GameOverAction::Restart), ("Quit", GameOverAction::Quit)] {
            p.spawn((action, widgets::menu_button())).with_children(|b| {
                b.spawn(widgets::label(label, theme::FONT, theme::TEXT));
            });
        }
    });
}

fn despawn(mut commands: Commands, q: Query<Entity, With<GameOverUi>>) {
    for e in q.iter() { commands.entity(e).despawn(); }
}

fn buttons(
    q: Query<(&GameOverAction, &Interaction), Changed<Interaction>>,
    mut intents: ResMut<IntentQueue>,
    mut exit: EventWriter<AppExit>,
) {
    for (action, interaction) in q.iter() {
        if *interaction == Interaction::Pressed {
            match action {
                GameOverAction::Restart => intents.push(Intent::Restart),
                GameOverAction::Quit => { exit.write(AppExit::Success); }
            }
        }
    }
}
```

Note: the snapshot must keep its last values on death. Since `assemble_world_snapshot` runs every frame and the player entity is gone after death, guard it so player-derived fields retain prior values when no player exists — in `assemble_world_snapshot`, only overwrite player fields inside the `if let Ok(...)` (already the case), but the top `*snap = s;` resets them. **Fix:** in `assemble_world_snapshot`, when there is no player, preserve `player_hp/kills/wave/...` by copying from the previous snapshot before overwrite, OR stop reassembling on non-`Playing` states. Simplest: gate `assemble_world_snapshot` on `Playing`, and separately keep the last snapshot for screens. Change the system's run condition to `.run_if(in_state(GameState::Playing))` and it will freeze the last values for GameOver/Pause. Update `NativeUiPlugin`/`SimPlugin` accordingly.

- [ ] **Step 2: Apply the snapshot-freeze fix**

In `SimPlugin`, change:
```rust
app.add_systems(Update, snapshot::assemble_world_snapshot.run_if(in_state(crate::game_state::GameState::Playing)));
```

- [ ] **Step 3: Build + verify**

Run: `cargo run -p horde` → die; the game-over screen shows frozen final stats; Restart/Enter starts a fresh run with reset counters.

- [ ] **Step 4: Commit**

```bash
git add examples/horde/src
git commit -m "feat(horde): game_over screen with frozen run stats"
```

---

### Task 30: `inventory` modal (toggle with `I`)

**Files:**
- Modify: `examples/horde/src/ui/native/screens/inventory.rs`

**Interfaces:**
- Produces: a modal listing owned weapons in a grid with stats; toggled by an `InventoryOpen` resource flipped on `Intent::ToggleInventory`.
- Consumes: `UiSnapshot.inventory`, `IntentQueue`, `weapon_stats`.

- [ ] **Step 1: Write `inventory.rs`**

```rust
use bevy::prelude::*;
use crate::game_state::GameState;
use crate::sim::{IntentQueue, Intent, UiSnapshot, weapon_stats};
use super::super::{theme, widgets};

#[derive(Resource, Default)] pub struct InventoryOpen(pub bool);
#[derive(Component)] struct InventoryUi;

pub struct InventoryPlugin;
impl Plugin for InventoryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InventoryOpen>()
            .add_systems(Update, (toggle, sync).chain().run_if(in_state(GameState::Playing)))
            .add_systems(OnExit(GameState::Playing), close);
    }
}

fn toggle(mut open: ResMut<InventoryOpen>, intents: Res<IntentQueue>) {
    for i in intents.0.iter() {
        if matches!(i, Intent::ToggleInventory) { open.0 = !open.0; }
    }
}

fn close(mut open: ResMut<InventoryOpen>) { open.0 = false; }

fn sync(
    mut commands: Commands,
    open: Res<InventoryOpen>,
    snap: Res<UiSnapshot>,
    ui: Query<Entity, With<InventoryUi>>,
) {
    let is_open = ui.iter().next().is_some();
    if open.0 && !is_open {
        commands.spawn((
            InventoryUi,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0), height: Val::Percent(100.0),
                justify_content: JustifyContent::Center, align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
        )).with_children(|p| {
            p.spawn((
                Node {
                    width: Val::Px(560.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(theme::SPACE),
                    padding: UiRect::all(Val::Px(16.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(theme::PANEL),
                BorderColor::all(theme::PANEL_BORDER),
                BorderRadius::all(Val::Px(theme::RADIUS)),
            )).with_children(|c| {
                c.spawn(widgets::label("Inventory  (I to close)", theme::FONT_LG, theme::TEXT));
                // Grid of owned weapons.
                c.spawn(Node {
                    display: Display::Grid,
                    grid_template_columns: vec![RepeatedGridTrack::flex(2, 1.0)],
                    column_gap: Val::Px(theme::SPACE),
                    row_gap: Val::Px(theme::SPACE),
                    ..default()
                }).with_children(|g| {
                    for slot in snap.inventory.iter() {
                        let s = weapon_stats(slot.kind);
                        let border = if slot.active { theme::ACCENT } else { theme::PANEL_BORDER };
                        g.spawn((
                            Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), padding: UiRect::all(Val::Px(theme::SPACE)), border: UiRect::all(Val::Px(2.0)), ..default() },
                            BackgroundColor(Color::srgb(0.16, 0.17, 0.24)),
                            BorderColor::all(border),
                            BorderRadius::all(Val::Px(theme::RADIUS)),
                        )).with_children(|w| {
                            w.spawn(widgets::label(slot.kind.name(), theme::FONT, theme::TEXT));
                            w.spawn(widgets::label(format!("DMG {:.0}   RoF {:.2}s", s.damage, s.fire_interval), theme::FONT_SM, theme::TEXT_DIM));
                            w.spawn(widgets::label(format!("Spread {:.2}   x{}", s.spread, s.projectiles), theme::FONT_SM, theme::TEXT_DIM));
                            w.spawn(widgets::label(format!("Mag {}   Reload {:.1}s", s.mag_size, s.reload_time), theme::FONT_SM, theme::TEXT_DIM));
                        });
                    }
                });
            });
        });
    } else if !open.0 && is_open {
        for e in ui.iter() { commands.entity(e).despawn(); }
    }
}
```

Note: verify `RepeatedGridTrack::flex` and `grid_template_columns` exist in Bevy 0.17 `bevy_ui`. If grid is unavailable, fall back to a flex-wrap row (`FlexDirection::Row`, `flex_wrap: FlexWrap::Wrap`) and leave a `// TODO(css-capability): grid` note.

- [ ] **Step 2: Build + verify**

Run: `cargo run -p horde` → press `I` mid-game; a modal grid of owned weapons with full stats appears; the active weapon's card is accent-bordered; `I` closes it. (Note: the game keeps running behind the modal — acceptable for this stage.)

- [ ] **Step 3: Commit**

```bash
git add examples/horde/src/ui/native/screens/inventory.rs
git commit -m "feat(horde): inventory modal grid with weapon stats (toggle I)"
```

---

### Task 31: `settings` panel (spawn-count slider proxy)

**Files:**
- Modify: `examples/horde/src/ui/native/screens/settings.rs`
- Modify: `examples/horde/src/ui/native/screens/main_menu.rs` (add a Settings button)

**Interfaces:**
- Produces: a settings overlay with +/- controls that mutate `SimConfig.enemy_cap` live (demonstrating form controls + the §7 knob), plus a backend note. Toggled from main menu.
- Consumes: `SimConfig`, `IntentQueue`/`GameState`.

- [ ] **Step 1: Write `settings.rs`**

```rust
use bevy::prelude::*;
use crate::game_state::GameState;
use crate::sim::SimConfig;
use super::super::{theme, widgets};

#[derive(Resource, Default)] pub struct SettingsOpen(pub bool);
#[derive(Component)] struct SettingsUi;
#[derive(Component)] struct EnemyCapText;
#[derive(Component)] enum SettingsAction { Inc, Dec, Close }

pub struct SettingsPlugin;
impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SettingsOpen>()
            .add_systems(Update, (sync, buttons, refresh).run_if(in_state(GameState::MainMenu)));
    }
}

fn sync(mut commands: Commands, open: Res<SettingsOpen>, cfg: Res<SimConfig>, ui: Query<Entity, With<SettingsUi>>) {
    let is_open = ui.iter().next().is_some();
    if open.0 && !is_open {
        commands.spawn((SettingsUi, Node {
            position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0),
            justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default()
        }, BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)))).with_children(|p| {
            p.spawn((widgets::panel(Val::Px(360.0), 16.0),)).with_children(|c| {
                c.spawn(widgets::label("Settings", theme::FONT_LG, theme::TEXT));
                c.spawn(Node { column_gap: Val::Px(theme::SPACE), align_items: AlignItems::Center, ..default() }).with_children(|row| {
                    row.spawn((SettingsAction::Dec, widgets::menu_button())).with_children(|b| { b.spawn(widgets::label("−", theme::FONT, theme::TEXT)); });
                    row.spawn((EnemyCapText, widgets::label(format!("Enemy cap: {}", cfg.enemy_cap), theme::FONT, theme::TEXT)));
                    row.spawn((SettingsAction::Inc, widgets::menu_button())).with_children(|b| { b.spawn(widgets::label("+", theme::FONT, theme::TEXT)); });
                });
                c.spawn(widgets::label("UI backend: native (bevy_ui)", theme::FONT_SM, theme::TEXT_DIM));
                c.spawn((SettingsAction::Close, widgets::menu_button())).with_children(|b| { b.spawn(widgets::label("Close", theme::FONT, theme::TEXT)); });
            });
        });
    } else if !open.0 && is_open {
        for e in ui.iter() { commands.entity(e).despawn(); }
    }
}

fn buttons(
    q: Query<(&SettingsAction, &Interaction), Changed<Interaction>>,
    mut cfg: ResMut<SimConfig>,
    mut open: ResMut<SettingsOpen>,
) {
    for (action, interaction) in q.iter() {
        if *interaction == Interaction::Pressed {
            match action {
                SettingsAction::Inc => cfg.enemy_cap = (cfg.enemy_cap + 20).min(800),
                SettingsAction::Dec => cfg.enemy_cap = cfg.enemy_cap.saturating_sub(20),
                SettingsAction::Close => open.0 = false,
            }
        }
    }
}

fn refresh(cfg: Res<SimConfig>, mut q: Query<&mut Text, With<EnemyCapText>>) {
    if cfg.is_changed() {
        if let Ok(mut t) = q.single_mut() { *t = Text::new(format!("Enemy cap: {}", cfg.enemy_cap)); }
    }
}
```

- [ ] **Step 2: Add a Settings button to the main menu**

In `main_menu.rs`, add a `MenuAction::Settings` variant + button, and in `buttons`, on Settings press set `SettingsOpen(true)` (add `mut settings: ResMut<super::settings::SettingsOpen>` to the `buttons` params).

- [ ] **Step 3: Build + verify**

Run: `cargo run -p horde` → from the main menu open Settings, use −/+ to change the enemy cap, close, Start — confirm the live cap affects on-screen enemy density (the §7 knob demonstrated through a form control).

- [ ] **Step 4: Commit**

```bash
git add examples/horde/src/ui/native/screens
git commit -m "feat(horde): settings panel with live enemy-cap control + backend note"
```

---

### Task 32: Acceptance pass — build matrix, wasm, polish, docs

**Files:**
- Modify: any panel/theme files needing polish
- Create: `examples/horde/README.md`

- [ ] **Step 1: Full native test + run**

Run: `cargo test -p horde`
Expected: all sim unit tests PASS.

Run: `cargo run -p horde`
Expected: main menu → play → all seven HUD panels + all five screens work; looks coherent (consistent palette, spacing, borders, health color ramps). Tweak `theme.rs` constants for polish if any panel looks off.

- [ ] **Step 2: Verify the backend panic seam**

Run: `cargo run -p horde --no-default-features`
Expected: panics with the Supersolid seam message (design §2).

- [ ] **Step 3: Verify the wasm build compiles**

Run: `cargo build -p horde --target wasm32-unknown-unknown`
Expected: compiles. If a native-only API leaks in (e.g. `std::env` on wasm), guard the `from_env` env reads with `#[cfg(not(target_arch = "wasm32"))]` and default to `SimConfig::play()` on wasm.

- [ ] **Step 4: Verify the knob presets**

Run (PowerShell): `$env:HORDE_PRESET="stress"; cargo run -p horde` then `$env:HORDE_PRESET="play"; cargo run -p horde`
Expected: stress visibly increases enemy/blip/nameplate counts vs. play; confirms §7 knobs change on-screen element counts.

- [ ] **Step 5: Write `examples/horde/README.md`**

```markdown
# Horde — native-UI horde-survival example

A complete top-down horde-survival game whose UI is built entirely in native `bevy_ui`.
The game simulation is decoupled from the UI behind a plain-data `UiSnapshot` seam; a future
Supersolid backend will consume the same seam behind the `ui-native` feature flag (see
`docs/superpowers/specs/2026-07-20-horde-native-ui-design.md`).

## Run

- `cargo run -p horde` — play (native UI, default).
- `cargo run -p horde --no-default-features` — panics: Supersolid backend not yet implemented.
- `cargo build -p horde --target wasm32-unknown-unknown` — wasm build.

## Controls

WASD/arrows move · mouse aim · hold LMB shoot · 1-4 / scroll switch weapon · `I` inventory · `Esc` pause · `Enter` start/restart.

## Knobs (env)

`HORDE_PRESET=play|stress`, `HORDE_SEED=<u64>`, `HORDE_ENEMY_CAP=<n>`, `HORDE_ARENA_HALF=<f32>`.
```

- [ ] **Step 6: Commit**

```bash
git add examples/horde
git commit -m "chore(horde): acceptance pass — build matrix, wasm guard, polish, README"
```

---

## Self-Review

**Spec coverage** (design §-by-§):
- §2 feature flag (`ui-native` default, absent → panic) → Tasks 1, 18, 32. ✓
- §3 three-layer separation + separate plugins → Tasks 5, 15, 18. ✓
- §4.1 `UiSnapshot` fields (keyed lists, derived readouts, log) → Task 15. ✓
- §4.2 screen-projection boundary (`project_snapshot`) → Task 18. ✓
- §4.3 `Intent`/`IntentQueue` → Tasks 4, 16; UI-raised intents → Tasks 23, 27-31. ✓
- §5 controls/world/sim/weapons/pickups/progression/damage-number lifecycle → Tasks 6-14, 16-17. ✓
- §6 states → Task 5; transitions → Task 17. ✓
- §7 all seven HUD panels → Tasks 19-26; all five screens → Tasks 27-31. ✓
- §8 config knobs/presets, fixed-tick, headless-capable systems, labeled boundaries → Tasks 2, 15, 18. `mcp_debug`/`debug-ui` features declared → Task 1 (Cargo). ✓
- §10 acceptance/build matrix → Task 32. ✓
- §11 Supersolid deferral seams (panic arm, placeholder asset dir, 1:1 names) → Tasks 1, 18-31. ✓

**Placeholder scan:** No requirement-level TBDs. The only deferred items are the intentional Supersolid seams. Several "Note:" blocks ask the implementer to **verify a specific Bevy 0.17 API name** (e.g. `BorderColor::all`, `Camera::world_to_viewport` Option-vs-Result, `ChildOf`/`.parent()`, `RepeatedGridTrack::flex`, recursive `despawn`) against the installed Bevy 0.17 — these are grounding checks, not placeholders, and each names the concrete fallback to use if the guess is wrong (guardrail §10 "no invented APIs").

**Type consistency:** Component/enum/field names are consistent across tasks — `WeaponKind::name()`, `Health { current, max }`, `Inventory { slots, active }` + `active_kind()`, `Ammo { current, size, reload }`, snapshot sub-types `Nameplate`/`FloatingNumber`/`Blip`/`BlipKind`/`WeaponSlot`/`LogLine`, `UiSnapshot` field names, `Intent` variants, `IntentQueue::{push,drain}`, `SimConfig` fields, `Rng::{new,next_u64,next_f32,range,unit_vec}`, `dps_over_window`, `push_log`, `assemble_world_snapshot`, `project_snapshot`. The `spawn_waves` signature gains a `Progression` param in Task 15 (noted with a matching test fix).

**One integration risk flagged for the executor:** the snapshot-freeze fix in Task 29 (gate `assemble_world_snapshot` on `Playing`) must land, or GameOver/Pause screens read a blanked snapshot. It is called out inline in Task 29 Steps 1-2.
