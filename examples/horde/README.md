# Horde — native-UI horde-survival example

A complete top-down horde-survival game whose UI is built entirely in native `bevy_ui`.
The game simulation is decoupled from the UI behind a plain-data `UiSnapshot` seam; a future
Supersolid backend will consume the same seam behind the `ui-native` feature flag (see
`docs/superpowers/specs/2026-07-20-horde-native-ui-design.md`).

## Run

```sh
# Default: native UI, hot-reload enabled.
cargo run -p horde

# Without default features: compiles fine, panics at startup with the Supersolid seam message.
cargo run -p horde --no-default-features

# Wasm build (no bundler/server setup included; see Known limitations).
cargo build -p horde --target wasm32-unknown-unknown

# FPS overlay in top-left corner (does not overlap the player-status panel at top: 48px).
cargo run -p horde --features debug-ui

# BRP + MCP screenshot/key-injection support.
cargo run -p horde --features mcp_debug
```

## Controls

| Key / Input | Action |
|---|---|
| WASD / Arrow keys | Move |
| Mouse | Aim |
| Hold LMB | Shoot |
| 1–4 / Scroll wheel | Switch weapon |
| I | Toggle inventory screen |
| Esc | Pause / unpause |
| Enter | Start game / restart after game over |

## Knobs (environment variables)

| Variable | Values | Default | Effect |
|---|---|---|---|
| `HORDE_PRESET` | `play`, `stress` | `play` | Loads a preset: `play` = 60 enemy cap, 0.8 s spawn; `stress` = 400 enemy cap, 0.15 s spawn |
| `HORDE_SEED` | `<u64>` (nonzero) | `0x00C0FFEED00D` | RNG seed for enemy and pickup spawns |
| `HORDE_ENEMY_CAP` | `<usize>` | preset value | Override maximum simultaneous enemies |
| `HORDE_ARENA_HALF` | `<f32>` | `600.0` | Half-size of the square arena in world units |

Per-field overrides are applied after the preset, so you can mix them:

```sh
# PowerShell
$env:HORDE_PRESET="stress"; $env:HORDE_ENEMY_CAP="200"; cargo run -p horde

# bash / sh
HORDE_PRESET=stress HORDE_ENEMY_CAP=200 cargo run -p horde
```

## Architecture

```
sim/           Pure game simulation (no bevy_ui / Boa dependency).
  config.rs    SimConfig + from_env() knobs.
  snapshot.rs  UiSnapshot — plain-data seam written by sim, read by UI.
  intent.rs    IntentQueue — UI → sim command bus.
ui/
  mod.rs       Feature-flag dispatch: ui-native → NativeUiPlugin; absent → panic seam.
  native/      Seven HUD panels + five screens, all in bevy_ui.
    hud/       player_status · minimap · weapon_bar · enemy_nameplates · damage_numbers
               · meters · combat_log
    screens/   main_menu · pause · game_over · inventory · settings
world_render.rs  Sprite-based world-space render (arena tiles, enemy/pickup sprites).
```

## Known limitations

- **Wasm: no wasm-bindgen/web runner included.** The wasm binary compiles cleanly
  (`cargo build -p horde --target wasm32-unknown-unknown`), but there is no `index.html`,
  `wasm-bindgen` post-processing, or asset server setup. Running on the web requires the
  standard Bevy wasm workflow (trunk / wasm-bindgen-cli). This is expected: the web backend
  is the future Supersolid path (design §11.3).
- **Supersolid backend not yet implemented.** Building without `--features ui-native`
  (i.e. `--no-default-features`) compiles successfully but panics at startup:
  `Supersolid UI backend not yet implemented — build with the default ui-native feature.`
