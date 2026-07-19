# superui — Component Framework Direction

Date: 2026-07-19
Status: Direction agreed (forward-looking). **Not** an implementation plan.

## 0. What this document is

This is the **agreed direction** for **Supersolid** — the component/reactivity framework layered
on top of the `bevy_superui` browser from `2026-07-18-bevy-superui-design.md`.

**Roadmap position (re-ordered from the base design):** Phase 1 (the browser core + a plain
HTML/CSS/JS TodoMVC) is **done and merged**. **Phase 2 is now Supersolid** — this framework, the
small lower-level DOM/JS additions it needs, and a Supersolid TodoMVC example — *not* broad
browser completeness. Fuller browser compatibility (scoped to what doesn't require large
underlying-crate changes) moves to **after** Supersolid. This supersedes the base design's §10
ordering (browser completeness as Phase 2, frameworks as Phase 3).

This is a direction/spec document; the implementation plan lives in `../plans/` (the Phase 2 /
Supersolid plan series).

## 1. Goal

The best UI-development experience for *any* Bevy game, specifically:
1. **Hot reload on par with React+Vite / Svelte** — including *state-preserving* HMR, not just
   file re-execution.
2. **Reusable component decomposition** — break a UI into components and share them.

The `2026-07-18` design already reaches "run plain HTML/CSS/JS," but raw HTML/CSS/JS gives
neither components nor reactivity. This document decides *how* we add them.

## 2. Core decision — a purpose-built, Solid-like fine-grained framework

We build **one** purpose-built component + reactivity framework — named **Supersolid** (workspace
crate `supersolid`) — modeled on **SolidJS's fine-grained signals** and authored in **Solid-style
JSX/TSX**. It is not literally Solid and it is not React.

**Why not React** (the model most authors/AI reach for first):
- Real React assumes a **Node/bundler build step** (JSX transpile), contradicting the Bevy dev's
  cargo-only toolchain.
- We **cannot run existing npm React components unchanged anyway** — different runtime, our arena
  DOM, and only the taffy/`bevy_ui` CSS subset. So React's one real draw, *ecosystem reuse*, is
  off the table regardless.
- React's per-update **VDOM re-render + diff** is the single most allocation-heavy reconciliation
  strategy — the worst possible fit for Boa, an interpreter with no JIT (§4).

With ecosystem reuse gone, React's only remaining advantage was **name familiarity** — not worth
carrying a second runtime, a VDOM reconciler, and cross-model boundary adapters. So we drop the
React-style path entirely.

**Why Solid:**
- Fine-grained signals are the **best fit for an interpreter** and the **cleanest path to
  state-preserving HMR** (§4).
- Solid's **JSX is React-shaped**, so the *authoring surface* stays familiar (components, props,
  `onClick`, `.map`/`<For>`); only the reactivity *semantics* differ.
- Solid is itself a **known framework present in AI training corpora**, so the original constraint
  — "close to a well-known framework, familiar to AI and humans" — is satisfied by anchoring on
  Solid instead of React.

## 3. We own the compiler

`.tsx` / `.ts` becomes a **Bevy asset type**, transpiled to JS **in Rust inside an asset
loader** (via `oxc` or equivalent: strip TS types, lower Solid-style JSX). Consequences:
- **No Node/npm at runtime, ever.** The transpiler is native / build-time only (see §11.3):
  native transpiles at asset-load; wasm ships **pre-transpiled** `.js` and carries no transpiler.
- Transpilation rides the existing hot-reload seam on native: `.tsx` edited →
  `AssetEvent::Modified` → re-transpile → reconcile. Same machinery as Phase 1's HTML/CSS/JS.
- **Real IDE tooling works for free.** `.tsx` files get the full TypeScript editor experience
  (highlighting, autocomplete, type-checking, go-to-def) from the TS language server — a
  *dev-time editor* concern, not a build/runtime dependency. We ship an ambient `.d.ts` for the
  runtime API + a `tsconfig` preset; VS Code / JetBrains "just work." We transpile (type-strip);
  we do **not** type-check at load — that stays a pure editor concern.

Because JSX→JS is a small, well-understood transform with mature Rust implementations, "own the
compiler" is tractable. (For contrast, a Svelte-style compiler would not be — see §8/§11.4.)
Owning the compiler is also what lets us apply Solid's key compile-time optimization ourselves
(§5): lower JSX to "build these nodes once, wire reactive bindings only to the dynamic parts."

## 4. Reactivity model — why fine-grained, and only fine-grained

Two models exist; we document the comparison because it is the whole rationale for choosing one.

**VDOM / coarse (React-shaped) — rejected.** The component body **re-runs** on every state
change, producing a new virtual tree that is **diffed** against the previous one to find the
minimal DOM patch. Per update, work ∝ **size of the component subtree**. On Boa (no JIT), all that
object churn and tree-walking is interpreted — the strategy and the engine's weakness line up
badly. State is also stored **positionally** (hook call-order in a fiber tree), so
state-preserving HMR requires building our own Fast-Refresh-equivalent machinery.

**Fine-grained / signals (Solid-shaped) — chosen.** The component body runs **once**; the runtime
holds an explicit dependency graph ("signal → the exact bindings that read it") and on change runs
**only** the affected bindings. Per update, work ∝ **number of things that actually changed**.
Near-zero allocation — exactly what an interpreter handles well. State lives in explicit **signal
cells** decoupled from the view, and the dependency graph is inspectable, so **state-preserving
HMR is cheap**: a component's code can be hot-swapped while its live signal cells persist.

Crucially for us, the *relative* advantage of fine-grained over VDOM is **larger on Boa than in a
JIT browser**, because the thing it avoids (allocating + diffing vnode trees) is precisely what
interpreters are worst at. And on the **wasm target there is never a JIT** (§6), so this is the
permanent, only lever for good web-build performance.

**The one semantic cost we accept.** A signal returns a **getter you must call** at the point of
use (`count()`), and derived logic placed in the component body runs once and is frozen — you keep
reactive reads inline (`{count() * 2}`) or use `createMemo`. This differs from React's "plain value
re-read each render," so React-by-reflex code (destructured props, `count` instead of `count()`,
derived-in-body) is subtly wrong. Because we own the compiler, we **lint these React-isms and emit
a pointed warning with the Solid-correct fix** (§11.1) — cheap, and it neutralizes most of the
onboarding cost for the AI-heavy authoring audience.

## 5. The runtime — one fine-grained model, no phases

A single Solid-like runtime living in Boa and driving the arena DOM. There is **no coarse/VDOM
path and no mode switching** — every component is fine-grained, so performance is **uniform across
native and wasm** by construction.

- **Reactivity primitives (Solid-style):** `createSignal` → `[get, set]`, `createEffect`,
  `createMemo`, `onMount`/`onCleanup`, `createContext`/`useContext`. Components are functions
  that take `props` and **run once**; dynamic bits in JSX become individual reactive bindings.
- **Compile strategy (Solid's real perf lever, adapted to our layer).** We own the transpiler, so
  JSX lowers to *build the node structure once + attach reactive effects only to the dynamic
  holes* — the analog of Solid's template-clone + surgical bindings, translated from browser
  `cloneNode` to our arena-DOM spawn ops. An update then pokes one text/attr through the
  reconciler instead of rebuilding a subtree.
- **Control-flow components:** `<Show>`, `<For>`/`<Index>`, `<Switch>`/`<Match>` (Solid-style) for
  conditional and keyed list rendering, mapping to minimal reconciler spawn/despawn/reorder.
- **Props are not destructured** (destructuring breaks reactivity) — the linter flags it (§11.1).

Note on `bevy_superui` engine phases: Supersolid is **Phase 2** (re-ordered — see §0); "no phases"
here means the *reactivity model* is single, not split into coarse-then-fine.

## 6. Engine posture (orthogonal to the reactivity model)

- **Boa on every target** to start.
- Later, a faster **native** backend via the existing `JsEngine` trait — `rquickjs` (a much
  faster interpreter; already the design's planned native fast-path) or a JITed engine.
- **The wasm JIT wall:** a JIT generates and executes native machine code, which the
  `wasm32-unknown-unknown` sandbox cannot do. So **any** JIT (Boa's Cranelift experiment, V8,
  SpiderMonkey) helps **native only**; wasm is interpreter-only *permanently*. This is exactly why
  the fine-grained model (§4) is non-negotiable for the web build.
- **Boa's Cranelift JIT** is a *hopeful maybe*, not a plan: treat as experimental / not
  production, and native-only in benefit even if it lands.

## 7. Where the component layer sits (boundary discipline)

The component framework is entirely a **transpile-time + JS concept living *above* the arena
DOM.** Components produce ordinary DOM mutations; **everything downstream is Phase 1 machinery,
untouched** — reconciler (DOM→ECS), taffy layout, `bevy_ui` render, picking, and the
`window.bevy` bridge. The framework **composes on top of** the browser; it does not disturb it.

## 8. Authoring surface

- **Solid-style TSX** (JSX with fine-grained semantics) is the **sole** authoring surface.
  Familiar shape for AI/humans, real IDE tooling, our own transpile. (A Svelte-shaped
  single-file-component skin was considered and **dropped** — bespoke syntax + weak IDE tooling +
  owning an editor extension weren't worth it; revisit only if a concrete need appears.)
- **bun/npm** appear **only** as a *publisher-side authoring convenience* — e.g. `bun add` a
  library to pull its source, then adapt the `.tsx` to our runtime + capability subset before
  publishing into our own tree. **Never** a consumer runtime requirement.

## 9. Component distribution — cargo crates + a thin projector CLI

The unit of distribution is an **ordinary cargo crate** carrying `.tsx` / `.css` / asset source,
its metadata in **`[package.metadata.superui]`** (runtime semver requirement, any `window.bevy`
bridge needs, entry/exports, asset globs, optional Rust glue — see §11.5; the CSS/HTML/DOM
capability footprint is *auto-derived*, not hand-written). Bevy devs consume it the way they
consume everything — `cargo add`, transitive/git/path deps, `Cargo.lock`, private registries.

A **thin CLI** (`cargo superui`, or a build script) reads the **already-resolved** dependency
graph via `cargo metadata`, finds crates that declare superui components, and **projects** their
UI source into `target/superui_modules/<crate>/…` — a flat, gitignored, regenerated-from-lockfile
tree the transpiler/loader imports from (`import { Card } from "@superui/card"` resolves against
it). It is `node_modules` in ergonomics, **derived from cargo** instead of a second package
universe.

Key property: **cargo is the package manager; the CLI is only a projector/linker.** No reinvented
resolution — we inherit semver, lockfiles, offline vendoring, private registries for free. The
CLI's only added jobs are the projection and a **capability-ledger compatibility check** during
sync (see §11.5: **warn-only** — it never blocks a sync). **Consumers need zero Node.** This turns
"will this adapted component work on our subset?" from a gamble into a manifest-declared,
CI-checkable signal — directly serving the AI-generation goal (the AI is handed a vetted,
capability-annotated catalog, not the open web).

## 10. Non-goals

- **Not React** — we do not match React's re-render semantics, do not implement a VDOM, and do not
  expose `useState`-style hooks. Solid-style fine-grained only.
- **Not** running arbitrary npm components (React *or* Solid) unchanged as binaries — they assume
  DOM/CSS features taffy/`bevy_ui` can't express, plus full framework internals. Reuse is via
  *adapted source* in our own tree, not binary drop-in.
- **Not** a Node/bundler build toolchain, and **not** a Svelte-style compiler.
- **Not now** — post Phase 1/2.

## 11. Resolved detail decisions

### 11.1 API surface, authoring rules, React-ism linting
- **Surface (Solid-style):** `createSignal` → `[get, set]`, `createEffect`, `createMemo`,
  `onMount`/`onCleanup`, `createContext`/`useContext`; function components taking `props` and
  running **once**; control-flow components `<Show>`/`<For>`/`<Index>`/`<Switch>`; `onClick`/
  `onInput`/… event props; a root `render()`.
- **Authoring rules that fall out of fine-grained semantics:** reactive reads stay inline or in
  `createMemo` (no frozen derived-in-body); **props are not destructured**; getters are *called*
  (`count()`).
- **React-ism linting (our compiler's job):** detect the common React reflexes — prop
  destructuring, `count` used where `count()` is required, derived logic in the body expecting a
  re-run, `useState`/`useEffect` names — and emit a warning pointing to the Solid-correct form.
  This is the mitigation for the one semantic cost in §4; it's cheap because we own the transpiler.
- **No mode inference, no mixing rules, no boundary adapters** — there is only one model, so all of
  that (from the earlier two-model draft) is gone.

### 11.2 HMR state preservation
Signal cells are keyed by **(module = asset path + export name) × (instance = tree position +
explicit `key`) × (cell index = signal creation order at setup)**. On hot-swap: re-run the new
setup but **rehydrate** matched cells instead of creating fresh, then dispose and recreate the
view effects, reusing the persisted signal cells. **Fallback:** if a component's signal *shape*
changes (add/remove/reorder), the positional mapping breaks → **remount that component, state
resets** — the accepted degradation, analogous to React Fast Refresh's rule.

### 11.3 Transpiler placement
The transpiler is **native / build-time only**; it never runs on the wasm runtime:
- **Native dev:** transpile `.tsx`→`.js` at asset-load (oxc as a normal Rust dep) → full hot
  reload via the existing `AssetEvent::Modified` seam.
- **wasm build:** **pre-transpile `.tsx`→`.js` at build time** (the `cargo superui` step); ship
  plain `.js` assets; the wasm binary contains no transpiler. Consistent with the base design's
  "wasm live-reload out of scope" — nothing lost.

oxc is the pick (swc the fallback), validated by the ordinary native build; there is **no**
oxc-in-wasm dependency.

### 11.4 Authoring surface — settled
The Svelte-shaped SFC skin is **dropped** (see §8). Solid-style TSX is the sole authoring surface.

### 11.5 Manifest + capability check
- Metadata lives in **`[package.metadata.superui]` in the component crate's `Cargo.toml`** — no
  separate file; `cargo metadata` surfaces it directly. It **hand-declares only what isn't
  statically inferable**: the required superui runtime semver, any `window.bevy` bridge commands,
  plus entry/exports and asset globs.
- The **capability footprint** — the HTML elements, CSS properties, and DOM/JS APIs a component
  touches — is **auto-derived** by parsing its `.tsx` (transpiler) and `.css` (flair) at
  publish/sync time. It can't drift and needs no manual upkeep.
- **Check policy = warn only.** `cargo superui sync` never blocks. It emits warnings for both
  runtime-version mismatches and capability gaps (a footprint feature marked 🟡 Roadmap / ⛔ in
  the consuming app's ledger). A genuinely-incompatible component surfaces at runtime with the
  usual no-op/warn degradation rather than a blocked build — matching the base design's
  graceful-degradation ethos and suiting AI-generated churn.

**Footprint example.** A `Card` whose `.tsx` renders `<div><h3>…</h3><button onClick=…>…</button></div>`
with CSS `display:flex; gap:8px; border-radius:8px; box-shadow:…` has footprint {HTML: `div`,
`h3`, `button`; CSS: `flex`, `gap`, `border-radius`, `box-shadow`; DOM: `onClick`, text
interpolation; runtime ≥ declared; bridge: none}. If the app's ledger marks `box-shadow` 🟡, sync
warns and the card renders shadow-less; if the app's runtime is older than the declared
requirement, sync warns and any missing API degrades at runtime.

## 12. Relationship to the existing roadmap

This **re-orders** `2026-07-18-bevy-superui-design.md` §10: Phase 1 (done) → **Phase 2 =
Supersolid** (this doc) → **Phase 3 = browser compatibility** (the old "browser completeness",
scoped to what doesn't need large underlying-crate changes). The one durable dependency: Supersolid
sits entirely above the arena DOM (see §7), which the Phase 1 DOM/reconciler boundary already
allows. Supersolid's concrete lower-level prerequisites (small DOM/JS additions) are enumerated in
the Phase 2 plan series, not here.
