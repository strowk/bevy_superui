---
name: supersolid
description: Use when writing or editing superui game UI — .tsx/.jsx components,
  createSignal/createEffect/createMemo, control flow (Show/For/Index/Keyed/Switch),
  styling a superui app, or wiring the Bevy bridge (bevy.send/bevy.on). superui is a
  Bevy plugin with a browser-like HTML/CSS/JS environment; supersolid is its reactive
  Solid-like .tsx layer. Its CSS/HTML/JS surface is a SUBSET — check it before use.
---

# supersolid (superui game UI)

**superui** is a Bevy plugin that renders HTML/CSS/JS with `bevy_ui`. **supersolid**
is its reactive `.tsx` layer — Solid-like, but running in an embedded JS engine, not a
browser. The API deliberately mirrors [Solid.js](https://www.solidjs.com/), so most
Solid knowledge transfers, with the differences below.

## The one rule that explains everything

**A component function runs exactly once**, when created. It is *not* re-run on state
change. Only the reactive expressions *inside* the returned markup re-run. So:

- **Read signals inline, where the value is used** — never into a top-level local.

```tsx
function Broken() {
  const [count] = createSignal(0);
  const n = count();              // ❌ read once, frozen at 0 forever
  return <span>{n}</span>;
}
function Works() {
  const [count] = createSignal(0);
  return <span>{count()}</span>;  // ✅ this binding re-runs when count changes
}
```

The same applies to `props`: read `props.x` in the markup, don't destructure at the top.

## Core API (all from `"supersolid"`)

```tsx
import { createSignal, createMemo, createEffect, Show, For, render } from "supersolid";
```

- `createSignal(v)` → `[get, set]`. Read `count()`, write `setCount(5)` or
  `setCount(n => n + 1)`. Replace arrays/objects **immutably** so the reference changes.
- `createMemo(fn)` — cached derived value; use when derivation is expensive or reused.
- `createEffect(fn)` — **side effects only** (logging, `bevy.send`, timers). Never use an
  effect to put a value on screen; a signal read in the markup already does that.
- `createContext`/`useContext`, `onMount`/`onCleanup`, `untrack`, `batch` — as in Solid.
- `render(() => <App/>, document.getElementById("root"))` — the mount entry.

Full reactivity guide → `references/authoring.md`.

## Control flow, not `if`/`for`

Because components run once, use the control-flow components (they stay reactive). They
can be placed directly as element children:

```tsx
<ul>
  <For each={todos()}>{(todo) => <li>{todo.title}</li>}</For>
</ul>
<Show when={open()}><Menu/></Show>
```

`<Show>` conditional · `<For>` list keyed by item identity · `<Index>` keyed by position
· `<Keyed>` high-frequency per-entity feeds (nameplates, damage numbers) · `<Switch>`/
`<Match>` first truthy branch. Which to pick → `references/control-flow.md`.

## One module, no cross-file imports

superui compiles each UI into a **single module** and strips imports between your own
files. Put every component for one UI in its `app.tsx`; the only import you keep is
`from "supersolid"`. No `import { X } from "./other.tsx"`.

## The support surface is a SUBSET — check before you use it

This is **not** a browser. Unknown CSS properties, HTML tags, and Web APIs are **silently
ignored** (no error), so unsupported code fails quietly. Before reaching for a CSS
property, HTML element, or JS/DOM API, confirm it in the ledger. High-frequency gotchas:

- **CSS:** `border: 1px #ccc` (NOT `1px solid` — no style keyword). No `opacity`,
  `visibility` (use `display: none`), `cursor`, `rem`/`em`, bold/italic yet. `transform`
  is 2D only, functions in order `translate scale rotate`. `calc()` single-unit only.
  Layout is flex/grid. Full table → `references/css.md`.
- **HTML:** only `<input type=text>` and `type=checkbox`; text editing is append +
  backspace-at-end only. No `<img>`, `<svg>`, `<select>` yet. Tags are flex boxes (no
  list markers, no inline bold). Full table → `references/html-dom.md`.
- **JS/DOM:** no `fetch`, `localStorage`, `alert`. `console.debug/table/group` **throw** —
  use `console.log/warn/error`. `setInterval`/`setTimeout` work (driven by Bevy's clock).
  `requestAnimationFrame` not yet. Full table → `references/html-dom.md`.

## Talking to the game: the Bevy bridge

`bevy` is a global (`window.bevy`). UI→game with `bevy.send`, game→UI with `bevy.on`.
The idiomatic pattern: land incoming payloads in a signal (set up in `onMount`) so the
rest of the UI reacts:

```tsx
onMount(() => { bevy.on("frame", (f) => setFrame(f)); });          // game → UI
<button onClick={() => bevy.send("HordeIntent", { kind: "Start" })}>Start</button>
```

Names must be registered on the Rust side (`add_superui_command`/`add_superui_event`).
Both directions, the Rust registration, observers, and mounting (`SuperUiPlugin`,
`SuperUiRoot`) → `references/bevy-bridge.md`.

## Building & running

`cargo run --features hmr` = live `.tsx` with state-preserving hot reload (dev). Plain
`cargo run` and wasm builds need pre-transpiled JS via a `build.rs`. Project layout
(`index.html` manifest, `style.css`), editor types (`cargo superui install`), and build
modes → `references/project-setup.md`.

## Reference files

| File | Covers |
|---|---|
| `references/authoring.md` | components, signals, effects, memos, context, lifecycle |
| `references/control-flow.md` | Show / For / Index / Keyed / Switch — and which to pick |
| `references/css.md` | CSS property / selector / value / at-rule support ledger |
| `references/html-dom.md` | HTML elements + JS/DOM/Web API ledger + reserved globals |
| `references/bevy-bridge.md` | bevy.send / bevy.on + the Rust side + the full loop |
| `references/project-setup.md` | project layout, build modes, hot reload, editor setup |
