# Effects

An **effect** runs code in response to change. Where a [signal](signals.md) holds
state and the markup renders it, an effect is for *side effects* — things that
reach outside the reactive graph: logging, talking to the Bevy world, starting a
timer, syncing to some external state.

## createEffect

`createEffect` takes a function. It runs the function once immediately, tracking
every signal it reads, and re-runs it whenever any of those signals changes.

```typescript
import { createSignal, createEffect } from "supersolid";

const [count, setCount] = createSignal(0);

createEffect(() => {
  console.log("count is", count());   // reads count → subscribes
});
// logs "count is 0" right away, then again after every setCount
```

The subscription is automatic and dynamic: an effect depends on exactly the
signals it read *on its last run*. Read three signals and it reacts to all three;
read one conditionally and its dependencies change with the condition.

## Effects are for side effects, not UI

If your goal is to put a value on screen, you do **not** need an effect — a signal
read in the markup already updates that spot on its own:

```typescript
// ✅ no effect needed
<span>{count()}</span>

// ❌ don't do this to render — the binding above already handles it
createEffect(() => { someLabel.textContent = count(); });
```

Reach for an effect when the reaction is something the render tree can't express
on its own. A common case in a game UI is pushing state into the ECS when it
changes:

```typescript
createEffect(() => {
  bevy.send("SetVolume", { level: volume() }); // send to the game on change
});
```

See [The Bevy Bridge](bevy-bridge.md) for `bevy.send`.

## Cleanup

An effect that owns a resource — a timer, a subscription — should release it when
the effect re-runs or when its owner is disposed. Register teardown with
`onCleanup` *inside* the effect:

```typescript
import { createEffect, onCleanup } from "supersolid";

createEffect(() => {
  const id = setInterval(() => tick(), 1000 / speed());
  onCleanup(() => clearInterval(id)); // runs before the next run and on dispose
});
```

Each time `speed()` changes, the effect re-runs: cleanup fires first (clearing the
old interval), then the body sets up a fresh one. When the component unmounts, the
final cleanup fires. See [Lifecycle](lifecycle.md) for more on `onCleanup`.

## Reading without subscribing

Sometimes an effect should react to one signal but only *read* another without
subscribing to it. Wrap the read you want to ignore in `untrack`:

```typescript
import { untrack } from "supersolid";

createEffect(() => {
  save(document(), untrack(() => cursor())); // re-runs on document, not cursor
});
```

## Batching

If an effect (or a handler) writes several signals, wrap them in `batch` so
dependents recompute once instead of once per write:

```typescript
import { batch } from "supersolid";

batch(() => {
  setX(10);
  setY(20);
});
```

## Next

- [Derived State](derived-state.md) — when you want a *value* out, not a side
  effect, use a memo instead of an effect.
- [Lifecycle](lifecycle.md) — `onMount` and `onCleanup`.
