# Preserve module-scope signals across HMR — design

Date: 2026-10-16
Status: approved for planning
Ticket: bevy-superui_3

## Problem

supersolid's state-preserving hot reload keeps the values of signals created
**inside a component render**, but drops every signal created at **module top
level**. A UI whose state lives in module singletons therefore loses all of it
on any `.tsx`/`.ts` edit under `--features hmr`.

Minimal repro: a module does

```tsx
const s = createSignal("menu");
export const screen = s[0];
```

the app reads `screen()` to pick its view, the developer edits any component's
markup and saves. The log shows `Reloaded <entry>.tsx`, the UI re-execs, and
`screen` is back at `"menu"` — every top-level signal resets with it. The DOM
edit is preserved; the run state is not, which defeats the point of
state-preserving reload for this class of app.

### Root cause

`crates/supersolid_runtime/src/render.js` collects signals for rehydration only
while a component **instance frame** is open:

```js
function onSignal(read, write) {
  var f = frameStack[frameStack.length - 1];
  if (f) f.cells.push({ read: read, write: write });
}
```

`withInstance` opens such a frame keyed by tree position; `commitRehydration`
restores a matched frame's values at frame close when the signal count matches.
A module-top-level `createSignal` runs with **no frame on the stack**, so it is
never snapshotted and is reconstructed fresh at its initial value on re-exec.
The one shape the collector preserves (component-scoped signals) is the one a
module-singleton app never uses.

## Scope

In scope:

- Preserve module-top-level signals across HMR re-exec, in **both** code shapes:
  the single-file entry and every imported module of a multi-file bundle, each
  keyed by its own module path.
- **On by default** whenever `__ssHmr` is on, consistent with how
  component-scoped signals already behave. No new authoring syntax, no opt-in.
- Reuse the existing count-match guard so an edit that adds or removes a
  top-level signal resets **only that module**, mirroring the component rule.

Out of scope:

- Signals whose `createSignal` executes **after** the module's top-level run
  (event handlers, timers, promises). See [Boundaries](#boundaries).
- Non-reactive module state (`let count = 0`); never in scope for
  state-preserving HMR.
- Any opt-in / per-module directive mechanism (rejected in favor of
  on-by-default).

## Key facts that shape the design

1. **Both code shapes share one execution bracket.** `supersolid::transpile`
   (single file) is a one-module wrapper over `transpile_module` + `assemble`,
   producing the same `$ssmod` bundle as a multi-file graph. Every module's
   top-level code therefore runs through the prelude's `req(id)` wrapping
   `m.factory(exports, module, require)`. That one call site is a uniform seam
   for **all** top-level execution — no per-shape hook points are needed.

2. **Module top-level code runs *during* re-exec**, before `render()` for the
   entry module and earlier still for its imports. So old values cannot be
   captured by `render()`'s per-call snapshot; they must already be held in
   state that survives `run_script`.

3. **`render.js` persists across `run_script`.** It is loaded once into the
   engine; its module-level state is the persistence anchor, exactly as the
   existing `roots` Map is for components. Module-top-level signals have no
   owner and are never disposed, so the previous execution's signals remain
   **live** and readable until we drop our reference to them.

## Approach

Bracket the `m.factory(...)` call in the `$ssmod` prelude with a module-frame
enter/exit, implemented in `render.js` against a render.js-retained per-module
store. Chosen over (a) transpiler-injected per-body brackets — materially more
codegen for no gain now that both shapes share the prelude — and (b) a single
global frame per re-exec — which would reset every module when any one module's
signal count changed, violating the per-module reset rule.

### Data model (render.js)

Two new pieces of persistent state, kept **separate** from the existing
`frameStack`/`roots` so component rehydration is provably undisturbed:

- `var moduleStack = []` — module frames open during the current (re-)exec.
  Distinct from `frameStack` (component/row frames), so `withInstance`,
  `indexArray`, and component instance keys are untouched.
- `var moduleFrames = new Map()` — `modulePath -> last committed frame`, where a
  frame is `{ path, cells: [{read, write}, …] }`. This is the cross-reload
  anchor holding the previous execution's top-level signals.

### Lifecycle of one module's top-level execution

```
modEnter(path)   push {path, cells:[]} onto moduleStack
  … factory body runs …
    createSignal → $ssOnSignal → collects into the module frame
                   (only when no component frame is open)
    require(dep)  → nested modEnter/modExit (stack nests)
    render(...)   → unchanged; its component frames live on frameStack
modExit(path)    pop frame; rehydrate from moduleFrames.get(path) when the
                 counts match; then moduleFrames.set(path, frame)
```

On a **fresh mount**, `moduleFrames` is empty → no rehydration → fresh defaults
(identical to today). On a **reload**, the prior frame supplies preserved
values.

## Runtime changes (`crates/supersolid_runtime/src/render.js`)

### Signal collection — `$ssOnSignal` fallback

`onSignal` gains a module-stack fallback, checked **only** when no component
frame is open:

```js
function onSignal(read, write) {
  var f = frameStack[frameStack.length - 1];
  if (f) { f.cells.push({ read: read, write: write }); return; }   // component/row signal — unchanged
  var m = moduleStack[moduleStack.length - 1];
  if (m) m.cells.push({ read: read, write: write });               // module-top-level signal
}
```

The `frameStack`-first order is what keeps component signals out of the module
frame: when `render()` runs inside a module factory, a component frame sits on
`frameStack` while the module frame sits on `moduleStack`, so a signal created
inside a component collects into the component frame as today. Only signals
created while **no** component frame is open — true top-level ones — land in the
module frame.

### Enter / exit

```js
function modEnter(path) {
  if (!hmrOn()) return;
  moduleStack.push({ path: path, cells: [] });
}

function modExit(path) {
  if (!hmrOn()) return;
  var frame = moduleStack.pop();
  var prev = moduleFrames.get(path);
  if (prev && prev.cells.length === frame.cells.length) {
    for (var i = 0; i < prev.cells.length; i++) {
      var v = untrack(prev.cells[i].read);           // old value, still live
      (function (val, cell) {                          // updater form: sets even a function value
        cell.write(function () { return val; });
      })(v, frame.cells[i]);
    }
  }
  moduleFrames.set(path, frame);                       // new frame anchors the next reload
}
```

Three properties:

- **Rehydrate reads the old frame, writes the new cells.** At `modExit` the
  previous frame's signals still hold the user's last values; the new factory
  just created fresh signals at their initial values. Copy old→new, then the old
  frame is dropped from the map and becomes garbage. No separate snapshot pass
  is needed — the old values are still live at exit time, unlike components,
  which `render()` must snapshot before disposing their owner.
- **Count-match guard = scoped reset.** A module whose top-level signal count
  changed skips rehydration and keeps fresh defaults, resetting exactly that
  module. Same `untrack` + updater-form write already used by
  `commitRehydration`.
- **`path` is the transpiler-supplied module id** (`id` in `$ssmod.def(id, …)`),
  the same input that already feeds `$ss.hot`.

The mechanism is independent of `currentRoot` and `render()` — a stateful logic
module that never renders is still preserved.

### ABI publication

Publish `modEnter`/`modExit` on the `$ss` object alongside `el`, `cmp`, `hot`,
etc., so the prelude can reference them.

## Transpiler change (`crates/supersolid/src/module.rs`)

A single edit to `SSMOD_PRELUDE`: wrap the existing `m.factory(...)` call in
`req()`.

Current:

```js
m.factory(module.exports, module, require);
```

becomes:

```js
if (typeof $ss !== "undefined" && $ss.modEnter) $ss.modEnter(id);
try {
  m.factory(module.exports, module, require);
} finally {
  if (typeof $ss !== "undefined" && $ss.modExit) $ss.modExit(id);
}
```

- **`try/finally`** keeps `moduleStack` balanced even if a factory throws, so one
  bad module cannot corrupt the stack for the rest of the bundle.
- **Defensive guard.** `module.rs` is the engine-agnostic module loader; the
  `typeof $ss !== "undefined" && $ss.modEnter` guard preserves that decoupling —
  a bundle executed without the render layer skips the bracket instead of
  throwing. In the real runtime `render.js` is always installed before the user
  bundle, so the guard is belt-and-suspenders. ES5-style (no optional chaining),
  matching the prelude's Boa/ES5 target.
- **`id`** is already the first argument to `def` / the key in `mods`, in scope
  in `req`. No new plumbing.
- **No `jsx.rs` change.** Module-body codegen is untouched; only the fixed
  prelude string changes.

## Gating and shipped builds

Everything keys off the existing `hmrOn()` (`!!globalThis.__ssHmr`):
`modEnter`/`modExit` return immediately when the gate is off, so no module stack
is built, nothing is collected, and `moduleFrames` stays empty. The gate-off
path and wasm/shipped builds behave exactly as today — the only cost is two
`typeof`/property checks per module execution (once per module at startup, not
per frame). The `hmr` cargo feature + watcher gate that already fronts component
HMR fronts this too; no new feature flag.

## Determinism

Per-module top-level creation order is deterministic across exactly the edits
HMR is meant to survive — markup and handler edits — because those do not alter
the sequence of top-level statements that call `createSignal`. The order is
defined by the module body's own linear execution, including top-level helper
calls, top-level loops, and top-level IIFEs, as long as that code path is
stable. This is the same guarantee component-instance ordinals already rely on.

An edit that changes the number of top-level signals (adding or removing one, or
flipping a top-level `if (flag) makeSignal()` branch) changes the new frame's
`cells.length`; the count-match guard fails and that one module keeps fresh
defaults. Only the edited module resets; every other module rehydrates. This is
the module-level analogue of today's "add/remove a component signal resets that
instance."

## Boundaries

The preservation rule is precisely: **a signal is preserved iff its
`createSignal` executes during the module's top-level run** (equivalently, while
the module frame is the active collection target). That subsumes signals created
via top-level helper calls, top-level loops, and top-level IIFEs — e.g.

```tsx
export function makeCounter() { return createSignal(0); }
const a = makeCounter();         // RUNS at top level → preserved
const b = createSignal("menu");  // → preserved
```

Both `a` and `b` are created during top-level execution, so both rehydrate; the
call indirection is irrelevant.

Deliberately **not** preserved (stated so it is not mistaken for a bug):

- **Deferred signals.** A `createSignal` that runs *after* top-level execution —
  from an event handler, timer, or promise that fires once the bundle has
  finished running and `modExit` has popped the module frame — is created while
  both stacks are empty, so `onSignal` no-ops and it is not tracked. It resets on
  reload. Such signals have no stable module-level identity to key on, which is
  why they are out of scope rather than merely unimplemented.
- **Component-body signals reached via `render()`** are preserved by the
  existing component mechanism, not this one — unchanged.
- **Non-reactive module state** (`let count = 0`) was never in scope.

## Testing

New coverage follows the existing HMR integration tests in
`crates/supersolid_runtime/src/lib.rs` (the `__ssHmr`/`$ss.hot` cases), which
drive a real JS engine through `render` → edit → `run_script` → assert, and the
transpiler/loader test patterns in `crates/supersolid/src/module.rs` /
`crates/superui/src/assets.rs`.

Runtime behavior (supersolid_runtime, engine-level):

1. **Top-level signal survives re-exec.** Set a top-level signal to a non-initial
   value, re-exec with edited markup, assert persistence. The ticket's repro.
2. **Fresh mount doesn't rehydrate.** First execution with empty `moduleFrames`
   yields initial values.
3. **Shape-change resets only the edited module.** Two-module bundle where one
   module adds a top-level signal: that module falls back to defaults, the other
   still rehydrates.
4. **Multi-module bundle, per-path keying.** Two modules each with top-level
   signals preserve independently; editing one does not cross-contaminate the
   other.
5. **Component signals unchanged.** An app mixing top-level and component-scoped
   signals preserves both; component instance keys are unaffected (guards the
   separate-stack invariant).
6. **Gate off = no preservation.** With `__ssHmr` false, top-level signals
   re-initialize and `moduleFrames` stays empty.
7. **Deferred-signal boundary.** A signal minted from an event handler after
   top-level execution resets on reload — locks in the documented boundary.
8. **Throwing factory keeps the stack balanced.** A module that throws during
   top-level exec still runs `modExit` (via `try/finally`); a subsequent module
   in the same bundle collects correctly.

Transpiler/loader (supersolid):

9. **Prelude emits the bracket.** `assemble` output wraps `m.factory(...)` with
   the guarded `modEnter`/`modExit`, asserted alongside the existing
   `$ssmod.def(` / `$ss.hot(` prelude tests.
10. **Guard is present.** Assert the `typeof $ss !== "undefined"` guard so a
    bundle run without the render layer is provably safe.

Manual verification (non-gating): the ticket's minimal repro
(`const s = createSignal("menu"); export const screen = s[0];`, edit a
component, save) preserved under `--features hmr`, exercised against
`examples/game_menu` or `examples/todomvc_supersolid`.

## Files touched

- `crates/supersolid_runtime/src/render.js` — `moduleStack`, `moduleFrames`,
  `modEnter`, `modExit`, the `onSignal` fallback, `$ss` publication; new engine
  tests in `crates/supersolid_runtime/src/lib.rs`.
- `crates/supersolid/src/module.rs` — the `SSMOD_PRELUDE` bracket; prelude
  assertions.
