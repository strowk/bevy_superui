# Derived State

Most values in a UI aren't stored directly — they're *computed* from other state:
the number of items left, a filtered list, a formatted label. In supersolid you
derive these values, and the framework keeps them up to date automatically.

## Deriving inline

The simplest derived value is just an expression that reads signals. Because a
JSX binding re-runs when its signals change, you can compute right in the markup:

```typescript
const [count, setCount] = createSignal(0);

<span>{count() * 2}</span>          {/* updates whenever count changes */}
```

Or in a small function that reads signals when called:

```typescript
const doubled = () => count() * 2;

<span>{doubled()}</span>            {/* doubled() reads count → reactive */}
```

This is perfect for cheap computations used in one place. Each place that calls
`doubled()` re-runs its own read when `count` changes.

## createMemo

When a derivation is **expensive**, or its result is used in **several places**,
compute it once with `createMemo`. A memo is a read-only signal whose value is
recomputed when its dependencies change — and cached in between.

```typescript
import { createSignal, createMemo } from "supersolid";

const [todos, setTodos] = createSignal([]);
const [filter, setFilter] = createSignal("all");

const remaining = createMemo(() => todos().filter((t) => !t.done).length);

const filtered = createMemo(() => {
  const f = filter();
  return todos().filter((t) =>
    f === "all" ? true : f === "active" ? !t.done : t.done);
});
```

Read a memo like any signal — call it:

```typescript
<span>{remaining()} items left</span>

<For each={filtered()}>
  {(todo) => <TodoItem todo={todo} />}
</For>
```

`filtered` recomputes only when `todos` or `filter` changes. If ten bindings read
`filtered()`, the filtering work still happens once; the ten readers share the
cached result.

## Memo vs. inline vs. effect

| You want… | Use |
|---|---|
| A value used once, cheap to compute | an inline expression (`{count() * 2}`) |
| A value used in several places, or expensive to compute | `createMemo` |
| A *side effect* (log, send to ECS, start a timer) | [`createEffect`](effects.md) |

The dividing line: a **memo returns a value** and should be pure (no side
effects); an **effect returns nothing** and exists for its side effects. If you
find yourself writing an effect just to stash a computed value in another signal,
reach for a memo instead.

## Memos are reactive sources too

A memo is itself a signal, so other memos and effects can depend on it, and it
only propagates a change when its result actually differs. This lets you build
small chains of derivations that each recompute exactly when needed:

```typescript
const activeCount = createMemo(() => filtered().length);
// recomputes only when `filtered` produces a new value
```

## Next

- [Control Flow](control-flow.md) — render the lists and branches your derived
  values feed.
- [Effects](effects.md) — for reactions that aren't about producing a value.
