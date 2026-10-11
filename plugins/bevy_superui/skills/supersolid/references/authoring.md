# Authoring: components, signals, effects, memos, context, lifecycle

> Everything imports from `"supersolid"`.

supersolid is Solid-like. If you know Solid.js the mental model is identical; the notes
below are the practical surface plus superui-specific caveats.

## Components run once

A component is a plain function returning JSX. **It runs exactly once**, when created —
never re-run on state change. Only reactive expressions inside the returned markup re-run.
The body is *setup code*: create signals, derive values, define handlers, all once.

```tsx
function Counter() {
  const [count, setCount] = createSignal(0);          // setup, runs once
  return (
    <button onClick={() => setCount(count() + 1)}>    // handler defined once
      clicked {count()} times                          // {count()} re-runs on change
    </button>
  );
}
```

**Read reactive values inline, at point of use** — not into a top-level local (that
captures one snapshot and freezes). This applies to signals, memos, and `props`:

```tsx
const n = count();          // ❌ frozen
<span>{count()}</span>      // ✅ reactive
function Item(props) { return <li>{props.todo.title}</li>; }  // ✅ read props in markup
```

## Signals — `createSignal`

```tsx
const [count, setCount] = createSignal(0);   // [get, set]; initial optional (→ undefined)
count();                                      // read (subscribes the enclosing scope)
setCount(5);                                  // write a value
setCount((n) => n + 1);                       // updater form (safe increment)
createSignal(0, { equals: (a, b) => a === b });  // custom change-gate; false = always notify
```

- **Reading subscribes.** Reading a signal inside a reactive scope (a JSX binding, an
  effect, or a memo) subscribes that scope. No `.subscribe`. The read *is* the subscription.
- **Store arrays/objects immutably** so the new reference signals a change:

```tsx
setTodos([...todos(), newTodo]);                              // add
setTodos(todos().filter((t) => t.id !== id));                // remove
setTodos(todos().map((t) => t.id === id ? { ...t, done: !t.done } : t));  // update one
```

## Derived state — inline vs `createMemo`

- **Inline / small function** for cheap values used in one place: `<span>{count() * 2}</span>`
  or `const doubled = () => count() * 2;` then `{doubled()}`.
- **`createMemo(fn)`** when the derivation is expensive **or** used in several places. A
  memo is a cached read-only signal; recomputes only when its deps change; is itself a
  reactive source other memos/effects can depend on.

```tsx
const remaining = createMemo(() => todos().filter((t) => !t.done).length);
<span>{remaining()} left</span>
```

| Want | Use |
|---|---|
| cheap value, one place | inline expression |
| expensive or reused value | `createMemo` |
| a side effect | `createEffect` |

## Effects — `createEffect` (side effects only)

Runs immediately, tracks the signals it reads, re-runs when any change. For things that
reach *outside* the reactive graph: logging, `bevy.send`, timers, external sync. **Do not
use an effect to render** — a signal read in the markup already updates that spot.

```tsx
createEffect(() => { bevy.send("SetVolume", { level: volume() }); });  // ✅ push to ECS
createEffect(() => { label.textContent = count(); });                  // ❌ markup does this
```

Dependencies are dynamic: an effect depends on exactly what it read on its last run.

## Reading without subscribing / batching

```tsx
untrack(() => theme());        // read current value, do NOT subscribe
batch(() => { setX(1); setY(2); });  // coalesce writes; dependents recompute once
```

## Lifecycle — `onMount` / `onCleanup`

- `onMount(fn)` — runs once, right after the component's initial DOM exists (focus, start
  a timer, subscribe to a game event).
- `onCleanup(fn)` — teardown when the scope is disposed: component unmounts (`<Show>`
  flips off, `<Switch>` changes branch), a list row is removed, or the UI is torn down.
  Inside an effect, cleanup also runs before each re-run.

```tsx
function Clock() {
  const [now, setNow] = createSignal(Date.now());
  onMount(() => {
    const id = setInterval(() => setNow(Date.now()), 1000);
    onCleanup(() => clearInterval(id));   // pair every resource with a cleanup
  });
  return <span>{now()}</span>;
}
```

## Context — `createContext` / `useContext`

```tsx
const ThemeContext = createContext("dark");                 // default value
function Label() { const t = useContext(ThemeContext); return <span class={t}/>; }
<ThemeContext.Provider value="light"><Label/></ThemeContext.Provider>  // overrides subtree
```

`useContext` returns the nearest provided value, else the default. Providers nest.

## Sharing state without prop-drilling

A **signal declared at module scope** is a simple global store any component that
imports it (or shares the file) can read/write — components reading the same signal
stay in sync. Or lift state to a common ancestor and pass props. Both are idiomatic.

## Splitting a UI across modules

A UI can split components across sibling files — superui resolves, loads, and
bundles relative imports (`import { X } from "./other"`, extensionless or
`.tsx`/`.ts`/`.js`), plus bare `superui_modules/`-package imports (no package
manager populates that dir yet, so this is unused today). `export` whatever a
module needs — functions, constants. The only import that doesn't resolve to a
file on disk is `from "supersolid"` itself (the framework globals). The one
exception is `.json`: `import x from "./data.json"` (default/namespace/named
alike) inlines the whole parsed value as `const x`; picking a sub-path or
member out of the JSON is not supported. Out of scope, logged and skipped:
`export * from` re-export, dynamic `import()`, `import.meta`,
`package.json` `main`/`module`-field resolution.
