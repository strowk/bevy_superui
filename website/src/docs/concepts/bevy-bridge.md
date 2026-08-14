# The Bevy Bridge

A game UI isn't standalone — it drives the game and reflects its state. superui
connects the two through a small, typed bridge: your `.tsx` talks to the ECS with
`bevy.send` and `bevy.on`, and your Rust registers which names are allowed to
cross. Everything over the bridge is JSON.

There are two directions:

- **UI → game** — a button fires an ECS event (`bevy.send`).
- **game → UI** — live game state streams into the interface (`bevy.on`).

## From the UI: `bevy.send` and `bevy.on`

`bevy` is a global available to your components (also reachable as `window.bevy`).

### Sending to the game

`bevy.send(name, data)` triggers the Bevy event registered under `name`,
deserializing `data` into it:

```typescript
<button onClick={() => bevy.send("HordeIntent", { kind: "StartGame" })}>
  Start
</button>

<button onClick={() => bevy.send("AdjustEnemyCap", { delta: -20 })}>
  Fewer enemies
</button>
```

Sending to an unregistered name warns and does nothing, so the game side decides
exactly what the UI is allowed to do.

### Receiving from the game

`bevy.on(name, cb)` subscribes to an ECS-emitted event; `cb` runs with the JSON
payload every time the game emits `name`. The idiomatic pattern is to land the
payload in a [signal](signals.md) so the rest of your UI reacts to it — set it up
in [`onMount`](lifecycle.md):

```typescript
function Hud() {
  const [frame, setFrame] = createSignal(null);

  onMount(() => {
    bevy.on("frame", (f) => setFrame(f)); // game pushes a frame → store it
  });

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

Once the payload is in a signal, everything downstream — bindings, memos,
[control flow](control-flow.md) — updates automatically. A per-entity feed inside
that payload (enemy nameplates, minimap blips) pairs naturally with
[`<Keyed>`](keyed.md).

## From the game: registering the surface

The names `send`/`on` use only work if Rust has registered them. The `SuperUiApp`
extension trait adds two registrations to your `App`:

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
    app.add_superui_command::<HordeIntent>("HordeIntent") // JS → ECS
        .add_superui_event::<FrameDto>("frame")           // ECS → JS
        .add_observer(on_horde_intent);
}
```

- **`add_superui_command::<T>("name")`** exposes `bevy.send("name", …)` to the UI.
  `T` must be an `Event` that is `Deserialize` (the JSON becomes `T`).
- **`add_superui_event::<T>("name")`** exposes `bevy.on("name", …)` to the UI. `T`
  must be an `Event` that is `Serialize` (it becomes the JSON payload).

### Handling a command

A command arrives as a normal Bevy event, so you handle it with an observer:

```rust
fn on_horde_intent(ev: On<HordeIntent>, mut intents: ResMut<IntentQueue>) {
    match ev.event().kind.as_str() {
        "StartGame" => intents.push(Intent::StartGame),
        "Pause"     => intents.push(Intent::Pause),
        other       => warn!("unknown HordeIntent kind '{other}'"),
    }
}
```

### Emitting an event to the UI

To push state to the UI, build the payload and `trigger` it — typically once per
frame from a system:

```rust
fn push_frame(mut commands: Commands, snap: Res<UiSnapshot>) {
    commands.trigger(FrameDto {
        player_hp: snap.player_hp,
        player_max_hp: snap.player_max_hp,
        wave: snap.wave,
        kills: snap.kills,
    });
}
```

Each trigger is serialized and delivered to every `bevy.on("frame", …)`
subscriber in the UI.

## The full loop

Putting both directions together, a superui game UI is a loop:

1. The game triggers registered events → `bevy.on` callbacks store them in signals.
2. Signals drive the rendered UI reactively.
3. The player interacts → handlers call `bevy.send`.
4. Registered commands become ECS events your observers act on → game state changes.
5. …which the next frame's event reflects back to the UI.

Keeping the boundary this narrow — a handful of named, JSON commands and events —
keeps the game logic serde-light and the UI ignorant of ECS internals; each side
only knows the names and shapes it registered.

## Reference

- [`window.bevy`](../reference/js-dom.md#globals) — the bridge in the JS/DOM ledger.
- [Picking & the world behind the UI](picking.md) — the other half of the seam:
  which clicks the UI keeps and which reach the game.
- [Keyed lists & performance](keyed.md) — rendering high-frequency
  per-entity data from a frame payload.
