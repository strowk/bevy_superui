# Module loading via complete `import` syntax — design

Date: 2026-10-11
Status: approved for planning

## Problem

A supersolid app is one file today. The entry HTML declares exactly one
`<script src>`; `TsxLoader` transpiles that single `.tsx`/`.ts` into a classic
(non-module) script, and `crates/supersolid/src/imports.rs` strips every
`import`:

- `solid-js` / `supersolid` / `superui/test` → stripped silently (runtime names
  are provided as globals).
- `./x.css` → recorded in `style_imports` (but never consumed anywhere today).
- `./x.json` → inlined as a top-level `const` (see
  `2026-10-03-superui-json-imports-design.md`).
- **anything else (`./counter.tsx`, a bare package) → warns "cross-module import
  … is not supported yet (stripped)".**

That last case forces every component, helper, and screen into one file.
`examples/game_menu` is the canonical victim: four screens and a tab switcher in
a single `app.tsx` with imports stripped.

This design makes `import { Counter } from "./counter.tsx"` (and the other ESM
import/export forms) actually resolve, load, and wire up — so an app can span
multiple files, and so vendored packages under `superui_modules/` can be
imported by bare specifier.

## Scope

In scope:

- **Relative imports** of `.tsx` / `.ts` / `.js` siblings and ancestors
  (`./x`, `../lib/util`).
- **Bare imports** resolved from a `superui_modules/` root (node_modules-style).
  The package manager that would populate that directory is *not* in scope; the
  resolution machinery must be ready for it.
- **Co-located `.css`** imported from any module in the graph (not just the
  entry), concatenated into the applied stylesheet.
- **Graph-wide hot reload**: editing any file in the import graph reloads and
  preserves signal state.

Explicitly out of scope (documented limitations; warn-and-skip, do not fail):

- `export * from "./barrel"` re-export and dynamic `import()`.
- `import.meta`.
- A package manager / lockfile / `package.json` `main`/`module` field
  resolution (candidate `index.*` resolution only; `main` is a fast-follow).
- Multi-file editing in the web playground UI (the resolver is made
  provider-shaped so the playground *can* adopt it later; the UI stays
  single-file for now).

## Constraints the design must respect

1. **`supersolid` stays I/O-free and wasm-capable.** It compiles into the wasm
   playground under the `transpiler` feature, so no `std::fs` may enter its
   always-on path. Today it only *records* import specifiers and lets callers
   resolve them; that stays true.
2. **The engine runs classic scripts, not ES modules.** Both engines execute via
   a plain eval (`V8Engine::run_script` / `engine_web.rs::eval`). The module
   system is emitted *into* the single script, not handed to the engine.
3. **Resolution is iterative.** What a file imports is known only after
   transpiling it, so the graph walk is transpile → discover deps → read →
   transpile → recurse. The native loader reads via **async**
   `LoadContext::read_asset_bytes`; `build.rs` and the test-engine read via
   **sync** `std::fs`; the playground has no filesystem (in-memory editor files).
4. **Four callers** already do "transpile + resolve imports" and must share one
   resolution algorithm: `TsxLoader` (`crates/superui/src/assets.rs`),
   `supersolid::build` (`build.rs` helper), the test-engine
   (`crates/superui_test_engine/src/transpile.rs`), and the web playground
   (`crates/superui_playground_web/src/lib.rs`).

## Architecture overview

```
┌─────────────────────────────────────────────────────────────┐
│ supersolid (pure, wasm-clean, no std::fs)                     │
│                                                               │
│  transpile_module(src, opts) -> Module {                      │
│      factory_code,   // body: imports→require, exports→exports │
│      deps,           // raw non-runtime specifiers            │
│      css_imports,    // raw .css specifiers                   │
│      diagnostics }                                            │
│                                                               │
│  resolve_candidates(importer_id, specifier) -> Vec<String>    │
│      // ordered candidate paths; pure (no existence check)    │
│                                                               │
│  assemble(entry_id, modules: Map<Id, ResolvedModule>)         │
│      -> Bundle { code, css, module_ids }                      │
│                                                               │
│  walk_sync(entry_id, opts, read: FnMut(&str)->io::Result<..>) │
│      -> Bundle   // generic graph walk for the sync callers   │
└─────────────────────────────────────────────────────────────┘
        ▲ std::fs read        ▲ std::fs read      ▲ in-mem map read
        │                     │                   │
   build.rs              test-engine          playground (wasm)
        │
        ▼ async read_asset_bytes (its own loop, reuses the pure fns)
   TsxLoader (crates/superui) ──► JsSource { code, css }
```

## Section 1 — Module registry ABI (`$ssmod`)

`assemble` emits this prelude at the top of every bundle. It is stateless
scaffolding, re-defined on each HMR reload; signal state is preserved by the
existing `$ss.hot` mechanism, not by the registry.

```js
var $ssmod = (function () {
  var mods = {}, cache = {};
  function def(id, edges, factory) { mods[id] = { edges: edges, factory: factory }; }
  function req(id) {
    if (cache[id]) return cache[id].exports;       // evaluated or evaluating
    var m = mods[id];
    if (!m) { console.error("superui: module not found: " + id); return {}; }
    var module = { exports: {} };
    cache[id] = module;                             // cache BEFORE factory → cycles see partial exports
    var require = function (spec) { return req(m.edges[spec]); };
    m.factory(module.exports, module, require);
    return module.exports;
  }
  return { def: def, run: req };
})();
```

- **Module id** = the canonical resolved asset path (forward-slash normalized).
  Two importers that reach the same file via different relative paths share one
  instance (singleton) because both edges resolve to the same id.
- **`edges`** = that module's `raw specifier → resolved id` map, built by the
  walker. Keeping resolution knowledge in `edges` lets the transpiler emit the
  raw specifier (`require("./label.tsx")`) without knowing the resolved path.
- **Cycles** behave like CommonJS: the importer sees whatever the imported
  module has exported so far. Documented, not an error.
- **Sentinel**: an unresolved dep maps (in `edges`) to a sentinel id with no
  registered factory; `req` logs `module not found` and returns `{}`.

Entry: `assemble` appends `$ssmod.run("<entry_id>");`.

## Section 2 — Transpiler: import/export lowering

A new per-module entry point `supersolid::transpile_module` does ESM→factory
lowering (pure). It returns the **factory body** plus `deps` / `css_imports` /
inlined JSON so the walker can recurse and `assemble` can wrap it. The existing
single-file `transpile` stays as a thin wrapper (`transpile_module` of one file,
assembled as a one-module bundle) so current callers and tests keep working
while they migrate to the walker.

Per **import** declaration, classify by specifier:

| Specifier | Action |
| --- | --- |
| runtime (`solid-js`, `supersolid`, `superui/test`, …) | strip; names stay global and are visible inside the factory scope (unchanged) |
| `*.css` | record in `css_imports`; strip from JS (applied at bundle level) |
| `*.json` | inline as a function-scoped `const` (unchanged behavior, now inside the factory) |
| anything else (relative or bare) | rewrite to `require("<raw specifier>")`; record the raw specifier in `deps` |

Import binding forms lower as:

- `import { a, b as c } from "x"` → `const { a, b: c } = require("x");`
- `import d from "x"` → `const d = require("x").default;`
- `import * as ns from "x"` → `const ns = require("x");`
- `import "x"` (side-effect) → `require("x");`

New **export** handling (none exists today):

- `export function F` / `export const x` / `export class C` → emit the
  declaration, then `exports.F = F;` (and still register `$ss.hot` for
  component-shaped bindings as today).
- `export { a, b as c }` → `exports.a = a; exports.c = b;`
- `export default E` → `exports.default = E;`
- `export * from "x"` and dynamic `import()` → warn-and-skip (out of scope).

The existing `$ss.hot` id is already module-qualified
(`"<module_id>#<Name>"`), so components from different files get distinct ids
for free — graph-wide HMR needs no id change.

## Section 3 — Resolution algorithm

`resolve_candidates(importer_id, raw_specifier)` returns an ordered candidate
list; the walker's read-closure doubles as the existence check (a read that
errors = candidate miss). First successful read = canonical resolved id.

1. **Relative** (`./x`, `../x`) → join against the importer's parent dir;
   normalize `.` / `..`; forward-slash normalized.
2. **Bare** (`pkg`, `pkg/sub`) → resolve under `superui_modules/<spec>` from the
   asset root.
3. **Extension / index** for a path without an exact file, in order:
   `<path>.tsx`, `<path>.ts`, `<path>.js`, then
   `<path>/index.{tsx,ts,js}`. For a bare root, also
   `superui_modules/<pkg>/index.{tsx,ts,js}` and `superui_modules/<pkg>.*`.
   `package.json` `main`/`module` is a documented fast-follow.

`.json` and `.css` specifiers keep their own handling (inline / concatenate)
and are not run through this JS-module candidate ladder.

## Section 4 — The walker and the four callers

Shared pure walker `walk_sync(entry_id, opts, read) -> Bundle`:

1. Queue of resolved ids starting at the entry; a `seen` set.
2. For each id: `read` the source, `transpile_module`; for each raw dep, try its
   `resolve_candidates` in order via `read`, take the first that reads as the
   edge target, enqueue unseen targets, and build the module's `edges` map;
   read + accumulate each `.css` import's text.
3. `assemble(entry_id, modules)` → `Bundle { code, css, module_ids }`.
4. Unresolved dep or unreadable file → warning diagnostic + an `edges` entry to
   a sentinel id (graceful degradation, never fails the bundle).

Callers:

- **`TsxLoader` (async, native, `crates/superui/src/assets.rs`)** — writes its
  own async loop using `lc.read_asset_bytes().await` as the read, reusing
  `transpile_module` / `resolve_candidates` / `assemble` (it cannot call
  `walk_sync` because reads are async). Registers **every resolved module id and
  every `.css` file** as a load dependency (which `read_asset_bytes` does
  implicitly) → editing any graph file re-runs the loader, re-bundles, and
  re-execs; `$ss.hot` preserves signal state (**graph-wide HMR**). Produces
  `JsSource { code, css }`.
- **`build.rs` helper (sync, `crates/supersolid/src/build.rs`)** — `walk_sync`
  with a `std::fs` read closure; emits `cargo:rerun-if-changed` for every
  resolved id + `.css` file; writes the bundled `.js` under
  `<dir>/.superui/build/`. `build.rs` does not parse `index.html`, so it keeps
  treating **every top-level `.tsx`/`.ts` as its own entry** and bundles that
  file's graph; a file that is only a dependency produces an unused (but
  correct) artifact. Only the entry's artifact is ever loaded at mount.
- **test-engine (sync, `crates/superui_test_engine/src/transpile.rs`)** —
  `walk_sync` with a `std::fs` read closure for spec imports; a missing file
  stays fatal there (matches today's test-authoring-bug behavior).
- **playground (wasm, `crates/superui_playground_web/src/lib.rs`)** — able to
  call `walk_sync` with an in-memory-map read closure over editor files. The
  playground UI stays single-file for now; the seam is left in place, the
  multi-file editor is a separate enhancement.

## Section 5 — CSS concatenation and wiring

Graph-wide `.css` imports (plus the entry HTML `<link rel=stylesheet>`)
concatenate in **post-order** (a module's CSS after its deps', so an importer
overrides an importee) into one merged sheet. flair's current "one stylesheet"
limit becomes "one merged sheet."

Wiring: `.css` imports are discovered only *during JS bundling* (inside
`TsxLoader`), but the applied `StyleSheet` is a separate handle discovered from
`<link>` in `mount.rs`. Resolution:

- **`JsSource` gains `css: String`** (default empty), produced by the bundler
  from the module-graph `.css` imports.
- `mount.rs` appends that bundled CSS after the `<link>` sheet when building the
  applied stylesheet. Keeps everything inside the existing two-phase mount; no
  new asset type.

## IDE / tsconfig alignment

`cargo superui install` writes a tsconfig with `moduleResolution: "bundler"`,
`baseUrl: "."`, and a `paths` entry per projected module
(`crates/cargo-superui/src/lib.rs`).

- **Relative imports** (`./x.tsx`) resolve in the IDE automatically under
  `moduleResolution: "bundler"` — no tsconfig change needed.
- **Bare `superui_modules` packages** resolve in the IDE only when they have an
  explicit `paths` entry (today just `supersolid` and `superui/test`). The
  future package manager will add those entries when it installs a package (or a
  wildcard `"*": ["./superui_modules/*"]`). The runtime resolver and the
  tsconfig stay aligned this way; this spec does not change the tsconfig.

## Error handling

Consistent with design §1 graceful degradation:

- Missing / unreadable / unresolvable module → warning diagnostic, sentinel
  edge, bundle still produced (native loader + playground). The test-engine
  keeps a missing import fatal.
- Invalid JSON / missing `.css` → warning, skip that binding / sheet
  (unchanged for JSON; same shape for CSS).
- `export *` / dynamic `import()` → warning, skipped.
- Import cycle → CommonJS partial-exports semantics; no diagnostic.

## Testing

- **Transpiler unit tests** (`crates/supersolid`): each import form lowers to the
  right `require`/`const`; each export form lowers to the right `exports.*`;
  runtime imports still strip silently; css/json still classify; `export *` and
  dynamic `import()` warn. Output reparses as plain JS.
- **Assemble unit tests**: registry prelude shape; `edges` map; entry `run`;
  sentinel for unresolved; post-order CSS concatenation; a two-node cycle yields
  partial exports rather than infinite recursion.
- **Resolver unit tests**: relative normalization, bare→`superui_modules`,
  extension/index ladder order.
- **`walk_sync` tests**: a small fixture graph over an in-memory map (diamond
  dependency shares a singleton; a `.css` import is collected).
- **`TsxLoader` integration tests** (`crates/superui/src/assets.rs` style): a
  two-file `.tsx` graph loads to one `JsSource` whose code runs; a missing dep
  degrades gracefully; bundled CSS appears on `JsSource.css`.
- **HMR integration**: editing an imported module re-runs the loader and
  preserves a signal (graph-wide HMR), reusing the existing HMR test harness.
- **Characterization baseline (before the split)**: `game_menu`'s
  `tests/game_menu.spec.ts` currently snapshots only the **main** and
  **settings** screens. Before touching `app.tsx`, expand it to also snapshot the
  **pause** and **game-over** screens — navigate via the tab bar
  (`.tabs .tab` with `hasText "PAUSE"` / `"GAME OVER"`, the same pattern as the
  existing SETTINGS test; new tests go *after* "main menu renders", which must
  stay first because the spec shares one mount), assert the screen is visible
  (`.pause-card`, `.screen.over`), and `toHaveScreenshot("pause.png")` /
  `"over.png"`. Generate and commit those baselines while the app is still a
  single file. All four screen snapshots are the green baseline the split must
  preserve.
- **Demonstrator (the split)**: split `examples/game_menu/app.tsx` across files
  (one module per screen + a shared tab switcher) and confirm the full,
  now-four-screen spec still passes with the committed snapshots unchanged.

## Demonstrator example

`examples/game_menu` is the example to split: single-file today, the right size,
and free of benchmark baggage (unlike `horde` / `citadel`). Splitting it into
per-screen modules plus a shared tab-switcher module both validates the feature
end-to-end and replaces the current "imports stripped, one giant file" authoring
gotcha recorded for that example.

## Rollout / sequencing (for the plan)

1. Transpiler: import/export lowering + `transpile_module` return shape
   (pure; fully unit-testable with no I/O).
2. `assemble` + registry ABI + `resolve_candidates` + `walk_sync` (pure).
3. `TsxLoader` async walk + load-dependency registration (graph-wide HMR).
4. `JsSource.css` + `mount.rs` merge.
5. `build.rs` and test-engine switch to the walker.
6. Expand `game_menu`'s spec to snapshot the pause + game-over screens and
   commit those baselines green — *while the app is still single-file* — so the
   split has a complete four-screen characterization baseline to preserve.
7. Split `examples/game_menu`; confirm the four-screen spec stays green;
   update docs and the authoring gotcha.
```
