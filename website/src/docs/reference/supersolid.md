# Supersolid framework API

**Legend:** ✅ supported today · 🟡 not supported yet, but planned · ⛔ won't be
supported. Tier T0–T3.

These globals are the Supersolid reactive framework. They are installed into every
runtime and are available to authored `.js` and to transpiled `.tsx` (whose
`import { … } from "solid-js"` is stripped in favour of these globals). They are a
separate layer from the browser DOM / Web API surface — see [JS / DOM](js-dom.md) for
that.

## Reactive core

| Global | Status | Since | Notes |
|---|---|---|---|
| `createSignal(v, {equals?})` → `[get, set]` | ✅ | T0 | fine-grained signal; updater-form set; `equals` gates notifications |
| `createEffect(fn, seed?)` | ✅ | T0 | tracks reads, re-runs on change; disposed with owner |
| `createMemo(fn, seed?, {equals?})` | ✅ | T0 | lazy, memoized derived value; is itself a source |
| `createRoot(fn => …)` | ✅ | T0 | disposable reactive scope |
| `onMount(fn)` / `onCleanup(fn)` | ✅ | T0 | run-once-after-setup / owner teardown |
| `createContext(default?)` / `useContext(ctx)` | ✅ | T0 | context via the owner tree |
| `untrack(fn)` / `batch(fn)` | ✅ | T0 | read without subscribing / coalesce writes |

## Render + control flow

| Global | Status | Since | Notes |
|---|---|---|---|
| `render(code, mountEl)` | ✅ | T0 | root entry; returns `dispose` |
| `<Show>` | ✅ | T0 | conditional; branch disposal via memo recompute |
| `<For>` | ✅ | T0 | keyed by item identity; per-row disposable roots; state preserved on reorder |
| `<Index>` | ✅ | T0 | keyed by position; item is an in-place-updated signal |
| `<Switch>` / `<Match>` | ✅ | T0 | first truthy branch, else fallback |

## State-preserving hot reload

When the `hmr` feature is enabled **and** the asset server is watching, a `.tsx`/`.js`
edit re-executes on the same engine and `render()` rehydrates each component's signal
cells (keyed by module × instance × creation-order), rebuilding the DOM fresh while
preserving values. A per-instance signal-shape change (add/remove a signal) resets that
instance. `<For>` rows preserve state by item identity, `<Index>` rows by position.
Off by default (feature off, or no watcher) — then `render()` takes its normal fast path.
