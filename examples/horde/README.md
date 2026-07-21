# Horde — Supersolid-UI horde-survival example

A complete top-down horde-survival game whose UI is built in a single-file `app.tsx` running
in the superui/Supersolid TSX engine. The game simulation is decoupled from the UI behind a
plain-data `UiSnapshot` seam; the native `bevy_ui` UI is kept as an opt-in backend behind the
`ui-native` feature flag.

## Run modes

```sh
# Default: supersolid TSX backend (pre-transpiled app.generated.js).
cargo run -p horde

# Native bevy_ui backend (opt-in).
cargo run -p horde --no-default-features --features ui-native

# Live .tsx hot-reload (native only — TsxLoader + asset watcher).
cargo run -p horde --features hmr

# Wasm build (no bundler/server setup included; see Known limitations).
cargo build -p horde --target wasm32-unknown-unknown

# BRP + MCP screenshot/key-injection support.
cargo run -p horde --features mcp_debug
```

## Controls

| Key / Input     | Action                          |
|-----------------|---------------------------------|
| WASD / Arrow    | Move                            |
| Mouse           | Aim                             |
| Hold LMB        | Shoot                           |
| 1–4 / Scroll    | Switch weapon                   |
| I               | Toggle inventory screen         |
| Esc             | Pause / unpause                 |
| Enter           | Start game / restart after death|

## Knobs (environment variables)

| Variable          | Values          | Default         | Effect                                         |
|-------------------|-----------------|-----------------|------------------------------------------------|
| `HORDE_PRESET`    | `play`, `stress`| `play`          | `play` = 60-cap, 0.8 s spawn; `stress` = 400-cap, 0.15 s spawn |
| `HORDE_SEED`      | `<u64>`         | `0x00C0FFEED00D`| RNG seed for enemy and pickup spawns           |
| `HORDE_ENEMY_CAP` | `<usize>`       | preset value    | Override maximum simultaneous enemies          |
| `HORDE_ARENA_HALF`| `<f32>`         | `600.0`         | Half-size of the square arena in world units   |

Per-field overrides are applied after the preset:

```sh
# PowerShell
$env:HORDE_PRESET="stress"; $env:HORDE_ENEMY_CAP="200"; cargo run -p horde

# bash / sh
HORDE_PRESET=stress HORDE_ENEMY_CAP=200 cargo run -p horde
```

## Architecture

```
sim/             Pure game simulation (no bevy_ui / Boa dependency).
  config.rs      SimConfig + from_env() knobs.
  snapshot.rs    UiSnapshot — plain-data seam written by sim, read by UI.
  intent.rs      IntentQueue — UI → sim command bus.
ui/
  mod.rs         Feature-flag dispatch: ui-native → NativeUiPlugin; default → SupersolidUiPlugin.
  project.rs     Shared project_snapshot system (world→viewport, used by both backends).
  native/        Seven HUD panels + five screens in bevy_ui (behind --features ui-native).
    hud/         player_status · minimap · weapon_bar · enemy_nameplates · damage_numbers
                 · meters · combat_log
    screens/     main_menu · pause · game_over · inventory · settings
  supersolid/    Supersolid backend (default).
    mod.rs       SupersolidUiPlugin: mounts SuperUiRoot, pushes frame each Update.
    bridge.rs    FrameDto + build_frame (UiSnapshot → JSON); HordeIntent / AdjustEnemyCap
                 commands (JS → Rust); ToggleInventoryFwd event (Rust → JS).
assets/ui/horde/
  app.tsx        Single-file TSX UI (all panels + screens). Transpiled to app.generated.js
                 at build time for wasm and no-HMR native builds.
  theme.css      Palette (:root custom properties) + all component/panel/screen rules.
  index.html     Root mount point (<div id="root"></div>).
world_render.rs  Sprite-based world-space render (arena tiles, enemy/pickup sprites).
```

### Data flow (supersolid backend)

Every `Update` tick, `push_ui_frame` serializes the current `UiSnapshot` + `GameState` into a
`FrameDto` and triggers it as a Bevy event. The `superui_bridge` forwards this to JS as
`bevy.on("frame", f => …)`. The single `App` component in `app.tsx` stores `f` in a `createSignal`,
derives `state` with a `createMemo`, and routes rendering through a `<Switch>` to the active
screen/HUD tree. Dynamic visual properties (bar widths, nameplate positions, blip locations) are
bound via inline `style` strings on individual elements. All panels and keyed lists
(`<For each={…}>`) read from the same `frame()` accessor, so a single signal update re-renders
exactly the nodes that changed.

### Intent bridge

TSX buttons call `bevy.send("HordeIntent", { kind, index })` where `kind` is one of:
`"StartGame"`, `"Pause"`, `"Resume"`, `"Restart"`, `"Quit"`, `"SwitchWeapon"`.
Settings adjustments use `bevy.send("AdjustEnemyCap", { delta })`. Keyboard `I`
(toggle inventory) is forwarded from Rust to JS as `bevy.on("toggleInventory", …)` so
the TSX inventory modal stays in sync with keyboard input without polling.

## Performance baseline (by design)

This example is a **real-world game-UI workload for benchmarking, profiling, and optimization
work** — not a tuned showcase. The supersolid backend deliberately ships the simplest honest
data path: every `Update` tick the entire `UiSnapshot` is serialized to JSON and pushed over the
event bridge, and hundreds of nameplate / damage-number divs reconcile each frame. Under a large
enemy swarm (~400 enemies, `stress` preset) this drives frame rate down to ~5 FPS; it recovers to
~60 FPS outside the Playing state (snapshot assembly freezes). Enable `--features debug-ui` (on by
default) to watch the frame-time graph go solid red under load.

That cost is the **point**: it gives profiling and benchmarking workflows a real, measurable
baseline to attack, and the `HORDE_PRESET` / `HORDE_ENEMY_CAP` knobs scale the on-screen element
count so the workload is tunable. The planned follow-up (design §9 — the reactive-store gap) is a
fine-grained store that mutates only changed fields instead of re-pushing the whole snapshot;
this example is the honest "before" it will be measured against.

## Known limitations

- **Wasm: no wasm-bindgen/web runner included.** The wasm binary compiles cleanly
  (`cargo build -p horde --target wasm32-unknown-unknown`), but there is no wasm-bindgen
  post-processing or asset server setup. Running on the web requires the standard Bevy wasm
  workflow (trunk / wasm-bindgen-cli).
