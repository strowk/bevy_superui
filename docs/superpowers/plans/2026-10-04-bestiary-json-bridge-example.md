# Bestiary JSON-asset-over-bridge Example — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Publish a gallery example where the game loads one JSON asset via the Bevy `AssetServer` and pushes it to the UI over the bridge, while tests import that same JSON file and replay it with `page.emit`.

**Architecture:** One Rust struct (`Bestiary`) is both the loadable asset and the bridge event, so the JSON the game emits is byte-for-byte what the test imports. A `uiReady` handshake defeats the asset-load/UI-mount race at runtime. The UI subscribes to `bevy.on("bestiary", ...)` and renders a card per creature; the test exercises that same receive path with `page.emit`.

**Tech Stack:** Rust, Bevy 0.19, superui / supersolid (TSX), `serde`/`serde_json`, superui_test_engine (Playwright-style `.spec.ts`).

**Spec:** `docs/superpowers/specs/2026-10-04-json-asset-bridge-example-design.md`

## Global Constraints

- Slug == package == UI dir name == `bestiary`; it is the permanent public URL — never rename.
- Bevy `0.19` via `{ workspace = true }`; observers use `On<E>`, events fire via `commands.trigger(...)`.
- No new workspace dependencies beyond `serde` (derive) + `serde_json`, pinned as plain versions in the example `Cargo.toml` (examples don't use workspace deps for these).
- `publish = false`; crate auto-registers via the `examples/*` workspace glob (no root `Cargo.toml` edit).
- wasm hooks copied verbatim from `examples/counter/src/main.rs`: `AssetMetaCheck::Never` on wasm, `canvas: "#superui-canvas"`, `fit_canvas_to_parent`.
- CSS obeys flair-0.6 limits (see counter's `style.css` header): no `cursor`, no `filter:brightness`, `border-width:0` not `border:none`, only loaded fonts.
- `.gitignore` ignores `**/.superui/` and `superui_modules/`.
- JSON `element` values are lowercase (`fire`/`water`/`air`/`earth`) so they double as CSS class names.

## Review Focus

- **Load/mount ordering (either order):** the data must reach the UI whether the asset finishes before or after `uiReady` arrives. → pinned in Task 2 (handshake test runs both orders).
- **Asset absent/not-yet-loaded:** `push_bestiary` must be a no-op (no panic, `sent` stays false) when the handle has no asset. → pinned in Task 2.
- **Empty render:** before any emit (and for an empty array) the UI renders zero `.card`s without error. → pinned in Task 3 (spec asserts 0 cards pre-emit).
- **Spec-dir-relative `..` import:** `../assets/data/bestiary.json` must resolve to the same on-disk file the game loads, or the spec transpile is fatal. → pinned in Task 3 (spec imports it and passes).
- **Emit payload == serialized event shape:** the hand-free test payload is the imported file verbatim, matching what `commands.trigger(Bestiary)` serializes to. → pinned in Task 3.

---

### Task 1: Crate skeleton, data model, and JSON asset loader

**Files:**
- Create: `examples/bestiary/Cargo.toml` (counter template + `serde = { version = "1", features = ["derive"] }`, `serde_json = "1"`)
- Create: `examples/bestiary/build.rs` — `fn main() { supersolid::build::transpile_dir("assets/ui/bestiary"); }`
- Create: `examples/bestiary/.gitignore`, `examples/bestiary/tsconfig.json` (mirror `examples/horde/tsconfig.json`, swapping nothing but comments)
- Create: `examples/bestiary/assets/data/bestiary.json` (the 4-creature document from the spec)
- Create: `examples/bestiary/assets/ui/bestiary/index.html` (manifest: links `style.css`, `<script type="module" src="app.tsx">`, `<div id="root">`)
- Create: `examples/bestiary/assets/ui/bestiary/style.css` (card layout; flair-0.6 safe)
- Create: `examples/bestiary/assets/ui/bestiary/app.tsx` (minimal static mount: `render(() => <div class="bestiary" />, document.getElementById("root"))` — real behavior lands in Task 3)
- Create: `examples/bestiary/src/main.rs` — `Bestiary`/`Creature`, `BestiaryLoader`, `BestiaryHandle`, wasm hooks, `main`, `setup`
- Test: `examples/bestiary/tests/bridge.rs`

**Interfaces:**
- Produces:
  - `struct Bestiary { creatures: Vec<Creature> }` — `#[derive(Asset, TypePath, Event, Clone, Serialize, Deserialize)]`. (Asset and Event derives are orthogonal and coexist: Asset adds TypePath + dependency-visit, Event adds the observer marker.)
  - `struct Creature { name: String, element: String, hp: u32, attack: u32 }` — `#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]`
  - `struct BestiaryLoader` with `AssetLoader<Asset = Bestiary, Settings = (), Error = std::io::Error>`; `extensions() -> &["json"]`; parse via `serde_json::from_str`, mapping errors to `io::ErrorKind::InvalidData`.
  - `#[derive(Resource)] struct BestiaryHandle(Handle<Bestiary>)`
  - `fn setup(commands, assets)` spawns `Camera2d`, inserts `BestiaryHandle(assets.load("data/bestiary.json"))`, spawns `SuperUiRoot::from_asset_dir("ui/bestiary", &assets)`.
  - `main()` registers the loader via `app.init_asset::<Bestiary>().register_asset_loader(BestiaryLoader)` and adds `SuperUiPlugin`, `Startup(setup)`. (Bridge wiring added in Task 2.)

- [ ] **Step 1: Write the failing test** in `examples/bestiary/tests/bridge.rs`

```rust
// Builds a headless app with AssetPlugin + the Bestiary loader, loads the real
// asset file, pumps until loaded, and checks the decoded contents.
#[test]
fn loads_bestiary_from_asset_server() {
    let mut app = test_app();                       // AssetPlugin + init_asset + loader
    let handle: Handle<Bestiary> =
        app.world().resource::<AssetServer>().load("data/bestiary.json");
    pump_until_loaded(&mut app, &handle);           // bounded loop over app.update()
    let assets = app.world().resource::<Assets<Bestiary>>();
    let b = assets.get(&handle).expect("asset loaded");
    assert_eq!(b.creatures.len(), 4);
    assert_eq!(b.creatures[0].name, "Ember Drake");
    assert_eq!(b.creatures[0].element, "fire");
    assert_eq!(b.creatures[0].hp, 120);
    assert_eq!(b.creatures[0].attack, 34);
}
```

The test needs the loader registered on the app and the asset root pointed at `examples/bestiary/assets`. Use `AssetPlugin { file_path: "assets".into(), ..default() }` (the test runs with the crate dir as CWD). Expose `Bestiary`, `Creature`, `BestiaryLoader` from the crate — add a tiny `src/lib.rs` re-exporting them, or mark `main.rs` items `pub` and declare `#[path="../src/main.rs"]`? Prefer a `src/lib.rs` holding the model + loader + bridge, with `main.rs` a thin binary that `use bestiary::*`. Update the Files list accordingly (`src/lib.rs` is where testable items live; `src/main.rs` keeps only `main`, `setup`, and the wasm hooks).

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p bestiary --test bridge loads_bestiary_from_asset_server`
Expected: FAIL to compile (`Bestiary` undefined) or asset-not-loaded.

- [ ] **Step 3: Implement the model, loader, and registration** in `examples/bestiary/src/lib.rs` (+ thin `src/main.rs`, `Cargo.toml`, `build.rs`, the UI asset stubs, and `assets/data/bestiary.json`). Use the signatures in Interfaces; loader body mirrors `JsLoader` in `crates/superui/src/assets.rs` but deserializes JSON.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p bestiary --test bridge loads_bestiary_from_asset_server`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add examples/bestiary
git commit -m "feat(bestiary): load bestiary data as a Bevy JSON asset"
```

---

### Task 2: Bridge registration and the readiness handshake

**Files:**
- Modify: `examples/bestiary/src/lib.rs` (add `UiReady`, `BridgeState`, `on_ui_ready`, `push_bestiary`, `register_bridge`)
- Modify: `examples/bestiary/src/main.rs` (after `add_plugins(SuperUiPlugin)`: `register_bridge(&mut app)`, `init_resource::<BridgeState>()`, `add_systems(Update, push_bestiary)` — `register_bridge` must run after the plugin so the bridge registry resource exists)
- Test: `examples/bestiary/tests/bridge.rs` (add handshake tests)

**Interfaces:**
- Consumes: `Bestiary`, `BestiaryHandle` (Task 1).
- Produces:
  - `#[derive(Event, Deserialize)] struct UiReady;` (deserializes from JSON `null`)
  - `#[derive(Resource, Default)] struct BridgeState { ui_ready: bool, sent: bool }`
  - `fn on_ui_ready(_: On<UiReady>, state: ResMut<BridgeState>)` → sets `ui_ready = true`
  - `fn push_bestiary(state: ResMut<BridgeState>, handle: Res<BestiaryHandle>, assets: Res<Assets<Bestiary>>, commands: Commands)` → when `ui_ready && !sent`, if the asset is present, `commands.trigger(b.clone())` and set `sent = true`.
  - `fn register_bridge(app: &mut App)` → `app.add_superui_event::<Bestiary>("bestiary").add_superui_command::<UiReady>("uiReady").add_observer(on_ui_ready);` (`use superui::prelude::SuperUiApp;`)

- [ ] **Step 1: Write the failing tests** in `examples/bestiary/tests/bridge.rs`

```rust
// A test observer counts how many Bestiary events were triggered.
#[derive(Resource, Default)]
struct Seen(u32);

// ui_ready BEFORE the asset is available: push latches and fires once the
// asset loads, exactly once across repeated updates.
#[test]
fn pushes_once_when_ready_then_loaded() {
    let mut app = bridge_app();                  // Task-1 app + BridgeState + push_bestiary + Seen observer
    app.world_mut().resource_mut::<BridgeState>().ui_ready = true;
    pump_until_loaded(&mut app, /* the inserted BestiaryHandle */);
    for _ in 0..3 { app.update(); }
    assert_eq!(app.world().resource::<Seen>().0, 1);
}

// Asset present but UI not ready: nothing is pushed and nothing panics.
#[test]
fn does_not_push_before_ui_ready() {
    let mut app = bridge_app();
    pump_until_loaded(&mut app, /* handle */);
    for _ in 0..3 { app.update(); }
    assert_eq!(app.world().resource::<Seen>().0, 0);
    assert!(!app.world().resource::<BridgeState>().sent);
}

// on_ui_ready flips the flag when UiReady is triggered (the command path).
#[test]
fn ui_ready_observer_sets_flag() {
    let mut app = bridge_app();
    app.world_mut().trigger(UiReady);
    app.update();
    assert!(app.world().resource::<BridgeState>().ui_ready);
}
```

`bridge_app()` builds the Task-1 app, inserts `BestiaryHandle`, adds `BridgeState`, `push_bestiary` on `Update`, `add_observer(on_ui_ready)`, and an `add_observer` that increments `Seen` on `On<Bestiary>`. It wires the observers directly rather than calling `register_bridge`, so the handshake tests stay independent of `SuperUiPlugin` and its bridge registry. `register_bridge` (the JS-name mapping) is exercised end-to-end by the Task-3 spec and by `main`, not by these unit tests.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p bestiary --test bridge`
Expected: FAIL (`UiReady`/`BridgeState`/`push_bestiary` undefined).

- [ ] **Step 3: Implement** `UiReady`, `BridgeState`, `on_ui_ready`, `push_bestiary`, `register_bridge` in `src/lib.rs` per Interfaces; wire them in `src/main.rs`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p bestiary --test bridge`
Expected: PASS (all four tests)

- [ ] **Step 5: Commit**

```bash
git add examples/bestiary
git commit -m "feat(bestiary): push loaded data over the bridge via a ready handshake"
```

---

### Task 3: UI rendering and the JSON-import spec test

**Files:**
- Modify: `examples/bestiary/assets/ui/bestiary/app.tsx` (subscribe, announce ready, render cards)
- Modify: `examples/bestiary/assets/ui/bestiary/style.css` (card styling if not already final)
- Create: `examples/bestiary/superui.test.toml`
- Create: `examples/bestiary/tests/bestiary.spec.ts`

**Interfaces:**
- Consumes: the `"bestiary"` bridge event name and the `{ creatures: [...] }` payload shape (Tasks 1–2); `assets/data/bestiary.json` (Task 1).
- Produces: UI DOM contract used by the spec — one `.card` per creature, each with `#creature-<i>`, a `.name`, `.element`, and two `.stat` spans.

- [ ] **Step 1: Write `superui.test.toml`**

```toml
project = "assets/ui/bestiary"
specDir = "tests"
width = 900
height = 600
```

- [ ] **Step 2: Write the failing spec** `examples/bestiary/tests/bestiary.spec.ts`

```ts
import { test, expect } from "superui/test";
import bestiary from "../assets/data/bestiary.json";

test("no cards before data is pushed", async ({ page }) => {
  await expect(page.locator(".card")).toHaveCount(0);
});

test("renders every creature pushed over the bridge", async ({ page }) => {
  await page.emit("bestiary", bestiary);
  await expect(page.locator(".card")).toHaveCount(bestiary.creatures.length);
  await expect(page.locator("#creature-0 .name"))
    .toHaveText(bestiary.creatures[0].name);
  await expect(page.locator("#creature-0 .element"))
    .toHaveText(bestiary.creatures[0].element);
});
```

(Specs share one mount per file; "no cards before" must stay first — it is the only test expecting the pre-emit state.)

- [ ] **Step 3: Run the spec to verify it fails**

Run: `cd examples/bestiary && cargo run -p superui_test_engine --bin superui_test`
Expected: FAIL — the static `app.tsx` renders no cards and ignores the emit.

- [ ] **Step 4: Implement `app.tsx`** per the spec design: `createSignal([])`, `onMount(() => { bevy.on("bestiary", d => setCreatures(d.creatures)); bevy.send("uiReady", null); })`, and a `<For each={creatures()}>` producing the `.card` DOM contract above. Finalize `style.css` for the card grid.

- [ ] **Step 5: Run the spec to verify it passes**

Run: `cd examples/bestiary && cargo run -p superui_test_engine --bin superui_test`
Expected: PASS (both tests)

- [ ] **Step 6: Commit**

```bash
git add examples/bestiary
git commit -m "feat(bestiary): render creatures from bridge data and cover it with a json-import spec"
```

---

### Task 4: Publish — gallery entry, local demo wiring, README

**Files:**
- Modify: `examples/gallery.json` (append an Apps entry)
- Modify: `tools/build-demos.sh` (add `bestiary` to the local slug list)
- Create: `examples/bestiary/README.md`

**Interfaces:**
- Consumes: the finished crate (Tasks 1–3).
- Produces: a gallery card at `/examples/bestiary/` and a run/test walkthrough.

- [ ] **Step 1: Append the gallery entry** to `examples/gallery.json` under the existing **Apps** category:

```json
{ "slug": "bestiary", "package": "bestiary", "category": "Apps",
  "title": "Bestiary (JSON over the bridge)",
  "description": "One JSON asset drives the UI two ways: the game loads it through the Bevy AssetServer and pushes it over the bridge, while the test imports the same file and replays it with page.emit." }
```

- [ ] **Step 2: Verify the manifest still parses**

Run: `python -c "import json;json.load(open('examples/gallery.json'))"`
Expected: no output, exit 0.

- [ ] **Step 3: Add `bestiary` to `tools/build-demos.sh`** (the slug array used for local `mdbook serve`; match the surrounding style, no `build_args`).

- [ ] **Step 4: Write `examples/bestiary/README.md`** — what the example shows (single source-of-truth JSON → asset-load + bridge at runtime; import + `page.emit` in tests), the data flow, and the run/test commands (`cargo run -p bestiary`; `cd examples/bestiary && cargo run -p superui_test_engine --bin superui_test`). Use the technical-writing voice; no emoji.

- [ ] **Step 5: Verify the crate still builds and all tests pass**

Run: `cargo build -p bestiary && cargo test -p bestiary`
Expected: builds; all Rust tests pass. (wasm build is verified in CI per `CONTRIBUTING.md`; not run here.)

- [ ] **Step 6: Commit**

```bash
git add examples/gallery.json tools/build-demos.sh examples/bestiary/README.md
git commit -m "feat(bestiary): publish the example to the gallery"
```
