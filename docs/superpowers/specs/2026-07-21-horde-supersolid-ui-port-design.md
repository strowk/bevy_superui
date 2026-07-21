# Horde — Supersolid UI port (design)

Date: 2026-07-21
Status: Design agreed.

Source design (native deliverable): `docs/superpowers/specs/2026-07-20-horde-native-ui-design.md`.
Wiring reference: `examples/todomvc_supersolid/`.
Bridge reference: `crates/superui_bridge/src/bevy_bridge.rs`.

## 0. What this document is

This is the design for porting the **entire UI** of the Horde example from native `bevy_ui`
to **superui TSX authored with supersolid**, running on the framework as it exists today. The
native UI is preserved behind an opt-in feature flag; the two backends are structurally parallel
and both read the identical `UiSnapshot` and raise the identical intents, so a future benchmark can
drive the sim once and compare backends like-for-like (the native design's "differential oracle").

It deliberately does **not** build new framework infrastructure. Instead it uses the existing
`window.bevy` event bridge and records — as an explicit finding — the gap that this reveals (see §9).

## 1. Goals & non-goals

**Goals**
- `cargo run -p horde` (default) plays the full game with the **supersolid** UI.
- `cargo run -p horde --no-default-features --features ui-native` plays identically on the
  **native** UI, unchanged.
- `ui-native` is **removed from `default`**; the two backends are mutually exclusive and
  structurally parallel, so a reader can read one or the other cleanly.
- Sim, input, world rendering, camera, and the game-state machine are **untouched**.
- Both backends consume the same `UiSnapshot` and raise the same intents (benchmark parity).
- Supersolid UI is visually close enough to native that a screenshot comparison is meaningful.

**Non-goals (this stage)**
- No new bridge/runtime primitive (no reactive store, no `bevy.query`). The whole-snapshot push
  is used as-is and its unrepresentativeness is documented, not fixed (§9).
- No benchmark harness, no profiling probes, no perf tuning (the seams already exist).
- No changes to sim behavior, weapons, spawn logic, or world rendering.
- No multi-file `.tsx` module splitting unless the transpiler is verified to support it (§3, §8).

## 2. Data path (decided: snapshot-signal, gap documented)

The novel problem vs. `todomvc_supersolid`: todomvc's TSX *owns* its state, whereas horde's TSX is
a **view over per-frame Rust-owned state**. We resolve it with the existing event bridge:

### 2.1 Rust → TSX (continuous state)
A backend-owned `push_ui_frame` system reads `Res<UiSnapshot>` + `State<GameState>`, serializes
them into one JSON payload, and pushes it via the existing `bevy.on("frame", cb)` mechanism
(`add_superui_event` / the INBOX emit path) **every frame**. This keeps `sim/`'s snapshot pure —
game state is joined in by the backend, not baked into the sim's `assemble_world_snapshot`.

TSX side: one root `snapshot` signal is set from the `bevy.on("frame", ...)` callback. Per-panel
`createMemo`s and keyed `<For>` (keyed by each item's stable `id`) derive the panels so only the
changed slices touch the DOM. This is idiomatic Solid authoring — memos gate downstream effects,
keyed `<For>` avoids row churn — which naturally avoids the naive "re-render everything" worst case
without any bespoke infrastructure.

### 2.2 TSX → Rust (intents)
Buttons call `bevy.send("HordeIntent", { kind, arg })`. The backend registers
`add_superui_command::<HordeIntent>("HordeIntent")` and an observer that maps `HordeIntent` onto
the **existing** `IntentQueue` (and `AppExit` for quit), so `apply_menu_intents` runs identically
for both backends. The intent surface is small:

`StartGame`, `Pause`, `Resume`, `Restart`, `SwitchWeapon(index)`, `Quit`.

Inventory and settings modal *visibility* is **local TSX state** — local UI state, not a sim intent
(native models it the same way via the `SettingsOpen` resource / local components). On-screen
buttons toggle those local signals directly. To preserve keyboard parity, the keyboard `I` key's
`ToggleInventory` intent (raised in `input.rs`) is **forwarded to TSX as a discrete event**
(`bevy.on("toggleInventory")`) by the backend, which the TSX flips the local inventory signal on.
`Esc`/pause needs no such forward — it flows through `GameState` and the pushed `state` field
already drives the pause screen.

### 2.3 Gameplay input (unchanged)
WASD/arrows/mouse-aim/click-shoot/scroll-switch continue to flow through `input.rs`
(`bevy_input` → `IntentQueue`), backend-independent. TSX never touches gameplay input.

## 3. Module structure

```
examples/horde/src/ui/
  mod.rs            # backend select — cfg(ui-native) → native | else → supersolid.
                    # add_ui() also adds the shared projection for BOTH backends.
  project.rs        # LIFTED here from native/project.rs: world→screen projection
                    # (fills screen_pos from world_pos), added by add_ui() for both backends.
  native/           # unchanged, EXCEPT project.rs moves out and NativeUiPlugin no longer adds it.
  supersolid/
    mod.rs          # SupersolidUiPlugin: spawn SuperUiRoot; register HordeIntent command +
                    #   observer; run push_ui_frame. Mirrors NativeUiPlugin's shape.
    bridge.rs       # push_ui_frame (snapshot+state → JSON, emitted as a "frame" event) and
                    #   HordeIntent event + observer → IntentQueue / AppExit.
```

```
examples/horde/assets/ui/horde/          # the reserved dir, now populated
  index.html
  theme.css        # palette / spacing / fonts / state colors ported from theme.rs
  components.css   # panels, bars, buttons, health-fraction ramp, hover/active (flair :hover/:active)
  app.tsx          # root <App>; <Switch>/<Show> on state → screens; HUD panels as components
  app.generated.js # build.rs output (wasm / no-hmr native)
  supersolid-shim.d.ts
```

`ui/mod.rs::add_ui()` becomes: add the shared `project_snapshot` system, then `cfg`-select the
backend plugin. The former `panic!` arm is replaced by `SupersolidUiPlugin`. Both backend plugins
have the same silhouette (mount + read snapshot + raise intents), so a reader diffs `native/` vs
`supersolid/` directly, and a future benchmark can swap `add_ui` for a null sink as the native
design intended.

**Projection lift is load-bearing:** `enemy_nameplates` and `damage_numbers` position off
`screen_pos`, which `project_snapshot` fills from `world_pos` using the `Camera`. It must run for
the supersolid backend too, so it moves from `native/` to the shared `ui/project.rs` and is added
by `add_ui()` regardless of backend. It stays ordered after `assemble_world_snapshot`.

**Single-file `.tsx` (proven path):** the supersolid transpiler is verified only on a single
`app.tsx` (todomvc). All panels/screens are authored as component functions **within one
`app.tsx`**. Multi-file `.tsx` splitting is a backlog item pending a transpiler-capability check
(§8); it is not on the critical path.

## 4. Build / HMR / wasm wiring

Mirror `todomvc_supersolid` exactly:
- `USE_LIVE_TSX = cfg!(all(not(target_arch = "wasm32"), feature = "hmr"))`. Native+`hmr` loads the
  live `app.tsx` through the transpiling loader (state-preserving HMR); wasm / no-hmr loads
  `app.generated.js`.
- A host-only `build.rs` pre-transpiles `app.tsx` → `app.generated.js` on every build
  (`supersolid::transpile_file`), warn-only on diagnostics.
- The supersolid backend spawns the `SuperUiRoot` with `html` / `css` / `js` handles.

## 5. Feature-flag arrangement

```toml
[features]
default = ["debug-ui"]                 # ui-native NO LONGER in default → default = supersolid
ui-native = []                         # opt-in native backend (else-arm = supersolid)
hmr = ["superui/hmr", "bevy/file_watcher"]
debug-ui = ["bevy/bevy_dev_tools"]
mcp_debug = ["dep:bevy_brp_extras", "bevy/bevy_remote"]
```

`ui/mod.rs`: `#[cfg(feature = "ui-native")]` → native; `#[cfg(not(feature = "ui-native"))]` →
supersolid. Because supersolid is simply "absence of `ui-native`," there is no both-on conflict to
guard. New dependencies: `superui`, `superui_css` (and `superui_bridge` if the click-injector is
mirrored), plus `supersolid` as a host-only `build-dependency`, matching `todomvc_supersolid`.

`getrandom` wasm feature and the `file_watcher` native-only dep are carried over from the existing
horde `Cargo.toml` / the todomvc_supersolid pattern.

## 6. Panels & screens (1:1 with native, all in TSX)

HUD (`assets/ui/horde/app.tsx` components): `player_status`, `enemy_nameplates`, `damage_numbers`,
`minimap`, `weapon_bar`, `meters`, `combat_log`.

Screens / modals: `main_menu`, `pause`, `game_over`, `inventory`, `settings`.

- Each component reads its snapshot slice via a `createMemo`.
- Screens gate on the pushed game `state` via `<Switch>`/`<Show>`.
- `enemy_nameplates` / `damage_numbers` position off `screen_pos` (from the lifted projection).
- `weapon_bar` slots and menu buttons emit `HordeIntent` via `bevy.send`.
- `inventory` / `settings` visibility are local TSX signals, toggled by on-screen buttons; the
  keyboard `I` (`ToggleInventory`) is forwarded from Rust to TSX as a discrete `toggleInventory`
  event (§2.2) so the key still opens the modal. `Esc`/pause flows through `GameState` and the
  pushed `state` field.
- The top-left corner stays empty for the FPS overlay (`debug-ui`), same as native.

CSS: `theme.css` ports the `theme.rs` constants (BG/PANEL/ACCENT/DANGER/GOOD/WARN, SPACE/RADIUS,
FONT sizes, the `hp_color` green→amber→red ramp expressed as discrete classes or inline width/color
bindings) and `components.css` styles the panels/bars/buttons with `:hover`/`:active` states
(flair 0.6 supports element/class/id/pseudo selectors).

## 7. Acceptance

- `cargo run -p horde` (default) plays the full game with the supersolid UI — all §6 panels/screens.
- `cargo run -p horde --no-default-features --features ui-native` plays identically on native.
- `cargo build -p horde --target wasm32-unknown-unknown` compiles (supersolid, generated JS).
- `sim/` still has zero dependency on `ui/` or `bevy_ui`; both backends consume the identical
  `UiSnapshot`.
- Supersolid UI is visually close enough to native that a screenshot comparison is meaningful.
- The "reactive store gap" finding (§9) is recorded in this spec and a follow-up plan is named.
- `cargo test -p horde` (sim tests) still passes; any new supersolid-side tests pass.

## 8. Risks & mitigations

- **Transpiler single-file only.** Mitigation: author one `app.tsx`; multi-file split is backlog
  gated on a capability check. (Verify early so the plan can note it.)
- **Per-frame full-snapshot JSON cost** (hundreds of enemies/damage numbers). Accepted as the
  honest baseline; idiomatic memos + keyed `<For>` limit DOM work; perf is a later, measured
  exercise. This is the point of §9.
- **Windows main-thread stack / Boa parse** (see memory: Boa parses render.js at mount; the repo
  already sets `/STACK:8MB`). Horde must launch in a *window*, not only pass worker-thread tests —
  verify an actual windowed launch, since green tests don't prove it.
- **Control-flow authoring gotcha:** `<For>`/`<Show>` must be `{...}`-wrapped inside plain elements
  or they silently render nothing (known supersolid TodoMVC gotcha). The plan calls this out.
- **Projection lift regression:** moving `project_snapshot` must not change native behavior; native
  still gets it via `add_ui()`. Verify native still positions nameplates correctly.

## 9. The documented gap (finding + follow-up)

Pushing the **entire** `UiSnapshot` over an event bus every frame is **not** how anyone would
realistically build a live game UI, and the `UiSnapshot` blob itself is partly a benchmark artifact
rather than idiomatic `bevy_ui`. The idiomatic model (as in Solid/React web games) is a **shared
fine-grained reactive store the game loop writes into and the UI reads from**, mutating only what
changed. The framework today has **no such primitive**: supersolid has `createSignal`/`createMemo`/
`For`/`Show` but no `createStore`, and the bridge is event-only (`bevy.query` is deferred).

**Follow-up plan (named, not scheduled here):** *"Reactive Bevy-state ↔ TSX store primitive"* — a
minimal Solid-style store (or resource-binding) that the sim mutates field-by-field and TSX reads
fine-grained, with horde as its first real consumer and the honest benchmark target (measuring the
data path real apps would use). This port deliberately ships on the event bus first so that the
follow-up has a concrete, measured baseline to improve on.
