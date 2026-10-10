# Components & JSX

A component is the basic building block of a superui interface. It is a plain
function that returns markup. You compose components to build a UI, and you use
them by writing them as tags.

```typescript
function HelloWorld() {
  return <div class="greeting">Hello, world</div>;
}

// use it like a tag:
render(() => <HelloWorld />, document.getElementById("root"));
```

## Components run once

This is the single most important thing to understand about supersolid, and it is
what makes the rest of the reactivity guide make sense.

**A component function runs exactly once** — when it is first created. It is not
re-run when state changes. Instead, the reactive expressions *inside* the returned
markup are wired up individually, and only those re-run when their data changes.

That means the body of a component is *setup code*. Create your signals, derive
your values, define your handlers — all of it runs a single time. Anything that
needs to stay live must be a reactive expression the framework can re-evaluate on
its own.

```typescript
function Counter() {
  // setup: runs once
  const [count, setCount] = createSignal(0);

  return (
    // {count()} is a reactive binding — it, not the function, re-runs
    <button onClick={() => setCount(count() + 1)}>
      clicked {count()} times
    </button>
  );
}
```

### Keep reactive reads inline

Because the body runs once, reading a signal into a plain variable at the top
captures a single snapshot that never updates:

```typescript
function Broken() {
  const [count] = createSignal(0);
  const now = count();          // ❌ read once, frozen forever
  return <span>{now}</span>;    //    always shows 0
}
```

Read the signal *where the value is used*, so the read happens inside a reactive
binding:

```typescript
function Works() {
  const [count] = createSignal(0);
  return <span>{count()}</span>; // ✅ re-runs when count changes
}
```

See [Signals](signals.md) for why the *location* of the read is what matters.

## JSX

The markup you return is JSX — an HTML-like syntax embedded in your code. superui
renders it into a browser-like DOM backed by `bevy_ui`, so the tags are ordinary
elements (`div`, `span`, `button`, `ul`, `li`, `input`, …), not a fixed component
set.

### Expressions

Anything in `{…}` is a JavaScript expression:

```typescript
<span>{user().name}</span>
<div>{2 + 2}</div>
```

When an expression reads a signal, that spot in the DOM becomes a live binding and
updates on its own.

### Attributes

Attributes take a string literal or an expression. `class` sets the CSS class;
`style` takes an inline style string:

```typescript
<li class={todo.done ? "todo completed" : "todo"}>
  {todo.title}
</li>

<div class="bar" style={`width: ${percent()}%`}></div>
```

For the DOM and CSS surface superui supports, see the
[JS / DOM](../reference/js-dom.md) and [CSS](../reference/css.md) references.

### Event handlers

`on*` attributes attach event handlers. The handler is a function:

```typescript
<button onClick={() => setOpen(true)}>Open</button>
<input onInput={(e) => setDraft(e.target.value)} />
<input type="checkbox" onChange={() => toggle()} />
```

Common events include `onClick`, `onInput`, `onChange`, `onKeyDown`, and
`onKeyUp`. The event object exposes `target`, `key`, and the standard propagation
controls — see [Events](../reference/js-dom.md#events).

## Props

A component receives its attributes as a single `props` object:

```typescript
function TodoItem(props) {
  return (
    <li class={props.todo.done ? "todo completed" : "todo"}>
      <span class="label">{props.todo.title}</span>
      <button class="destroy" onClick={() => props.onRemove(props.todo.id)}>x</button>
    </li>
  );
}

// pass props as attributes:
<TodoItem todo={item} onRemove={remove} />
```

Props carry anything — values, objects, and callbacks (`onRemove` above) that let
a child ask its parent to do something.

> **Read props where you use them.** Just like signals, reactive props should be
> read inside the markup (`{props.todo.title}`), not destructured into locals at
> the top of the body — destructuring reads once and loses reactivity.

## Composition

Components nest freely. Break a screen into small, focused components and assemble
them:

```typescript
function App() {
  const [todos, setTodos] = createSignal([]);
  return (
    <div id="app">
      <h1>todos</h1>
      <Header onAdd={(t) => setTodos([...todos(), t])} />
      <ul>
        <For each={todos()}>
          {(todo) => <TodoItem todo={todo} />}
        </For>
      </ul>
    </div>
  );
}
```

`<For>` and the other list/branch helpers are covered in
[Control Flow](control-flow.md).

## One module, no cross-file imports

superui compiles each UI into a **single module**, and its transpiler strips
imports between your own source files. In practice: put every component for one UI
in that UI's `app.tsx`, as plain functions in one file. The only import you keep
is the framework itself:

```typescript
import { createSignal, createMemo, For, Show, render } from "supersolid";
```

Everything else — your components, types, helpers — just lives in the same file
and refers to each other directly.

### Importing JSON

<div class="since-note"><strong>0.3.6</strong></div>

The one exception to "no cross-file imports" is `.json`. `import x from
"./data.json"` inlines the file's parsed value as a top-level `const x`,
available to the rest of the module — `import * as x` and `import { x }` bind
the same whole value under `x`. The specifier resolves relative to the
importing file's own directory. Only the whole value is bound: picking a
sub-path or named member out of the JSON at import time is not supported.
Non-JSON cross-file imports (`.tsx`, `.ts`, …) are still stripped, as above.

```typescript
import skills from "./skills.json";

function SkillList() {
  return <For each={skills}>{(s) => <li>{s.name}</li>}</For>;
}
```

A missing or invalid JSON file fails the whole transpile when run through
`cargo superui test` (see [Testing](../reference/testing.md)); in the running
app — including hot reload — and at build time it logs a warning and skips
that binding instead. Editing the JSON file hot-reloads the `.tsx` that
imports it, the same as editing the `.tsx` itself.

## Next

- [Signals](signals.md) — reactive state, and why read location matters.
- [Control Flow](control-flow.md) — rendering lists and conditional branches.
