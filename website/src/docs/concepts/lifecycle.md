# Lifecycle

Components and reactive scopes have a lifecycle: they are created, they live, and
they are disposed. supersolid gives you two hooks to run code at the edges —
`onMount` when something appears, and `onCleanup` when it goes away.

## onMount

`onMount` registers a callback that runs once, right after the component's initial
render. Use it for setup that should happen exactly once when the component
appears — subscribing to a game event, kicking off a timer, focusing an element.

```typescript
import { onMount } from "supersolid";

function Hud() {
  onMount(() => {
    console.log("HUD mounted");
    bevy.send("HudReady");   // tell the game the UI is up
  });
  return <div id="hud">…</div>;
}
```

Because a component body already runs once, `onMount` is mostly about *timing*:
its callback runs after the initial DOM for the component exists, which is the
right moment for anything that needs the rendered node to be present.

## onCleanup

`onCleanup` registers teardown that runs when the enclosing reactive scope is
disposed. A scope is disposed when:

- its component **unmounts** (e.g. a `<Show>` condition flips it off, or a
  `<Switch>` moves to another branch), or
- a **list row is removed** (`<For>`/`<Index>`), or
- the whole UI is torn down.

```typescript
import { onMount, onCleanup } from "supersolid";

function Clock() {
  const [now, setNow] = createSignal(Date.now());
  onMount(() => {
    const id = setInterval(() => setNow(Date.now()), 1000);
    onCleanup(() => clearInterval(id)); // stop the timer when Clock goes away
  });
  return <span>{now()}</span>;
}
```

Pair every subscription or resource with an `onCleanup` so it's released when the
component disappears — otherwise timers keep firing and event handlers keep running
against a component that's no longer on screen.

## Cleanup inside effects

`onCleanup` also works inside an [effect](effects.md), where it runs before each
re-run as well as on final disposal. This is how an effect that owns a
re-creatable resource keeps it in sync:

```typescript
createEffect(() => {
  const handle = subscribe(channel());
  onCleanup(() => handle.close()); // close old before opening new, and on dispose
});
```

Each time `channel()` changes: cleanup closes the previous handle, then the body
opens a new one. See [Effects → Cleanup](effects.md#cleanup).

## Disposal and hot reload

Disposal is also what makes state-preserving [hot reload](../project-structure.md#hot-reload)
correct: on reload the old tree is disposed (so cleanups run and resources are
released) before the new tree is built and rehydrated with the preserved values.

## Next

- [Context](context.md) — share values down the component tree.
- [The Bevy Bridge](bevy-bridge.md) — subscribe to game events (a common
  `onMount` / `onCleanup` pairing).
