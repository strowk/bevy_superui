# The Bevy bridge + Rust registration + mounting

> Mirrors `website/src/docs/concepts/bevy-bridge.md` and `getting-started.md`. Keep in sync.

A superui UI drives the game and reflects its state through a small, typed, JSON bridge.
`bevy` is a JS global (also `window.bevy`). Two directions:

- **UI → game:** `bevy.send(name, data)` triggers a registered Bevy event.
- **game → UI:** `bevy.on(name, cb)` subscribes; `cb(payload)` runs each emit.

Names only work if the Rust side registered them; sending an unregistered name warns and
does nothing.

## JS side

```tsx
// UI → game: fire an ECS event on interaction
<button onClick={() => bevy.send("HordeIntent", { kind: "StartGame" })}>Start</button>
<button onClick={() => bevy.send("AdjustEnemyCap", { delta: -20 })}>Fewer enemies</button>

// game → UI: land the payload in a signal (set up in onMount) so the UI reacts
function Hud() {
  const [frame, setFrame] = createSignal(null);
  onMount(() => { bevy.on("frame", (f) => setFrame(f)); });
  return (
    <Show when={frame()}>
      <div id="hud">
        <span>HP: {frame().player_hp} / {frame().player_max_hp}</span>
        <span>Wave {frame().wave} · {frame().kills} kills</span>
      </div>
    </Show>
  );
}
```

Once a payload is in a signal, everything downstream (bindings, memos, control flow)
updates automatically. A per-entity feed inside the payload (nameplates, blips) pairs
with `<Keyed>` (see `control-flow.md`).

## Rust side — registering the surface

The `SuperUiApp` extension trait adds two registrations to your `App`. Events crossing the
bridge are serde types.

```rust
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use superui::prelude::SuperUiApp;

// UI → game. Deserialized from the JSON passed to bevy.send(...).
#[derive(Event, Deserialize)]
struct HordeIntent { kind: String, #[serde(default)] index: i64 }

// game → UI. Serialized to the JSON delivered to bevy.on("frame", ...).
#[derive(Event, Serialize)]
struct FrameDto { player_hp: f32, player_max_hp: f32, wave: u32, kills: u32 }

fn register_bridge(app: &mut App) {
    app.add_superui_command::<HordeIntent>("HordeIntent") // exposes bevy.send("HordeIntent", …)
        .add_superui_event::<FrameDto>("frame")           // exposes bevy.on("frame", …)
        .add_observer(on_horde_intent);
}
```

- **`add_superui_command::<T>("name")`** — exposes `bevy.send("name", …)`. `T`: `Event` +
  `Deserialize` (JSON → `T`).
- **`add_superui_event::<T>("name")`** — exposes `bevy.on("name", …)`. `T`: `Event` +
  `Serialize` (`T` → JSON payload).

### Handle a command (an observer)

```rust
fn on_horde_intent(ev: On<HordeIntent>, mut intents: ResMut<IntentQueue>) {
    match ev.event().kind.as_str() {
        "StartGame" => intents.push(Intent::StartGame),
        "Pause"     => intents.push(Intent::Pause),
        other       => warn!("unknown HordeIntent kind '{other}'"),
    }
}
```

### Emit an event to the UI (typically once per frame)

```rust
fn push_frame(mut commands: Commands, snap: Res<UiSnapshot>) {
    commands.trigger(FrameDto {
        player_hp: snap.player_hp, player_max_hp: snap.player_max_hp,
        wave: snap.wave, kills: snap.kills,
    });
}
```

Each `trigger` is serialized and delivered to every `bevy.on("frame", …)` subscriber.

## Mounting the UI

```rust
use bevy::prelude::*;
use superui::prelude::{SuperUiPlugin, SuperUiRoot};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(SuperUiPlugin)
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    commands.spawn(Camera2d);
    commands.spawn(SuperUiRoot::from_asset_dir("ui/counter", &assets)); // loads assets/ui/counter/index.html
}
```

`from_asset_dir(dir, &assets)` loads `<dir>/index.html` and everything it references, and
bundles a **full-viewport root `Node`** so percentage/inset children resolve against the
window. For a custom root node, use `SuperUiRoot::from_asset_dir_with(dir, node, &assets)`.

## The full loop

1. Game `trigger`s a registered event → `bevy.on` callbacks store it in signals.
2. Signals drive the rendered UI reactively.
3. Player interacts → handlers call `bevy.send`.
4. Registered commands become ECS events your observers act on → game state changes.
5. …reflected back to the UI on the next frame's event.

Keep the boundary narrow — a handful of named JSON commands/events — so game logic stays
serde-light and the UI stays ignorant of ECS internals.
