# bevy_superui — Design (Program Overview + Phase 1 Spec)

Date: 2026-07-18
Status: Approved for planning

## 1. Vision

`bevy_superui` is a **thin, standards-shaped browser** embedded in a Bevy app. Authors
write ordinary **HTML + CSS + JavaScript** and it renders and behaves as close to a real
browser as is feasible, *within the box model that Bevy's UI (`bevy_ui` + taffy) can
express*. The long-term goal is to run framework output (React, then Svelte) unchanged.

There is **no bespoke markup or widget DSL** (unlike `bevy_hui`'s `<node>`). The only
non-web API surface is an explicit, clearly-namespaced **Bevy bridge** for talking to the
rest of the game.

### North-star principle

> Supported surface = `{ what taffy/bevy_ui can express }` ∩ `{ standards-shaped HTML/CSS/JS }`.
> Limitations of taffy/bevy_ui simply become "things our browser subset doesn't support."
> We never invent web-incompatible syntax to work around a gap.

### Secondary principle — breadth + graceful degradation

HTML/CSS/JS for this system will frequently be **AI-generated**. Therefore:

- Prefer implementing **as broad a standards-shaped API surface as feasible**, not a minimal one.
- **Unsupported features degrade gracefully** — unknown tags render as plain boxes, unknown
  CSS is skipped, unimplemented JS methods no-op/warn — rather than hard-throwing. AI-generated
  code that touches an unimplemented corner should keep running, not crash.
- A machine-loadable **capability ledger** (see §7) documents exactly what works, what is
  planned, and what will never be supported, so it can be loaded into an AI's context.

## 2. Goals / Non-goals

### Goals
- Author UI in plain `.html` / `.css` / `.js`; run via `cargo run` and build for
  `wasm32-unknown-unknown`, on par with Bevy's own target support.
- Reuse `bevy_ui`/taffy for layout, rendering, and picking.
- Reuse and fork `bevy_flair` for the CSS engine (cascade, selectors, animations).
- A pure-Rust, wasm-capable JS engine (Boa) behind a swap-able trait.
- A browser-like DOM/Web API surface exposed to JS, broad enough that AI-generated todo-class
  apps "just work."
- A clearly-namespaced Bevy bridge (`window.bevy`) shaped like a browser API.
- Hot reload via Bevy's standard asset system (native dev).
- Phase 1: a runnable, hot-reloadable **TodoMVC** example authored in HTML/CSS/JS.

### Non-goals (now and, where noted, ever)
- **`fetch`/network/backends — will not support** (games don't load UI data from servers).
  Kept only as a warn-and-reject stub for graceful degradation.
- Full CSS layout fidelity beyond taffy (floats, complex inline text flow) — out, by the
  north-star principle.
- State-preserving HMR — Phase 1 uses full JS re-execution on reload.
- wasm live-reload — out of Phase 1 (no browser filesystem; optional dev-server enhancement later).
- `rquickjs`/native-only fast JS engine — optional, later, behind the trait.

## 3. Architecture — three trees, one direction of truth

```
 .html ─▶ [html5ever]  ─▶  DOM tree (arena)     ◀── JS mutates this (the DOM API, in Boa)
 .css  ─▶ [flair CSS]  ─▶  CSSOM / cascade            │
 .js   ─▶ [Boa]        ─▶  event handlers             │  reconcile (diff) per frame / on dirty
                                                       ▼
                                          Bevy entities (Node + flair Style)
                                                       │
                                          taffy layout + bevy_ui render + bevy_picking
                                                       │
                                    input / picking ─▶ DOM event dispatch (capture/bubble) ─▶ JS
```

**Source of truth for structure is the retained arena DOM**, not the ECS. Rationale:
frameworks and AI code assume a synchronous DOM they can read back immediately
(`el.appendChild(x); x.parentNode === el` must hold *now*). A `Vec<NodeData>` + generational
ids gives that; making the ECS the DOM would force every JS DOM read through Bevy queries and
fight the frame boundary. JS `Element`/`Node` objects in Boa wrap a `NodeId`, never a Bevy
`Entity`.

**The reconciler is the single coupling point** between the "web world" and the "ECS world".
Once per frame (gated by a dirty flag) it diffs the DOM against spawned entities and emits
minimal ECS commands: spawn/despawn/reparent, update text, push class/attribute/inline-style
changes into flair. Everything downstream of a styled node — selector matching, cascade,
animation, writing `bevy_ui` components, taffy layout, rendering — is **flair + bevy_ui,
reused**.

**Events loop back unidirectionally**: `bevy_picking`/input/focus → translated into DOM
events → dispatched through the DOM tree with capture/bubble → JS listeners run in Boa → they
mutate the DOM → next frame reconciles.

## 4. Workspace / crate layout

A Cargo workspace. `bevy_flair` is **vendored/forked in-tree** so we can extend its CSS engine
(real HTML element/attribute selectors, more properties, `:hover`/`:focus`/`:checked` wired to
our event state) without waiting on upstream. **Fork base = `bevy_flair` 0.6.0** (the newest
release targeting Bevy 0.17). The 0.8/Bevy-0.19 copy currently vendored in-tree is kept only as
*reference for the future 0.19 upgrade*, not the fork base.

```
bevy_superui/
├─ crates/
│  ├─ superui_dom      # arena DOM: NodeData, mutation ops, event dispatch (capture/bubble).
│  │                   #   Knows nothing about Bevy or JS. Headless-testable.
│  ├─ superui_html     # HTML subset → DOM, via html5ever. Headless-testable.
│  ├─ superui_js       # JsEngine trait + Boa backend; DOM<->JS handle marshalling.
│  │                   #   Knows nothing about Bevy. Headless-testable.
│  ├─ superui_api      # web-standard API surface on top of JsEngine:
│  │                   #   document / Node / Element / Event / classList / style,
│  │                   #   console, timers, (stub) fetch, and the `window.bevy` bridge.
│  ├─ superui_css      # forked bevy_flair (its sub-crates vendored), extended for HTML.
│  ├─ superui_bridge   # reconciler: DOM diff → ECS commands; picking/input → DOM events.
│  └─ superui          # umbrella plugin: SuperUiPlugin, asset loaders, hot-reload wiring.
├─ examples/
│  └─ todomvc/         # authored index.html + style.css + app.js (native + wasm)
└─ docs/
   └─ support/         # the capability ledger (see §7): html.md, css.md, js-dom.md, README.md
```

**Boundary discipline:** only `superui_bridge` and `superui` depend on Bevy; only `superui_js`
and `superui_api` depend on Boa. `superui_dom`, `superui_html`, and the JS/DOM API layers are
testable headlessly without an app or window.

## 5. Dependencies and wasm posture

All **runtime** dependencies compile to `wasm32-unknown-unknown` (verified):

| Dependency | wasm | Notes |
|---|---|---|
| `boa_engine` (Boa) | ✅ official | Needs getrandom JS backend flag (`--cfg getrandom_backend="wasm_js"` + getrandom `js` feature) — same gotcha as Bevy itself. Date/time via Boa's JS clock. |
| `html5ever` / markup5ever / tendril | ✅ proven | Pure Rust, no C, no time/random. |
| `cssparser` 0.37 / `selectors` 0.38 / `cssparser-color` | ✅ very likely | Pure Rust Servo/Stylo stack; validate with an early wasm build. |
| `bevy_flair` / `bevy_ui` / taffy | ✅ | Bevy runs on wasm. |

**Intentionally native-only (never on the wasm runtime path):**
- **`notify`** — NOT used. We rely on Bevy's asset system for file watching instead (§6).
- **`rquickjs`** — optional native-only fast JS backend behind the `JsEngine` trait; Boa is the
  engine on every target.

**Bevy version: 0.17** (the version currently in use). Fork base is `bevy_flair` 0.6.0, which
targets Bevy 0.17; `bevy_hui` 0.5.0 is the matching 0.17 reference. Newer Bevy (0.18/0.19) and
the corresponding flair (0.7/0.8, already vendored) are a later, explicitly-planned upgrade —
the crate boundaries (§4) are designed so the Bevy-touching layers (`superui_bridge`, `superui`,
forked `superui_css`) absorb version bumps while `superui_dom`/`superui_html`/`superui_js` stay
version-agnostic.

## 6. Hot reload — via Bevy's asset system (no `notify`)

Each UI "screen" is an **asset folder** (`assets/ui/todomvc/{index.html, style.css, app.js}`).
We register Bevy `AssetLoader`s for `.html`, `.css` (flair's), and `.js`, producing
`HtmlAsset` / `StyleSheet` / `JsAsset`.

Hot reload is one seam: **`AssetEvent::<T>::Modified` → re-parse / re-execute → reconcile.**
- HTML/CSS changed → re-parse and re-reconcile.
- JS changed → **full re-execution**: tear down the JS context, re-run the script(s) against
  the current DOM (state resets — acceptable for Phase 1; state-preserving HMR is later).

Platform behavior is inherited from Bevy for free:
- **Native dev run** with Bevy's `file_watcher` feature (the `bevy_new_2d` pattern: dev-only
  cargo feature + `AssetPlugin` watch override) → edits fire `Modified` automatically.
- **wasm** → file watcher inactive → hot reload is a no-op by default. Optional future
  dev-server push over the *same* `AssetEvent` seam if wasm live-reload is ever wanted.

## 7. The capability ledger (`docs/support/`) — a first-class deliverable

An exhaustive, maintained record of what the HTML/CSS/JS subset supports. It is simultaneously
the **AI-context document** and the **roadmap tracker**, and is kept in sync with the
implementation.

**Structure:** three files plus a legend:
- `docs/support/README.md` — legend, scope, how to load into AI context, sync policy.
- `docs/support/html.md` — every HTML element/attribute we care about.
- `docs/support/css.md` — every CSS property/selector/at-rule/pseudo we care about.
- `docs/support/js-dom.md` — every DOM/Web API object/method/event we care about.

**Each row carries:**
- **Status:** ✅ Supported · 🟡 Roadmap (achievable in theory, planned) · ⛔ Won't support.
- **Priority tier** (ordering, by game-UI usefulness): **T0** essential (layout, text, click,
  class toggling) · **T1** common (inputs, lists, hover/focus, transitions) · **T2** advanced
  (SVG, canvas, animations, transforms) · **T3** niche.
- **Notes:** taffy/bevy_ui constraint, degradation behavior, or link to tracking issue.

**🟡 Roadmap** = achievable in theory *regardless of what bevy_ui can do today* (so SVG,
canvas, transforms are 🟡 Roadmap, not ⛔). **⛔ Won't** is reserved for things fundamentally
out of scope — currently: `fetch`/network, cookies, navigation/history,
multi-document/iframes-to-servers.

Ordering within each file is by tier (T0 first), so the top of each file is the highest-value
surface. The ledger is authored during Phase 1 covering the full landscape (even unimplemented
rows), giving an immediate roadmap.

## 8. The Bevy bridge (`window.bevy`) — the one non-web API

Exposed to JS as a single namespaced global, shaped like a browser API so it feels native to
JS/AI:

```js
bevy.send("SpawnEnemy", { x: 10, y: 4 });                    // fire a command/event into ECS
bevy.on("ScoreChanged", e => scoreEl.textContent = e.value); // subscribe to an ECS event
const hp = await bevy.query("player.health");                // read game state (async)
```

Game side registers the allowed surface in Rust, and the bridge is built on **Bevy observers**:
```rust
app.add_superui_command::<SpawnEnemy>()   // JS bevy.send("SpawnEnemy", p) → commands.trigger(SpawnEnemy)
   .add_superui_event::<ScoreChanged>()   // game triggers ScoreChanged → forwarded to JS bevy.on callbacks
   .add_superui_query("player.health", ...);
```
Flow:
- **JS → ECS (`bevy.send`)**: deserialize the JS payload into the registered type and
  `commands.trigger(...)` it. The game reacts with its *own* observers — SuperUI adds nothing
  game-specific, it just injects the trigger. Idiomatic Bevy 0.17.
- **ECS → JS (`bevy.on`)**: SuperUI registers an **observer** per `add_superui_event::<T>()`;
  when the game triggers `T`, the observer marshals it (serde/reflect, JSON-shaped) and invokes
  the matching JS callbacks in Boa.
- **`bevy.query`**: async read of registered state, resolved back into the JS promise.

This is the *only* API JS sees that is not a web standard, and it is deliberately quarantined
behind one global.

## 9. Phase 1 — "TodoMVC that runs"

**Deliverable:** `cargo run --example todomvc` (native, hot-reloading) and a working
`wasm32-unknown-unknown` build, rendering an interactive TodoMVC authored as plain
`index.html` + `style.css` + `app.js`.

### HTML subset (T0/T1)
`div`, `span`, `p`, `ul`/`li`, `button`, `input` (text + checkbox), `label`, `h1`–`h6`, text
nodes; attributes `class`/`id`/`type`/`value`/`placeholder`/`checked`. Unknown tags render as
plain boxes; unknown attributes ignored.

### CSS subset (flair + wired pseudo-classes)
Whatever flair supports today (flex layout, color, spacing, border, font, sizing) plus wired
`:hover`, `:focus`, `:checked`. Selectors: type / class / id / descendant. Unsupported rules
skipped, never fatal.

### JS / DOM API (Boa) — broad, not minimal
- `document`: `getElementById`, `querySelector`/`querySelectorAll` (basic selectors),
  `createElement`, `createTextNode`.
- `Node`/`Element`: `appendChild`, `removeChild`, `insertBefore`, `replaceChild`, `parentNode`,
  `childNodes`, `children`, `textContent`, `innerText`, `setAttribute`/`getAttribute`,
  `classList` (`add`/`remove`/`toggle`/`contains`), `.value`, `.checked`, `.style.*` (common).
- Events: `addEventListener`/`removeEventListener`, capture/bubble dispatch, event object with
  `target`/`currentTarget`/`preventDefault`/`stopPropagation`; wired types: `click`, `input`,
  `change`, `keydown`/`keyup`, `submit`.
- Globals: `console.*`, `setTimeout`/`setInterval`/`clear*`. `fetch` present but a warn-and-reject
  stub.
- `window.bevy`: minimal `send`/`on`, enough for the example to demo one ECS round-trip
  (proves the seam; optional in the todo UI itself).

### Out of Phase 1
Real `fetch`/network (and it's ⛔ forever), SVG/canvas (🟡 roadmap), CSS animation/transition
polish, React/Svelte (Phase 3), state-preserving HMR, wasm live-reload, `rquickjs` backend.

### Definition of done
Add / toggle / delete todos; filter all / active / completed; live "N items left" counter — all
via standard DOM/JS. Runs native; builds and loads on wasm. Editing `app.js`/`style.css` on
native hot-reloads (full JS re-exec). The `docs/support/` ledger exists and covers the full
HTML/CSS/JS landscape (implemented rows marked ✅, the rest 🟡/⛔).

## 10. Roadmap beyond Phase 1

- **Phase 2 — browser-ish completeness:** fuller DOM/event model, timers hardening, richer CSS
  (transitions/animations, transforms), the full `window.bevy` bridge (queries, typed events),
  possibly wasm dev-server live-reload. **SVG** lands here or early P3 (AI emits it often).
  **canvas** later in the tier.
- **Phase 3 — framework support:** get a React UMD bundle rendering against the DOM API, then
  Svelte's compiled output. Driven by expanding the DOM/CSSOM surface until the frameworks'
  assumptions are met; the capability ledger tracks the gap.

## 11. Testing strategy

- **Headless unit tests** in `superui_dom` (tree ops, event dispatch order), `superui_html`
  (parse → tree shape), `superui_js`/`superui_api` (run JS snippets, assert DOM mutations) —
  no Bevy app needed.
- **Reconciler tests** in `superui_bridge`: given a DOM diff, assert the ECS command set.
- **Example-as-integration-test:** a headless run of the TodoMVC script driving synthetic
  events, asserting resulting DOM state.
- **wasm build check** in CI early: `cargo build --target wasm32-unknown-unknown` for the
  workspace to catch any dependency regressions (esp. the getrandom flag).
- **Ledger/impl sync:** a test that flags ledger rows marked ✅ but with no corresponding
  implemented binding (best-effort, tightened over time).

## 12. Key risks

- **Boa performance** — mitigated by the `JsEngine` trait (native `rquickjs` later) and by the
  fact that UI JS is event-driven, not hot-loop.
- **flair fork drift** — vendoring means we own merges from upstream; acceptable, and it buys us
  the HTML-selector extensions we need.
- **taffy box-model gaps vs. author expectation** — mitigated by the ledger making the boundary
  explicit and by graceful degradation.
- **getrandom/wasm flag** — well-known Bevy-on-wasm setup; documented in the example's build
  instructions.
