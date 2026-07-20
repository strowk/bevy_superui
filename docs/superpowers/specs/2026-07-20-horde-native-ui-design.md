# Horde — Native-UI-first game example (design)

Date: 2026-07-20
Status: Design agreed. Scope = the **native-only** deliverable of the Horde build brief.

Source brief: `target/horde-prompt.md`.
Direction context: `docs/superpowers/specs/2026-07-19-superui-component-framework-direction.md`.
Wiring reference: `examples/todomvc/`.

## 0. What this document is

This is the design for the **first deliverable** of the Horde example: a complete, playable,
good-looking top-down horde-survival game whose entire UI is built in **native `bevy_ui`**.

It deliberately covers only build-brief phases §9.1 and §9.2 (playable sim + full native HUD &
screens + styling polish). The Supersolid TSX/CSS backend (§9.3) is **out of scope here** — this
design only leaves its *seams* in place (the `UiSnapshot`/`Intent` boundary, the backend-select
panic arm, the reserved asset dir and 1:1 panel names). Supersolid gets its own spec + plan later,
once the runtime render/insert seam lands.

## 1. Goals & non-goals

**Goals**
- A real, complete game (not a tech demo) on plain `bevy_ui`, runnable with `cargo run -p horde`.
- Game simulation fully decoupled from UI behind a plain-data snapshot seam, so a future
  benchmark can drive the sim headlessly and feed either UI backend the same data.
- Each HUD panel and screen is its own module, with names reserved 1:1 for the future Supersolid
  side, so panels can later be compared like-for-like.
- Deterministic-capable sim (seeded RNG, fixed logical tick) so it is replayable later.
- A coherent, styled native design system — looks genuinely polished.

**Non-goals (this stage)**
- No Supersolid authoring, no `window.bevy` intent-bridge wiring, no CSS-showcase parity.
- No benchmark harness, no profiling probes, no perf tuning (build the seams, not the harness).
- No complex AI/pathfinding, audio, art assets, save/load, or netcode.

## 2. Feature-flag arrangement (decided)

One flag: **`ui-native`**, included in `default`.

- **`ui-native` present** (the default, and today the only working path) → native `bevy_ui` UI.
- **`ui-native` absent** → the Supersolid backend. Not implemented yet, so the backend-select
  code **panics** with a `TODO(supersolid-runtime)` message. This makes
  `cargo run --no-default-features` fail loudly rather than silently render nothing.
- Later, when Supersolid lands, `ui-native` is *removed from default*; absent → Supersolid,
  present → native. No other flags need to change.

```rust
// src/ui/mod.rs
#[cfg(feature = "ui-native")]
pub fn add_ui(app: &mut App) { app.add_plugins(native::NativeUiPlugin); }

#[cfg(not(feature = "ui-native"))]
pub fn add_ui(_app: &mut App) {
    panic!("Supersolid UI backend not yet implemented — build with the default \
            `ui-native` feature. TODO(supersolid-runtime).");
}
```

`Cargo.toml` features (mirroring todomvc):
- `default = ["ui-native"]`
- `ui-native = []`
- `debug-ui = []` — opt-in diagnostics
- `mcp_debug = ["dep:bevy_brp_extras", "bevy/bevy_remote"]`

## 3. Architecture overview

Three separated layers, wired in `main.rs` as **independent plugins** so the UI can later be
swapped for a null sink:

```
SimPlugin ──produces──▶ UiSnapshot (resource, plain data) ──read──▶ NativeUiPlugin
   ▲                                                                     │
   └──────────────── IntentQueue (resource) ◀── input.rs / UI buttons ───┘
```

- **`sim/`** — the game. Runs on `FixedUpdate`, seeded RNG. **Zero** dependency on `ui/`,
  `bevy_ui`, or Boa. Reads drained `Intent`s + time, mutates ECS game state, owns all entity
  lifecycles (player, enemies, projectiles, pickups, and short-lived damage-number entities).
  Headless-runnable (does not require windowing/render).
- **`UiSnapshot`** — one plain-data resource, rebuilt every frame; the *only* thing any UI sees.
- **`ui/native/`** — HUD panels + screens in raw `bevy_ui`. Each panel is its own module and a
  pure function of the snapshot (+ its own local UI state, e.g. "inventory modal open"). Reads the
  snapshot, raises `Intent`s.

**Instrumentation-friendly boundaries.** Keep these as distinct, labeled systems so timing probes
can later attribute cost per span (no probes added now):
`advance sim` → `assemble snapshot` → `project snapshot` → `UI build/reconcile`.

## 4. The state seam (the crux)

### 4.1 `UiSnapshot`
A plain-data `Resource` defined in `sim/snapshot.rs`, rebuilt each frame. No `bevy_ui` types.
Fields (indicative):

- Scalars: `player_hp`, `player_max_hp`, `xp`, `level`, `wave`, `kills`, `pickups`,
  `active_weapon`, `ammo`, `cooldown_frac`.
- `inventory: Vec<WeaponSlot>` — `{ index, kind, active }` plus per-weapon stats for the modal.
- Keyed per-entity lists (each item carries a stable `id`):
  - `enemies: Vec<Nameplate>` — `{ id, world_pos, screen_pos, hp, max_hp, kind }`
  - `damage_numbers: Vec<DamageNumber>` — `{ id, world_pos, screen_pos, amount, crit, age }`
  - `blips: Vec<Blip>` — `{ id, world_pos, screen_pos, kind }`
- Derived readouts: `dps`, `threat` (computed at assembly over a sliding window).
- `log: VecDeque<LogEvent>` — recent events ("Picked up Shotgun", "Wave 3"), pruned to a cap.

Stable `id`s are required so the future Supersolid `<For>` bindings do minimal work, and so the
native and Supersolid backends form a **differential oracle** (same data in, equivalent UI out).

### 4.2 Screen-projection boundary (decided)
To keep `sim/` headless (no camera dependency), the sim fills **world-space** positions only. A
single shared boundary system `project_snapshot` — added in `main.rs` *outside* `SimPlugin`, given
the `Camera` — fills each list's `screen_pos` from `world_pos` when a window exists, and is skipped
in headless runs. Both UI backends consume the identical projected snapshot. This resolves the
brief's "screen_pos in snapshot" vs. "sim has no render knowledge" tension: the *type* lives in
`sim/`, but the camera-using projection is a boundary concern.

### 4.3 `Intent`
An enum in `sim/intent.rs` (plain data), drained from an `IntentQueue` resource:
`Move(Vec2)`, `Aim(Vec2)`, `Shoot(bool)`, `SwitchWeapon(usize)`, `ToggleInventory`,
`Pause`, `Resume`, `Restart`, `StartGame`, `Quit`.
- `input.rs` (uses `bevy_input`, **not** `bevy_ui`) raises gameplay intents.
- Native UI buttons raise menu/switch intents.
- Sim consumes intents identically regardless of source. This is the shape the Supersolid
  `window.bevy` bridge will later target (`bevy.send("Intent…", …)`), following todomvc's
  `add_superui_command` pattern — deferred.

## 5. Game specifics

- **Controls:** WASD/arrows move; **mouse aim**, hold/click to shoot; number keys + scroll wheel
  switch weapons; `I` toggles inventory modal; `Esc` pauses.
- **World rendering:** colored 2D shapes (sprites/mesh quads — circles/rects), tinted by
  type/state. No art assets.
- **Sim model:** deterministic `FixedUpdate` step; a seeded, deterministic RNG **resource**
  (not thread-rng — wasm-safe). Enemies: straight-line-toward-player + neighbor separation.
  Enemies spawn in waves at arena edges, deal melee damage on contact, can be shot and killed;
  constant spawn/despawn churn is intentional.
- **Weapons/pickups/inventory:** 4 data-driven weapon configs (pistol / shotgun / SMG / rocket)
  differing in fire-rate / spread / damage / projectile count. Dropped as ground pickups; walk
  over to grab into inventory; switch active weapon. Exists primarily to make the inventory UI
  real.
- **Progression/feedback:** XP or kill count, wave/level counter, floating damage numbers,
  pickups counter — the numbers that drive the HUD.
- **Damage-number lifecycle in sim:** short-lived entities with an `age`/ttl that drift and fade
  deterministically on the fixed tick; the snapshot exposes `{ id, screen_pos, amount, crit, age }`
  so both backends render identically and the create/dispose churn lands in a keyed list.

## 6. Game states

Bevy `States`: `GameState = MainMenu | Playing | Paused | GameOver`.
- Sim systems run only in `Playing`.
- Screens render per state. `Intent::StartGame/Pause/Resume/Restart` drive transitions.

## 7. Native UI — panels & screens

All in raw `bevy_ui`, each its own module, each a pure function of the snapshot (+ local UI
state). Names reserved 1:1 for the future Supersolid side.

**Live HUD panels** (`ui/native/hud/`):

| Module | Contents | Driven by |
|---|---|---|
| `player_status` | HP bar, XP/level bar, active-weapon badge, ammo, cooldown radial | player fields |
| `enemy_nameplates` | per on-screen enemy: HP bar (width% + color by %), name/type | enemy list |
| `damage_numbers` | short-lived floaters: spawn, drift up, fade, despawn | damage-number list |
| `minimap` | blips for enemies/pickups/player at scaled positions | blip list |
| `weapon_bar` | slots for held weapons, active highlight, switch on click/scroll | inventory |
| `meters` | DPS / kill / wave readouts | derived readouts |
| `combat_log` | scrolling recent events, append + prune | log ring |

**Screens / modals** (`ui/native/screens/`):
- `main_menu` — title, Start, settings, quit; hero styling.
- `pause` — dimmed backdrop, resume/restart/quit.
- `game_over` — stats summary (kills, waves, time survived), restart.
- `inventory` — larger grid of owned weapons with stats (damage/fire-rate/spread); opened with `I`.
- `settings` — toggles/sliders (e.g. spawn-count knob), to show form controls.

**Styling:** a shared native "design system" in `ui/native/theme.rs` — a coherent palette,
spacing scale, font sizes/weights, and state feedback (hover/active/selected, health-based color).
Aim for a genuinely polished look; get close enough to a future CSS version that a screenshot
comparison is meaningful.

**Layout reservation:** the **top-left corner is kept empty** for the Bevy FPS debug overlay
(`bevy::dev_tools::fps_overlay`, enabled under the `debug-ui` feature, which pulls in
`bevy/bevy_dev_tools`). No HUD panel occupies the top-left corner; the player-status panel sits
just below the reserved strip.

## 8. Benchmark-readiness seams (built, not exercised)

- **`sim/config.rs`** `SimConfig` resource: enemy cap / spawn rate, damage-number rate, minimap
  blip count, inventory size, arena size, **RNG seed**. Overridable via CLI arg / env. Provide a
  `stress` preset (hundreds of enemies) and a `play` preset. Knobs must actually change on-screen
  element counts.
- **Fixed-tick capability:** sim advances on a deterministic step given `dt` + intents. Wall-clock
  UI logic is forbidden in `sim/`.
- **Headless path:** `SimPlugin` + snapshot assembly must not require windowing. `main.rs` adds
  the sim plugin and the UI plugin separately so the UI can later be swapped for a null sink. A
  minimal scripted/headless entry can follow; the seams exist now.
- **`mcp_debug` / `debug-ui`** opt-in features reused from todomvc for inspection/screenshots.

## 9. Module layout

```
examples/horde/
  Cargo.toml                 # features: ui-native (default), mcp_debug, debug-ui
  assets/ui/horde/           # placeholder dir only (Supersolid TSX/CSS deferred)
  src/
    main.rs                  # app assembly: SimPlugin + project_snapshot + ui::add_ui; camera; States
    game_state.rs            # GameState (Bevy States)
    input.rs                 # raw input -> Intent (shared)
    sim/
      mod.rs                 # SimPlugin (FixedUpdate schedule; no bevy_ui/ui/Boa dep)
      config.rs              # SimConfig resource + presets + CLI/env override
      intent.rs              # Intent enum + IntentQueue resource
      snapshot.rs            # UiSnapshot plain-data struct + assemble system (world-space)
      player.rs enemy.rs weapon.rs projectile.rs pickup.rs spawn.rs damage.rs
    ui/
      mod.rs                 # cfg backend select: native | panic seam
      native/
        mod.rs               # NativeUiPlugin
        theme.rs             # shared native design-system constants
        hud/     player_status.rs enemy_nameplates.rs damage_numbers.rs
                 minimap.rs weapon_bar.rs meters.rs combat_log.rs
        screens/ main_menu.rs pause.rs game_over.rs inventory.rs settings.rs
```

Boundary rules enforced by module structure:
- `sim/` imports neither `ui` nor `bevy_ui` nor Boa.
- No panel touches an ECS query; panels read `Res<UiSnapshot>` and write to `IntentQueue`.
- `project_snapshot` is the only system permitted both snapshot and `Camera` access.

## 10. Acceptance for this deliverable

- `cargo run -p horde` (default) plays as a real, styled game with all §7 panels/screens.
- `cargo run -p horde --no-default-features` panics with the `TODO(supersolid-runtime)` message.
- `cargo build -p horde --target wasm32-unknown-unknown` compiles (native UI backend).
- `sim/` compiles with no dependency on `ui/` or `bevy_ui`.
- Config knobs actually change on-screen element counts; `stress`/`play` presets work.
- It looks good — a coherent native design system, not placeholder boxes.

## 11. Deferred to the Supersolid plan (seams only here)

Supersolid TSX/CSS authoring for every panel/screen, the `window.bevy` intent bridge wiring, the
CSS-showcase breadth (`theme.css`/`components.css`) and visual parity with native, and the
benchmark harness. All left as marked `TODO(supersolid-runtime)` / `TODO(css-capability)` with the
`UiSnapshot`/`Intent` boundary and 1:1 panel names already in place.
