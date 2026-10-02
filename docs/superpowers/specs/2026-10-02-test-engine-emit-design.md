# Test-engine `page.emit` — drive the game→UI bridge from a spec

Date: 2026-10-02
Status: design approved, pre-implementation

## Problem

`superui_test_engine` mounts a `.tsx` UI headless with **no game side**. Nothing
registers `add_superui_command`/`add_superui_event` and no ECS system calls
`commands.trigger`, so the UI's `bevy.on(name, cb)` handlers never fire on their
own. Any UI that reacts to game→UI bridge events (the common `onMount(() =>
bevy.on("frame", …))` pattern) therefore cannot be exercised by a test.

`page.emit(name, value)` closes that gap: a spec plays the game's role and
delivers a named bridge event with a JSON payload, so the UI's `bevy.on`
handlers run and the DOM reacts.

A rough implementation of this already existed on the working tree; it has been
stashed so implementation starts from a clean tree. This spec supersedes it.

## Goal

A spec can write:

```typescript
import { test, expect } from "superui/test";

test("hud reflects a pushed frame", async ({ page }) => {
  await page.emit("frame", { player_hp: 7, player_max_hp: 10 });
  await expect(page.locator("#hp")).toHaveText("7 / 10");
});
```

Scope is the **game→UI direction only**. The reverse (asserting UI→game
`bevy.send`) is explicitly out of scope (see Non-goals).

## Background: the bridge path

`superui_bridge::install_bevy_bridge` runs during `UiRuntime` construction
(`runtime.rs:169`), so **every** mounted superui UI — including the headless
test host — has `window.bevy`, `bevy.on`, `bevy._emit`, and the
`globalThis.__ss_emit(name, value)` hook installed.

In a real app the game→UI leg is:

```
commands.trigger(T)                     // game
  → forward_event_observer::<T>         // serde_json::to_value(T)
  → INBOX
  → emit_bevy_inbox_system              // rt.engine.emit(name, json); rt.pump()
  → __ss_emit(name, value)              // JsEngine::emit's documented hook
  → bevy._emit(name, value)
  → every bevy.on(name, cb)
```

`JsEngine::emit(name, value)` is documented as "invoke the optional
`globalThis.__ss_emit(name, value)` hook"; it marshals the `serde_json::Value`
into native JS args and calls `__ss_emit`.

## Semantics: level B

`page.emit` delivers through the production ECS→JS leg — `JsEngine::emit` — i.e.
it enters the path at the `engine.emit` step above and reuses the exact
marshalling + `__ss_emit` seam a real app uses. It does **not** invoke
`world.trigger`, a typed `Event`, or the observer/serde serialization.

Three levels were considered:

| | Mechanism | Needs Rust event types? | Payload fidelity |
|---|---|---|---|
| A | string-eval `__ss_emit(...)` | no | hand-authored |
| **B (chosen)** | `engine.emit` → `__ss_emit` | no | hand-authored |
| C | `world.trigger(T)` → observer → serde | **yes** | guaranteed real |

- **C** is the only level that guarantees the payload shape matches the game's
  real `#[derive(Serialize)]` output, but it requires naming Rust event types.
  Specs are pure TypeScript and the headless host registers no event types by
  design, so C is unavailable without compiling game Rust into the test — which
  defeats the engine's UI-only model.
- **A vs. B** deliver an identical JS payload to `bevy.on`; they differ only in
  marshalling. B is chosen because its delivery leg *is* the production
  `engine.emit` function, so the test path cannot drift from production
  marshalling, and it removes the bespoke `emit_script` string-eval seam.

### Documented caveat

Because the spec hand-authors the payload JSON (true at both levels A and B),
it must match the shape the game's registered `Serialize` event actually
produces. `page.emit` verifies the UI's **reaction**, not the payload
**contract**. A future contract-level check (diffing a spec payload against the
game's real serialization) is out of scope.

## API surface

In `superui/test`, on the `page` object:

```typescript
/**
 * Deliver a game→UI bridge event to the UI's bevy.on(name, …) handlers, as the
 * running game would via commands.trigger. Lets a spec supply data the UI pulls
 * over the bridge; the headless host has no game side to send it.
 */
emit(name: string, value?: unknown): Promise<void>;
```

- `value` omitted or `undefined` is delivered as `null`.
- Resolves once the command is drained and delivered (one job-pump later). The
  DOM update the handler triggers is observed by a subsequent auto-waiting
  `expect`, mirroring the existing action/assert model.

The JS surface (`prelude.js`) enqueues an `{ type: "emit", name, value }`
command via the same `enqueue` path as every other `page` call.

## Implementation

Files, all in `crates/superui_test_engine/`:

1. **`command.rs`** — the `Emit { name: String, value: serde_json::Value }`
   variant of `Command` (serde tag `"emit"`), with a doc comment stating it
   delivers a game→UI event to `bevy.on` subscribers.

2. **`driver.rs`** (`run_one`, blocking loop) — the `Emit` arm calls
   `engine.emit` through the existing `with_engine` seam and resolves:

   ```rust
   Command::Emit { name, value } => {
       with_engine(app, |e| {
           e.emit(name, value);
           abi::resolve(e, q.id, r#"{"ok":true,"value":null}"#);
       });
   }
   ```

   Remove the `emit_script` helper.

3. **`ui_driver.rs`** (`step_running`, in-world stepper) — the `Emit` arm does
   the same against `world` via its `with_engine`. No dependency on
   `driver::emit_script` (deleted).

4. **`prelude.js`** — `globalThis.page.emit` enqueues the `emit` command,
   coercing `undefined` → `null`.

### Timing

Resolve immediately (next pump). The event fires in the command-drain step; the
frame's `app.update()` (blocking driver) / the next reconcile (stepper) applies
the `bevy.on` callback's signal writes; the spec's following `expect`
auto-waits. No explicit `rt.pump()` — consistent with how `click`/`fill`/`press`
already rely on the frame's reconcile. (Production's `emit_bevy_inbox_system`
pumps because it runs mid-frame and wants same-frame flush; the drivers get the
same effect from the per-iteration update.)

## Testing

New integration test `crates/superui_test_engine/tests/emit.rs`, built on the
`actions.rs` template (`build_headless_app` + inline TSX + `run_spec`):

1. **Scalar payload.** TSX: `onMount(() => bevy.on("score", s => setScore(s)))`
   rendering `#score`. Spec: `await page.emit("score", 42)` then
   `await expect(page.locator("#score")).toHaveText("42")`.
2. **Object payload.** Emit `{ player_hp: 7, player_max_hp: 10 }`; UI reads
   nested fields into `#hp` as `"7 / 10"`. Exercises JSON marshalling of a
   non-scalar through `engine.emit`.
3. **No listeners.** Emit a name no `bevy.on` subscribes to; assert the spec
   still passes (promise resolves, no error) and the DOM is unchanged.
4. **Undefined value.** `page.emit("ping")` (no value); a `bevy.on("ping", v =>
   …)` handler observes `null`.

ABI-level unit coverage in the `async_bridge.rs` style: a spec body that
`enqueue`s a raw `{type:"emit",name,value}` command drains to one `Command::Emit`
and resolves over the `JsEngine` boundary (no mount), guarding the
enqueue/drain/resolve wiring independent of a UI.

Run both drivers' paths where practical: the integration tests above cover
`run_spec` (blocking `driver`); the in-world stepper shares the `Emit` arm logic
and is covered by its existing `inworld_stepper.rs` harness if an emit case is
added there (nice-to-have, not required for parity since the arm is identical).

## Docs

1. **`crates/superui/superui-test.d.ts`** — restore the `emit` typing with the
   doc comment above. This is the primary developer-facing reference for the
   test API.
2. **`website/src/docs/concepts/bevy-bridge.md`** — a short "Testing the bridge"
   note under the game→UI section: in a `superui_test_engine` spec,
   `page.emit(name, value)` stands in for the game's `commands.trigger`,
   delivering a payload to `bevy.on` so the UI's reaction can be asserted. Note
   the hand-authored-payload caveat.
3. **Root `CHANGELOG.md`** (the website changelog `#include`s it) — add the
   entry under the correct unreleased/Since marker per the repo convention,
   applied via the `documenting-new-features` skill at write time.

No new website testing page: none exists today (the test engine is only
referenced from `project-structure.md`), and documenting the whole framework is
a separate, larger effort.

## Non-goals

- **Reverse direction** — asserting what the UI sent to the game via
  `bevy.send` (e.g. `page.waitForSend(name)` / `expect(page).toHaveSent(...)`).
  A separate future feature.
- **Payload-contract validation** (level C) — guaranteeing the spec payload
  matches the game's real serialization. Needs Rust event types in the spec,
  which the engine deliberately lacks.
- **A dedicated website testing guide.**
