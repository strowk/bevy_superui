# Context

Context lets a value be read deep in the component tree without threading it
through every layer of props. supersolid provides the two primitives you'd
expect: `createContext` to define a context, and `useContext` to read it.

## Defining and reading

`createContext` takes a default value and returns a context handle. `useContext`
reads it from within a component:

```typescript
import { createContext, useContext } from "supersolid";

const ThemeContext = createContext("dark");

function Label() {
  const theme = useContext(ThemeContext); // "dark" (the default) unless provided
  return <span class={`label ${theme}`}>Ready</span>;
}
```

`useContext` looks up the nearest value provided for that context in the current
scope, and falls back to the context's default when none has been provided.

> **Providing values is still evolving.** A declarative provider component for
> overriding a context's value for a subtree is on the roadmap. Today `useContext`
> resolves to the context's **default value**, which makes context useful right
> now for app-wide constants and defaults.

## Sharing state today

Because a superui UI compiles to a [single module](components.md#one-module-no-cross-file-imports),
the practical ways to share state across components are straightforward:

- **Lift state to a common ancestor and pass it down as props.** This is the
  default approach and keeps data flow explicit:

  ```typescript
  function App() {
    const [todos, setTodos] = createSignal([]);
    return (
      <div>
        <List todos={todos()} />
        <Footer remaining={todos().filter((t) => !t.done).length} />
      </div>
    );
  }
  ```

- **Hold shared signals at module scope.** Since everything lives in one module,
  a signal declared at the top of the file is reachable from any component in that
  UI — a simple global store:

  ```typescript
  const [volume, setVolume] = createSignal(0.8);

  function Slider() {
    return <input value={volume()} onInput={(e) => setVolume(+e.target.value)} />;
  }
  function VolumeReadout() {
    return <span>{Math.round(volume() * 100)}%</span>; // reacts to the same signal
  }
  ```

  Both components read the same signal, so they stay in sync automatically.

## Next

- [Signals](signals.md) — the state a module-scope store is built from.
- [The Bevy Bridge](bevy-bridge.md) — share state with the game world, not just
  within the UI.
