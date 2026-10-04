# Design: `bestiary` example — JSON asset over the bridge, mirrored by JSON-import in tests

Date: 2026-10-04
Status: Approved (self-directed) — proceeding to writing-plans
Branch: `shot/example-json-import-in-test`

## Intent

Publish one gallery example that demonstrates the two recently-added
capabilities working together around a **single source-of-truth JSON file**:

- **Runtime (the game):** a data file is loaded through the normal Bevy
  `AssetServer`, then handed to the UI over the Bevy bridge
  (`commands.trigger(...)` → `bevy.on(...)`).
- **Tests:** the *same* JSON file is read via the transpiler's `.json`
  **import** feature and pushed into the UI with `page.emit(...)`, exercising
  the identical bridge-receive path without a running game.

The teaching point: author your data once as a JSON asset; the game delivers it
at runtime and the test replays it at author time, both through the same bridge
event. The example is runnable on the website and documented in a README.

Who it's for: superui users learning how to drive data-driven UI from Bevy and
how to test that UI deterministically.

Success looks like:
- `cargo run -p bestiary` shows a list of creatures rendered from
  `assets/data/bestiary.json`, delivered over the bridge.
- `cd examples/bestiary && cargo run -p superui_test_engine --bin superui_test`
  passes a spec that imports that same JSON and asserts the rendered UI.
- The example appears in the gallery and builds for wasm.

## Theme & scope

A **Bestiary**: a small catalogue of creatures (name, element, hp, attack).
Visually simple, obviously data-driven, no game logic. Category: **Apps**.

Slug / package / UI dir name: `bestiary` (the permanent public URL
`/examples/bestiary/`; slug == package == `assets/ui/bestiary/`).

YAGNI: no interactivity, no screenshots baseline, no playground variant, no
sim. The only moving parts are the three the example exists to show: asset-load,
bridge push, and import-in-test.

## Single source of truth: the data shape

`assets/data/bestiary.json`:

```json
{
  "creatures": [
    { "name": "Ember Drake",  "element": "fire",  "hp": 120, "attack": 34 },
    { "name": "Tide Serpent", "element": "water", "hp": 140, "attack": 28 },
    { "name": "Gale Roc",     "element": "air",   "hp":  90, "attack": 41 },
    { "name": "Crag Golem",   "element": "earth", "hp": 200, "attack": 22 }
  ]
}
```

One Rust struct is the whole contract. It is simultaneously the **asset** (so
the `AssetServer` can load it) and the **bridge event** (so it can be
triggered), and it (de)serializes to exactly the JSON above:

```rust
#[derive(Asset, TypePath, Event, Clone, Serialize, Deserialize)]
struct Bestiary { creatures: Vec<Creature> }

#[derive(Clone, Serialize, Deserialize)]
struct Creature { name: String, element: String, hp: u32, attack: u32 }
```

Because the loaded asset is serialized back out when triggered, the JSON the
game emits is byte-for-byte the shape the test imports. That identity is the
point of the example.

## Components

### 1. JSON asset loader (`src/main.rs`)

A minimal `AssetLoader` for `Bestiary`, modelled on `JsLoader`
(`crates/superui/src/assets.rs`). Error type is `std::io::Error` (no extra
deps); JSON parse failures map to `ErrorKind::InvalidData`.

```rust
impl AssetLoader for BestiaryLoader {
    type Asset = Bestiary;
    type Settings = ();
    type Error = std::io::Error;
    async fn load(&self, reader, _settings, _lc) -> Result<Bestiary, io::Error> {
        let mut s = String::new();
        reader.read_to_string(&mut s).await?;
        serde_json::from_str(&s)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }
    fn extensions(&self) -> &[&str] { &["json"] }
}
```

Registered with `app.init_asset::<Bestiary>().register_asset_loader(BestiaryLoader)`.
This is the "normal asset server" path the intent calls for — no `include_str!`,
no `std::fs`.

### 2. Bridge wiring (`src/main.rs`)

Register one event (Rust → JS) and one command (JS → Rust), mirroring the
citadel bridge module shape:

```rust
app.add_superui_event::<Bestiary>("bestiary")     // bevy.on("bestiary", ...)
   .add_superui_command::<UiReady>("uiReady")      // bevy.send("uiReady", null)
   .add_observer(on_ui_ready);
```

`UiReady` is a unit event (`#[derive(Event, Deserialize)] struct UiReady;`,
deserialized from JSON `null`).

### 3. Ready handshake (solves the runtime race)

The asset and the UI source both load asynchronously, so a one-shot emit at
`Startup` would race the UI's `bevy.on` subscription and be lost. The UI
announces readiness; the game replies with the data exactly once. This also
demonstrates the JS → Rust bridge direction.

```rust
#[derive(Resource, Default)]
struct BridgeState { ui_ready: bool, sent: bool }

fn on_ui_ready(_: On<UiReady>, mut state: ResMut<BridgeState>) {
    state.ui_ready = true;
}

// Update: fire once both the UI is ready AND the asset has finished loading.
fn push_bestiary(
    mut state: ResMut<BridgeState>,
    handle: Res<BestiaryHandle>,
    assets: Res<Assets<Bestiary>>,
    mut commands: Commands,
) {
    if state.sent || !state.ui_ready { return; }
    if let Some(b) = assets.get(&handle.0) {
        commands.trigger(b.clone());
        state.sent = true;
    }
}
```

The single `Update` system handles either load/mount ordering: it latches when
both conditions hold, regardless of which happened first.

### 4. Startup (`src/main.rs`)

```rust
fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    commands.spawn(Camera2d);
    commands.insert_resource(BestiaryHandle(assets.load("data/bestiary.json")));
    commands.spawn(SuperUiRoot::from_asset_dir("ui/bestiary", &assets));
}
```

Plus the standard `web_window` / `web_asset_plugin` wasm hooks copied from
`counter`/`citadel` (`AssetMetaCheck::Never` on wasm).

### 5. UI (`assets/ui/bestiary/`)

`index.html` (manifest) + `style.css` + `app.tsx`. The TSX subscribes on mount,
announces readiness, and renders a card per creature with `<For>`:

```tsx
import { createSignal, For, onMount, render } from "supersolid";

function App() {
  const [creatures, setCreatures] = createSignal([]);
  onMount(() => {
    bevy.on("bestiary", (data) => setCreatures(data.creatures));
    bevy.send("uiReady", null);
  });
  return (
    <div class="bestiary">
      <For each={creatures()}>{(c, i) => (
        <div class={`card ${c.element}`} id={`creature-${i()}`}>
          <span class="name">{c.name}</span>
          <span class="element">{c.element}</span>
          <span class="stat">HP {c.hp}</span>
          <span class="stat">ATK {c.attack}</span>
        </div>
      )}</For>
    </div>
  );
}
render(() => <App />, document.getElementById("root"));
```

`style.css` follows the flair-0.6 constraints documented in counter's CSS
(no `cursor`, no `filter:brightness`, `border-width:0` instead of
`border:none`, only loaded fonts).

### 6. Test (`tests/bestiary.spec.ts` + `superui.test.toml`)

`superui.test.toml` points at the UI project and the spec dir:

```toml
project = "assets/ui/bestiary"
specDir = "tests"
width = 900
height = 600
```

The spec imports the **same** JSON (resolved by `transpile_spec` as
`tests/../assets/data/bestiary.json` → `assets/data/bestiary.json`) and pushes
it with `page.emit`, which delivers to `bevy.on("bestiary", ...)` through the
real ECS→JS leg — exactly what `commands.trigger(Bestiary {..})` does at runtime:

```ts
import { test, expect } from "superui/test";
import bestiary from "../assets/data/bestiary.json";

test("renders every creature pushed over the bridge", async ({ page }) => {
  await page.emit("bestiary", bestiary);
  await expect(page.locator(".card")).toHaveCount(bestiary.creatures.length);
  await expect(page.locator("#creature-0 .name"))
    .toHaveText(bestiary.creatures[0].name);
});
```

No screenshot baseline (keeps the example dependency-light and portable).

## Data flow

```
                       assets/data/bestiary.json   ← single source of truth
                        /                        \
         AssetServer.load (runtime)        .json import (test transpile)
                |                                     |
         Assets<Bestiary>                       const bestiary = {...}
                |                                     |
   push_bestiary: commands.trigger(Bestiary)    page.emit("bestiary", bestiary)
                \                                   /
                 \_____ bevy.on("bestiary", ...) __/   ← same receive path
                                 |
                      setCreatures(data.creatures) → <For> cards
```

Handshake (runtime only): `onMount → bevy.send("uiReady")` →
`on_ui_ready` sets `ui_ready` → `push_bestiary` latches and fires once.

## Error handling

- Loader: invalid/missing JSON → `io::Error` → asset load fails; `push_bestiary`
  simply never fires (empty UI) and Bevy logs the load error. Acceptable for a
  demo; the JSON is authored and valid.
- Test import: a missing JSON file is **fatal** in `transpile_spec` — correct,
  since it's an authoring bug.
- `bevy.send("uiReady")` in the test harness: the JS→Rust command isn't
  registered in the test app, so it is a no-op (at most a benign warn). The test
  exercises only the receive leg via `page.emit`, which is the documented intent
  of `page.emit` testing (see `docs/concepts/bevy-bridge.md`).

## Testing strategy

Two layers, both TDD:

1. **Rust** (`tests/bridge.rs` in the example crate): headless `App` with the
   loader + bridge registered.
   - Loader test: load `data/bestiary.json`, pump until loaded, assert 4
     creatures with expected first-row values.
   - Handshake test: trigger `UiReady`, run `push_bestiary` with the asset
     present, assert the `Bestiary` event is observed exactly once (capture via
     an observer writing to a resource).
2. **TS spec** (`tests/bestiary.spec.ts`): the showcase test above, run by the
   superui test engine. This is the capability the example exists to publish.

## Files

New crate `examples/bestiary/`:
- `Cargo.toml` — counter template + `serde` (derive) + `serde_json`; `hmr`
  feature; wasm target deps (`getrandom`, `bevy/webgl2`); `supersolid`
  build-dependency.
- `build.rs` — `supersolid::build::transpile_dir("assets/ui/bestiary")`.
- `src/main.rs` — loader, bridge, handshake, startup, wasm hooks.
- `tsconfig.json`, `.gitignore` (ignore `**/.superui/`, `superui_modules/`).
- `assets/ui/bestiary/{index.html,style.css,app.tsx}`.
- `assets/data/bestiary.json`.
- `superui.test.toml`, `tests/bestiary.spec.ts`, `tests/bridge.rs`.
- `README.md` — the pattern walkthrough (asset→bridge→UI; import→emit→UI),
  run/test commands.

Edits:
- `examples/gallery.json` — append an Apps entry
  (`slug/package/category/title/description`).
- `tools/build-demos.sh` — add `bestiary` to the local-demo slug list.

Workspace: `members = ["examples/*", ...]` is globbed — no root `Cargo.toml`
edit needed.

## Out of scope

- Website docs prose beyond the README (the bridge/testing concept docs already
  cover `page.emit` and the bridge API; a one-line cross-link may be added but
  is not required).
- Playground/live-edit variant, screenshots, benchmarks.
```
