# Signals

Signals are the fundamental unit of state in supersolid. A signal holds a value,
and it remembers who read it — so when the value changes, exactly the parts of the
UI that depend on it update, and nothing else.

## Creating a signal

`createSignal` returns a `[get, set]` pair:

```typescript
import { createSignal } from "supersolid";

const [count, setCount] = createSignal(0);
```

- `count` is a **getter** — call it to read the current value: `count()`.
- `setCount` is a **setter** — call it to write a new value: `setCount(5)`.

The initial value is optional; `createSignal()` starts as `undefined`.

## Reading subscribes

Here is the idea the whole framework is built on: **reading a signal inside a
reactive scope subscribes that scope to the signal.** There is no separate
`.subscribe` call — the read *is* the subscription.

A "reactive scope" is anywhere the framework can re-run on its own: a JSX
expression, an [effect](effects.md), or a [memo](derived-state.md). When you write
`{count()}` in your markup, that specific spot reads `count`, so it becomes a
subscriber. When `count` changes, that spot re-runs — and only that spot.

```typescript
function Counter() {
  const [count, setCount] = createSignal(0);
  return (
    <div>
      <span>{count()}</span>                              {/* subscribes here */}
      <button onClick={() => setCount(count() + 1)}>+1</button>
    </div>
  );
}
```

Clicking the button calls `setCount`, which re-runs the `<span>`'s binding. The
component function itself does **not** re-run — see
[Components run once](components.md#components-run-once).

## Writing

Pass the setter a new value, or an **updater function** that receives the previous
value and returns the next one:

```typescript
setCount(5);                 // set to a specific value
setCount((n) => n + 1);      // derive from the previous value
```

The updater form is the safe way to increment or otherwise build on the current
value, since it doesn't depend on a possibly-stale read.

A write notifies subscribers when the value actually changes. Storing objects and
arrays is fine; replace them immutably so the new reference signals a change:

```typescript
const [todos, setTodos] = createSignal([]);

setTodos([...todos(), newTodo]);                       // add
setTodos(todos().filter((t) => t.id !== id));          // remove
setTodos(todos().map((t) =>                            // update one
  t.id === id ? { ...t, done: !t.done } : t));
```

## Read location matters

Because a component body runs once, *where* you read a signal decides whether the
read is reactive. Reading into a local variable at the top of the body captures
one snapshot:

```typescript
function Broken() {
  const [count] = createSignal(0);
  const value = count();          // ❌ runs once, never again
  return <span>{value}</span>;    //    frozen at 0
}
```

Read the signal at the point of use instead, so the read lives inside the binding
that should react:

```typescript
function Works() {
  const [count] = createSignal(0);
  return <span>{count()}</span>;  // ✅ read re-runs on change
}
```

The rule generalizes: keep signal reads as close as possible to where the value is
consumed. If you need a value derived from signals in more than one place, don't
cache it in a variable — compute it with a [memo](derived-state.md).

## Reading without subscribing

Occasionally you want a signal's current value without subscribing to it — for
example inside an effect that should react to *one* signal but merely *read*
another. Wrap the read in `untrack`:

```typescript
import { untrack } from "supersolid";

createEffect(() => {
  const c = count();                       // subscribes to count
  const t = untrack(() => theme());        // reads theme, does NOT subscribe
  console.log(c, t);
});
```

## Batching writes

Multiple writes normally each notify their subscribers. When you make several
writes together and want dependents to recompute only once, wrap them in `batch`:

```typescript
import { batch } from "supersolid";

batch(() => {
  setFirstName("Ada");
  setLastName("Lovelace");
}); // subscribers of both run once, after the batch
```

## Next

- [Effects](effects.md) — run side effects that react to signals.
- [Derived State](derived-state.md) — compute values from signals with memos.
