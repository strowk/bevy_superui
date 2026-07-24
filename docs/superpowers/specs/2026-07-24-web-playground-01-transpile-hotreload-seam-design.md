# Web playground, part 1: in-browser transpile + state-preserving hot-reload seam

- **Date:** 2026-07-24
- **Status:** Approved design, pending implementation plan
- **Scope:** `superui` (new `transpiler` feature), `superui_bridge` (surface JS runtime
  errors), a new wasm-only crate `superui_playground_web`, and the `counter` example (a
  playground wasm build + a throwaway proof harness).
- **Assumes:** the "HTML-as-manifest" boilerplate refactor
  (`2026-07-24-supersolid-example-boilerplate-design.md`, **Model 2**) is already
  implemented — `SuperUiRoot` carries a single `html` handle and CSS/JS are discovered
  subresources tracked in `SuperUiSubresources { css, js }`.

## Why

`oxc` (the `.tsx`→JS transpiler) is a pure-Rust dependency that **compiles to
`wasm32-unknown-unknown`** — verified this session by building `supersolid` (its only
oxc-bearing crate) for the wasm target with zero source changes. The transpiler is held
out of wasm today purely by a `cfg(not(target_arch = "wasm32"))` gate and a target-cfg
dependency, **not** by any oxc limitation.

Compiling oxc into a wasm build is cheap: for `counter`, the full transpile pipeline adds
**~1.9 MiB uncompressed / ~727 KiB gzipped** to the shipped `_bg.wasm` (measured this
session: baseline 13.70 MiB gzipped → 14.41 MiB with oxc in, same
`--release` → `wasm-bindgen` → `wasm-opt -Oz` pipeline the site's CI uses). That is small
enough to make the gallery demos **live-editable in the browser**.

The goal of the overall effort is the redesigned example page (see
`drafts/redesign_website_with_examples.html`, the CODE tab): a file-tree + code browser
next to the running demo, where editing a demo's `.tsx` and pressing Run re-transpiles
in-browser and hot-swaps the UI **with signal state preserved** — the framework's headline
feature, live on the web instead of the current "clone the repo, it's native-only" note.

## Decomposition (this spec is part 1 of 3)

The full effort is too large for one spec and splits cleanly along a JS↔wasm interface:

- **A — the seam (this spec):** compile oxc into a playground wasm build; a bridge crate
  that takes edited source from JS, transpiles it, and drives the *existing*
  hot-reload seam to a state-preserving reload; return diagnostics/errors to JS. All the
  technical risk lives here. Proven on `counter` with a throwaway harness (no real UI).
- **B — the playground page (website):** the mockup's editable code browser (file tree
  with type badges/folders, editor, DEMO/CODE tabs, console, Run, EXPAND, COPY) consuming
  A's exported API. Pure frontend against a settled contract.
- **C — rollout + CI:** build every supersolid-TSX demo with the feature, ship its source,
  wire the pipeline. Mechanical once A + B exist.

Ordering: **boilerplate Model 2 → A → B → C.**

## Product decisions (fixed during brainstorming)

- **Editable demos = the supersolid `.tsx` demos only** (`counter`, `todomvc_supersolid`,
  `game_menu`, `citadel`, `horde`). The one classic HTML/JS demo (`todomvc`) gets the
  **same example page**, minus the Run button — a static build, exactly like today. (The
  page shell in B is universal; only the editable/Run wiring is capability-gated.)
- **Apply edits via state-preserving HMR**, reusing the native hot-reload seam (not a full
  remount). This is the whole point — the web playground and native `--features hmr` must
  exercise the same rehydration path so they cannot drift.
- **Run button applies edits; first load auto-runs.** A playground build loads its
  `app.tsx` **live** — the initial mount transpiles it in-browser via `TsxLoader` (the same
  `.tsx` the editor shows and the user edits), so there is a single source of truth and no
  separate generated-JS artifact to keep in sync. Run only applies *subsequent* edits.
- **Console shows transpile diagnostics + JS runtime errors** (not a full `console.log`
  stream).
- **The website shell is file-tree-agnostic:** it displays/edits whatever files it is
  handed and notifies the wasm build when one changes. It does **not** encode framework
  limits — a demo may present multiple `.tsx` files in its tree even though what actually
  *runs* stays single-entry. **No module resolution is added** (cross-module imports are
  still stripped, per `supersolid/src/imports.rs`); demos remain single-entry.

## Non-goals

- The real playground UI (file tree / editor / console / tabs / expand) — that is **B**.
- **CSS/HTML live-edit wiring.** The same seam covers them (a `.css` edit restyles via
  flair's `StyleSheet` asset watching; an `index.html` edit rides Model 2's full remount),
  but CSS needs parsing edited text into a flair `StyleSheet` (not a raw-string swap), and
  neither is needed to prove the reactive-TSX path. Wired in a later slice.
- Multi-demo rollout, per-demo source bundling, CI changes — that is **C**.
- Module resolution / multi-file imports, `console.log` streaming, multiple stylesheets.

## The transpiler feature (additive-for-wasm)

The rule is **preserve today's behavior, add only the new case** — the feature must be on
"in the same situation the related functionality is on today," plus the new playground
case:

| Build | oxc in binary? | `TsxLoader` registered? | HMR rehydration |
|---|---|---|---|
| native (today, unchanged) | yes | yes | with `hmr` feature |
| wasm, normal gallery demo (unchanged) | **no** | no | no |
| **wasm, playground** (`transpiler` + `hmr` + bridge crate) | **yes** | yes | **yes** |

Mechanics in `crates/superui/Cargo.toml`:

- Keep the native `supersolid` dependency **unconditional** (today's behavior — native
  always links oxc via `TsxLoader`). *Note:* this corrects an earlier assumption that
  native releases omit oxc; they do not, and this spec keeps it that way.
- Add, under the wasm target, `supersolid = { path = "../supersolid", optional = true }`,
  and a feature `transpiler = ["dep:supersolid"]`.
- Gate `TsxLoader` — its struct, `impl AssetLoader`, the `pub use` re-export in `lib.rs`,
  and the `register_asset_loader` call in `mount.rs` — on
  `cfg(any(not(target_arch = "wasm32"), feature = "transpiler"))`.

So on native the predicate is always true (unchanged); on wasm it is true only with the
feature. The **bridge crate enables `superui/transpiler`**, so oxc enters a wasm binary
only when the playground bridge is pulled in — a normal gallery wasm build stays oxc-free
exactly as now.

This **supersedes** the boilerplate spec's "oxc must not enter the wasm binary" statement
(that spec's lines 30 and 232) for the feature-on case: the `transpiler` feature is the
sanctioned exception, used only by playground builds.

### Loading `.tsx` live on wasm (seam predicate extension)

With `TsxLoader` registered on wasm, a playground build should load `app.tsx` **live**
(transpile at load) rather than the host-generated `.superui/build/app.js`, so the file the
editor shows is the file that runs. This is one change to the boilerplate spec's script
seam, which today decides:

```
live = cfg!(all(not(target_arch = "wasm32"), feature = "hmr"))
// live      → load the .tsx as-is (TsxLoader transpiles)
// !live     → load superui_paths::generated_js(src)
```

The `transpiler` feature adds a disjunct so wasm-playground builds are also `live`:

```
live = cfg!(all(not(target_arch = "wasm32"), feature = "hmr"))
     || cfg!(all(target_arch = "wasm32", feature = "transpiler"))
```

Consequences: playground demos need **no** generated-JS artifact (they transpile their
`.tsx` in-browser at mount); normal gallery wasm demos are unchanged (`!live` → generated
JS). The bridge still owns *edit* transpilation and overwrites the mounted `JsSource` (both
`TsxLoader` and the bridge call `supersolid::transpile`; oxc is linked once).

Note: oxc's parser is recursive-descent, and a very large `.tsx` could stress the wasm
stack at mount. `counter` is tiny, so this is a non-issue for A; flagged as a
watch-item for the larger demos in **C**.

## New crate: `superui_playground_web`

A small wasm-only crate — the "browser watcher." Isolated in its own crate so it is pulled
in **only** for website playground builds; a normal demo build never sees it (and never
sees oxc). Depends on `superui` (features `transpiler`, `hmr`), `supersolid` (for the
transpile call), `bevy`, `wasm-bindgen`.

### Exported (wasm-bindgen) JS API — the contract handed to B

```
// Returns a JSON string: { "ok": bool, "diagnostics": [ { "severity": "...", "message": "..." } ] }
// The transpile is synchronous, so diagnostics come back immediately. The actual
// hot-swap is applied on the next Bevy frame by drain_playground_edits.
apply_source(path: &str, src: &str) -> String

// Returns a JSON string: [ { "message": "..." }, ... ]
// Drains JS runtime errors captured since the last poll (for the console panel).
// The page polls this (e.g. once per rAF / on a short interval).
poll_diagnostics() -> String
```

`apply_source` classifies by extension:

- `.tsx` / `.ts` → `supersolid::transpile(src, opts)`; return `{ok, diagnostics}` now;
  enqueue `Edit::Js(transpiled_code)`.
- `.css` → enqueue `Edit::Css(src)` *(wiring deferred — see Non-goals)*.
- the entry `.html` → enqueue `Edit::Html(src)` *(wiring deferred — rides Model 2 remount)*.

### Internals

- `thread_local! { static QUEUE: RefCell<Vec<Edit>> }`. wasm is single-threaded, so the
  wasm-bindgen exports and the Bevy systems run on the same thread; a `RefCell` queue is
  safe (no `Mutex`, no reentrancy). `apply_source` pushes; a system drains.
- A matching `thread_local` error sink holds JS runtime errors for `poll_diagnostics`.
- `PlaygroundBridgePlugin` adds `drain_playground_edits`, an exclusive system ordered
  **before** `detect_hot_reload` in `Update`. For each queued edit it resolves the mounted
  `SuperUiRoot` + `SuperUiSubresources`, then for a `Js` edit does
  `Assets::<JsSource>::get_mut(subresources.js).0 = code`. `get_mut` auto-emits
  `AssetEvent::Modified`, which the **existing** `detect_hot_reload` → `apply_hot_reload`
  seam turns into a state-preserving `run_script` re-exec. **No new reload logic.**

## `superui_bridge` change: surface JS runtime errors

`UiRuntime::run_script(&mut self, src: &str)` returns `()` today — uncaught Boa eval errors
are logged, not returned, so the console cannot show them. Add a runtime error sink:

- `UiRuntime` accumulates uncaught JS eval errors (from `run_script` and per-frame
  callback execution) into a `Vec<String>`.
- `pub fn take_errors(&mut self) -> Vec<String>` returns and clears them.

The bridge drains these into its `thread_local` sink each frame (or reads them directly in
a system) and returns them from `poll_diagnostics`. Transpile *diagnostics* (the common
"broken TSX" case) already come from `supersolid::transpile`; this adds the *runtime*
half of the chosen "diagnostics + JS errors" console.

## State-preservation enablement (playground build config)

`hmr_active = cfg!(feature = "hmr") && watching`. The boilerplate spec removes superui's
`AssetPlugin` watch override (superui must not flip the game's global watcher), and on wasm
`watching` is `false` by default → no rehydration. Resolution, consistent with that spec's
"the app owns its watch setting" stance (it explicitly blesses `todomvc` doing this):

- The **playground demo build** sets `AssetPlugin { watch_for_changes_override: Some(true),
  .. }` in its own `main.rs`/setup and compiles `superui/hmr`, so `hmr_active` is true and
  rehydration is on. It adds `PlaygroundBridgePlugin`.
- No file watcher runs on wasm; the bridge fires `Modified` manually via `get_mut`. The
  seam is already watcher-agnostic (its own docstring says so).
- This config is scoped to the wasm-playground build and does not disturb `counter`'s
  native builds.

## Data flow (the vertical slice A proves)

```
textarea edit
  → JS apply_source("app.tsx", src)
    → supersolid::transpile  (diagnostics returned to JS now)
    → QUEUE.push(Edit::Js(jsCode))
  → next frame: drain_playground_edits
    → Assets<JsSource>::get_mut(subresources.js).0 = jsCode   // emits AssetEvent::Modified
  → detect_hot_reload sets HotReloadFlags.js
  → apply_hot_reload re-execs rt.run_script(js) with HMR on
    → $ss signal-cell rehydration
  → reconcile_system
  → counter UI updates, count preserved
uncaught throw → runtime error sink → poll_diagnostics() → console
```

## Proof harness (A's deliverable, not the real UI)

A minimal static HTML page hosting the `counter` playground wasm plus:

- a `<textarea>` prefilled with `app.tsx`,
- a **Run** button calling `apply_source("app.tsx", textarea.value)`,
- a `<pre>` showing returned diagnostics and the result of polling `poll_diagnostics()`.

Served locally (standalone or via `mdbook serve website` once staged). Its only job is to
prove the seam end-to-end and serve as A's manual verification. The mockup's real UI is B.

## Testing

- **Native (big-stack) integration** — the bridge system is testable on native even though
  the wasm-bindgen exports are not: drive a `drain_playground_edits`-equivalent, overwrite
  `Assets<JsSource>` for the mounted subresource, and assert `AssetEvent::Modified` fires,
  `apply_hot_reload` re-execs, the new script ran, **and** a signal cell was preserved
  across the swap.
- **Transpile-diagnostics passthrough:** broken `.tsx` → `apply_source` returns
  `{ok:false, diagnostics:[…]}`, no panic; the harness stays alive.
- **Runtime-error capture:** a script that throws → surfaced via `take_errors` /
  `poll_diagnostics`.
- **wasm smoke (manual):** build the `counter` playground for wasm, serve, edit the source,
  Run → the count is preserved in a real browser. Per the Windows-main-thread-stack finding
  (`/STACK:8MB`), green tests run on big-stack worker threads and do **not** prove a
  windowed/wasm app launches — this manual check is required.
- **No regression:** native `counter --features hmr` still hot-reloads live `.tsx`.

## Main risk

`get_mut`-emitted `AssetEvent::Modified` must be observed by `detect_hot_reload` in the
same frame it is produced. Mitigated by ordering `drain_playground_edits` **before**
`detect_hot_reload`; if ordering proves insufficient, push the `AssetEvent` directly.
Verified by the wasm smoke test.

## Implementation notes

- Land on `main` (project does not use PRs). Do **not** use worktrees (`target/` is huge —
  per `CLAUDE.md`).
- Do the transpiler-feature edits as real changes, not the throwaway spike used for the
  size measurement this session (that spike made the dep unconditional and was reverted).
```
