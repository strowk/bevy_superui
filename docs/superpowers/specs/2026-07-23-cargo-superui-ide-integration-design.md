# `cargo-superui` — IDE integration slice (design)

Date: 2026-07-23
Status: approved design, ready for implementation plan

## Problem

Every supersolid `.tsx` example carries a hand-maintained pair of editor-only files so
VS Code / the TS language server stops treating the files as React JSX and can resolve
the `supersolid` import:

- `assets/ui/<slug>/supersolid-shim.d.ts` — an ambient `declare module "supersolid"`
  plus a permissive global `JSX` namespace.
- `assets/ui/<slug>/tsconfig.json` — `jsx: preserve`, bundler resolution, `noEmit`.

These are duplicated across all four examples (`citadel`, `game_menu`, `horde`,
`todomvc_supersolid`) and **already drifting**: `game_menu`'s shim is missing the `Keyed`
declaration the others have. The files also sit *inside* the runtime asset tree, their
name/location is arbitrary, the types will need to evolve (and gain doc comments) over
time, and there is no process for getting updated types onto downstream developer
machines.

Audience priority: **downstream developers** who add superui as a dependency and author
their own `.tsx`, with the same mechanism also cleaning up this repo's own examples.

## Prior art in this repo

`docs/superpowers/specs/2026-07-19-superui-component-framework-direction.md` §9 already
designs the distribution model this slice bootstraps:

- Component source is distributed as **ordinary cargo crates**; cargo is the package
  manager.
- A **thin CLI** reads the already-resolved graph via `cargo metadata` and *projects*
  UI source into a flat, gitignored `superui_modules/` tree that imports resolve
  against — "`node_modules` in ergonomics, derived from cargo".
- "Real IDE tooling works for free … We ship an ambient `.d.ts` for the runtime API + a
  `tsconfig` preset" (§3).

This slice implements the smallest useful subset: stand up the CLI + the
`superui_modules/` resolution convention, and project **only** supersolid's own types +
a tsconfig. Component-crate projection, `[package.metadata.superui]` parsing, and the
capability ledger (the rest of §9) are explicitly deferred.

## Design

### 1. Source of truth for the types

The canonical types are a single hand-authored data file shipped **inside the crate that
owns the runtime JS API surface**:

```
crates/supersolid/supersolid.d.ts
```

- It is the one place types + doc comments are edited.
- It contains **both** the `supersolid` module surface (`createSignal`, `createMemo`,
  `render`, `For`, `Show`, `Keyed`, `Index`, `Switch`, …) **and** the ambient global
  `JSX` namespace (`IntrinsicElements`, `Element`, `ElementChildrenAttribute`) — unified
  from the current per-example shim, with doc comments added.
- Because it ships **with the crate**, the types are automatically **version-matched** to
  whatever `supersolid` version the downstream `Cargo.lock` resolved. No separate
  versioning.
- `crates/supersolid/Cargo.toml` must ensure `supersolid.d.ts` is included in the
  published tarball (cargo `include`/`exclude`: a non-`.rs` data file must not be
  excluded).

**Runtime decoupling.** The `import { … } from "supersolid"` is *editor-only*: the
transpiler erases the import and the runtime injects `createSignal` et al. via its ABI
(`$ss.*`). The `.d.ts` never participates in build or runtime — it only *describes* the
runtime-injected surface. Keeping it truthful is a manual authoring discipline on the
crate file, which is exactly why it lives next to the runtime it documents.

### 2. `superui_modules/` layout + resolution

`install` targets an **app/crate directory** and writes, **beside `assets/` — never
inside it** (`assets/` is the runtime-loaded, wasm-packaged tree; dev-only `.d.ts` cruft
must stay out of it):

```
<app-dir>/
  Cargo.toml
  tsconfig.json                       # created if absent (see §4)
  superui_modules/                    # gitignored, fully derived
    supersolid/
      index.d.ts                      # copy of crates/supersolid/supersolid.d.ts
  assets/ui/<slug>/*.tsx              # .tsx stays here; TS walks up to the tsconfig
```

`superui_modules/supersolid/` is a **real resolvable module**, not a `declare module`
fake. TypeScript resolves `import … from "supersolid"` through the **nearest ancestor
`tsconfig.json`**, whose `paths`/`baseUrl` point at it:

```jsonc
{
  "compilerOptions": {
    "jsx": "preserve",
    "module": "esnext",
    "moduleResolution": "bundler",
    "target": "esnext",
    "noEmit": true,
    "baseUrl": ".",
    "paths": {
      // one explicit entry per projected module; install owns/regenerates this block
      "supersolid": ["./superui_modules/supersolid/index.d.ts"]
    }
  },
  "include": ["superui_modules/**/*.d.ts", "assets/**/*.ts", "assets/**/*.tsx"]
}
```

Result: `import { createSignal, Keyed, render } from "supersolid"` resolves with full
IntelliSense + doc comments, and the dev's project carries **zero hand-authored type
content**. Explicit per-module `paths` entries (rather than a `"*": ["superui_modules/*"]`
wildcard) are used so the block is deterministic, greppable, and ready to grow one entry
per projected crate when §9 projection lands.

Placement is otherwise free: a workspace with several apps gets one
`superui_modules/` + `tsconfig.json` per app (self-contained; each can later install
different component crates). A downstream dev with a single app sees this collapse to a
clean, node_modules-like project-root layout. A repo-root tsconfig kept for other tooling
does not conflict, because the nearest (app-level) tsconfig wins for the `.tsx` files.

### 3. `cargo superui install` algorithm

```
cargo superui install [--path <app-dir>]

1. Resolve target app dir:
     --path if given, else the current package's manifest dir
     (cargo locate-project / cwd).

2. cargo metadata (reads the already-resolved graph; builds nothing):
     find the `supersolid` package in the dependency graph
     (direct OR transitive via `superui`).
     -> manifest_path -> parent dir -> read <dir>/supersolid.d.ts.
     If not found: error "no supersolid dependency resolved -- add it to Cargo.toml".

3. Project types:
     write <app-dir>/superui_modules/supersolid/index.d.ts   (copy of the .d.ts)
     (overwrite unconditionally -- derived artifact)

4. tsconfig:
     if <app-dir>/tsconfig.json absent   -> write the full preset (§2)
     else if it lacks the supersolid paths entry -> print the snippet to add
     else                                -> nothing

5. Ensure <app-dir>/.gitignore contains `superui_modules/` (append if missing).

6. Print a short summary of what was written.
```

Package identification is **hardcoded to the `supersolid` package** in this slice.
Generalizing to "any crate declaring `[package.metadata.superui]`" is deferred to §9.

### 4. Existing-`tsconfig.json` behavior (this slice)

- **Absent** → write the full preset from §2.
- **Present** → do not rewrite. Detect whether it already has the `supersolid`
  `paths` entry; if not, **print the exact snippet to add** and exit cleanly (non-fatal).
  `superui_modules/supersolid/index.d.ts` is projected either way, so types land on disk
  regardless.

A JSONC-preserving "managed `paths` block" merge is **deferred** to when §9 component
projection makes the `paths` map actually churn — at which point a marked,
surgically-managed block earns its parsing cost.

### 5. Re-run / update / sync story

- `superui_modules/` is gitignored and fully derived — never hand-edited, never
  committed. Delete-and-re-`install` is always safe.
- **Updating types** = bump the `supersolid`/`superui` dependency version, then re-run
  `cargo superui install`. Because the `.d.ts` ships with the crate version resolved in
  `Cargo.lock`, projected types always match the runtime being edited against. That is
  the "sync to developer machines" answer: it rides `cargo update` + a re-`install`.
- Auto-run-on-build and staleness enforcement are **deferred**; re-running install is the
  mechanism for now.

### 6. CLI crate

- New workspace member `crates/cargo-superui` (binary crate; binary name `cargo-superui`
  so it is invocable as `cargo superui …` and also directly as `cargo-superui …`).
- **In-workspace first, publish later.** Prove it on the four examples now; publish to
  crates.io as a follow-up once solid.

### 7. Migrate this repo's four examples (dogfood + integration test)

- Delete the four `assets/ui/<slug>/supersolid-shim.d.ts` +
  `assets/ui/<slug>/tsconfig.json` pairs (kills the `game_menu` `Keyed` drift).
- Author `crates/supersolid/supersolid.d.ts` (unified surface + JSX namespace + doc
  comments), seeded from the current shim.
- Run `cargo superui install` in each example (path dep → reads the in-repo
  `crates/supersolid/supersolid.d.ts`) → each gains `examples/<slug>/superui_modules/` +
  `examples/<slug>/tsconfig.json`, gitignored.
- The examples become the living integration test for the CLI.

## Non-goals (this slice)

- Component-crate projection, `[package.metadata.superui]` parsing, capability
  ledger / footprint derivation — all §9, deferred.
- JSONC-preserving tsconfig merge — deferred until `paths` churns.
- Auto-run on build / staleness enforcement — re-run `install` manually.
- crates.io publish — follow-up, not part of this slice.

## Success criteria

- `crates/supersolid/supersolid.d.ts` exists, unifies the current shim + JSX namespace,
  and is included in the crate's published tarball.
- `cargo-superui` is a workspace member; `cargo superui install` (and
  `cargo-superui install`) both work.
- Running install in an example produces a gitignored `superui_modules/supersolid/index.d.ts`
  and a working `tsconfig.json` beside `assets/`, with no file left inside `assets/`.
- In VS Code, a `.tsx` under an installed example resolves
  `import { createSignal, Keyed, render } from "supersolid"` with hover doc comments and
  no React-JSX errors.
- The four examples no longer carry hand-maintained shim/tsconfig files; `game_menu`
  regains `Keyed`.
