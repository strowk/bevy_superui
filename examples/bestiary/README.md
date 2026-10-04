# Bestiary — JSON over the bridge

Demonstrates a single JSON asset as the source of truth for a UI, consumed two different ways
through the same Bevy bridge event. `assets/data/bestiary.json` lists four creatures
(name/element/hp/attack). At runtime the game loads that file through the normal Bevy
`AssetServer` and pushes it to the UI as a bridge event. In tests, the `.spec.ts` imports the
same JSON file directly and replays it against the identical receive path — no running game
required.

## Data flow

```
Runtime (cargo run -p bestiary):
  assets/data/bestiary.json --AssetServer+BestiaryLoader--> Bestiary asset
                             --commands.trigger-->  bridge event "bestiary"
                                                     --> bevy.on("bestiary", ...) --> cards

Test (bestiary.spec.ts):
  assets/data/bestiary.json --import--> page.emit("bestiary", bestiary)
                                                     --> bevy.on("bestiary", ...) --> cards
```

Both paths end at the same `bevy.on("bestiary", ...)` handler in `assets/ui/bestiary/app.tsx`,
so the UI code under test is exactly the UI code that runs for real.

`BestiaryLoader` (`src/lib.rs`) deserializes the JSON into the `Bestiary` asset type, which
doubles as the bridge event type — `commands.trigger(bestiary.clone())` and
`app.add_superui_event::<Bestiary>("bestiary")` both operate on the same struct.

## The `uiReady` handshake

The game loads the asset and the UI mounts independently, in no fixed order; without a
handshake the bestiary could be pushed before the UI has registered its `bevy.on("bestiary", ...)`
listener and the event would be lost. The UI sends `bevy.send("uiReady", null)` on mount, and
`push_bestiary` only triggers the `Bestiary` event once both the UI has announced readiness and
the asset has finished loading — sent exactly once, regardless of which condition is met first.

## Run / test

```sh
# Run the game.
cargo run -p bestiary

# Run the UI spec (tests/bestiary.spec.ts).
cd examples/bestiary && cargo run -p superui_test_engine --bin superui_test

# Run the Rust tests (asset loading + handshake logic).
cargo test -p bestiary
```
