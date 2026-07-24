# Introduction

superui is a Bevy plugin that gives you a browser-like environment for building
game UI. You author interfaces the way you would for the web — an `index.html`,
a stylesheet, and components — and superui renders them with `bevy_ui`,
styles them with a CSS engine (a modified `bevy_flair`), and runs their logic in
an embedded JavaScript engine.

On top of that browser-like base sits **supersolid**: a reactive `.tsx`
authoring layer. You write components as plain functions that return markup, hold
state in signals, and let the framework update only the parts of the UI that
actually changed. This is the recommended way to build UI with superui, and it is
what this documentation focuses on.

```typescript
import { createSignal, render } from "supersolid";

function Counter() {
  const [count, setCount] = createSignal(0);
  return (
    <button class="counter" onClick={() => setCount(count() + 1)}>
      clicked {count()} times
    </button>
  );
}

render(() => <Counter />, document.getElementById("root"));
```

## Why superui

- **Familiar model.** Markup, CSS, and a reactive component layer — the concepts
  and much of the API surface deliberately mirror what you already know from the
  web, so existing UI knowledge carries over.
- **Fast iteration.** With hot reload enabled, editing a `.tsx` file updates the
  running game's UI in place — and preserves component state while it does.
- **Made for games.** UI talks to your Bevy world through a small, typed bridge
  (`bevy.send` / `bevy.on`), so buttons fire ECS events and live game data flows
  back into the interface.

## How the pieces fit

| Layer | What it is |
|---|---|
| **superui** | The framework: the browser-like HTML/CSS/JS environment and the Bevy plugin that hosts it. |
| **supersolid** | The reactive `.tsx` layer you author components in — signals, effects, control flow, and rendering. |
| **The Bevy bridge** | `bevy.send` / `bevy.on`, the channel between your UI and the ECS. |

## Where to go next

- [Getting Started](getting-started.md) — add the plugin and mount your first UI.
- [Project Structure & Build](project-structure.md) — the UI asset layout, editor
  setup, hot reload, and web builds.
- [Concepts](concepts/components.md) — how components, signals, and reactivity work.

## Status

This is in very early development. Several working examples already run — see the
[gallery](../examples/). The code is largely AI-generated and not yet fully
reviewed; APIs are expected to be in flux, though the surface deliberately mirrors
familiar web APIs. Use at your own risk.
